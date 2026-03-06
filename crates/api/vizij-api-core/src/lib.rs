//! vizij-api-core: unified Shape & Value API (core, engine-agnostic)

/// Blend helpers shared by animation and graph runtimes.
pub mod blend;
/// Coercion helpers for adapting values across shape boundaries.
pub mod coercion;
/// Serde helpers for converting between JSON payloads and core value types.
pub mod json;
/// Canonical shape descriptors used across Vizij crates and wasm bridges.
pub mod shape;
/// Typed path parsing and formatting for blackboard/write targets.
pub mod typed_path;
/// Runtime value enums plus normalization helpers.
pub mod value;
/// Batched write operations emitted by graphs and orchestrators.
pub mod write_ops;

/// Canonical shape descriptors and aliases exported for downstream hosts.
pub use shape::{Shape, ShapeId};
/// Parsed typed-path contract used for graph inputs, sinks, and blackboard lookups.
pub use typed_path::TypedPath;
/// Normalized runtime value representation and its discriminant.
pub use value::{Value, ValueKind};
/// Ordered write operations collected during a frame.
pub use write_ops::{WriteBatch, WriteOp};
