//! Value: runtime instances that conform to Shapes.
//! All numeric types use f32 as requested.

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

/// Coarse kind tag for [`Value`] variants.
///
/// This stays local to `value.rs`; shape-based checks live in `shape.rs`.
/// Use it for lightweight pattern matching and dispatch where a full [`ShapeId`]
/// is unnecessary.
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
    Record,
    Array,
    List,
    Tuple,
    Enum,
    Text,
}

/// Runtime value that conforms to a [`ShapeId`].
///
/// This enum is serialized with `serde` using a `{ "type": "...", "data": ... }`
/// tag layout and lowercase variant names to preserve stable JSON payloads.
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

    /// Enum with tag and nested value (value is optional depending on variant)
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

    /// Creates a scalar float value.
    pub fn f(v: f32) -> Self {
        Value::Float(v)
    }

    /// Creates a 3D vector value from components.
    pub fn vec3(x: f32, y: f32, z: f32) -> Self {
        Value::Vec3([x, y, z])
    }

    /// Creates a quaternion value from (x, y, z, w) components.
    pub fn quat(x: f32, y: f32, z: f32, w: f32) -> Self {
        Value::Quat([x, y, z, w])
    }

    /// Creates a transform value from translation, rotation (quat), and scale.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::Value;
    ///
    /// let v = Value::transform([0.0, 1.0, 2.0], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]);
    /// assert_eq!(v.kind(), vizij_api_core::ValueKind::Transform);
    /// ```
    pub fn transform(translation: [f32; 3], rotation: [f32; 4], scale: [f32; 3]) -> Self {
        Value::Transform {
            translation,
            rotation,
            scale,
        }
    }
}
