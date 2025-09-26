#![allow(dead_code)]
//! Track/animation sampling utilities for the canonical StoredAnimation schema.
//!
//! Model:
//! - Each Track has ordered Keypoints with normalized stamps in [0,1].
//! - Segment [Pi -> P(i+1)] timing is a cubic-bezier determined by:
//!   cp0 = Pi.transitions.out or default {x:0.42, y:0.0}
//!   cp1 = P(i+1).transitions.in or default {x:0.58, y:1.0}
//! - For Bool/Text value kinds we use true step behavior (hold left).
//! - All other kinds use bezier easing on time, then linear/nlerp blend on value.
//!
//! API:
//! - sample_track(&Track, u) where u is normalized time in [0,1] over the whole clip.

use crate::data::{Keypoint, Track};
use crate::interp::functions::{bezier_value, step_value};
use vizij_api_core::{Value, ValueKind};

const DERIV_EPSILON: f32 = 1e-3;

#[derive(Clone, Debug)]
pub struct SampledValue {
    pub value: Value,
    pub derivative: Option<Value>,
}

const DEFAULT_OUT_X: f32 = 0.42;
const DEFAULT_OUT_Y: f32 = 0.0;
const DEFAULT_IN_X: f32 = 0.58;
const DEFAULT_IN_Y: f32 = 1.0;

/// Find the segment [i, i+1] that contains normalized time u, and return (i, i+1, local_t),
/// where local_t is normalized to [0, 1] between points[i].stamp .. points[i+1].stamp.
/// Edge cases:
/// - If u <= first.stamp, returns (0, 0, 0) and caller should pick points[0].
/// - If u >= last.stamp, returns (last, last, 0) and caller should pick points[last].
fn find_segment(points: &[Keypoint], u: f32) -> (usize, usize, f32) {
    let n = points.len();
    if n == 0 {
        return (0, 0, 0.0);
    }
    if n == 1 || u <= points[0].stamp {
        return (0, 0, 0.0);
    }
    if u >= points[n - 1].stamp {
        return (n - 1, n - 1, 0.0);
    }
    // Linear scan (could be optimized to binary search if needed)
    for i in 0..(n - 1) {
        let t0 = points[i].stamp;
        let t1 = points[i + 1].stamp;
        if u >= t0 && u <= t1 {
            let denom = (t1 - t0).max(f32::EPSILON);
            let lt = (u - t0) / denom;
            return (i, i + 1, lt.clamp(0.0, 1.0));
        }
    }
    (n - 1, n - 1, 0.0)
}

