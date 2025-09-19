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
use vizij_api_core::WriteBatch;

pub mod eval_node;
mod graph_runtime;
mod numeric;
mod shape_helpers;
mod urdfik;
mod value_layout;
mod variadic;

pub use eval_node::eval_node;
pub use graph_runtime::{GraphRuntime, StagedInput};
pub use value_layout::PortValue;

#[cfg(test)]
mod tests;

/// Evaluate every node in `spec`, updating `rt` in-place.
///
/// The runtime is cleared before evaluation and is repopulated as nodes are visited in topological
/// order. Any error propagated from an individual node halts evaluation.
pub fn evaluate_all(rt: &mut GraphRuntime, spec: &GraphSpec) -> Result<(), String> {
    rt.advance_epoch();
    rt.outputs.clear();
    rt.writes = WriteBatch::new();
    rt.node_states
        .retain(|id, _| spec.nodes.iter().any(|node| node.id == *id));

    let order = crate::topo::topo_order(&spec.nodes)?;
    for id in order {
        if let Some(node) = spec.nodes.iter().find(|n| n.id == id) {
            eval_node::eval_node(rt, node)?;
        }
    }
    Ok(())
}
