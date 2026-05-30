#![allow(dead_code)]
//! Track/animation sampling utilities for the canonical Studio StoredAnimation schema.
//!
//! Model:
//! - Studio v2 keypoint stamps are in milliseconds.
//! - Numeric tracks use Studio's world-space cubic Bezier handles: explicit `{x,y}` transitions
//!   are anchor-relative time/value deltas; named transitions resolve to Studio easing presets.
//! - Bool/Text value kinds use step behavior.
//! - Studio parity is exact for scalar numeric tracks. Complex values are supported for Vizij host
//!   compatibility, but complex Studio `RawValue` tracks are being phased out upstream and use the
//!   core's generic interpolation path rather than Studio's object-shaped control-point evaluator.
//! - Legacy Vizij assets are migrated into this shape at the importer boundary.
//!
//! API:
//! - `sample_track(&Track, stamp)` where `stamp` is expressed in the same domain as track points.

use crate::data::{AuthoredTransition, Keypoint, Track, Transitions, Vec2};
use crate::interp::functions::{bezier_value, step_value};
use vizij_api_core::{Value, ValueKind};

/// Symmetric finite difference offset applied around the normalized parameter when approximating
/// derivatives. Smaller values reduce smoothing but increase numerical noise; larger values trade
/// the opposite. Consider exposing via configuration if tooling needs to tune accuracy.
pub(crate) const DEFAULT_DERIVATIVE_EPSILON: f32 = 1.0;

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
                translation: pa,
                rotation: ra,
                scale: sa,
            },
            Value::Transform {
                translation: pb,
                rotation: rb,
                scale: sb,
            },
        ) => Some(Value::Transform {
            translation: [pa[0] - pb[0], pa[1] - pb[1], pa[2] - pb[2]],
            rotation: [ra[0] - rb[0], ra[1] - rb[1], ra[2] - rb[2], ra[3] - rb[3]],
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
        Value::Transform {
            translation,
            rotation,
            scale: s,
        } => Some(Value::Transform {
            translation: [
                translation[0] * scale,
                translation[1] * scale,
                translation[2] * scale,
            ],
            rotation: [
                rotation[0] * scale,
                rotation[1] * scale,
                rotation[2] * scale,
                rotation[3] * scale,
            ],
            scale: [s[0] * scale, s[1] * scale, s[2] * scale],
        }),
        _ => None,
    }
}

const DEFAULT_TRANSITION: &str = "cubic";

const AUTO_MAGIC_SCALE: f32 = 2.5614;
const ASYMMETRY_CLAMP_FACTOR: f32 = 5.0;
const PARAMETER_SOLVE_MAX_ITERATIONS: usize = 24;
const PARAMETER_SOLVE_EPSILON: f32 = 1e-6;

#[derive(Clone, Copy, Debug)]
struct NumericPoint {
    stamp: f32,
    value: f32,
}

#[derive(Clone, Copy, Debug)]
struct WorldPoint {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy, Debug)]
enum TransitionSide {
    Out,
    In,
}

/// Find the segment [i, i+1] that contains a local stamp, and return (i, i+1, local_t),
/// where local_t is normalized to [0, 1] between points[i].stamp .. points[i+1].stamp.
/// Edge cases:
/// - If stamp <= first.stamp, returns (0, 0, 0) and caller should pick points[0].
/// - If stamp >= last.stamp, returns (last, last, 0) and caller should pick points[last].
fn find_segment(points: &[Keypoint], stamp: f32) -> (usize, usize, f32) {
    let n = points.len();
    if n == 0 {
        return (0, 0, 0.0);
    }
    if n == 1 || stamp <= points[0].stamp {
        return (0, 0, 0.0);
    }
    if stamp >= points[n - 1].stamp {
        return (n - 1, n - 1, 0.0);
    }
    // Linear scan (could be optimized to binary search if needed)
    for i in 0..(n - 1) {
        let t0 = points[i].stamp;
        let t1 = points[i + 1].stamp;
        if stamp >= t0 && stamp <= t1 {
            let denom = (t1 - t0).max(f32::EPSILON);
            let lt = (stamp - t0) / denom;
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
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 0.0],
            scale: [0.0, 0.0, 0.0],
        },
        Value::Vector(v) => Value::Vector(vec![0.0; v.len()]),
        _ => Value::Float(0.0),
    }
}

