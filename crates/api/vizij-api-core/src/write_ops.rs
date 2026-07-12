//! Write operations produced by engines (node graph, animation) to describe
//! writes into a blackboard / external world using typed paths.
//!
//! `WriteOp` serializes with the path as a string and the value in Arora
//! `Value` serde form:
//! `{ "path": "robot1/Arm/Joint3.angle", "value": { "struct": { ... } } }`
//! (a scalar value would be `{ "f32": 1.0 }`). The optional `shape` field
//! carries declared [`Shape`] metadata and is omitted when absent.
//!
//! `WriteBatch` is a simple `Vec<WriteOp>` with helpers.

use crate::{typed_path::TypedPath, Shape, Value};
use serde::{Deserialize, Serialize};
use std::fmt;

/// One write of a value (with optional declared shape) to a typed path.
///
/// The value payload is generic over the runtime value type `V`, defaulting to
/// [`Value`] so existing callers keep naming `WriteOp` unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WriteOp<V = Value> {
    /// Destination typed path for the write.
    pub path: TypedPath,
    /// Value payload to write.
    pub value: V,
    /// Optional explicit shape metadata carried with the write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<Shape>,
}

impl<V> WriteOp<V> {
    /// Construct a write op without explicit shape metadata.
    pub fn new(path: TypedPath, value: V) -> Self {
        Self::new_with_shape(path, value, None)
    }

    /// Construct a write op with optional explicit shape metadata.
    pub fn new_with_shape(path: TypedPath, value: V, shape: Option<Shape>) -> Self {
        Self { path, value, shape }
    }
}

/// A batch of write operations. Engines can emit a WriteBatch each tick.
///
/// Batch order is preserved. Duplicate paths are allowed and are resolved later by the consumer
/// that applies the batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WriteBatch<V = Value>(pub Vec<WriteOp<V>>);

impl<V> Default for WriteBatch<V> {
    fn default() -> Self {
        WriteBatch(Vec::new())
    }
}

impl<V> WriteBatch<V> {
    /// Construct an empty batch.
    pub fn new() -> Self {
        WriteBatch(Vec::new())
    }

    /// Append one write op to the batch.
    pub fn push(&mut self, op: WriteOp<V>) {
        self.0.push(op);
    }

    /// Extend the batch with multiple write ops in iteration order.
    pub fn extend(&mut self, other: impl IntoIterator<Item = WriteOp<V>>) {
        self.0.extend(other);
    }

    /// Consume the batch and return the underlying vector.
    pub fn into_vec(self) -> Vec<WriteOp<V>> {
        self.0
    }

    /// Iterate over the batch in append order.
    pub fn iter(&self) -> impl Iterator<Item = &WriteOp<V>> {
        self.0.iter()
    }

    /// Return `true` when the batch has no writes.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Merge another batch in-place by appending it after the existing writes.
    ///
    /// This does not deduplicate or reconcile duplicate paths.
    pub fn append(&mut self, mut other: WriteBatch<V>) {
        self.0.append(&mut other.0)
    }
}

impl<V: Serialize> fmt::Display for WriteOp<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let val = serde_json::to_string(&self.value).map_err(|_| fmt::Error)?;
        if let Some(shape) = &self.shape {
            let shape_json = serde_json::to_string(shape).map_err(|_| fmt::Error)?;
            write!(
                f,
                "{{ path: {}, value: {}, shape: {} }}",
                self.path, val, shape_json
            )
        } else {
            write!(f, "{{ path: {}, value: {} }}", self.path, val)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{float, text, vec3};
    use crate::{Shape, ShapeId};

    #[test]
    fn writeop_roundtrip_json() {
        let tp = TypedPath::parse("robot1/Arm/Joint3.angle").unwrap();
        let op = WriteOp::new(tp, vec3([1.0, 2.0, 3.0]));
        let s = serde_json::to_string(&op).unwrap();
        let parsed: WriteOp = serde_json::from_str(&s).unwrap();
        assert_eq!(op, parsed);
    }

    #[test]
    fn writeop_serializes_path_as_string_and_value_as_arora_serde() {
        let tp = TypedPath::parse("r/t.a").unwrap();
        let op = WriteOp::new(tp, float(0.5));
        let json: serde_json::Value = serde_json::to_value(&op).unwrap();
        assert_eq!(json["path"], "r/t.a");
        assert_eq!(json["value"]["f32"], 0.5);
        assert!(json.get("shape").is_none(), "absent shape is omitted");
    }

    #[test]
    fn writebatch_json_array() {
        let mut b = WriteBatch::new();
        b.push(WriteOp::new(TypedPath::parse("r/t.a").unwrap(), float(0.5)));
        b.push(WriteOp::new(TypedPath::parse("r/t.b").unwrap(), text("hi")));
        let s = serde_json::to_string(&b).unwrap();
        let parsed: WriteBatch = serde_json::from_str(&s).unwrap();
        assert_eq!(b, parsed);
    }

    #[test]
    fn writeop_roundtrip_with_shape() {
        let tp = TypedPath::parse("robot1/Arm/Joint3.angle").unwrap();
        let shape = Shape::new(ShapeId::Vec3);
        let op = WriteOp::new_with_shape(tp, vec3([1.0, 2.0, 3.0]), Some(shape));
        let s = serde_json::to_string(&op).unwrap();
        let parsed: WriteOp = serde_json::from_str(&s).unwrap();
        assert_eq!(op, parsed);
    }
}
