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
use crate::interp::functions::{bezier_value_with_derivative, linear_derivative, step_value};
use vizij_api_core::{Value, ValueKind};

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

#[derive(Clone, Debug)]
pub struct SampledValue {
    pub value: Value,
    pub derivative: Value,
}

fn zero_like(value: &Value) -> Value {
    match value {
        Value::Float(_) => Value::Float(0.0),
        Value::Vec2(_) => Value::Vec2([0.0, 0.0]),
        Value::Vec3(_) => Value::Vec3([0.0, 0.0, 0.0]),
        Value::Vec4(_) => Value::Vec4([0.0, 0.0, 0.0, 0.0]),
        Value::Quat(_) => Value::Quat([0.0, 0.0, 0.0, 0.0]),
        Value::ColorRgba(_) => Value::ColorRgba([0.0, 0.0, 0.0, 0.0]),
        Value::Transform { .. } => Value::Transform {
            pos: [0.0, 0.0, 0.0],
            rot: [0.0, 0.0, 0.0, 0.0],
            scale: [0.0, 0.0, 0.0],
        },
        Value::Vector(v) => Value::Vector(vec![0.0; v.len()]),
        _ => Value::Float(0.0),
    }
}

/// Sample a single track at normalized time u ∈ [0,1], returning value and derivative.
pub fn sample_track_with_derivative(track: &Track, u: f32) -> SampledValue {
    let points = &track.points;
    let n = points.len();
    match n {
        0 => SampledValue {
            value: Value::Float(0.0),
            derivative: Value::Float(0.0),
        },
        1 => SampledValue {
            value: points[0].value.clone(),
            derivative: zero_like(&points[0].value),
        },
        _ => {
            let (i0, i1, lt) = find_segment(points, u.clamp(0.0, 1.0));
            if i0 == i1 {
                let v = points[i0].value.clone();
                return SampledValue {
                    value: v.clone(),
                    derivative: zero_like(&v),
                };
            }
            let left = &points[i0];
            let right = &points[i1];

            // Step behavior for Bool/Text tracks regardless of transitions.
            match left.value.kind() {
                ValueKind::Bool | ValueKind::Text => {
                    let value = step_value(&left.value);
                    return SampledValue {
                        value,
                        derivative: Value::Float(0.0),
                    };
                }
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

            let segment_len = (right.stamp - left.stamp).abs();
            if segment_len <= f32::EPSILON {
                let value = left.value.clone();
                return SampledValue {
                    value: value.clone(),
                    derivative: zero_like(&value),
                };
            }

            let (value, eased_t, ease_derivative) =
                bezier_value_with_derivative(&left.value, &right.value, lt, [x1, y1, x2, y2]);
            let du = ease_derivative / segment_len.max(f32::EPSILON);
            let derivative = linear_derivative(&left.value, &right.value, eased_t, du);
            SampledValue { value, derivative }
        }
    }
}

/// Sample a single track at normalized time u ∈ [0,1].
pub fn sample_track(track: &Track, u: f32) -> Value {
    sample_track_with_derivative(track, u).value
}
