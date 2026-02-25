//! Evaluation pipeline for the Vizij node graph.
//!
//! The `eval` module hosts the runtime utilities that turn a [`GraphSpec`](crate::types::GraphSpec)
//! into concrete values by walking the graph in topological order. The submodules are organised to
//! keep domain concerns isolated:
//!
//! - [`graph_runtime`] tracks per-node state and staging buffers between frames.
//! - [`value_layout`] flattens structured values for numeric operators.
//! - [`shape_helpers`] validates declared output shapes.
//! - [`numeric`] and [`variadic`] provide shared math helpers.
//! - [`eval_node`] houses the dispatch logic for individual [`NodeType`](crate::types::NodeType)s.
//! - [`urdfik`] is gated behind the `urdf_ik` feature and packages the IK solver helpers.
//!
//! Integration code should primarily interact with [`GraphRuntime`] and [`evaluate_all`].

use crate::types::GraphSpec;
use std::mem;

pub mod eval_node;
mod graph_runtime;
mod noise;
mod numeric;
mod plan;
mod shape_helpers;
mod urdfik;
mod value_layout;
mod variadic;

pub use eval_node::eval_node;
pub use graph_runtime::{GraphRuntime, StagedInput};
pub use plan::{fingerprint_spec, PlanCache};
pub use value_layout::PortValue;

#[cfg(test)]
mod blend_tests;
#[cfg(test)]
mod tests;

/// Evaluate every node in `spec`, updating `rt` in-place.
///
/// The runtime is cleared before evaluation and is repopulated as nodes are visited in topological
/// order. Any error propagated from an individual node halts evaluation.
pub fn evaluate_all(rt: &mut GraphRuntime, spec: &GraphSpec) -> Result<(), String> {
    if spec.version > 0 {
        rt.plan.ensure_versioned(spec)?;
    } else {
        rt.plan.ensure(spec)?;
    }
    rt.advance_epoch();
    rt.outputs.clear();
    rt.outputs.reserve(spec.nodes.len());
    rt.writes.0.clear();
    rt.node_states
        .retain(|id, _| spec.nodes.iter().any(|node| node.id == *id));

    let plan = mem::take(&mut rt.plan);
    let result = (|| {
        // Ensure output storage is sized/reset for the upcoming frame.
        if rt.outputs_vec.len() != spec.nodes.len() {
            rt.outputs_vec.resize_with(spec.nodes.len(), Vec::new);
        }
        for idx in 0..plan.layouts.len() {
            let bucket = rt.outputs_vec.get_mut(idx).expect("outputs vec present");
            bucket.clear();
        }

        for &idx in plan.order.iter() {
            let node = spec
                .nodes
                .get(idx)
                .ok_or_else(|| format!("plan referenced missing node at index {}", idx))?;
            let (inputs_vec, present_vec) = eval_node::read_inputs(rt, idx, &plan)?;
            let inputs =
                eval_node::InputSlots::new(&inputs_vec, &present_vec, &plan.layouts[idx].inputs);
            let mut vec_out = mem::take(rt.outputs_vec.get_mut(idx).expect("outputs vec present"));
            resize_and_clear(&mut vec_out);
            {
                let mut outputs =
                    eval_node::OutputSlots::new(&mut vec_out, &plan.layouts[idx].outputs);
                outputs.clear();
                eval_node::eval_node(rt, node, &inputs, &mut outputs)?;
            }

            let compat = eval_node::materialize_outputs(&plan.layouts[idx].outputs, &vec_out);
            rt.outputs_vec[idx] = vec_out;
            rt.outputs.insert(node.id.clone(), compat);
        }
        Ok(())
    })();
    rt.plan = plan;
    result
}

fn resize_and_clear(bucket: &mut Vec<PortValue>) {
    // OutputSlots::set() grows the vector on demand, so clearing is sufficient here.
    bucket.clear();
}

/// Evaluate using the existing plan cache without rebuilding it. This assumes the provided
/// `spec` matches the cached plan; it returns an error if the layouts are missing or mis-sized.
/// Intended for callers that manage plan invalidation themselves (e.g., WASM wrapper with
/// immutable specs).
pub fn evaluate_all_cached(rt: &mut GraphRuntime, spec: &GraphSpec) -> Result<(), String> {
    if rt.plan.layouts.len() != spec.nodes.len() {
        return Err("plan cache not initialised for this spec".to_string());
    }
    if rt.plan.input_bindings.len() != spec.nodes.len() {
        return Err("plan cache not initialised for this spec".to_string());
    }

    // Defensive validation: callers of evaluate_all_cached() promise the plan cache matches `spec`.
    // If that promise is broken, avoid panics from indexing plan/order and return a clear error.
    for &idx in rt.plan.order.iter() {
        if idx >= spec.nodes.len() {
            return Err(format!(
                "plan cache is inconsistent with spec: order referenced node index {} (nodes len {})",
                idx,
                spec.nodes.len()
            ));
        }
    }

    rt.advance_epoch();
    rt.outputs.clear();
    rt.outputs.reserve(spec.nodes.len());
    rt.writes.0.clear();
    rt.node_states
        .retain(|id, _| spec.nodes.iter().any(|node| node.id == *id));

    let plan = mem::take(&mut rt.plan);
    let result = (|| {
        if rt.outputs_vec.len() != spec.nodes.len() {
            rt.outputs_vec.resize_with(spec.nodes.len(), Vec::new);
        }
        for idx in 0..plan.layouts.len() {
            let bucket = rt.outputs_vec.get_mut(idx).expect("outputs vec present");
            bucket.clear();
        }

        for &idx in plan.order.iter() {
            let node = spec
                .nodes
                .get(idx)
                .ok_or_else(|| format!("plan referenced missing node at index {}", idx))?;
            let (inputs_vec, present_vec) = eval_node::read_inputs(rt, idx, &plan)?;
            let inputs =
                eval_node::InputSlots::new(&inputs_vec, &present_vec, &plan.layouts[idx].inputs);
            let mut vec_out = mem::take(rt.outputs_vec.get_mut(idx).expect("outputs vec present"));
            resize_and_clear(&mut vec_out);
            {
                let mut outputs =
                    eval_node::OutputSlots::new(&mut vec_out, &plan.layouts[idx].outputs);
                outputs.clear();
                eval_node::eval_node(rt, node, &inputs, &mut outputs)?;
            }

            let compat = eval_node::materialize_outputs(&plan.layouts[idx].outputs, &vec_out);
            rt.outputs_vec[idx] = vec_out;
            rt.outputs.insert(node.id.clone(), compat);
        }
        Ok(())
    })();
    rt.plan = plan;
    result
}
