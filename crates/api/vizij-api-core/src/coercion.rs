//! Coercion helpers between Value shapes.
//! Minimal implementation to support migration: scalar<->vector broadcasting,
//! bool->float, vecN <-> Vector conversions.

use crate::Value;

/// Attempt to coerce a Value into a scalar f32.
/// Rules:
/// - Float -> its value
/// - Bool -> 1.0 / 0.0
/// - Vec2/3/4 -> first component
/// - Vector -> first element or 0.0 if empty
pub fn to_float(v: &Value) -> f32 {
    match v {
        Value::Float(f) => *f,
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Vec2(a) => a[0],
        Value::Vec3(a) => a[0],
        Value::Vec4(a) => a[0],
        Value::Quat(a) => a[0],
        Value::ColorRgba(a) => a[0],
        Value::Transform { pos, .. } => pos[0],
        Value::Vector(vec) => vec.first().copied().unwrap_or(0.0),
        Value::Record(map) => map.values().next().map(to_float).unwrap_or(0.0),
        Value::Array(items) => items.first().map(to_float).unwrap_or(0.0),
        Value::List(items) => items.first().map(to_float).unwrap_or(0.0),
        Value::Tuple(items) => items.first().map(to_float).unwrap_or(0.0),
        Value::Enum(_, boxed) => to_float(boxed),
        Value::Text(_) => 0.0,
    }
}

/// Convert a Value into a Vec<f32> (generic vector).
/// - VecN -> vector of components
/// - Float -> single-element vec
/// - Bool -> single 0/1
/// - Vector -> clone
/// - Transform -> pos components
/// - Enum -> recurse into payload
pub fn to_vector(v: &Value) -> Vec<f32> {
    match v {
        Value::Float(f) => vec![*f],
        Value::Bool(b) => vec![if *b { 1.0 } else { 0.0 }],
        Value::Vec2(a) => vec![a[0], a[1]],
        Value::Vec3(a) => vec![a[0], a[1], a[2]],
        Value::Vec4(a) => vec![a[0], a[1], a[2], a[3]],
        Value::Quat(a) => vec![a[0], a[1], a[2], a[3]],
        Value::ColorRgba(a) => vec![a[0], a[1], a[2], a[3]],
        Value::Transform { pos, .. } => vec![pos[0], pos[1], pos[2]],
        Value::Vector(vec) => vec.clone(),
        Value::Record(map) => map.values().flat_map(to_vector).collect(),
        Value::Array(items) => items.iter().flat_map(to_vector).collect(),
        Value::List(items) => items.iter().flat_map(to_vector).collect(),
        Value::Tuple(items) => items.iter().flat_map(to_vector).collect(),
        Value::Enum(_, boxed) => to_vector(boxed),
        Value::Text(_) => vec![],
    }
}

/// Try to coerce a Value into a Vec3. If impossible, returns a default [0,0,0].
/// Uses broadcasting/coercion rules: scalar -> [s,0,0]? Here we choose scalar -> [s,s,s].
pub fn to_vec3(v: &Value) -> [f32; 3] {
    match v {
        Value::Vec3(a) => *a,
        Value::Vec2(a) => [a[0], a[1], 0.0],
        Value::Vec4(a) => [a[0], a[1], a[2]],
        Value::Float(f) => [*f, *f, *f],
        Value::Bool(b) => {
            if *b {
                [1.0, 1.0, 1.0]
            } else {
                [0.0, 0.0, 0.0]
            }
        }
        Value::Vector(vec) => {
            let mut out = [0.0f32; 3];
            for (i, slot) in out.iter_mut().enumerate() {
                *slot = *vec.get(i).unwrap_or(&0.0);
            }
            out
        }
        Value::Transform { pos, .. } => *pos,
        Value::Record(map) => {
            let mut out = [0.0f32; 3];
            let mut iter = map.values().flat_map(to_vector);
            for slot in out.iter_mut() {
                *slot = iter.next().unwrap_or(0.0);
            }
            out
        }
        Value::Array(items) | Value::List(items) | Value::Tuple(items) => {
            let mut out = [0.0f32; 3];
            let mut iter = items.iter().flat_map(to_vector);
            for slot in out.iter_mut() {
                *slot = iter.next().unwrap_or(0.0);
            }
            out
        }
        Value::Enum(_, boxed) => to_vec3(boxed),
        _ => [0.0, 0.0, 0.0],
    }
}
