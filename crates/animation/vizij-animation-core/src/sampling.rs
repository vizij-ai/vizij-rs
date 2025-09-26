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

fn value_difference(a: &Value, b: &Value) -> Option<Value> {
    match (a, b) {
        (Value::Float(va), Value::Float(vb)) => Some(Value::Float(va - vb)),
        (Value::Vec2(va), Value::Vec2(vb)) => Some(Value::Vec2([va[0] - vb[0], va[1] - vb[1]])),
        (Value::Vec3(va), Value::Vec3(vb)) => {
            Some(Value::Vec3([va[0] - vb[0], va[1] - vb[1], va[2] - vb[2]]))
        }
        (Value::Vec4(va), Value::Vec4(vb)) | (Value::ColorRgba(va), Value::ColorRgba(vb)) => {
            Some(Value::Vec4([
                va[0] - vb[0],
                va[1] - vb[1],
                va[2] - vb[2],
                va[3] - vb[3],
            ]))
        }
        (Value::Quat(qa), Value::Quat(qb)) => Some(Value::Quat([
            qa[0] - qb[0],
            qa[1] - qb[1],
            qa[2] - qb[2],
            qa[3] - qb[3],
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
        ) => Some(Value::Transform {
            pos: [pa[0] - pb[0], pa[1] - pb[1], pa[2] - pb[2]],
            rot: [ra[0] - rb[0], ra[1] - rb[1], ra[2] - rb[2], ra[3] - rb[3]],
            scale: [sa[0] - sb[0], sa[1] - sb[1], sa[2] - sb[2]],
        }),
        _ => None,
    }
}

fn value_scale(value: &Value, scale: f32) -> Option<Value> {
    match value {
        Value::Float(v) => Some(Value::Float(v * scale)),
        Value::Vec2(v) => Some(Value::Vec2([v[0] * scale, v[1] * scale])),
        Value::Vec3(v) => Some(Value::Vec3([v[0] * scale, v[1] * scale, v[2] * scale])),
        Value::Vec4(v) => Some(Value::Vec4([
            v[0] * scale,
            v[1] * scale,
            v[2] * scale,
            v[3] * scale,
        ])),
        Value::ColorRgba(v) => Some(Value::ColorRgba([
            v[0] * scale,
            v[1] * scale,
            v[2] * scale,
            v[3] * scale,
        ])),
        Value::Quat(v) => Some(Value::Quat([
            v[0] * scale,
            v[1] * scale,
            v[2] * scale,
            v[3] * scale,
        ])),
        Value::Transform { pos, rot, scale: s } => Some(Value::Transform {
            pos: [pos[0] * scale, pos[1] * scale, pos[2] * scale],
            rot: [
                rot[0] * scale,
                rot[1] * scale,
                rot[2] * scale,
                rot[3] * scale,
            ],
            scale: [s[0] * scale, s[1] * scale, s[2] * scale],
        }),
        _ => None,
    }
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

/// Sample a track and approximate its time derivative (seconds).
pub fn sample_track_with_derivative(
    track: &Track,
    u: f32,
    duration_s: f32,
) -> (Value, Option<Value>) {
    let value = sample_track(track, u);
    if track.points.len() <= 1 || duration_s <= 0.0 {
        return (value, None);
    }

    let u0 = (u - DERIV_EPSILON).clamp(0.0, 1.0);
    let u1 = (u + DERIV_EPSILON).clamp(0.0, 1.0);
    if (u1 - u0).abs() < f32::EPSILON {
        return (value, None);
    }

    let prev = sample_track(track, u0);
    let next = sample_track(track, u1);
    let dt = (u1 - u0) * duration_s;
    if dt.abs() < 1e-6 {
        return (value, None);
    }

    let derivative = value_difference(&next, &prev)
        .and_then(|diff| value_scale(&diff, dt.recip()))
        .map(|v| match v {
            Value::Vec4(arr) if matches!(value, Value::ColorRgba(_)) => Value::ColorRgba(arr),
            other => other,
        });
    (value, derivative)
}
