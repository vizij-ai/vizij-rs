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

const FINITE_DIFF_EPS: f32 = 1e-3;

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

fn finite_difference(forward: &Value, backward: &Value, denom: f32) -> Option<Value> {
    if denom <= 0.0 {
        return None;
    }
    match (forward, backward) {
        (Value::Float(a), Value::Float(b)) => Some(Value::Float((a - b) / denom)),
        (Value::Vec2(a), Value::Vec2(b)) => {
            Some(Value::Vec2([(a[0] - b[0]) / denom, (a[1] - b[1]) / denom]))
        }
        (Value::Vec3(a), Value::Vec3(b)) => Some(Value::Vec3([
            (a[0] - b[0]) / denom,
            (a[1] - b[1]) / denom,
            (a[2] - b[2]) / denom,
        ])),
        (Value::Vec4(a), Value::Vec4(b)) => Some(Value::Vec4([
            (a[0] - b[0]) / denom,
            (a[1] - b[1]) / denom,
            (a[2] - b[2]) / denom,
            (a[3] - b[3]) / denom,
        ])),
        (Value::Quat(a), Value::Quat(b)) => Some(Value::Quat([
            (a[0] - b[0]) / denom,
            (a[1] - b[1]) / denom,
            (a[2] - b[2]) / denom,
            (a[3] - b[3]) / denom,
        ])),
        (Value::ColorRgba(a), Value::ColorRgba(b)) => Some(Value::ColorRgba([
            (a[0] - b[0]) / denom,
            (a[1] - b[1]) / denom,
            (a[2] - b[2]) / denom,
            (a[3] - b[3]) / denom,
        ])),
        (
            Value::Transform {
                pos: ap,
                rot: ar,
                scale: a_scale,
            },
            Value::Transform {
                pos: bp,
                rot: br,
                scale: b_scale,
            },
        ) => Some(Value::Transform {
            pos: [
                (ap[0] - bp[0]) / denom,
                (ap[1] - bp[1]) / denom,
                (ap[2] - bp[2]) / denom,
            ],
            rot: [
                (ar[0] - br[0]) / denom,
                (ar[1] - br[1]) / denom,
                (ar[2] - br[2]) / denom,
                (ar[3] - br[3]) / denom,
            ],
            scale: [
                (a_scale[0] - b_scale[0]) / denom,
                (a_scale[1] - b_scale[1]) / denom,
                (a_scale[2] - b_scale[2]) / denom,
            ],
        }),
        (Value::Vector(a), Value::Vector(b)) => {
            if a.len() != b.len() {
                return None;
            }
            let mut out = Vec::with_capacity(a.len());
            for (aa, bb) in a.iter().zip(b.iter()) {
                out.push((aa - bb) / denom);
            }
            Some(Value::Vector(out))
        }
        _ => None,
    }
}

/// Sample a track and estimate its first derivative using symmetric finite differences.
pub fn sample_track_with_derivative(
    track: &Track,
    u: f32,
    duration_seconds: f32,
) -> (Value, Option<Value>) {
    let value = sample_track(track, u);

    match value.kind() {
        ValueKind::Bool | ValueKind::Text => return (value, None),
        _ => {}
    }

    if duration_seconds <= 0.0 {
        return (value, None);
    }

    let eps = FINITE_DIFF_EPS;
    let forward_u = (u + eps).clamp(0.0, 1.0);
    let backward_u = (u - eps).clamp(0.0, 1.0);
    let forward = sample_track(track, forward_u);
    let backward = sample_track(track, backward_u);
    let dt = 2.0 * eps * duration_seconds;
    let derivative = finite_difference(&forward, &backward, dt);

    (value, derivative)
}
