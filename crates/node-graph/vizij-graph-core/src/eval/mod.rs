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

use crate::types::{GraphSpec, InputConnection};
use hashbrown::HashMap;
use std::mem;

pub mod eval_node;
mod graph_runtime;
mod numeric;
mod plan;
mod shape_helpers;
mod urdfik;
mod value_layout;
mod variadic;

pub use eval_node::eval_node;
pub use graph_runtime::{GraphRuntime, StagedInput};
pub use plan::PlanCache;
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
    rt.plan.ensure(spec)?;
    rt.advance_epoch();
    rt.outputs.clear();
    rt.outputs.reserve(spec.nodes.len());
    rt.writes.0.clear();
    rt.node_states
        .retain(|id, _| spec.nodes.iter().any(|node| node.id == *id));

    let plan = mem::take(&mut rt.plan);
    let result = (|| {
        let empty_inputs: HashMap<String, InputConnection> = HashMap::new();
        for &idx in plan.order.iter() {
            let node = spec
                .nodes
                .get(idx)
                .ok_or_else(|| format!("plan referenced missing node at index {}", idx))?;
            let connections = plan.inputs.get(idx).unwrap_or(&empty_inputs);
            eval_node::eval_node(rt, node, connections)?;
        }
        Ok(())
    })();
    rt.plan = plan;
    result
}
