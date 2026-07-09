#![allow(dead_code)]
//! Interpolation helpers over POD [`TrackValue`]s:
//! - step_value (step semantics)
//! - linear_value (component-wise + quat NLERP)
//! - bezier_value (cubic-bezier timing -> linear blend)
//! - quaternion NLERP with shortest-arc normalization
//!
//! Step-only kinds (Bool/Text/Vector/NumericArray/Step) fall back to the
//! left operand in every blend, so mismatched pairs are fail-soft.

use crate::value::{TrackValue, Transform};

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
pub fn step_value(a: &TrackValue) -> TrackValue {
    a.clone()
}

/// Linear interpolation across TrackValue kinds (Transform uses TRS with quat NLERP).
pub fn linear_value(a: &TrackValue, b: &TrackValue, t: f32) -> TrackValue {
    match (a, b) {
        (TrackValue::Float(va), TrackValue::Float(vb)) => TrackValue::Float(lerp_f32(*va, *vb, t)),
        (TrackValue::Vec2(va), TrackValue::Vec2(vb)) => TrackValue::Vec2(lerp_vec2(*va, *vb, t)),
        (TrackValue::Vec3(va), TrackValue::Vec3(vb)) => TrackValue::Vec3(lerp_vec3(*va, *vb, t)),
        (TrackValue::Vec4(va), TrackValue::Vec4(vb)) => TrackValue::Vec4(lerp_vec4(*va, *vb, t)),
        (TrackValue::Quat(qa), TrackValue::Quat(qb)) => TrackValue::Quat(nlerp_quat(*qa, *qb, t)),
        (TrackValue::ColorRgba(ca), TrackValue::ColorRgba(cb)) => {
            TrackValue::ColorRgba(lerp_vec4(*ca, *cb, t))
        }
        (TrackValue::Transform(ta), TrackValue::Transform(tb)) => {
            TrackValue::Transform(Transform {
                translation: lerp_vec3(ta.translation, tb.translation, t),
                rotation: nlerp_quat(ta.rotation, tb.rotation, t),
                scale: lerp_vec3(ta.scale, tb.scale, t),
            })
        }
        // Fallback: step-only kinds and mismatched pairs prefer left (fail-soft).
        _ => a.clone(),
    }
}

/// Linear interpolation derivative across TrackValue kinds.
pub fn linear_derivative(a: &TrackValue, b: &TrackValue, t: f32, dt_du: f32) -> TrackValue {
    match (a, b) {
        (TrackValue::Float(va), TrackValue::Float(vb)) => TrackValue::Float((*vb - *va) * dt_du),
        (TrackValue::Vec2(va), TrackValue::Vec2(vb)) => {
            TrackValue::Vec2([(vb[0] - va[0]) * dt_du, (vb[1] - va[1]) * dt_du])
        }
        (TrackValue::Vec3(va), TrackValue::Vec3(vb)) => TrackValue::Vec3([
            (vb[0] - va[0]) * dt_du,
            (vb[1] - va[1]) * dt_du,
            (vb[2] - va[2]) * dt_du,
        ]),
        (TrackValue::Vec4(va), TrackValue::Vec4(vb)) => TrackValue::Vec4([
            (vb[0] - va[0]) * dt_du,
            (vb[1] - va[1]) * dt_du,
            (vb[2] - va[2]) * dt_du,
            (vb[3] - va[3]) * dt_du,
        ]),
        (TrackValue::Quat(qa), TrackValue::Quat(qb)) => {
            let (raw, raw_dt) = quat_derivative_components(*qa, *qb, t, dt_du);
            TrackValue::Quat(normalize4_derivative(raw, raw_dt))
        }
        (TrackValue::ColorRgba(ca), TrackValue::ColorRgba(cb)) => TrackValue::ColorRgba([
            (cb[0] - ca[0]) * dt_du,
            (cb[1] - ca[1]) * dt_du,
            (cb[2] - ca[2]) * dt_du,
            (cb[3] - ca[3]) * dt_du,
        ]),
        (TrackValue::Transform(ta), TrackValue::Transform(tb)) => {
            let translation = [
                (tb.translation[0] - ta.translation[0]) * dt_du,
                (tb.translation[1] - ta.translation[1]) * dt_du,
                (tb.translation[2] - ta.translation[2]) * dt_du,
            ];
            let scale = [
                (tb.scale[0] - ta.scale[0]) * dt_du,
                (tb.scale[1] - ta.scale[1]) * dt_du,
                (tb.scale[2] - ta.scale[2]) * dt_du,
            ];
            let (raw, raw_dt) = quat_derivative_components(ta.rotation, tb.rotation, t, dt_du);
            TrackValue::Transform(Transform {
                translation,
                rotation: normalize4_derivative(raw, raw_dt),
                scale,
            })
        }
        _ => TrackValue::Float(0.0),
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

/// Bezier easing across TrackValue kinds: compute eased t, then use linear blend.
/// Control points are (x1, y1, x2, y2).
#[inline]
pub fn bezier_value(a: &TrackValue, b: &TrackValue, t: f32, ctrl: [f32; 4]) -> TrackValue {
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
    a: &TrackValue,
    b: &TrackValue,
    t: f32,
    ctrl: [f32; 4],
) -> (TrackValue, f32, f32) {
    let (eased, deriv) = bezier_ease_with_derivative(t, ctrl[0], ctrl[1], ctrl[2], ctrl[3]);
    (linear_value(a, b, eased), eased, deriv)
}
