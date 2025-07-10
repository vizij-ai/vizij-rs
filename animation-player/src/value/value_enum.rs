use crate::value::color::Color;
use crate::value::euler::Euler;
use crate::value::transform::Transform;
use crate::value::utils::hash_f64;
use crate::value::vector2::Vector2;
use crate::value::vector3::Vector3;
use crate::value::vector4::Vector4;
use crate::AnimationError;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::ops::{Add, Mul, Sub};

/// Enum representing the type of a `Value`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValueType {
    Float,
    Integer,
    Boolean,
    String,
    Vector2,
    Vector3,
    Vector4,
    Euler,
    Color,
    Transform,
}

impl ValueType {
    pub fn name(&self) -> &'static str {
        match self {
            ValueType::Float => "Float",
            ValueType::Integer => "Integer",
            ValueType::Boolean => "Boolean",
            ValueType::String => "String",
            ValueType::Vector2 => "Vector2",
            ValueType::Vector3 => "Vector3",
            ValueType::Vector4 => "Vector4",
            ValueType::Color => "Color",
            ValueType::Transform => "Transform",
            ValueType::Euler => "Euler",
        }
    }
}

/// Primary value type supporting all animation data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    /// 64-bit floating point number
    Float(f64),
    /// 64-bit signed integer
    Integer(i64),
    /// Boolean value
    Boolean(bool),
    /// UTF-8 string
    String(String),
    /// 2D vector
    Vector2(Vector2),
    /// 3D vector
    Vector3(Vector3),
    /// 4D vector (often used for colors or quaternions)
    Vector4(Vector4),
    /// Euler angles for rotation
    Euler(Euler),
    /// Color in various formats
    Color(Color),
    /// 3D transform (position, rotation, scale)
    Transform(Transform),
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::Float(f) => {
                0u8.hash(state); // discriminant
                hash_f64(*f, state);
            }
            Value::Integer(i) => {
                1u8.hash(state); // discriminant
                i.hash(state);
            }
            Value::Boolean(b) => {
                2u8.hash(state); // discriminant
                b.hash(state);
            }
            Value::String(s) => {
                3u8.hash(state); // discriminant
                s.hash(state);
            }
            Value::Vector2(v) => {
                4u8.hash(state); // discriminant
                v.hash(state);
            }
            Value::Vector3(v) => {
                5u8.hash(state); // discriminant
                v.hash(state);
            }
            Value::Vector4(v) => {
                6u8.hash(state); // discriminant
                v.hash(state);
            }
            Value::Euler(e) => {
                7u8.hash(state); // discriminant
                e.hash(state);
            }
            Value::Color(c) => {
                8u8.hash(state); // discriminant
                c.hash(state);
            }
            Value::Transform(t) => {
                9u8.hash(state); // discriminant
                t.hash(state);
            }
        }
    }
}

impl Value {
    /// Get the type of this value as a `ValueType` enum.
    pub fn value_type(&self) -> ValueType {
        match self {
            Value::Float(_) => ValueType::Float,
            Value::Integer(_) => ValueType::Integer,
            Value::Boolean(_) => ValueType::Boolean,
            Value::String(_) => ValueType::String,
            Value::Vector2(_) => ValueType::Vector2,
            Value::Vector3(_) => ValueType::Vector3,
            Value::Vector4(_) => ValueType::Vector4,
            Value::Euler(_) => ValueType::Euler,
            Value::Color(_) => ValueType::Color,
            Value::Transform(_) => ValueType::Transform,
        }
    }

    pub fn as_transform(&self) -> Option<&Transform> {
        if let Self::Transform(v) = self {
            Some(v)
        } else {
            None
        }
    }