/// Sample a single track at a local stamp expressed in the same domain as its keypoints.
pub fn sample_track(track: &Track, stamp: f32) -> Value {
    let points = &track.points;
    let n = points.len();
    match n {
        0 => {
            // No points: return a neutral scalar 0.0 (fail-soft). Adapters can choose policy.
            Value::Float(0.0)
        }
        1 => points[0].value.clone(),
        _ => {
            let min_stamp = points.first().map(|point| point.stamp).unwrap_or(0.0);
            let max_stamp = points.last().map(|point| point.stamp).unwrap_or(min_stamp);
            let (i0, i1, lt) = find_segment(points, stamp.clamp(min_stamp, max_stamp));
            if i0 == i1 {
                return points[i0].value.clone();
            }
            let left = &points[i0];
            let right = &points[i1];

            // Step behavior for Bool/Text tracks regardless of transitions.
            match left.value.kind() {
                ValueKind::Bool | ValueKind::Text => {
                    let value = if lt >= 1.0 { &right.value } else { &left.value };
                    return step_value(value);
                }
                _ => {}
            }

            if let Some(segment) = numeric_segment(points, i0, i1) {
                if is_straight_numeric_segment(&segment) {
                    return Value::Float(
                        segment.start.value + (segment.end.value - segment.start.value) * lt,
                    );
                }
                let parameter = solve_for_parameter(
                    segment.start.stamp,
                    segment.cp1.x,
                    segment.cp2.x,
                    segment.end.stamp,
                    left.stamp + lt * (right.stamp - left.stamp),
                );
                return Value::Float(evaluate_cubic(
                    segment.start.value,
                    segment.cp1.y,
                    segment.cp2.y,
                    segment.end.value,
                    parameter,
                ));
            }

            let ctrl = fallback_parametric_ctrl(left, right);
            bezier_value(&left.value, &right.value, lt, ctrl)
        }
    }
}

/// Sample a track and approximate its time derivative (seconds).
///
/// Derivatives are estimated with a symmetric finite difference in the canonical millisecond stamp
/// domain. This captures velocity-like behaviour for numeric tracks but intentionally returns
/// `None` for non-numeric kinds such as Bool/Text to avoid misleading data. Quaternion derivatives
/// are currently computed component-wise which is a reasonable first approximation for small deltas
/// but does not map to angular velocity; replace with a proper log/exp-based interpolation when
/// higher fidelity is required.
///
/// TODO: expose derivative configuration (epsilon, strategy) via `BakingConfig` or a sampling
/// struct so hosts can balance accuracy and performance.
pub fn sample_track_with_derivative(
    track: &Track,
    stamp: f32,
    _duration_s: f32,
) -> (Value, Option<Value>) {
    sample_track_with_derivative_epsilon(track, stamp, 0.001, 1.0)
}

