//! Re-export of the value abstraction the evaluator is generic over.
//!
//! [`GraphValue`] and its vocabulary companions live in `vizij-api-core`
//! alongside the value vocabulary they reify (see
//! [`vizij_api_core::graph_value`]). Graph-core is generic over `GraphValue`
//! and carries no concrete value semantics of its own; it re-exports the trait
//! here so its modules and downstream hosts can name it as
//! `vizij_graph_core::GraphValue`.

pub use vizij_api_core::graph_value::{GraphValue, Transform, VizijKind};
