//! Deterministic node-graph evaluation for Vizij.
//!
//! The crate turns [`types::GraphSpec`] documents into evaluated outputs and write batches using
//! a cached plan and a reusable [`eval::GraphRuntime`]. Most consumers only need the re-exported
//! helpers in this module.

pub mod eval;
pub mod schema;
pub mod topo;
pub mod types;

pub use eval::{
    eval_node, evaluate_all, evaluate_all_cached, GraphRuntime, PortValue, StagedInput,
};
pub use schema::registry;
pub use topo::topo_order;
pub use types::*;
