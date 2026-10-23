//! Vizij's node graph on Arora's behavior tree.
//!
//! This crate defines the graph format ([`types`]), the editor registry
//! ([`schema`]) and topology helpers ([`topo`]); the functions of the graph
//! format's own module, `@vizij/graph` ([`vizij_functions`]); the importer
//! from a graph to a behavior tree ([`import`]); and
//! [`TreeRuntime`](tree_runtime::TreeRuntime), which runs a graph on the
//! tree. A graph's nodes are calls of `arora-dataflow-nodes`' functions, whose
//! modules this crate re-exports.

pub use arora_dataflow_nodes::{functions, kind, params, ports, select, task, vocab};

pub mod eval;
pub mod import;
pub mod schema;
pub mod topo;
pub mod tree_runtime;
pub mod types;
pub mod vizij_functions;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tree_tests;

pub use eval::{Context, NodeFunction, NodeFunctionRegistry, NodeFunctions, PortValue};
pub use schema::registry;
pub use topo::topo_order;
pub use types::*;

#[cfg(test)]
mod vocabulary_tests {
    use vizij_api_core::value as vizij;

    use crate::vocab;

    /// Vizij's value vocabulary and the node vocabulary declare records and
    /// enumerations under the same ids, so a value built by either reads in
    /// both.
    #[test]
    fn records_and_enumerations_have_the_same_ids_in_both_vocabularies() {
        assert_eq!(vocab::RECORD_TYPE, vizij::RECORD_TYPE);
        assert_eq!(vocab::ENUM_TYPE, vizij::ENUM_TYPE);
        assert_eq!(
            vocab::record([("x", vocab::float(1.0))]),
            vizij::record([("x", vizij::float(1.0))])
        );
        assert_eq!(
            vocab::enumeration("on", vocab::float(1.0)),
            vizij::enumeration("on", vizij::float(1.0))
        );
    }
}