    /// Check if this value can be interpolated with another value
    pub fn can_interpolate_with(&self, other: &Value) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }

    /// Get the interpolatable components of this value
    pub fn interpolatable_components(&self) -> Vec<f64> {
        match self {
            Value::Float(f) => vec![*f],
            Value::Integer(i) => vec![*i as f64],
            Value::Boolean(b) => vec![if *b { 1.0 } else { 0.0 }],
            Value::String(_) => vec![], // Strings are not interpolatable
            Value::Vector2(v) => vec![v.x, v.y],
            Value::Vector3(v) => vec![v.x, v.y, v.z],
            Value::Vector4(v) => vec![v.x, v.y, v.z, v.w],
            Value::Euler(e) => vec![e.r, e.p, e.y],
            Value::Color(c) => {
                let (r, g, b, a) = c.to_rgba();
                vec![r, g, b, a]
            }
            Value::Transform(t) => vec![
                t.position.x,
                t.position.y,
                t.position.z,
                t.rotation.x,
                t.rotation.y,
                t.rotation.z,
                t.rotation.w,
                t.scale.x,
                t.scale.y,
                t.scale.z,
            ],
        }
    }

    /// Calculate numerical derivative between two values
    pub fn calculate_derivative(
        value_before: &Value,
        value_after: &Value,
        delta_time: f64,
    ) -> Option<Value> {
        if !value_before.can_interpolate_with(value_after) {
            return None;
        }

        match (value_before, value_after) {
            (Value::Float(a), Value::Float(b)) => Some(Value::Float((b - a) / delta_time)),

            (Value::Vector2(a), Value::Vector2(b)) => Some(Value::Vector2(Vector2::new(
                (b.x - a.x) / delta_time,
                (b.y - a.y) / delta_time,
            ))),

            (Value::Vector3(a), Value::Vector3(b)) => Some(Value::Vector3(Vector3::new(
                (b.x - a.x) / delta_time,
                (b.y - a.y) / delta_time,
                (b.z - a.z) / delta_time,
            ))),

            (Value::Vector4(a), Value::Vector4(b)) => Some(Value::Vector4(Vector4::new(
                (b.x - a.x) / delta_time,
                (b.y - a.y) / delta_time,
                (b.z - a.z) / delta_time,
                (b.w - a.w) / delta_time,
            ))),

            (Value::Euler(a), Value::Euler(b)) => Some(Value::Euler(Euler::new(
                (b.r - a.r) / delta_time,
                (b.p - a.p) / delta_time,
                (b.y - a.y) / delta_time,
            ))),

            (Value::Transform(a), Value::Transform(b)) => {
                // Position velocity
                let pos_vel = Vector3::new(
                    (b.position.x - a.position.x) / delta_time,
                    (b.position.y - a.position.y) / delta_time,
                    (b.position.z - a.position.z) / delta_time,
                );

                // Angular velocity (simplified - difference in quaternion components)
                let rot_vel = Vector4::new(
                    (b.rotation.x - a.rotation.x) / delta_time,
                    (b.rotation.y - a.rotation.y) / delta_time,
                    (b.rotation.z - a.rotation.z) / delta_time,
                    (b.rotation.w - a.rotation.w) / delta_time,
                );

                // Scale rate
                let scale_vel = Vector3::new(
                    (b.scale.x - a.scale.x) / delta_time,
                    (b.scale.y - a.scale.y) / delta_time,
                    (b.scale.z - a.scale.z) / delta_time,
                );

                Some(Value::Transform(Transform::new(
                    pos_vel, rot_vel, scale_vel,
                )))
            }

            (Value::Color(a), Value::Color(b)) => {
                let (r1, g1, b1, a1) = a.to_rgba();
                let (r2, g2, b2, a2) = b.to_rgba();
                Some(Value::Color(Color::rgba(
                    (r2 - r1) / delta_time,
                    (g2 - g1) / delta_time,
                    (b2 - b1) / delta_time,
                    (a2 - a1) / delta_time,
                )))
            }

            // Discrete values have zero derivative
            (Value::Boolean(_), Value::Boolean(_)) => Some(Value::Float(0.0)),
            (Value::String(_), Value::String(_)) => None, // Strings not differentiable
            (Value::Integer(a), Value::Integer(b)) => {
                Some(Value::Float((*b - *a) as f64 / delta_time))
            }

            _ => None, // Type mismatch
        }
    }

    /// Create a value from interpolatable components
    pub fn from_components(
        value_type: ValueType,
        components: &[f64],
    ) -> Result<Value, AnimationError> {
        match value_type {
            ValueType::Float => {
                if components.len() != 1 {
                    return Err(AnimationError::InvalidValue {
                        reason: format!("Float requires 1 component, got {}", components.len()),
                    });
                }
                Ok(Value::Float(components[0]))
            }
            ValueType::Integer => {
                if components.len() != 1 {
                    return Err(AnimationError::InvalidValue {
                        reason: format!("Integer requires 1 component, got {}", components.len()),
                    });
                }
                Ok(Value::Integer(components[0] as i64))
            }
            ValueType::Boolean => {
                if components.len() != 1 {
                    return Err(AnimationError::InvalidValue {
                        reason: format!("Boolean requires 1 component, got {}", components.len()),
                    });
                }
                Ok(Value::Boolean(components[0] >= 0.5))
            }
            ValueType::String => Err(AnimationError::InvalidValue {
                reason: "String values cannot be created from components".to_string(),
            }),
            ValueType::Vector2 => {
                if components.len() != 2 {
                    return Err(AnimationError::InvalidValue {
                        reason: format!("Vector2 requires 2 components, got {}", components.len()),
                    });
                }
                Ok(Value::Vector2(Vector2::new(components[0], components[1])))
            }
            ValueType::Vector3 => {
                if components.len() != 3 {
                    return Err(AnimationError::InvalidValue {
                        reason: format!("Vector3 requires 3 components, got {}", components.len()),
                    });
                }
                Ok(Value::Vector3(Vector3::new(
                    components[0],
                    components[1],
                    components[2],
                )))
            }
            ValueType::Vector4 => {
                if components.len() != 4 {
                    return Err(AnimationError::InvalidValue {
                        reason: format!("Vector4 requires 4 components, got {}", components.len()),
                    });
                }
                Ok(Value::Vector4(Vector4::new(
                    components[0],
                    components[1],
                    components[2],
                    components[3],
                )))
            }
            ValueType::Euler => {
                if components.len() != 3 {
                    return Err(AnimationError::InvalidValue {
                        reason: format!("Euler requires 3 components, got {}", components.len()),
                    });
                }
                Ok(Value::Euler(Euler::new(
                    components[0],
                    components[1],
                    components[2],
                )))
            }
            ValueType::Color => {
                if components.len() != 4 {
                    return Err(AnimationError::InvalidValue {
                        reason: format!("Color requires 4 components, got {}", components.len()),
                    });
                }
                Ok(Value::Color(Color::rgba(
                    components[0],
                    components[1],
                    components[2],
                    components[3],
                )))
            }
            ValueType::Transform => {
                if components.len() != 10 {
                    return Err(AnimationError::InvalidValue {
                        reason: format!(
                            "Transform requires 10 components, got {}",
                            components.len()
                        ),
                    });
                }
                // Extract position, rotation (quaternion), and scale components
                let position = Vector3::new(components[0], components[1], components[2]);
                let rotation_components: [f64; 4] =
                    [components[3], components[4], components[5], components[6]];
                let scale = Vector3::new(components[7], components[8], components[9]);

                // Reconstruct Transform
                Ok(Value::Transform(Transform::new(
                    position,
                    Vector4::new(
                        rotation_components[0],
                        rotation_components[1],
                        rotation_components[2],
                        rotation_components[3],
                    ),
                    scale,
                )))
            }
        }
    }
}

