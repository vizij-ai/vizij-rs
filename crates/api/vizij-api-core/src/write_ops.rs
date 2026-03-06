//! Write operations produced by engines (node graph, animation) to describe
//! writes into a blackboard / external world using typed paths.
//!
//! WriteOp serializes to JSON as:
//!   { "path": "robot1/Arm/Joint3.angle", "value": { "vec3": [1,2,3] } }
//!
//! WriteBatch is a simple Vec<WriteOp> with helpers.

use crate::{typed_path::TypedPath, Shape, Value};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct WriteOp {
    /// Destination typed path for the write.
    pub path: TypedPath,
    /// Value payload to write.
    pub value: Value,
    /// Optional explicit shape metadata carried with the write.
    pub shape: Option<Shape>,
}

impl WriteOp {
    /// Construct a write op without explicit shape metadata.
    pub fn new(path: TypedPath, value: Value) -> Self {
        Self::new_with_shape(path, value, None)
    }

    /// Construct a write op with optional explicit shape metadata.
    pub fn new_with_shape(path: TypedPath, value: Value, shape: Option<Shape>) -> Self {
        Self { path, value, shape }
    }
}

// Serialize WriteOp as { "path": "<string>", "value": <ValueJSON> }
impl Serialize for WriteOp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = if self.shape.is_some() {
            serializer.serialize_struct("WriteOp", 3)?
        } else {
            serializer.serialize_struct("WriteOp", 2)?
        };
        state.serialize_field("path", &self.path)?;
        state.serialize_field("value", &self.value)?;
        if let Some(shape) = &self.shape {
            state.serialize_field("shape", shape)?;
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for WriteOp {
    fn deserialize<D>(deserializer: D) -> Result<WriteOp, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize into an intermediate map
        let v = serde_json::Value::deserialize(deserializer).map_err(de::Error::custom)?;
        let path_s = v
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| de::Error::custom("missing or invalid 'path' field"))?;

        let tp = TypedPath::parse(path_s).map_err(de::Error::custom)?;

        let val = v
            .get("value")
            .ok_or_else(|| de::Error::custom("missing 'value' field"))?;

        // Deserialize the value JSON into Value using serde_json -> Value
        let value: Value = serde_json::from_value(val.clone()).map_err(de::Error::custom)?;

        let shape = match v.get("shape") {
            Some(shape_value) => {
                let parsed: Shape =
                    serde_json::from_value(shape_value.clone()).map_err(de::Error::custom)?;
                Some(parsed)
            }
            None => None,
        };

        Ok(WriteOp {
            path: tp,
            value,
            shape,
        })
    }
}

/// A batch of write operations. Engines can emit a WriteBatch each tick.
///
/// Batch order is preserved. Duplicate paths are allowed and are resolved later by the consumer
/// that applies the batch.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WriteBatch(pub Vec<WriteOp>);

impl WriteBatch {
    /// Construct an empty batch.
    pub fn new() -> Self {
        WriteBatch(Vec::new())
    }

    /// Append one write op to the batch.
    pub fn push(&mut self, op: WriteOp) {
        self.0.push(op);
    }

    /// Extend the batch with multiple write ops in iteration order.
    pub fn extend(&mut self, other: impl IntoIterator<Item = WriteOp>) {
        self.0.extend(other);
    }

    /// Consume the batch and return the underlying vector.
    pub fn into_vec(self) -> Vec<WriteOp> {
        self.0
    }

    /// Iterate over the batch in append order.
    pub fn iter(&self) -> impl Iterator<Item = &WriteOp> {
        self.0.iter()
    }

    /// Return `true` when the batch has no writes.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Merge another batch in-place by appending it after the existing writes.
    ///
    /// This does not deduplicate or reconcile duplicate paths.
    pub fn append(&mut self, mut other: WriteBatch) {
        self.0.append(&mut other.0)
    }
}

impl fmt::Display for WriteOp {
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
    use crate::{Shape, ShapeId, Value};

    #[test]
    fn writeop_roundtrip_json() {
        let tp = TypedPath::parse("robot1/Arm/Joint3.angle").unwrap();
        let op = WriteOp::new(tp, Value::Vec3([1.0, 2.0, 3.0]));
        let s = serde_json::to_string(&op).unwrap();
        let parsed: WriteOp = serde_json::from_str(&s).unwrap();
        assert_eq!(op, parsed);
    }

    #[test]
    fn writebatch_json_array() {
        let mut b = WriteBatch::new();
        b.push(WriteOp::new(
            TypedPath::parse("r/t.a").unwrap(),
            Value::Float(0.5),
        ));
        b.push(WriteOp::new(
            TypedPath::parse("r/t.b").unwrap(),
            Value::Text("hi".to_string()),
        ));
        let s = serde_json::to_string(&b).unwrap();
        let parsed: WriteBatch = serde_json::from_str(&s).unwrap();
        assert_eq!(b, parsed);
    }

    #[test]
    fn writeop_roundtrip_with_shape() {
        let tp = TypedPath::parse("robot1/Arm/Joint3.angle").unwrap();
        let shape = Shape::new(ShapeId::Vec3);
        let op = WriteOp::new_with_shape(tp, Value::Vec3([1.0, 2.0, 3.0]), Some(shape));
        let s = serde_json::to_string(&op).unwrap();
        let parsed: WriteOp = serde_json::from_str(&s).unwrap();
        assert_eq!(op, parsed);
    }
}
