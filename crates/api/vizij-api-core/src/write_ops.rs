//! Write operations produced by engines (node graph, animation) to describe
//! writes into a blackboard or external world using typed paths.
//!
//! `WriteOp` serializes to JSON as:
//! `{ "path": "robot1/Arm/Joint3.angle", "value": { "type": "vec3", "data": [1,2,3] } }`.
//! The optional `shape` field is only included when present.
//!
//! `WriteBatch` is a thin `Vec<WriteOp>` wrapper with convenience helpers.

use crate::{typed_path::TypedPath, Shape, Value};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A single write into a target path.
///
/// # Examples
///
/// ```rust
/// use vizij_api_core::{TypedPath, Value, WriteOp};
///
/// let path = TypedPath::parse("robot/Arm/Joint3.angle")?;
/// let op = WriteOp::new(path, Value::vec3(1.0, 2.0, 3.0));
/// # Ok::<(), String>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct WriteOp {
    /// Canonical target path.
    pub path: TypedPath,
    /// Value payload for the target.
    pub value: Value,
    /// Optional shape metadata for the value.
    pub shape: Option<Shape>,
}

impl WriteOp {
    /// Create a write op without shape metadata.
    pub fn new(path: TypedPath, value: Value) -> Self {
        Self::new_with_shape(path, value, None)
    }

    /// Create a write op with an explicit shape.
    pub fn new_with_shape(path: TypedPath, value: Value, shape: Option<Shape>) -> Self {
        Self { path, value, shape }
    }
}

// Serialize WriteOp as { "path": "<string>", "value": <ValueJSON> }
impl Serialize for WriteOp {
    /// Internal helper for `serialize` (returns an error on invalid input).
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
    /// Internal helper for `deserialize` (returns an error on invalid input).
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

/// A batch of write operations.
///
/// Engines typically emit a `WriteBatch` each tick to describe side effects.
///
/// # Examples
///
/// ```rust
/// use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};
///
/// let mut batch = WriteBatch::new();
/// batch.push(WriteOp::new(
///     TypedPath::parse("robot/Arm/Joint3.angle")?,
///     Value::vec3(0.0, 0.0, 1.0),
/// ));
/// # Ok::<(), String>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WriteBatch(pub Vec<WriteOp>);

impl WriteBatch {
    /// Create an empty batch.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::WriteBatch;
    ///
    /// let batch = WriteBatch::new();
    /// assert!(batch.is_empty());
    /// ```
    pub fn new() -> Self {
        WriteBatch(Vec::new())
    }

    /// Append a single write operation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};
    ///
    /// let mut batch = WriteBatch::new();
    /// batch.push(WriteOp::new(
    ///     TypedPath::parse("robot/Arm/Joint.angle")?,
    ///     Value::Float(1.0),
    /// ));
    /// assert_eq!(batch.iter().count(), 1);
    /// # Ok::<(), String>(())
    /// ```
    pub fn push(&mut self, op: WriteOp) {
        self.0.push(op);
    }

    /// Extend the batch with another iterator of operations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};
    ///
    /// let mut batch = WriteBatch::new();
    /// let ops = vec![
    ///     WriteOp::new(TypedPath::parse("robot/Arm/Joint.angle")?, Value::Float(1.0)),
    ///     WriteOp::new(TypedPath::parse("robot/Arm/Joint.enabled")?, Value::Bool(true)),
    /// ];
    /// batch.extend(ops);
    /// assert_eq!(batch.iter().count(), 2);
    /// # Ok::<(), String>(())
    /// ```
    pub fn extend(&mut self, other: impl IntoIterator<Item = WriteOp>) {
        self.0.extend(other);
    }

    /// Consume the batch and return the inner vector.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};
    ///
    /// let mut batch = WriteBatch::new();
    /// batch.push(WriteOp::new(TypedPath::parse("robot/Arm/Joint.angle")?, Value::Float(1.0)));
    /// let ops = batch.into_vec();
    /// assert_eq!(ops.len(), 1);
    /// # Ok::<(), String>(())
    /// ```
    pub fn into_vec(self) -> Vec<WriteOp> {
        self.0
    }

    /// Iterate over operations in the batch.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::WriteBatch;
    ///
    /// let batch = WriteBatch::new();
    /// assert_eq!(batch.iter().count(), 0);
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &WriteOp> {
        self.0.iter()
    }

    /// Return true when no operations are present.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::WriteBatch;
    ///
    /// let batch = WriteBatch::new();
    /// assert!(batch.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Merge another batch in-place (append). Dedup/merge semantics can be added later.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};
    ///
    /// let mut a = WriteBatch::new();
    /// let mut b = WriteBatch::new();
    /// a.push(WriteOp::new(TypedPath::parse("robot/Arm/Joint.angle")?, Value::Float(1.0)));
    /// b.push(WriteOp::new(TypedPath::parse("robot/Arm/Joint.enabled")?, Value::Bool(true)));
    /// a.append(b);
    /// assert_eq!(a.iter().count(), 2);
    /// # Ok::<(), String>(())
    /// ```
    pub fn append(&mut self, mut other: WriteBatch) {
        self.0.append(&mut other.0)
    }
}

impl fmt::Display for WriteOp {
    /// Internal helper for `fmt`.
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
    /// Internal helper for `writeop_roundtrip_json`.
    fn writeop_roundtrip_json() {
        let tp = TypedPath::parse("robot1/Arm/Joint3.angle").unwrap();
        let op = WriteOp::new(tp, Value::Vec3([1.0, 2.0, 3.0]));
        let s = serde_json::to_string(&op).unwrap();
        let parsed: WriteOp = serde_json::from_str(&s).unwrap();
        assert_eq!(op, parsed);
    }

    #[test]
    /// Internal helper for `writebatch_json_array`.
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
    /// Internal helper for `writeop_roundtrip_with_shape`.
    fn writeop_roundtrip_with_shape() {
        let tp = TypedPath::parse("robot1/Arm/Joint3.angle").unwrap();
        let shape = Shape::new(ShapeId::Vec3);
        let op = WriteOp::new_with_shape(tp, Value::Vec3([1.0, 2.0, 3.0]), Some(shape));
        let s = serde_json::to_string(&op).unwrap();
        let parsed: WriteOp = serde_json::from_str(&s).unwrap();
        assert_eq!(op, parsed);
    }
}
