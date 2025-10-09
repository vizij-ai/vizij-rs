#![allow(dead_code)]
//! Interpolation helpers:
//! - step_value (step semantics)
//! - linear_value (component-wise + quat NLERP)
//! - bezier_value (cubic-bezier timing -> linear blend)
//! - quaternion NLERP with shortest-arc normalization

use vizij_api_core::Value;

#[inline]
fn sub_vec4(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

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

#[inline]
fn quat_derivative_components(
    a: [f32; 4],
    mut b: [f32; 4],
    t: f32,
    dt_du: f32,
) -> ([f32; 4], [f32; 4]) {
    let d = dot4(a, b);
    if d < 0.0 {
        b = [-b[0], -b[1], -b[2], -b[3]];
    }

    let raw = [
        lerp_f32(a[0], b[0], t),
        lerp_f32(a[1], b[1], t),
        lerp_f32(a[2], b[2], t),
        lerp_f32(a[3], b[3], t),
    ];

    let diff = sub_vec4(b, a);
    let raw_dt = [
        diff[0] * dt_du,
        diff[1] * dt_du,
        diff[2] * dt_du,
        diff[3] * dt_du,
    ];

    (raw, raw_dt)
}

#[inline]
fn normalize4_derivative(raw: [f32; 4], raw_dt: [f32; 4]) -> [f32; 4] {
    let norm_sq = dot4(raw, raw);
    if norm_sq <= 0.0 {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let norm = norm_sq.sqrt();
    let inv_norm = norm.recip();
    let dot = dot4(raw, raw_dt);
    let inv_norm3 = inv_norm * inv_norm * inv_norm;
    [
        raw_dt[0] * inv_norm - raw[0] * dot * inv_norm3,
        raw_dt[1] * inv_norm - raw[1] * dot * inv_norm3,
        raw_dt[2] * inv_norm - raw[2] * dot * inv_norm3,
        raw_dt[3] * inv_norm - raw[3] * dot * inv_norm3,
    ]
}

/// Step interpolation: choose left value.
#[inline]
pub fn step_value(a: &Value) -> Value {
    a.clone()
}

/// Linear interpolation across Value kinds (Transform uses TRS with quat NLERP).
pub fn linear_value(a: &Value, b: &Value, t: f32) -> Value {
    match (a, b) {
        (Value::Float(va), Value::Float(vb)) => Value::Float(lerp_f32(*va, *vb, t)),
        (Value::Vec2(va), Value::Vec2(vb)) => Value::Vec2(lerp_vec2(*va, *vb, t)),
        (Value::Vec3(va), Value::Vec3(vb)) => Value::Vec3(lerp_vec3(*va, *vb, t)),
        (Value::Vec4(va), Value::Vec4(vb)) => Value::Vec4(lerp_vec4(*va, *vb, t)),
        (Value::Quat(qa), Value::Quat(qb)) => Value::Quat(nlerp_quat(*qa, *qb, t)),
        (Value::ColorRgba(ca), Value::ColorRgba(cb)) => Value::ColorRgba(lerp_vec4(*ca, *cb, t)),
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

/// Linear interpolation derivative across Value kinds.
pub fn linear_derivative(a: &Value, b: &Value, t: f32, dt_du: f32) -> Value {
    match (a, b) {
        (Value::Float(va), Value::Float(vb)) => Value::Float((*vb - *va) * dt_du),
        (Value::Vec2(va), Value::Vec2(vb)) => {
            Value::Vec2([(vb[0] - va[0]) * dt_du, (vb[1] - va[1]) * dt_du])
        }
        (Value::Vec3(va), Value::Vec3(vb)) => Value::Vec3([
            (vb[0] - va[0]) * dt_du,
            (vb[1] - va[1]) * dt_du,
            (vb[2] - va[2]) * dt_du,
        ]),
        (Value::Vec4(va), Value::Vec4(vb)) => Value::Vec4([
            (vb[0] - va[0]) * dt_du,
            (vb[1] - va[1]) * dt_du,
            (vb[2] - va[2]) * dt_du,
            (vb[3] - va[3]) * dt_du,
        ]),
        (Value::Quat(qa), Value::Quat(qb)) => {
            let (raw, raw_dt) = quat_derivative_components(*qa, *qb, t, dt_du);
            Value::Quat(normalize4_derivative(raw, raw_dt))
        }
        (Value::ColorRgba(ca), Value::ColorRgba(cb)) => Value::ColorRgba([
            (cb[0] - ca[0]) * dt_du,
            (cb[1] - ca[1]) * dt_du,
            (cb[2] - ca[2]) * dt_du,
            (cb[3] - ca[3]) * dt_du,
        ]),
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
        ) => {
            let pos = [
                (tb[0] - ta[0]) * dt_du,
                (tb[1] - ta[1]) * dt_du,
                (tb[2] - ta[2]) * dt_du,
            ];
            let scale = [
                (sb[0] - sa[0]) * dt_du,
                (sb[1] - sa[1]) * dt_du,
                (sb[2] - sa[2]) * dt_du,
            ];
            let (raw, raw_dt) = quat_derivative_components(*ra, *rb, t, dt_du);
            Value::Transform {
                translation: pos,
                rotation: normalize4_derivative(raw, raw_dt),
                scale,
            }
        }
        _ => Value::Float(0.0),
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

#[inline]
fn cubic_bezier_derivative(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * (p1 - p0) + 6.0 * u * t * (p2 - p1) + 3.0 * t * t * (p3 - p2)
}

#[inline]
fn bezier_ease_with_derivative(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> (f32, f32) {
    let t = t.clamp(0.0, 1.0);
    if x1 == 0.0 && y1 == 0.0 && x2 == 1.0 && y2 == 1.0 {
        return (t, 1.0);
    }
    let mut lo = 0.0f32;
    let mut hi = 1.0f32;
    let mut mid = t;
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
    let eased = cubic_bezier(0.0, y1, y2, 1.0, mid);
    let dx_dt = cubic_bezier_derivative(0.0, x1, x2, 1.0, mid);
    let dy_dt = cubic_bezier_derivative(0.0, y1, y2, 1.0, mid);
    let deriv = if dx_dt.abs() > 1e-6 {
        dy_dt / dx_dt
    } else {
        0.0
    };
    (eased, deriv)
}

pub fn bezier_value_with_derivative(
    a: &Value,
    b: &Value,
    t: f32,
    ctrl: [f32; 4],
) -> (Value, f32, f32) {
    let (eased, deriv) = bezier_ease_with_derivative(t, ctrl[0], ctrl[1], ctrl[2], ctrl[3]);
    (linear_value(a, b, eased), eased, deriv)
}
