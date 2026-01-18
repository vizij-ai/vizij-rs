//! Core contracts shared by all Vizij runtimes and tooling.
//!
//! This crate defines the canonical [`Value`], [`Shape`], and [`TypedPath`] data
//! types plus write-batch helpers used by animation, graph, and orchestrator
//! stacks. JSON serialization uses a stable `{ "type": "...", "data": ... }`
//! envelope so Rust, wasm, and host tooling can exchange payloads safely.

pub mod blend;
pub mod coercion;
pub mod json;
pub mod shape;
pub mod typed_path;
pub mod value;
pub mod write_ops;

pub use shape::{Shape, ShapeId};
pub use typed_path::TypedPath;
pub use value::{Value, ValueKind};
pub use write_ops::{WriteBatch, WriteOp};