// Conversion implementations
impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Value::Float(value as f64)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Integer(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Integer(value as i64)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Boolean(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_string())
    }
}

impl From<Vector2> for Value {
    fn from(value: Vector2) -> Self {
        Value::Vector2(value)
    }
}

impl From<Vector3> for Value {
    fn from(value: Vector3) -> Self {
        Value::Vector3(value)
    }
}

impl From<Vector4> for Value {
    fn from(value: Vector4) -> Self {
        Value::Vector4(value)
    }
}

impl From<Euler> for Value {
    fn from(value: Euler) -> Self {
        Value::Euler(value)
    }
}

impl From<Color> for Value {
    fn from(value: Color) -> Self {
        Value::Color(value)
    }
}

impl From<Transform> for Value {
    fn from(value: Transform) -> Self {
        Value::Transform(value)
    }
}

// TryFrom implementations for extracting values
impl TryFrom<Value> for f64 {
    type Error = AnimationError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Float(f) => Ok(f),
            _ => Err(AnimationError::ValueTypeMismatch {
                expected: ValueType::Float,
                actual: value.value_type(),
            }),
        }
    }
}

impl TryFrom<Value> for i64 {
    type Error = AnimationError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Integer(i) => Ok(i),
            _ => Err(AnimationError::ValueTypeMismatch {
                expected: ValueType::Integer,
                actual: value.value_type(),
            }),
        }
    }
}

