//! The per-node code is `arora-dataflow-nodes`' ([`arora_dataflow_nodes::eval`],
//! re-exported here). What belongs to Vizij's graph format is this crate's:
//!
//! - `shape_helpers` infers and checks declared shapes.
//! - `vizij` holds the `input` node's fitting of its value to its declared
//!   shape, the check of declared output shapes, a port's value with its
//!   shape ([`PortValue`]) and a keyed batch's writes.

pub use arora_dataflow_nodes::eval::*;

pub mod shape_helpers;
pub(crate) mod vizij;

pub use vizij::PortValue;
