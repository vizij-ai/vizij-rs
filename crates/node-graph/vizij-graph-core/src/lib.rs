//! Deterministic node-graph evaluation for Vizij.
//!
//! The crate turns [`types::GraphSpec`] documents into evaluated outputs and write batches using
//! a cached plan and a reusable [`eval::GraphRuntime`]. Most consumers only need the re-exported
//! helpers in this module.

/// Evaluation runtime and helpers.
pub mod eval;
/// Schema types and registry helpers for node graphs.
pub mod schema;
/// Topological sorting helpers for graph specs.
pub mod topo;
/// Core graph types and JSON-friendly data structures.
pub mod types;

pub use eval::{
    eval_node, evaluate_all, evaluate_all_cached, GraphRuntime, PortValue, StagedInput,
};
pub use schema::registry;
pub use topo::topo_order;
pub use types::*;
