//! vizij-api-core: unified Shape & Value API (core, engine-agnostic)

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
