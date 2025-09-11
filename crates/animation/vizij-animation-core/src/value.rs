#![allow(dead_code)]
//! Core value kinds and typed values for animation sampling/blending.

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValueKind {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    Color,
    Transform,
    Bool,
    Text,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum Value {
    Scalar(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    /// Quaternion (x, y, z, w)
    Quat([f32; 4]),
    /// RGBA color
    Color([f32; 4]),
    /// Transform split to TRS for blending/decomposition
    Transform {
        translation: [f32; 3],
        rotation: [f32; 4], // quat (x,y,z,w)
        scale: [f32; 3],
    },
    /// Step-only boolean value (no blending)
    Bool(bool),
    /// Step-only string/text value (no blending)
    Text(String),
}

impl Value {
    #[inline]
    pub fn kind(&self) -> ValueKind {
        match self {
            Value::Scalar(_) => ValueKind::Scalar,
            Value::Vec2(_) => ValueKind::Vec2,
            Value::Vec3(_) => ValueKind::Vec3,
            Value::Vec4(_) => ValueKind::Vec4,
            Value::Quat(_) => ValueKind::Quat,
            Value::Color(_) => ValueKind::Color,
            Value::Transform { .. } => ValueKind::Transform,
            Value::Bool(_) => ValueKind::Bool,
            Value::Text(_) => ValueKind::Text,
        }
    }
}
