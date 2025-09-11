#![allow(dead_code)]
//! Interpolation helpers:
//! - step_value (step semantics)
//! - linear_value (component-wise + quat NLERP)
//! - bezier_value (cubic-bezier timing -> linear blend)
//! - quaternion NLERP with shortest-arc normalization

use crate::value::Value;

/// Linear interpolation of scalars.
#[inline]
pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
pub fn lerp_vec2(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [lerp_f32(a[0], b[0], t), lerp_f32(a[1], b[1], t)]
}

#[inline]
pub fn lerp_vec3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        lerp_f32(a[0], b[0], t),
        lerp_f32(a[1], b[1], t),
        lerp_f32(a[2], b[2], t),
    ]
}

#[inline]
pub fn lerp_vec4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        lerp_f32(a[0], b[0], t),
        lerp_f32(a[1], b[1], t),
        lerp_f32(a[2], b[2], t),
        lerp_f32(a[3], b[3], t),
    ]
}

#[inline]
fn dot4(a: [f32; 4], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

#[inline]
fn normalize4(mut q: [f32; 4]) -> [f32; 4] {
    let len2 = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3];
    if len2 > 0.0 {
        let inv_len = len2.sqrt().recip();
        q[0] *= inv_len;
        q[1] *= inv_len;
        q[2] *= inv_len;
        q[3] *= inv_len;
    }
    q
}

/// Quaternion NLERP with shortest-arc correction.
/// If dot < 0, negate the second quaternion to ensure the shortest path.
/// Returns a normalized quaternion (x,y,z,w).
#[inline]
pub fn nlerp_quat(a: [f32; 4], mut b: [f32; 4], t: f32) -> [f32; 4] {
    let d = dot4(a, b);
    if d < 0.0 {
        b[0] = -b[0];
        b[1] = -b[1];
        b[2] = -b[2];
        b[3] = -b[3];
    }
    let q = [
        lerp_f32(a[0], b[0], t),
        lerp_f32(a[1], b[1], t),
        lerp_f32(a[2], b[2], t),
        lerp_f32(a[3], b[3], t),
    ];
    normalize4(q)
}

/// Step interpolation: choose left value.
#[inline]
pub fn step_value(a: &Value) -> Value {
    a.clone()
}

/// Linear interpolation across Value kinds (Transform uses TRS with quat NLERP).
pub fn linear_value(a: &Value, b: &Value, t: f32) -> Value {
    match (a, b) {
        (Value::Scalar(va), Value::Scalar(vb)) => Value::Scalar(lerp_f32(*va, *vb, t)),
        (Value::Vec2(va), Value::Vec2(vb)) => Value::Vec2(lerp_vec2(*va, *vb, t)),
        (Value::Vec3(va), Value::Vec3(vb)) => Value::Vec3(lerp_vec3(*va, *vb, t)),
        (Value::Vec4(va), Value::Vec4(vb)) => Value::Vec4(lerp_vec4(*va, *vb, t)),
        (Value::Quat(qa), Value::Quat(qb)) => Value::Quat(nlerp_quat(*qa, *qb, t)),
        (Value::Color(ca), Value::Color(cb)) => Value::Color(lerp_vec4(*ca, *cb, t)),
        (
            Value::Transform {
                translation: ta,
                rotation: ra,
                scale: sa,
            },
            Value::Transform {
                translation: tb,
                rotation: rb,
                scale: sb,
            },
        ) => Value::Transform {
            translation: lerp_vec3(*ta, *tb, t),
            rotation: nlerp_quat(*ra, *rb, t),
            scale: lerp_vec3(*sa, *sb, t),
        },
        // Fallback: if types mismatch, prefer left (fail-soft).
        _ => a.clone(),
    }
}

/// Cubic Bezier basis function
#[inline]
fn cubic_bezier(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3
}

/// Given control points (x1, y1, x2, y2) and an input t in [0,1],
/// compute the eased y by inverting the x bezier via binary search.
#[inline]
fn bezier_ease_t(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    // Fast path: Bezier(0,0,1,1) is exactly linear -> eased t == t
    if x1 == 0.0 && y1 == 0.0 && x2 == 1.0 && y2 == 1.0 {
        return t;
    }
    // Monotonic X in [0,1] assumed for x1/x2 ∈ [0,1]
    let mut lo = 0.0f32;
    let mut hi = 1.0f32;
    let mut mid = t;
    // Increase precision to reduce error for near-linear curves
    for _ in 0..24 {
        let x = cubic_bezier(0.0, x1, x2, 1.0, mid);
        if (x - t).abs() < 1e-6 {
            break;
        }
        if x < t {
            lo = mid;
        } else {
            hi = mid;
        }
        mid = 0.5 * (lo + hi);
    }
    cubic_bezier(0.0, y1, y2, 1.0, mid)
}

/// Bezier easing across Value kinds: compute eased t, then use linear blend.
/// Control points are (x1, y1, x2, y2).
#[inline]
pub fn bezier_value(a: &Value, b: &Value, t: f32, ctrl: [f32; 4]) -> Value {
    let eased = bezier_ease_t(t, ctrl[0], ctrl[1], ctrl[2], ctrl[3]);
    linear_value(a, b, eased)
}
