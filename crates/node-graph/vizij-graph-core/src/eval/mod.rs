//! Evaluation pipeline for the Vizij node graph.
//!
//! The `eval` module hosts the runtime utilities that turn a [`GraphSpec`]
//! into concrete values by walking the graph in topological order. The submodules are organised to
//! keep domain concerns isolated:
//!
//! - `graph_runtime` tracks per-node state and staging buffers between frames.
//! - `value_layout` flattens structured values for numeric operators.
//! - `shape_helpers` validates declared output shapes.
//! - `numeric` and `variadic` provide shared math helpers.
//! - [`mod@eval_node`] houses the dispatch logic for individual [`NodeType`](crate::types::NodeType)s.
//! - `urdfik` is gated behind the `urdf_ik` feature and packages the IK solver helpers.
//!
//! Integration code should primarily interact with [`GraphRuntime`] and [`evaluate_all`].

use crate::types::{GraphSpec, NodeSpec, NodeType};
use std::mem;

pub mod eval_node;
mod graph_runtime;
mod node_function;
mod noise;
mod numeric;
mod plan;
mod shape_helpers;
#[cfg(feature = "urdf_ik")]
mod urdfik;
mod value_layout;
mod variadic;

pub use eval_node::eval_node;
pub use graph_runtime::{GraphRuntime, SpawnRequest, StagedInput};
pub use node_function::{NodeFunction, NodeFunctionRegistry, NodeFunctions};
pub use plan::{fingerprint_spec, PlanCache};
pub use value_layout::PortValue;

#[cfg(test)]
mod blend_tests;
#[cfg(test)]
mod external_function_tests;
#[cfg(test)]
mod tests;

/// Evaluate every node in `spec`, updating `rt` in-place.
///
/// The runtime is cleared before evaluation and is repopulated as nodes are visited in topological
/// order. Any error propagated from an individual node halts evaluation.
///
/// This path has no [`NodeFunctions`] host, so any `ExternalFunction` node in `spec` errors.
/// Use [`evaluate_all_with_functions`] to run graphs that invoke node-functions.
pub fn evaluate_all(rt: &mut GraphRuntime, spec: &GraphSpec) -> Result<(), String> {
    evaluate_all_inner(rt, spec, None, ByName::Kept)
}

/// Evaluate every node in `spec`, threading `functions` through to any `ExternalFunction` nodes.
///
/// Behaves exactly like [`evaluate_all`] except that `ExternalFunction` nodes dispatch through
/// `functions` (the host node-function interface) instead of erroring.
pub fn evaluate_all_with_functions(
    rt: &mut GraphRuntime,
    spec: &GraphSpec,
    functions: &mut dyn NodeFunctions,
) -> Result<(), String> {
    evaluate_all_inner(rt, spec, Some(functions), ByName::Kept)
}

/// Evaluate like [`evaluate_all_with_functions`], for a host that takes the
/// evaluation's results from [`GraphRuntime::writes`] and
/// [`GraphRuntime::spawns`]: every port's value stays in
/// [`GraphRuntime::outputs_vec`], indexed by the plan, and
/// [`GraphRuntime::outputs`] — a copy of every port by node id and port
/// name — is left empty. That copy clones each value the graph computes, a
/// second time, every evaluation.
pub fn evaluate_writes_with_functions(
    rt: &mut GraphRuntime,
    spec: &GraphSpec,
    functions: &mut dyn NodeFunctions,
) -> Result<(), String> {
    evaluate_all_inner(rt, spec, Some(functions), ByName::Skipped)
}

/// Whether an evaluation keeps [`GraphRuntime::outputs`], the copy of every
/// port by node id and port name.
#[derive(Clone, Copy, PartialEq)]
enum ByName {
    Kept,
    Skipped,
}