/// Sample a single track at normalized time u ∈ [0,1].
pub fn sample_track(track: &Track, u: f32) -> Value {
    let points = &track.points;
    let n = points.len();
    match n {
        0 => {
            // No points: return a neutral scalar 0.0 (fail-soft). Adapters can choose policy.
            Value::Float(0.0)
        }
        1 => points[0].value.clone(),
        _ => {
            let (i0, i1, lt) = find_segment(points, u.clamp(0.0, 1.0));
            if i0 == i1 {
                return points[i0].value.clone();
            }
            let left = &points[i0];
            let right = &points[i1];

            // Step behavior for Bool/Text tracks regardless of transitions.
            match left.value.kind() {
                ValueKind::Bool | ValueKind::Text => return step_value(&left.value),
                _ => {}
            }

            // Derive per-segment cubic-bezier control points from keypoint transitions.
            let (x1, y1) = left
                .transitions
                .as_ref()
                .and_then(|t| t.r#out.as_ref())
                .map(|v| (v.x, v.y))
                .unwrap_or((DEFAULT_OUT_X, DEFAULT_OUT_Y));

            let (x2, y2) = right
                .transitions
                .as_ref()
                .and_then(|t| t.r#in.as_ref())
                .map(|v| (v.x, v.y))
                .unwrap_or((DEFAULT_IN_X, DEFAULT_IN_Y));

            bezier_value(&left.value, &right.value, lt, [x1, y1, x2, y2])
        }
    }
}

fn quaternion_adjust(a: [f32; 4], mut b: [f32; 4]) -> [f32; 4] {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    if dot < 0.0 {
        b[0] = -b[0];
        b[1] = -b[1];
        b[2] = -b[2];
        b[3] = -b[3];
    }
    b
}

fn derivative_between(a: &Value, b: &Value, dt: f32) -> Option<Value> {
    if dt <= 0.0 {
        return None;
    }
    let inv_dt = dt.recip();
    match (a, b) {
        (Value::Float(va), Value::Float(vb)) => Some(Value::Float((vb - va) * inv_dt)),
        (Value::Vec2(va), Value::Vec2(vb)) => Some(Value::Vec2([
            (vb[0] - va[0]) * inv_dt,
            (vb[1] - va[1]) * inv_dt,
        ])),
        (Value::Vec3(va), Value::Vec3(vb)) => Some(Value::Vec3([
            (vb[0] - va[0]) * inv_dt,
            (vb[1] - va[1]) * inv_dt,
            (vb[2] - va[2]) * inv_dt,
        ])),
        (Value::Vec4(va), Value::Vec4(vb)) => Some(Value::Vec4([
            (vb[0] - va[0]) * inv_dt,
            (vb[1] - va[1]) * inv_dt,
            (vb[2] - va[2]) * inv_dt,
            (vb[3] - va[3]) * inv_dt,
        ])),
        (Value::Quat(qa), Value::Quat(qb)) => {
            let qb_adj = quaternion_adjust(*qa, *qb);
            Some(Value::Quat([
                (qb_adj[0] - qa[0]) * inv_dt,
                (qb_adj[1] - qa[1]) * inv_dt,
                (qb_adj[2] - qa[2]) * inv_dt,
                (qb_adj[3] - qa[3]) * inv_dt,
            ]))
        }
        (Value::ColorRgba(ca), Value::ColorRgba(cb)) => Some(Value::ColorRgba([
            (cb[0] - ca[0]) * inv_dt,
            (cb[1] - ca[1]) * inv_dt,
            (cb[2] - ca[2]) * inv_dt,
            (cb[3] - ca[3]) * inv_dt,
        ])),
        (
            Value::Transform {
                pos: pa,
                rot: ra,
                scale: sa,
            },
            Value::Transform {
                pos: pb,
                rot: rb,
                scale: sb,
            },
        ) => {
            let rb_adj = quaternion_adjust(*ra, *rb);
            Some(Value::Transform {
                pos: [
                    (pb[0] - pa[0]) * inv_dt,
                    (pb[1] - pa[1]) * inv_dt,
                    (pb[2] - pa[2]) * inv_dt,
                ],
                rot: [
                    (rb_adj[0] - ra[0]) * inv_dt,
                    (rb_adj[1] - ra[1]) * inv_dt,
                    (rb_adj[2] - ra[2]) * inv_dt,
                    (rb_adj[3] - ra[3]) * inv_dt,
                ],
                scale: [
                    (sb[0] - sa[0]) * inv_dt,
                    (sb[1] - sa[1]) * inv_dt,
                    (sb[2] - sa[2]) * inv_dt,
                ],
            })
        }
        _ => None,
    }
}

fn neutral_derivative(value: &Value) -> Option<Value> {
    match value {
        Value::Float(_) => Some(Value::Float(0.0)),
        Value::Vec2(_) => Some(Value::Vec2([0.0, 0.0])),
        Value::Vec3(_) => Some(Value::Vec3([0.0, 0.0, 0.0])),
        Value::Vec4(_) => Some(Value::Vec4([0.0, 0.0, 0.0, 0.0])),
        Value::Quat(_) => Some(Value::Quat([0.0, 0.0, 0.0, 0.0])),
        Value::ColorRgba(_) => Some(Value::ColorRgba([0.0, 0.0, 0.0, 0.0])),
        Value::Transform { .. } => Some(Value::Transform {
            pos: [0.0, 0.0, 0.0],
            rot: [0.0, 0.0, 0.0, 0.0],
            scale: [0.0, 0.0, 0.0],
        }),
        _ => None,
    }
}

pub fn sample_track_with_derivative(track: &Track, u: f32, duration_s: f32) -> SampledValue {
    let value = sample_track(track, u);
    let derivative = if duration_s <= 0.0 || track.points.len() <= 1 {
        neutral_derivative(&value)
    } else {
        let delta = DERIV_EPSILON;
        let mut u0 = (u - delta).clamp(0.0, 1.0);
        let mut u1 = (u + delta).clamp(0.0, 1.0);
        if (u1 - u0).abs() < 1e-6 {
            // Fallback to forward difference near the boundary.
            u0 = u;
            u1 = (u + delta).clamp(0.0, 1.0);
        }
        let prev = sample_track(track, u0);
        let next = sample_track(track, u1);
        let dt = (u1 - u0).abs() * duration_s;
        derivative_between(&prev, &next, dt).or_else(|| neutral_derivative(&value))
    };

    SampledValue { value, derivative }
}