impl TryFrom<Value> for bool {
    type Error = AnimationError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Boolean(b) => Ok(b),
            _ => Err(AnimationError::ValueTypeMismatch {
                expected: ValueType::Boolean,
                actual: value.value_type(),
            }),
        }
    }
}

impl TryFrom<Value> for String {
    type Error = AnimationError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(s) => Ok(s),
            _ => Err(AnimationError::ValueTypeMismatch {
                expected: ValueType::String,
                actual: value.value_type(),
            }),
        }
    }
}

impl TryFrom<Value> for Vector2 {
    type Error = AnimationError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Vector2(v) => Ok(v),
            _ => Err(AnimationError::ValueTypeMismatch {
                expected: ValueType::Vector2,
                actual: value.value_type(),
            }),
        }
    }
}

impl TryFrom<Value> for Vector3 {
    type Error = AnimationError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Vector3(v) => Ok(v),
            _ => Err(AnimationError::ValueTypeMismatch {
                expected: ValueType::Vector3,
                actual: value.value_type(),
            }),
        }
    }
}

impl TryFrom<Value> for Vector4 {
    type Error = AnimationError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Vector4(v) => Ok(v),
            _ => Err(AnimationError::ValueTypeMismatch {
                expected: ValueType::Vector4,
                actual: value.value_type(),
            }),
        }
    }
}

impl TryFrom<Value> for Euler {
    type Error = AnimationError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Euler(e) => Ok(e),
            _ => Err(AnimationError::ValueTypeMismatch {
                expected: ValueType::Euler,
                actual: value.value_type(),
            }),
        }
    }
}

impl TryFrom<Value> for Color {
    type Error = AnimationError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Color(c) => Ok(c),
            _ => Err(AnimationError::ValueTypeMismatch {
                expected: ValueType::Color,
                actual: value.value_type(),
            }),
        }
    }
}

impl TryFrom<Value> for Transform {
    type Error = AnimationError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Transform(t) => Ok(t),
            _ => Err(AnimationError::ValueTypeMismatch {
                expected: ValueType::Transform,
                actual: value.value_type(),
            }),
        }
    }
}

impl Add for Value {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        let c1 = self.interpolatable_components();
        let c2 = rhs.interpolatable_components();
        let res: Vec<f64> = c1.iter().zip(c2.iter()).map(|(a, b)| a + b).collect();
        Value::from_components(self.value_type(), &res).unwrap_or(self)
    }
}

impl Sub for Value {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        let c1 = self.interpolatable_components();
        let c2 = rhs.interpolatable_components();
        let res: Vec<f64> = c1.iter().zip(c2.iter()).map(|(a, b)| a - b).collect();
        Value::from_components(self.value_type(), &res).unwrap_or(self)
    }
}

impl Mul<f64> for Value {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self::Output {
        let c1 = self.interpolatable_components();
        let res: Vec<f64> = c1.iter().map(|c| c * rhs).collect();
        Value::from_components(self.value_type(), &res).unwrap_or(self)
    }
}
