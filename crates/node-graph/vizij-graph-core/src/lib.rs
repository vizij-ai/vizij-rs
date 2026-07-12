//! Core graph evaluation runtime for Vizij.
//!
//! This crate defines the canonical graph schema, node registry metadata, topology helpers,
//! and evaluation runtime used by Bevy adapters, wasm bindings, and the orchestrator.

pub mod eval;
pub mod graph_value;
pub mod schema;
pub mod topo;
pub mod types;

pub use eval::{
    eval_node, evaluate_all, evaluate_all_cached, evaluate_all_with_functions, GraphRuntime,
    NodeFunction, NodeFunctionRegistry, NodeFunctions, PortValue, StagedInput,
};
pub use graph_value::GraphValue;
pub use schema::registry;
pub use topo::topo_order;
pub use types::*;
