//! Shape definitions (schema/type) for vizij-api-core.

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

/// A field in a Record shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Field {
    /// Field name as it appears in the record payload.
    pub name: String,
    /// Declared shape of the field value.
    pub shape: ShapeId,
}

/// The ShapeId expresses the structural type of a value.
/// This mirrors the design in the API report but keeps the initial
/// surface area focused on the requested set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "id", content = "data")]
pub enum ShapeId {
    // Primitives
    /// Single floating-point scalar.
    Scalar,
    /// Boolean value.
    Bool,
    /// Fixed-size 2D float vector.
    Vec2,
    /// Fixed-size 3D float vector.
    Vec3,
    /// Fixed-size 4D float vector.
    Vec4,
    /// Quaternion stored as `[x, y, z, w]`.
    Quat,
    /// RGBA color value.
    ColorRgba,
    /// Translation/rotation/scale transform payload.
    Transform, // TRS: { pos: vec3, rot: quat, scale: vec3 }
    /// UTF-8 text payload.
    Text,
    /// Homogeneous numeric vector (float). Optional length hint enables UIs to
    /// pre-size controls but evaluation remains dynamic.
    Vector {
        #[serde(skip_serializing_if = "Option::is_none")]
        len: Option<usize>,
    },

    // Composite
    /// Named-field record/struct payload.
    Record(Vec<Field>),
    /// Fixed-size array of a nested shape
    Array(Box<ShapeId>, usize),
    /// Variable-length list
    List(Box<ShapeId>),
    /// Heterogeneous ordered tuple
    Tuple(Vec<ShapeId>),

    /// Tagged enum: list of (tag, shape) pairs. The shape describes associated payload.
    Enum(Vec<(String, ShapeId)>),
}

impl ShapeId {
    /// Convenience: create a Record from a list of (name, shape) pairs
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

/// A Shape pairs an identity (ShapeId) with optional metadata.
/// Metadata can include units, space, ranges, color model, etc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Shape {
    /// Structural identity of the value.
    pub id: ShapeId,
    /// Optional metadata such as units, spaces, ranges, or color model hints.
    ///
    /// Common keys include `unit`, `space`, `range`, and `color_space`, but the map remains
    /// open-ended so hosts can carry adapter-specific hints.
    #[serde(default)]
    pub meta: HashMap<String, String>,
}

impl Shape {
    /// Construct a shape with empty metadata.
    pub fn new(id: ShapeId) -> Self {
        Shape {
            id,
            meta: HashMap::new(),
        }
    }

    /// Attach one metadata entry and return the updated shape.
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.meta.insert(key.into(), value.into());
        self
    }
}