fn evaluate_all_inner(
    rt: &mut GraphRuntime,
    spec: &GraphSpec,
    mut functions: Option<&mut dyn NodeFunctions>,
    by_name: ByName,
) -> Result<(), String> {
    if spec.version > 0 {
        rt.plan.ensure_versioned(spec)?;
    } else {
        rt.plan.ensure(spec)?;
    }
    rt.advance_epoch();
    rt.outputs.clear();
    if by_name == ByName::Kept {
        rt.outputs.reserve(spec.nodes.len());
    }
    rt.writes.0.clear();
    rt.spawns.clear();
    rt.node_states
        .retain(|id, _| spec.nodes.iter().any(|node| node.id == *id));

    let mut plan = mem::take(&mut rt.plan);
    let result = (|| {
        prepare_buckets(rt, spec, &mut plan);

        for &idx in plan.order.iter() {
            let node = spec
                .nodes
                .get(idx)
                .ok_or_else(|| format!("plan referenced missing node at index {}", idx))?;
            // A constant every read of which the plan resolved has no
            // reader here — only the by-name copy shows it.
            if by_name == ByName::Skipped && plan.unread_constant(idx) {
                continue;
            }
            if holds_its_constant(node, &rt.outputs_vec[idx]) {
                if by_name == ByName::Kept {
                    let compat = eval_node::materialize_outputs(
                        &plan.layouts[idx].outputs,
                        &rt.outputs_vec[idx],
                    );
                    rt.outputs.insert(node.id.clone(), compat);
                }
                continue;
            }
            let (inputs_vec, present_vec) = eval_node::read_inputs(rt, idx, &plan)?;
            let inputs =
                eval_node::InputSlots::new(&inputs_vec, &present_vec, &plan.layouts[idx].inputs);
            let mut vec_out = mem::take(rt.outputs_vec.get_mut(idx).expect("outputs vec present"));
            resize_and_clear(&mut vec_out);
            {
                let mut outputs =
                    eval_node::OutputSlots::new(&mut vec_out, &plan.layouts[idx].outputs);
                outputs.clear();
                // Reborrow the optional host with a fresh, per-iteration lifetime so the mutable
                // borrow does not outlive a single node evaluation.
                let functions_ref: Option<&mut dyn NodeFunctions> = match functions {
                    Some(ref mut f) => Some(&mut **f),
                    None => None,
                };
                eval_node::eval_node_inner(rt, node, &inputs, &mut outputs, functions_ref)?;
            }

            if by_name == ByName::Kept {
                let compat = eval_node::materialize_outputs(&plan.layouts[idx].outputs, &vec_out);
                rt.outputs.insert(node.id.clone(), compat);
            }
            rt.outputs_vec[idx] = vec_out;
        }
        Ok(())
    })();
    rt.plan = plan;
    result
}

/// Size the output buckets for `spec` and clear them for an evaluation, but
/// for the constants' — a constant keeps its port from one evaluation to the
/// next ([`holds_its_constant`]) — as long as the buckets follow `plan`. When
/// the plan has changed since they were filled, its node indices with it,
/// every bucket starts empty.
fn prepare_buckets(rt: &mut GraphRuntime, spec: &GraphSpec, plan: &mut PlanCache) {
    if !plan.buckets_filled() {
        rt.outputs_vec.clear();
        plan.mark_buckets_filled();
    }
    if rt.outputs_vec.len() != spec.nodes.len() {
        rt.outputs_vec.resize_with(spec.nodes.len(), Vec::new);
    }
    for (bucket, node) in rt.outputs_vec.iter_mut().zip(&spec.nodes) {
        if !matches!(node.kind, NodeType::Constant) {
            bucket.clear();
        }
    }
}

/// Whether `ports`, the bucket of constant `node` from the last evaluation,
/// holds what evaluating it gives now: its value, with the shape that value
/// infers. A constant's port depends on its `value` param alone, unless it
/// declares output shapes; a changed `value` is evaluated again.
fn holds_its_constant(node: &NodeSpec, ports: &[PortValue]) -> bool {
    if !matches!(node.kind, NodeType::Constant) || !node.output_shapes.is_empty() {
        return false;
    }
    let [port] = ports else {
        return false;
    };
    match &node.params.value {
        Some(value) => port.value == *value,
        None => port.value == vizij_api_core::value::float(0.0),
    }
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

    let mut plan = mem::take(&mut rt.plan);
    let result = (|| {
        prepare_buckets(rt, spec, &mut plan);

        for &idx in plan.order.iter() {
            let node = spec
                .nodes
                .get(idx)
                .ok_or_else(|| format!("plan referenced missing node at index {}", idx))?;
            if holds_its_constant(node, &rt.outputs_vec[idx]) {
                let compat = eval_node::materialize_outputs(
                    &plan.layouts[idx].outputs,
                    &rt.outputs_vec[idx],
                );
                rt.outputs.insert(node.id.clone(), compat);
                continue;
            }
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
