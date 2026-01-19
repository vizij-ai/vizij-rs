//! Shape definitions (schema/type) for vizij-api-core.
//!
//! Shapes describe the structural layout of values (including nested
//! composites) and serialize with a stable `{ "id": ..., "data": ... }`
//! envelope for interchange with wasm bindings and tooling.

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

/// A field in a record-shaped value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Field {
    /// Field name as it appears in serialized JSON.
    pub name: String,
    /// Shape identifier for the field payload.
    pub shape: ShapeId,
}

/// Structural identifier for a [`Value`](crate::Value).
///
/// This mirrors the public API contract and is serialized using `{ "id": "...",
/// "data": ... }` so tooling can inspect shapes without additional context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "id", content = "data")]
pub enum ShapeId {
    // Primitives
    /// Scalar `f32` value.
    Scalar,
    /// Boolean value (step-only).
    Bool,
    /// Two-component float vector.
    Vec2,
    /// Three-component float vector.
    Vec3,
    /// Four-component float vector.
    Vec4,
    /// Quaternion stored as `[x, y, z, w]`.
    Quat,
    /// RGBA color stored as four floats.
    ColorRgba,
    /// Transform stored as translation, rotation, and scale.
    Transform, // TRS: { pos: vec3, rot: quat, scale: vec3 }
    /// UTF-8 text value.
    Text,
    /// Homogeneous numeric vector (float). Optional length hint enables UIs to
    /// pre-size controls but evaluation remains dynamic.
    Vector {
        #[serde(skip_serializing_if = "Option::is_none")]
        len: Option<usize>,
    },

    // Composite
    /// Record of named fields with shape metadata.
    Record(Vec<Field>),
    /// Fixed-size array of a nested shape.
    Array(Box<ShapeId>, usize),
    /// Variable-length list.
    List(Box<ShapeId>),
    /// Heterogeneous ordered tuple.
    Tuple(Vec<ShapeId>),

    /// Tagged enum: list of (tag, shape) pairs. The shape describes the payload.
    Enum(Vec<(String, ShapeId)>),
}

impl ShapeId {
    /// Convenience: create a record from a list of `(name, shape)` pairs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::ShapeId;
    ///
    /// let record = ShapeId::record_from_pairs([
    ///     ("gain", ShapeId::Scalar),
    ///     ("enabled", ShapeId::Bool),
    /// ]);
    /// ```
    pub fn record_from_pairs(
        pairs: impl IntoIterator<Item = (impl Into<String>, ShapeId)>,
    ) -> Self {
        let fields = pairs
            .into_iter()
            .map(|(n, s)| Field {
                name: n.into(),
                shape: s,
            })
            .collect();
        ShapeId::Record(fields)
    }
}

/// A shape definition with optional metadata.
///
/// Metadata can include units, coordinate space, ranges, color model, etc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Shape {
    /// Structural identifier for the value.
    pub id: ShapeId,
    /// Optional metadata describing units, ranges, or other hints.
    #[serde(default)]
    pub meta: HashMap<String, String>,
}

impl Shape {
    /// Create a shape with an empty metadata map.
    pub fn new(id: ShapeId) -> Self {
        Shape {
            id,
            meta: HashMap::new(),
        }
    }

    /// Add a metadata entry (for example `"units" -> "meters"`).
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.meta.insert(key.into(), value.into());
        self
    }
}