/// Variant of [`sample_track_with_derivative`] that allows callers to specify the finite
/// difference epsilon used during derivative estimation.
pub fn sample_track_with_derivative_epsilon(
    track: &Track,
    stamp: f32,
    stamp_delta_seconds: f32,
    epsilon: f32,
) -> (Value, Option<Value>) {
    let value = sample_track(track, stamp);
    if track.points.len() <= 1 || stamp_delta_seconds <= 0.0 {
        return (value, None);
    }

    let eps = if epsilon.is_finite() && epsilon > 0.0 {
        epsilon
    } else {
        DEFAULT_DERIVATIVE_EPSILON
    };

    let min_stamp = track.points.first().map(|point| point.stamp).unwrap_or(0.0);
    let max_stamp = track
        .points
        .last()
        .map(|point| point.stamp)
        .unwrap_or(min_stamp);
    let t0 = (stamp - eps).clamp(min_stamp, max_stamp);
    let t1 = (stamp + eps).clamp(min_stamp, max_stamp);
    if (t1 - t0).abs() < f32::EPSILON {
        return (value, None);
    }

    let prev = sample_track(track, t0);
    let next = sample_track(track, t1);
    let dt = (t1 - t0) * stamp_delta_seconds;
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

#[derive(Clone, Copy, Debug)]
struct NumericSegment {
    start: NumericPoint,
    end: NumericPoint,
    cp1: WorldPoint,
    cp2: WorldPoint,
}

fn numeric_segment(
    points: &[Keypoint],
    start_index: usize,
    end_index: usize,
) -> Option<NumericSegment> {
    let left = &points[start_index];
    let right = &points[end_index];
    let start_value = match left.value {
        Value::Float(value) => value,
        _ => return None,
    };
    let end_value = match right.value {
        Value::Float(value) => value,
        _ => return None,
    };

    let start = NumericPoint {
        stamp: left.stamp,
        value: start_value,
    };
    let end = NumericPoint {
        stamp: right.stamp,
        value: end_value,
    };
    if (end.stamp - start.stamp).abs() < f32::EPSILON {
        return None;
    }

    let neighbors = SegmentNeighbors {
        start_prev: start_index
            .checked_sub(1)
            .and_then(|index| numeric_point_at(points, index)),
        end_next: numeric_point_at(points, end_index + 1),
    };

    let authored_start_out = transition_or_default(left.transitions.as_ref(), TransitionSide::Out);
    let authored_end_in = transition_or_default(right.transitions.as_ref(), TransitionSide::In);

    let raw_start_out = resolve_side_transition(
        TransitionSide::Out,
        &authored_start_out,
        start,
        end,
        &neighbors,
    );
    let raw_end_in =
        resolve_side_transition(TransitionSide::In, &authored_end_in, start, end, &neighbors);

    let mut cp1 = world_cp_for_transition(TransitionSide::Out, start, end, &raw_start_out);
    let mut cp2 = world_cp_for_transition(TransitionSide::In, start, end, &raw_end_in);
    clamp_monotonic_x(start, end, &mut cp1, &mut cp2);

    if is_inferred_auto_clamped(&authored_start_out) {
        cp1 = apply_auto_clamped_limits(TransitionSide::Out, start, end, neighbors.start_prev, cp1);
    }
    if is_inferred_auto_clamped(&authored_end_in) {
        cp2 = apply_auto_clamped_limits(TransitionSide::In, start, end, neighbors.end_next, cp2);
    }

    Some(NumericSegment {
        start,
        end,
        cp1,
        cp2,
    })
}

fn numeric_point_at(points: &[Keypoint], index: usize) -> Option<NumericPoint> {
    points.get(index).and_then(|point| match point.value {
        Value::Float(value) => Some(NumericPoint {
            stamp: point.stamp,
            value,
        }),
        _ => None,
    })
}

#[derive(Clone, Copy, Debug)]
struct SegmentNeighbors {
    start_prev: Option<NumericPoint>,
    end_next: Option<NumericPoint>,
}

fn transition_or_default(
    transitions: Option<&Transitions>,
    side: TransitionSide,
) -> AuthoredTransition {
    let transition = match side {
        TransitionSide::Out => transitions.and_then(|t| t.r#out.clone()),
        TransitionSide::In => transitions.and_then(|t| t.r#in.clone()),
    };
    transition.unwrap_or_else(|| AuthoredTransition::name(DEFAULT_TRANSITION))
}

fn resolve_side_transition(
    side: TransitionSide,
    authored: &AuthoredTransition,
    start: NumericPoint,
    end: NumericPoint,
    neighbors: &SegmentNeighbors,
) -> AuthoredTransition {
    match authored {
        AuthoredTransition::Explicit(_) => authored.clone(),
        AuthoredTransition::Name(name) if is_directive(name) => {
            let inferred = infer_from_segment_neighborhood(side, start, end, neighbors);
            match inferred {
                Some(delta) if name == "inferred-auto-clamped" => AuthoredTransition::Explicit(
                    clamp_auto_raw_delta_to_segment(side, start, end, delta),
                ),
                Some(delta) => AuthoredTransition::Explicit(delta),
                None => AuthoredTransition::name(DEFAULT_TRANSITION),
            }
        }
        AuthoredTransition::Name(_) => authored.clone(),
    }
}

fn is_directive(name: &str) -> bool {
    name == "explicit-handles" || name == "inferred-auto-clamped"
}

fn is_inferred_auto_clamped(transition: &AuthoredTransition) -> bool {
    matches!(transition, AuthoredTransition::Name(name) if name == "inferred-auto-clamped")
}

fn infer_from_segment_neighborhood(
    side: TransitionSide,
    start: NumericPoint,
    end: NumericPoint,
    neighbors: &SegmentNeighbors,
) -> Option<Vec2> {
    match side {
        TransitionSide::Out => {
            infer_auto_handle_deltas(neighbors.start_prev, start, Some(end)).map(|deltas| deltas.1)
        }
        TransitionSide::In => {
            infer_auto_handle_deltas(Some(start), end, neighbors.end_next).map(|deltas| deltas.0)
        }
    }
}

fn infer_auto_handle_deltas(
    prev: Option<NumericPoint>,
    anchor: NumericPoint,
    next: Option<NumericPoint>,
) -> Option<(Vec2, Vec2)> {
    let prev = prev?;
    let next = next?;
    let prev_delta = Vec2 {
        x: anchor.stamp - prev.stamp,
        y: anchor.value - prev.value,
    };
    let next_delta = Vec2 {
        x: next.stamp - anchor.stamp,
        y: next.value - anchor.value,
    };
    let prev_norm = normalize_with_fallback(prev_delta);
    let next_norm = normalize_with_fallback(next_delta);
    let direction = Vec2 {
        x: prev_norm.0.x + next_norm.0.x,
        y: prev_norm.0.y + next_norm.0.y,
    };
    let direction_length = direction.x.hypot(direction.y);
    let scale_base = direction_length * AUTO_MAGIC_SCALE;
    if !scale_base.is_finite() || scale_base <= 0.0 {
        return None;
    }

    let prev_length = prev_norm.1.min(ASYMMETRY_CLAMP_FACTOR * next_norm.1);
    let next_length = next_norm.1.min(ASYMMETRY_CLAMP_FACTOR * prev_norm.1);
    let in_scale = prev_length / scale_base;
    let out_scale = next_length / scale_base;

    Some((
        Vec2 {
            x: -direction.x * in_scale,
            y: -direction.y * in_scale,
        },
        Vec2 {
            x: direction.x * out_scale,
            y: direction.y * out_scale,
        },
    ))
}

fn normalize_with_fallback(delta: Vec2) -> (Vec2, f32) {
    let length = delta.x.hypot(delta.y);
    if !length.is_finite() || length == 0.0 {
        return (Vec2 { x: 0.0, y: 0.0 }, 1.0);
    }
    (
        Vec2 {
            x: delta.x / length,
            y: delta.y / length,
        },
        length,
    )
}

fn clamp_auto_raw_delta_to_segment(
    side: TransitionSide,
    start: NumericPoint,
    end: NumericPoint,
    delta: Vec2,
) -> Vec2 {
    let anchor = match side {
        TransitionSide::Out => start,
        TransitionSide::In => end,
    };
    let opposite = match side {
        TransitionSide::Out => end,
        TransitionSide::In => start,
    };
    let mut cp = WorldPoint {
        x: anchor.stamp + delta.x,
        y: anchor.value + delta.y,
    };
    let min_x = start.stamp.min(end.stamp);
    let max_x = start.stamp.max(end.stamp);
    cp = clamp_handle_to_segment_stamp_range(anchor, cp, min_x, max_x);
    cp = clamp_handle_to_segment_value_range(anchor, opposite, cp);
    cp = clamp_handle_to_segment_stamp_range(anchor, cp, min_x, max_x);
    Vec2 {
        x: cp.x - anchor.stamp,
        y: cp.y - anchor.value,
    }
}

fn apply_auto_clamped_limits(
    side: TransitionSide,
    start: NumericPoint,
    end: NumericPoint,
    neighbor: Option<NumericPoint>,
    cp: WorldPoint,
) -> WorldPoint {
    let anchor = match side {
        TransitionSide::Out => start,
        TransitionSide::In => end,
    };
    let opposite = match side {
        TransitionSide::Out => end,
        TransitionSide::In => start,
    };
    let mut clamped = clamp_handle_to_segment_value_range(anchor, opposite, cp);
    let segment_delta = end.value - start.value;
    let should_flatten = match side {
        TransitionSide::Out => should_flatten_start_tangent(neighbor, start, segment_delta),
        TransitionSide::In => should_flatten_end_tangent(neighbor, end, segment_delta),
    };
    if should_flatten {
        clamped = clamp_handle_to_target_y(anchor, clamped, anchor.value);
    }
    clamp_handle_to_segment_stamp_range(
        anchor,
        clamped,
        start.stamp.min(end.stamp),
        start.stamp.max(end.stamp),
    )
}

fn should_flatten_start_tangent(
    start_prev: Option<NumericPoint>,
    start: NumericPoint,
    segment_delta: f32,
) -> bool {
    let Some(start_prev) = start_prev else {
        return false;
    };
    let prev_delta = start.value - start_prev.value;
    nearly_zero(prev_delta)
        || nearly_zero(segment_delta)
        || prev_delta.signum() != segment_delta.signum()
}

fn should_flatten_end_tangent(
    end_next: Option<NumericPoint>,
    end: NumericPoint,
    segment_delta: f32,
) -> bool {
    let Some(end_next) = end_next else {
        return false;
    };
    let next_delta = end_next.value - end.value;
    nearly_zero(next_delta)
        || nearly_zero(segment_delta)
        || segment_delta.signum() != next_delta.signum()
}

fn nearly_zero(value: f32) -> bool {
    value.abs() < 1e-6
}

fn world_cp_for_transition(
    side: TransitionSide,
    start: NumericPoint,
    end: NumericPoint,
    transition: &AuthoredTransition,
) -> WorldPoint {
    match transition {
        AuthoredTransition::Explicit(delta) => {
            let anchor = match side {
                TransitionSide::Out => start,
                TransitionSide::In => end,
            };
            WorldPoint {
                x: anchor.stamp + delta.x,
                y: anchor.value + delta.y,
            }
        }
        AuthoredTransition::Name(name) => {
            let params = standard_transition_params(side, name);
            let span = end.stamp - start.stamp;
            let value_delta = end.value - start.value;
            WorldPoint {
                x: start.stamp + params.x * span,
                y: start.value + params.y * value_delta,
            }
        }
    }
}

fn fallback_parametric_ctrl(left: &Keypoint, right: &Keypoint) -> [f32; 4] {
    let span = (right.stamp - left.stamp).max(f32::EPSILON);
    let out = transition_or_default(left.transitions.as_ref(), TransitionSide::Out);
    let in_transition = transition_or_default(right.transitions.as_ref(), TransitionSide::In);
    let cp1 = transition_parametric(TransitionSide::Out, &out, span);
    let cp2 = transition_parametric(TransitionSide::In, &in_transition, span);
    [cp1.x, cp1.y, cp2.x, cp2.y]
}

fn transition_parametric(side: TransitionSide, transition: &AuthoredTransition, span: f32) -> Vec2 {
    match transition {
        AuthoredTransition::Explicit(delta) => match side {
            TransitionSide::Out => Vec2 {
                x: delta.x / span,
                y: delta.y,
            },
            TransitionSide::In => Vec2 {
                x: 1.0 + delta.x / span,
                y: 1.0 + delta.y,
            },
        },
        AuthoredTransition::Name(name) => standard_transition_params(side, name),
    }
}

fn standard_transition_params(side: TransitionSide, name: &str) -> Vec2 {
    match (side, name) {
        (TransitionSide::Out, "sine") => Vec2 { x: 0.37, y: 0.0 },
        (TransitionSide::Out, "cubic") => Vec2 { x: 0.65, y: 0.0 },
        (TransitionSide::Out, "quint") => Vec2 { x: 0.83, y: 0.0 },
        (TransitionSide::Out, "circ") => Vec2 { x: 0.85, y: 0.0 },
        (TransitionSide::Out, "quad") => Vec2 { x: 0.45, y: 0.0 },
        (TransitionSide::Out, "quart") => Vec2 { x: 0.76, y: 0.0 },
        (TransitionSide::Out, "expo") => Vec2 { x: 0.87, y: 0.0 },
        (TransitionSide::Out, "back") => Vec2 { x: 0.68, y: -0.6 },
        (TransitionSide::Out, "linear") => Vec2 { x: 0.33, y: 0.33 },
        (TransitionSide::In, "sine") => Vec2 { x: 0.63, y: 1.0 },
        (TransitionSide::In, "cubic") => Vec2 { x: 0.35, y: 1.0 },
        (TransitionSide::In, "quint") => Vec2 { x: 0.17, y: 1.0 },
        (TransitionSide::In, "circ") => Vec2 { x: 0.15, y: 1.0 },
        (TransitionSide::In, "quad") => Vec2 { x: 0.55, y: 1.0 },
        (TransitionSide::In, "quart") => Vec2 { x: 0.24, y: 1.0 },
        (TransitionSide::In, "expo") => Vec2 { x: 0.13, y: 1.0 },
        (TransitionSide::In, "back") => Vec2 { x: 0.32, y: 1.6 },
        (TransitionSide::In, "linear") => Vec2 { x: 0.66, y: 0.66 },
        _ => standard_transition_params(side, DEFAULT_TRANSITION),
    }
}

fn clamp_monotonic_x(
    start: NumericPoint,
    end: NumericPoint,
    cp1: &mut WorldPoint,
    cp2: &mut WorldPoint,
) {
    let span = end.stamp - start.stamp;
    if span.abs() < f32::EPSILON {
        *cp1 = clamp_handle_to_target_x(start, *cp1, start.stamp);
        *cp2 = clamp_handle_to_target_x(end, *cp2, end.stamp);
        return;
    }

    if cp1.x < start.stamp {
        *cp1 = clamp_handle_to_target_x(start, *cp1, start.stamp);
    }
    if cp2.x > end.stamp {
        *cp2 = clamp_handle_to_target_x(end, *cp2, end.stamp);
    }

    let cp1_normalized = (cp1.x - start.stamp) / span;
    let cp2_normalized = (cp2.x - start.stamp) / span;
    if is_monotonic_normalized(cp1_normalized, cp2_normalized) {
        return;
    }

    let (n_cp1, n_cp2) = clamp_normalized_for_monotonic(cp1_normalized, cp2_normalized);
    *cp1 = clamp_handle_to_target_x(start, *cp1, start.stamp + n_cp1 * span);
    *cp2 = clamp_handle_to_target_x(end, *cp2, start.stamp + n_cp2 * span);
}

fn is_monotonic_normalized(cp1: f32, cp2: f32) -> bool {
    cubic_derivative_minimum(cp1, cp2) >= -1e-6
}

fn clamp_normalized_for_monotonic(cp1: f32, cp2: f32) -> (f32, f32) {
    let mut low = 0.0;
    let mut high = 1.0;
    let mut best = (0.0, 1.0);
    for _ in 0..30 {
        let mid = (low + high) / 2.0;
        let candidate = (cp1 * mid, 1.0 + (cp2 - 1.0) * mid);
        if is_monotonic_normalized(candidate.0, candidate.1) {
            best = candidate;
            low = mid;
        } else {
            high = mid;
        }
    }
    best
}

fn cubic_derivative_minimum(cp1: f32, cp2: f32) -> f32 {
    let a = 1.0 + 3.0 * (cp1 - cp2);
    let b = 3.0 * (cp2 - 2.0 * cp1);
    let c = 3.0 * cp1;
    let a2 = 3.0 * a;
    let b2 = 2.0 * b;
    let derivative = |t: f32| a2 * t * t + b2 * t + c;
    let mut min = derivative(0.0).min(derivative(1.0));
    if a2.abs() > 1e-6 {
        let t = -b2 / (2.0 * a2);
        if (0.0..=1.0).contains(&t) {
            min = min.min(derivative(t));
        }
    }
    min
}

fn clamp_handle_to_segment_value_range(
    anchor: NumericPoint,
    opposite: NumericPoint,
    cp: WorldPoint,
) -> WorldPoint {
    let min_y = anchor.value.min(opposite.value);
    let max_y = anchor.value.max(opposite.value);
    if cp.y >= min_y && cp.y <= max_y {
        return cp;
    }
    clamp_handle_to_target_y(anchor, cp, if cp.y < min_y { min_y } else { max_y })
}

fn clamp_handle_to_segment_stamp_range(
    anchor: NumericPoint,
    cp: WorldPoint,
    min_x: f32,
    max_x: f32,
) -> WorldPoint {
    if cp.x >= min_x && cp.x <= max_x {
        return cp;
    }
    clamp_handle_to_target_x(anchor, cp, if cp.x < min_x { min_x } else { max_x })
}

fn clamp_handle_to_target_y(anchor: NumericPoint, cp: WorldPoint, target_y: f32) -> WorldPoint {
    if (target_y - anchor.value).abs() < 1e-6 {
        return WorldPoint {
            x: cp.x,
            y: target_y,
        };
    }
    let dy = cp.y - anchor.value;
    if dy == 0.0 {
        return WorldPoint {
            x: anchor.stamp,
            y: target_y,
        };
    }
    let ratio = (target_y - anchor.value) / dy;
    if !ratio.is_finite() {
        return WorldPoint {
            x: anchor.stamp,
            y: target_y,
        };
    }
    WorldPoint {
        x: anchor.stamp + (cp.x - anchor.stamp) * ratio,
        y: anchor.value + dy * ratio,
    }
}

fn clamp_handle_to_target_x(anchor: NumericPoint, cp: WorldPoint, target_x: f32) -> WorldPoint {
    let dx = cp.x - anchor.stamp;
    if dx == 0.0 {
        return WorldPoint {
            x: target_x,
            y: anchor.value,
        };
    }
    let ratio = (target_x - anchor.stamp) / dx;
    if !ratio.is_finite() {
        return WorldPoint {
            x: target_x,
            y: anchor.value,
        };
    }
    WorldPoint {
        x: anchor.stamp + dx * ratio,
        y: anchor.value + (cp.y - anchor.value) * ratio,
    }
}

fn solve_for_parameter(x0: f32, x1: f32, x2: f32, x3: f32, x: f32) -> f32 {
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..PARAMETER_SOLVE_MAX_ITERATIONS {
        let mid = (low + high) * 0.5;
        let estimate = evaluate_cubic(x0, x1, x2, x3, mid);
        if (estimate - x).abs() < PARAMETER_SOLVE_EPSILON {
            return mid;
        }
        if estimate < x {
            low = mid;
        } else {
            high = mid;
        }
    }
    (low + high) * 0.5
}

fn evaluate_cubic(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    p0 * u * u * u + 3.0 * p1 * u * u * t + 3.0 * p2 * u * t * t + p3 * t * t * t
}

fn is_straight_numeric_segment(segment: &NumericSegment) -> bool {
    let dx = segment.end.stamp - segment.start.stamp;
    let dy = segment.end.value - segment.start.value;
    if dx.abs() < f32::EPSILON {
        return false;
    }
    let line_y = |x: f32| segment.start.value + dy * ((x - segment.start.stamp) / dx);
    (segment.cp1.y - line_y(segment.cp1.x)).abs() < 1e-5
        && (segment.cp2.y - line_y(segment.cp2.x)).abs() < 1e-5
}
