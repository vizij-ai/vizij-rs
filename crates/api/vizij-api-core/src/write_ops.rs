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
    pub path: TypedPath,
    pub value: Value,
    pub shape: Option<Shape>,
}

impl WriteOp {
    pub fn new(path: TypedPath, value: Value) -> Self {
        Self::new_with_shape(path, value, None)
    }

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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WriteBatch(pub Vec<WriteOp>);

impl WriteBatch {
    pub fn new() -> Self {
        WriteBatch(Vec::new())
    }

    pub fn push(&mut self, op: WriteOp) {
        self.0.push(op);
    }

    pub fn extend(&mut self, other: impl IntoIterator<Item = WriteOp>) {
        self.0.extend(other);
    }

    pub fn into_vec(self) -> Vec<WriteOp> {
        self.0
    }

    pub fn iter(&self) -> impl Iterator<Item = &WriteOp> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Merge another batch in-place (append). Dedup/merge semantics can be added later.
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
