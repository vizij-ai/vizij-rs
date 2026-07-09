#![allow(dead_code)]
//! Track/animation sampling utilities for the canonical StoredAnimation schema.
//!
//! Model:
//! - Each Track has ordered Keypoints with normalized stamps in `[0, 1]`,
//!   holding POD [`TrackValue`]s decoded once at load.
//! - Segment [Pi -> P(i+1)] timing is a cubic-bezier determined by:
//!   cp0 = Pi.transitions.out or default {x:0.42, y:0.0}
//!   cp1 = P(i+1).transitions.in or default {x:0.58, y:1.0}
//! - Bool/Text kinds use true step behavior (hold left).
//! - All other kinds use bezier easing on time, then linear/nlerp blend on
//!   value (step-only kinds hold left through the blend fallback).
//!
//! API:
//! - `sample_track(&Track, u)` where `u` is normalized time in `[0, 1]` over the whole clip.

use crate::data::{Keypoint, Track};
use crate::interp::functions::{bezier_value, step_value};
use crate::value::{TrackValue, Transform};

/// Symmetric finite difference offset applied around the normalized parameter when approximating
/// derivatives. Smaller values reduce smoothing but increase numerical noise; larger values trade
/// the opposite. Consider exposing via configuration if tooling needs to tune accuracy.
pub(crate) const DEFAULT_DERIVATIVE_EPSILON: f32 = 1e-3;

fn value_difference(a: &TrackValue, b: &TrackValue) -> Option<TrackValue> {
    match (a, b) {
        (TrackValue::Float(va), TrackValue::Float(vb)) => Some(TrackValue::Float(va - vb)),
        (TrackValue::Vec2(va), TrackValue::Vec2(vb)) => {
            Some(TrackValue::Vec2([va[0] - vb[0], va[1] - vb[1]]))
        }
        (TrackValue::Vec3(va), TrackValue::Vec3(vb)) => Some(TrackValue::Vec3([
            va[0] - vb[0],
            va[1] - vb[1],
            va[2] - vb[2],
        ])),
        (TrackValue::Vec4(va), TrackValue::Vec4(vb)) => Some(TrackValue::Vec4([
            va[0] - vb[0],
            va[1] - vb[1],
            va[2] - vb[2],
            va[3] - vb[3],
        ])),
        (TrackValue::ColorRgba(va), TrackValue::ColorRgba(vb)) => Some(TrackValue::ColorRgba([
            va[0] - vb[0],
            va[1] - vb[1],
            va[2] - vb[2],
            va[3] - vb[3],
        ])),
        (TrackValue::Quat(qa), TrackValue::Quat(qb)) => Some(TrackValue::Quat([
            qa[0] - qb[0],
            qa[1] - qb[1],
            qa[2] - qb[2],
            qa[3] - qb[3],
        ])),
        (TrackValue::Transform(ta), TrackValue::Transform(tb)) => {
            Some(TrackValue::Transform(Transform {
                translation: [
                    ta.translation[0] - tb.translation[0],
                    ta.translation[1] - tb.translation[1],
                    ta.translation[2] - tb.translation[2],
                ],
                rotation: [
                    ta.rotation[0] - tb.rotation[0],
                    ta.rotation[1] - tb.rotation[1],
                    ta.rotation[2] - tb.rotation[2],
                    ta.rotation[3] - tb.rotation[3],
                ],
                scale: [
                    ta.scale[0] - tb.scale[0],
                    ta.scale[1] - tb.scale[1],
                    ta.scale[2] - tb.scale[2],
                ],
            }))
        }
        _ => None,
    }
}

fn value_scale(value: &TrackValue, scale: f32) -> Option<TrackValue> {
    match value {
        TrackValue::Float(v) => Some(TrackValue::Float(v * scale)),
        TrackValue::Vec2(v) => Some(TrackValue::Vec2([v[0] * scale, v[1] * scale])),
        TrackValue::Vec3(v) => Some(TrackValue::Vec3([v[0] * scale, v[1] * scale, v[2] * scale])),
        TrackValue::Vec4(v) => Some(TrackValue::Vec4([
            v[0] * scale,
            v[1] * scale,
            v[2] * scale,
            v[3] * scale,
        ])),
        TrackValue::ColorRgba(v) => Some(TrackValue::ColorRgba([
            v[0] * scale,
            v[1] * scale,
            v[2] * scale,
            v[3] * scale,
        ])),
        TrackValue::Quat(v) => Some(TrackValue::Quat([
            v[0] * scale,
            v[1] * scale,
            v[2] * scale,
            v[3] * scale,
        ])),
        TrackValue::Transform(t) => Some(TrackValue::Transform(Transform {
            translation: [
                t.translation[0] * scale,
                t.translation[1] * scale,
                t.translation[2] * scale,
            ],
            rotation: [
                t.rotation[0] * scale,
                t.rotation[1] * scale,
                t.rotation[2] * scale,
                t.rotation[3] * scale,
            ],
            scale: [t.scale[0] * scale, t.scale[1] * scale, t.scale[2] * scale],
        })),
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

/// Sample a single track at normalized time `u` in `[0, 1]`.
pub fn sample_track(track: &Track, u: f32) -> TrackValue {
    let points = &track.points;
    let n = points.len();
    match n {
        0 => {
            // No points: return a neutral scalar 0.0 (fail-soft). Adapters can choose policy.
            TrackValue::Float(0.0)
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
            if matches!(left.value, TrackValue::Bool(_) | TrackValue::Text(_)) {
                return step_value(&left.value);
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
///
/// Derivatives are estimated with a symmetric finite difference of width `DEFAULT_DERIVATIVE_EPSILON`
/// normalized domain, scaled by the clip duration. This captures velocity-like behaviour for
/// numeric tracks but intentionally returns `None` for non-numeric kinds such as Bool/Text to avoid
/// misleading data. Quaternion derivatives are currently computed component-wise which is a
/// reasonable first approximation for small deltas but does not map to angular velocity; replace
/// with a proper log/exp-based interpolation when higher fidelity is required.
///
/// TODO: expose derivative configuration (epsilon, strategy) via `BakingConfig` or a sampling
/// struct so hosts can balance accuracy and performance.
pub fn sample_track_with_derivative(
    track: &Track,
    u: f32,
    duration_s: f32,
) -> (TrackValue, Option<TrackValue>) {
    sample_track_with_derivative_epsilon(track, u, duration_s, DEFAULT_DERIVATIVE_EPSILON)
}

/// Variant of [`sample_track_with_derivative`] that allows callers to specify the finite
/// difference epsilon used during derivative estimation.
pub fn sample_track_with_derivative_epsilon(
    track: &Track,
    u: f32,
    duration_s: f32,
    epsilon: f32,
) -> (TrackValue, Option<TrackValue>) {
    let value = sample_track(track, u);
    if track.points.len() <= 1 || duration_s <= 0.0 {
        return (value, None);
    }

    let eps = if epsilon.is_finite() && epsilon > 0.0 {
        epsilon
    } else {
        DEFAULT_DERIVATIVE_EPSILON
    };

    let u0 = (u - eps).clamp(0.0, 1.0);
    let u1 = (u + eps).clamp(0.0, 1.0);
    if (u1 - u0).abs() < f32::EPSILON {
        return (value, None);
    }

    let prev = sample_track(track, u0);
    let next = sample_track(track, u1);
    let dt = (u1 - u0) * duration_s;
    if dt.abs() < 1e-6 {
        return (value, None);
    }

    let derivative = value_difference(&next, &prev).and_then(|diff| value_scale(&diff, dt.recip()));
    (value, derivative)
}
