//! Value: runtime instances that conform to Shapes.
//! All numeric types use f32 as requested.

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

/// Lightweight kind enum for convenience. This is intentionally local to
/// `value.rs` for now; the Shape/ShapeId types live in `shape.rs` and will
/// be used to perform richer checks. This helper is useful for pattern-matching
/// and quick dispatch during migration.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ValueKind {
    /// Single floating-point scalar.
    Float,
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
    Transform,
    /// Variable-length homogeneous float vector.
    Vector,
    /// Named-field record/struct.
    Record,
    /// Fixed-size homogeneous array.
    Array,
    /// Variable-length homogeneous list.
    List,
    /// Ordered heterogeneous tuple.
    Tuple,
    /// Tagged enum with payload.
    Enum,
    /// UTF-8 text payload.
    Text,
}

/// Normalized runtime value payload shared across Vizij crates and wasm bridges.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data", rename_all = "lowercase")]
pub enum Value {
    /// Scalar float
    Float(f32),

    /// Boolean (step)
    Bool(bool),

    /// 2D vector
    Vec2([f32; 2]),

    /// 3D vector
    Vec3([f32; 3]),

    /// 4D vector
    Vec4([f32; 4]),

    /// Quaternion (x, y, z, w)
    Quat([f32; 4]),

    /// RGBA color (linear by convention)
    ColorRgba([f32; 4]),

    /// Transform with translation, rotation (quat), scale
    Transform {
        translation: [f32; 3],
        rotation: [f32; 4], // quat (x,y,z,w)
        scale: [f32; 3],
    },

    /// Generic, variable-length numeric vector
    Vector(Vec<f32>),

    /// Enum with tag and nested payload value.
    Enum(String, Box<Value>),

    /// Text / string; step-only for interpolation
    Text(String),

    /// Record of named fields (order is not guaranteed)
    Record(HashMap<String, Value>),

    /// Fixed-size homogeneous array
    Array(Vec<Value>),

    /// Variable-length list (alias of Vec but distinct ShapeId)
    List(Vec<Value>),

    /// Heterogeneous tuple (ordered elements)
    Tuple(Vec<Value>),
}

impl Value {
    /// Return the coarse kind of this value.
    #[inline]
    pub fn kind(&self) -> ValueKind {
        match self {
            Value::Float(_) => ValueKind::Float,
            Value::Bool(_) => ValueKind::Bool,
            Value::Vec2(_) => ValueKind::Vec2,
            Value::Vec3(_) => ValueKind::Vec3,
            Value::Vec4(_) => ValueKind::Vec4,
            Value::Quat(_) => ValueKind::Quat,
            Value::ColorRgba(_) => ValueKind::ColorRgba,
            Value::Transform { .. } => ValueKind::Transform,
            Value::Vector(_) => ValueKind::Vector,
            Value::Record(_) => ValueKind::Record,
            Value::Array(_) => ValueKind::Array,
            Value::List(_) => ValueKind::List,
            Value::Tuple(_) => ValueKind::Tuple,
            Value::Enum(_, _) => ValueKind::Enum,
            Value::Text(_) => ValueKind::Text,
        }
    }

    /// Convenience constructor for a scalar float.
    pub fn f(v: f32) -> Self {
        Value::Float(v)
    }

    /// Convenience constructor for a 3D vector.
    pub fn vec3(x: f32, y: f32, z: f32) -> Self {
        Value::Vec3([x, y, z])
    }

    /// Convenience constructor for a quaternion.
    pub fn quat(x: f32, y: f32, z: f32, w: f32) -> Self {
        Value::Quat([x, y, z, w])
    }

    /// Convenience constructor for a transform payload.
    pub fn transform(translation: [f32; 3], rotation: [f32; 4], scale: [f32; 3]) -> Self {
        Value::Transform {
            translation,
            rotation,
            scale,
        }
    }
}
