//! Value: runtime instances that conform to Shapes.
//! All numeric types use f32 as requested.

use serde::{Deserialize, Serialize};

/// Lightweight kind enum for convenience. This is intentionally local to
/// `value.rs` for now; the Shape/ShapeId types live in `shape.rs` and will
/// be used to perform richer checks. This helper is useful for pattern-matching
/// and quick dispatch during migration.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ValueKind {
    Float,
    Bool,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    ColorRgba,
    Transform,
    Vector,
    Enum,
    Text,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
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
        pos: [f32; 3],
        rot: [f32; 4], // quat (x,y,z,w)
        scale: [f32; 3],
    },

    /// Generic, variable-length numeric vector
    Vector(Vec<f32>),

    /// Enum with tag and nested value (value is optional depending on variant)
    Enum(String, Box<Value>),

    /// Text / string; step-only for interpolation
    Text(String),
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
            Value::Enum(_, _) => ValueKind::Enum,
            Value::Text(_) => ValueKind::Text,
        }
    }

    /// Convenience constructors
    pub fn f(v: f32) -> Self {
        Value::Float(v)
    }

    pub fn vec3(x: f32, y: f32, z: f32) -> Self {
        Value::Vec3([x, y, z])
    }

    pub fn quat(x: f32, y: f32, z: f32, w: f32) -> Self {
        Value::Quat([x, y, z, w])
    }

    pub fn transform(pos: [f32; 3], rot: [f32; 4], scale: [f32; 3]) -> Self {
        Value::Transform { pos, rot, scale }
    }
}
