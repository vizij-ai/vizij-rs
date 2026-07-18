//! `vizij-api-core`: the vizij vocabulary over `arora_types::value::Value`,
//! plus the Shape, typed-path, write-batch, blend, coercion, and JSON helpers
//! shared across Vizij runtimes.

/// Blend helpers shared by animation and graph runtimes.
pub mod blackboard;
pub mod blend;
/// Coercion helpers for adapting values across shape boundaries.
pub mod coercion;
/// JSON normalization: legacy payload forms in, Arora `Value` serde out.
pub mod json;
/// Canonical shape descriptors used across Vizij crates and wasm bridges.
pub mod shape;
/// Typed path parsing and formatting for blackboard/write targets.
pub mod typed_path;
/// The vizij value vocabulary: type ids, constructors, and accessors over
/// `arora_types::value::Value`.
pub mod value;
/// Batched write operations emitted by graphs and orchestrators.
pub mod write_ops;

/// Canonical shape descriptors and aliases exported for downstream hosts.
pub use shape::{Shape, ShapeId};
/// Parsed typed-path contract used for graph inputs, sinks, and blackboard lookups.
pub use typed_path::TypedPath;
/// The runtime value (Arora's), its vizij classifier, and the transform POD.
pub use value::{kind, Transform, Value, VizijKind};
/// Ordered write operations collected during a frame.
pub use write_ops::{WriteBatch, WriteOp};
