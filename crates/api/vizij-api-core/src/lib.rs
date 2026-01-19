//! Core contracts shared by all Vizij runtimes and tooling.
//!
//! This crate defines the canonical [`Value`], [`Shape`], and [`TypedPath`] data
//! types plus write-batch helpers used by animation, graph, and orchestrator
//! stacks. JSON serialization uses a stable `{ "type": "...", "data": ... }`
//! envelope so Rust, wasm, and host tooling can exchange payloads safely.

/// Blending helpers for compatible [`Value`] payloads.
pub mod blend;
/// Coercion utilities for safe numeric and structural conversion.
pub mod coercion;
/// JSON normalization helpers used by wasm and host tooling.
pub mod json;
/// Shape metadata used to describe [`Value`] payloads.
pub mod shape;
/// Typed path addressing for nested values.
pub mod typed_path;
/// Core value enums and helpers.
pub mod value;
/// Write op and batch utilities for bulk updates.
pub mod write_ops;

pub use shape::{Shape, ShapeId};
pub use typed_path::TypedPath;
pub use value::{Value, ValueKind};
pub use write_ops::{WriteBatch, WriteOp};
