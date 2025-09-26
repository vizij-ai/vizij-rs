#![allow(dead_code)]
//! Helpers for computing time derivatives of animation [`Value`]s.

use vizij_api_core::Value;

fn diff_vec(dst: &mut [f32], a: &[f32], b: &[f32], inv_dt: f32) {
    for (out, (cur, prev)) in dst.iter_mut().zip(a.iter().zip(b.iter())) {
        *out = (cur - prev) * inv_dt;
    }
}

/// Compute the time derivative `(current - previous) / dt` for a [`Value`].
///
/// Returns `None` for value kinds where derivatives are not well defined
/// (booleans, text, enums, complex aggregates, etc.) or when `dt <= 0`.
pub(crate) fn derivative_value(current: &Value, previous: &Value, dt: f32) -> Option<Value> {
    if dt <= 0.0 {
        return None;
    }
    let inv_dt = dt.recip();
    match (current, previous) {
        (Value::Float(c), Value::Float(p)) => Some(Value::Float((c - p) * inv_dt)),
        (Value::Vec2(c), Value::Vec2(p)) => {
            let mut out = [0.0; 2];
            diff_vec(&mut out, c, p, inv_dt);
            Some(Value::Vec2(out))
        }
        (Value::Vec3(c), Value::Vec3(p)) => {
            let mut out = [0.0; 3];
            diff_vec(&mut out, c, p, inv_dt);
            Some(Value::Vec3(out))
        }
        (Value::Vec4(c), Value::Vec4(p)) => {
            let mut out = [0.0; 4];
            diff_vec(&mut out, c, p, inv_dt);
            Some(Value::Vec4(out))
        }
        (Value::Quat(c), Value::Quat(p)) => {
            let mut out = [0.0; 4];
            diff_vec(&mut out, c, p, inv_dt);
            Some(Value::Quat(out))
        }
        (Value::ColorRgba(c), Value::ColorRgba(p)) => {
            let mut out = [0.0; 4];
            diff_vec(&mut out, c, p, inv_dt);
            Some(Value::ColorRgba(out))
        }
        (
            Value::Transform {
                pos: c_pos,
                rot: c_rot,
                scale: c_scale,
            },
            Value::Transform {
                pos: p_pos,
                rot: p_rot,
                scale: p_scale,
            },
        ) => {
            let mut d_pos = [0.0; 3];
            diff_vec(&mut d_pos, c_pos, p_pos, inv_dt);
            let mut d_rot = [0.0; 4];
            diff_vec(&mut d_rot, c_rot, p_rot, inv_dt);
            let mut d_scale = [0.0; 3];
            diff_vec(&mut d_scale, c_scale, p_scale, inv_dt);
            Some(Value::Transform {
                pos: d_pos,
                rot: d_rot,
                scale: d_scale,
            })
        }
        (Value::Vector(c), Value::Vector(p)) if c.len() == p.len() => {
            let mut out = Vec::with_capacity(c.len());
            for (cur, prev) in c.iter().zip(p.iter()) {
                out.push((cur - prev) * inv_dt);
            }
            Some(Value::Vector(out))
        }
        _ => None,
    }
}
