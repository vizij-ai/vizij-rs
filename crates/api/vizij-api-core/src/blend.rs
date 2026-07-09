//! Blending of [`Value`]s for animation/node-graph semantics.
//!
//! Each blend decodes both operands into PODs through the vocabulary
//! accessors, blends the PODs, and re-encodes the result through the
//! constructors — the dynamic `Value` never carries the arithmetic:
//! - floats and vector components lerp linearly;
//! - quaternions slerp (shortest arc);
//! - transforms blend TRS-wise (translation/scale lerp, rotation slerp);
//! - generic vectors blend elementwise (missing elements read as 0);
//! - records blend fields present on both sides, keeping the `t < 0.5` side
//!   as the base for the rest; sequences blend index-wise;
//! - mismatched kinds fall back to numeric coercion (vector-shaped operands
//!   blend as vectors, anything else as scalars);
//! - [`step_blend`] picks an operand whole (`t < 0.5` -> left), for
//!   step-only kinds such as booleans, text, and enumerations.

use crate::coercion;
use crate::value::{
    array, as_array, as_color_rgba, as_float, as_quat, as_record, as_transform, as_vec2, as_vec3,
    as_vec4, as_vector, color_rgba, float, kind, quat, record, transform, vec2, vec3, vec4, vector,
    Transform, VizijKind,
};
use crate::Value;

/// Linear interpolation for f32
#[inline]
fn lerp_f(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Lerp for fixed-size arrays
fn lerp_array<const N: usize>(a: &[f32; N], b: &[f32; N], t: f32) -> [f32; N] {
    let mut out = [0.0f32; N];
    for i in 0..N {
        out[i] = lerp_f(a[i], b[i], t);
    }
    out
}

/// Normalize a quaternion represented as [x,y,z,w]
fn normalize_quat(q: [f32; 4]) -> [f32; 4] {
    let mag = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if mag == 0.0 {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [q[0] / mag, q[1] / mag, q[2] / mag, q[3] / mag]
    }
}

/// Slerp between two unit quaternions q1, q2
fn slerp(q1: [f32; 4], q2: [f32; 4], t: f32) -> [f32; 4] {
    // Ensure inputs are normalized
    let qa = normalize_quat(q1);
    let mut qb = normalize_quat(q2);

    // Compute dot product
    let mut dot = qa[0] * qb[0] + qa[1] * qb[1] + qa[2] * qb[2] + qa[3] * qb[3];

    // If the dot product is negative, slerp won't take the short path.
    // Fix by reversing one quaternion.
    if dot < 0.0 {
        qb = [-qb[0], -qb[1], -qb[2], -qb[3]];
        dot = -dot;
    }

    // If quaternions are close, use lerp
    const DOT_THRESHOLD: f32 = 0.9995;
    if dot > DOT_THRESHOLD {
        let res = [
            lerp_f(qa[0], qb[0], t),
            lerp_f(qa[1], qb[1], t),
            lerp_f(qa[2], qb[2], t),
            lerp_f(qa[3], qb[3], t),
        ];
        return normalize_quat(res);
    }

    // Compute the angle between them and slerp
    let theta_0 = dot.clamp(-1.0, 1.0).acos(); // angle between input quaternions
    let theta = theta_0 * t;
    let sin_theta = theta.sin();
    let sin_theta_0 = theta_0.sin();

    let s0 = ((theta_0 - theta).sin()) / sin_theta_0;
    let s1 = sin_theta / sin_theta_0;

    [
        s0 * qa[0] + s1 * qb[0],
        s0 * qa[1] + s1 * qb[1],
        s0 * qa[2] + s1 * qb[2],
        s0 * qa[3] + s1 * qb[3],
    ]
}

/// Blend two generic vectors elementwise. If lengths differ, treat missing elements as 0.0.
fn blend_vector(a: &[f32], b: &[f32], t: f32) -> Vec<f32> {
    let n = std::cmp::max(a.len(), b.len());
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let ai = *a.get(i).unwrap_or(&0.0);
        let bi = *b.get(i).unwrap_or(&0.0);
        out.push(lerp_f(ai, bi, t));
    }
    out
}

/// Blend two Values according to their vizij kinds.
///
/// Matching kinds blend structurally (see the module header). For mismatched
/// kinds: if either side is a generic vector, both coerce to vectors and
/// blend elementwise; otherwise both coerce to scalars and lerp.
pub fn blend_values(a: &Value, b: &Value, t: f32) -> Value {
    use VizijKind as K;
    match (kind(a), kind(b)) {
        (K::Float, K::Float) => float(lerp_f(
            as_float(a).unwrap_or(0.0),
            as_float(b).unwrap_or(0.0),
            t,
        )),

        (K::Vec2, K::Vec2) => {
            let (aa, bb) = (
                as_vec2(a).unwrap_or([0.0; 2]),
                as_vec2(b).unwrap_or([0.0; 2]),
            );
            vec2(lerp_array(&aa, &bb, t))
        }
        (K::Vec3, K::Vec3) => {
            let (aa, bb) = (
                as_vec3(a).unwrap_or([0.0; 3]),
                as_vec3(b).unwrap_or([0.0; 3]),
            );
            vec3(lerp_array(&aa, &bb, t))
        }
        (K::Vec4, K::Vec4) => {
            let (aa, bb) = (
                as_vec4(a).unwrap_or([0.0; 4]),
                as_vec4(b).unwrap_or([0.0; 4]),
            );
            vec4(lerp_array(&aa, &bb, t))
        }
        (K::ColorRgba, K::ColorRgba) => {
            let (aa, bb) = (
                as_color_rgba(a).unwrap_or([0.0; 4]),
                as_color_rgba(b).unwrap_or([0.0; 4]),
            );
            color_rgba(lerp_array(&aa, &bb, t))
        }
        (K::Quat, K::Quat) => {
            let (aa, bb) = (
                as_quat(a).unwrap_or([0.0, 0.0, 0.0, 1.0]),
                as_quat(b).unwrap_or([0.0, 0.0, 0.0, 1.0]),
            );
            quat(slerp(aa, bb, t))
        }
        (K::Transform, K::Transform) => {
            let ta = as_transform(a).unwrap_or(IDENTITY_TRANSFORM);
            let tb = as_transform(b).unwrap_or(IDENTITY_TRANSFORM);
            transform(Transform {
                translation: lerp_array(&ta.translation, &tb.translation, t),
                rotation: slerp(ta.rotation, tb.rotation, t),
                scale: lerp_array(&ta.scale, &tb.scale, t),
            })
        }

        (K::Record, K::Record) => {
            let ra = as_record(a).unwrap_or_default();
            let rb = as_record(b).unwrap_or_default();
            let (base, other) = if t < 0.5 { (&ra, &rb) } else { (&rb, &ra) };
            let entries: Vec<(&str, Value)> = base
                .iter()
                .map(|(name, base_value)| {
                    let blended = other
                        .iter()
                        .find(|(other_name, _)| other_name == name)
                        .map(|(_, other_value)| {
                            // Keep a->b orientation regardless of which side is base.
                            if t < 0.5 {
                                blend_values(base_value, other_value, t)
                            } else {
                                blend_values(other_value, base_value, t)
                            }
                        })
                        .unwrap_or_else(|| (*base_value).clone());
                    (*name, blended)
                })
                .collect();
            record(entries)
        }

        (K::Array, K::Array) => {
            let (ia, ib) = (
                as_array(a).unwrap_or_default(),
                as_array(b).unwrap_or_default(),
            );
            array(blend_list_like(ia, ib, t))
        }

        (K::Vector, K::Vector) => {
            let (va, vb) = (
                as_vector(a).unwrap_or_default(),
                as_vector(b).unwrap_or_default(),
            );
            vector(blend_vector(va, vb, t))
        }

        // Scalars broadcast to the vector's length before blending.
        (K::Float, K::Vector) => {
            let vb = as_vector(b).unwrap_or_default();
            let va = vec![as_float(a).unwrap_or(0.0); vb.len()];
            vector(blend_vector(&va, vb, t))
        }
        (K::Vector, K::Float) => {
            let va = as_vector(a).unwrap_or_default();
            let vb = vec![as_float(b).unwrap_or(0.0); va.len()];
            vector(blend_vector(va, &vb, t))
        }

        // Mismatched kinds: vector-shaped operands blend as vectors, the
        // rest as scalars.
        (ka, kb) => {
            if ka == K::Vector || kb == K::Vector {
                let va = coercion::to_vector(a);
                let vb = coercion::to_vector(b);
                vector(blend_vector(&va, &vb, t))
            } else {
                float(lerp_f(coercion::to_float(a), coercion::to_float(b), t))
            }
        }
    }
}

const IDENTITY_TRANSFORM: Transform = Transform {
    translation: [0.0; 3],
    rotation: [0.0, 0.0, 0.0, 1.0],
    scale: [1.0; 3],
};

fn blend_list_like(a: &[Value], b: &[Value], t: f32) -> Vec<Value> {
    let len = a.len().max(b.len());
    let mut out = Vec::with_capacity(len);
    for idx in 0..len {
        match (a.get(idx), b.get(idx)) {
            (Some(va), Some(vb)) => out.push(blend_values(va, vb, t)),
            (Some(va), None) => {
                if t < 0.5 {
                    out.push(va.clone());
                }
            }
            (None, Some(vb)) => {
                if t >= 0.5 {
                    out.push(vb.clone());
                }
            }
            (None, None) => {}
        }
    }
    out
}

/// Step blending for step-only types: choose a for t < 0.5, else b.
pub fn step_blend(a: &Value, b: &Value, t: f32) -> Value {
    if t < 0.5 {
        a.clone()
    } else {
        b.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{as_bool, bool_, text};

    #[test]
    fn blend_floats() {
        let r = blend_values(&float(0.0), &float(1.0), 0.5);
        assert_eq!(as_float(&r), Some(0.5));
    }

    #[test]
    fn blend_vec3() {
        let a = vec3([0.0, 0.0, 0.0]);
        let b = vec3([1.0, 2.0, 3.0]);
        let r = blend_values(&a, &b, 0.5);
        assert_eq!(as_vec3(&r), Some([0.5, 1.0, 1.5]));
    }

    #[test]
    fn blend_quats_slerp() {
        let a = quat([0.0, 0.0, 0.0, 1.0]);
        let b = quat([1.0, 0.0, 0.0, 0.0]);
        let r = blend_values(&a, &b, 0.5);
        let q = as_quat(&r).expect("quat");
        // Halfway between identity and a 180-degree X rotation: 90 degrees.
        let inv_sqrt2 = 1.0 / 2.0f32.sqrt();
        assert!((q[0] - inv_sqrt2).abs() < 1e-4);
        assert!((q[3] - inv_sqrt2).abs() < 1e-4);
    }

    #[test]
    fn blend_transform_trs() {
        let a = transform(Transform {
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        });
        let b = transform(Transform {
            translation: [2.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [3.0; 3],
        });
        let r = as_transform(&blend_values(&a, &b, 0.5)).expect("transform");
        assert_eq!(r.translation, [1.0, 0.0, 0.0]);
        assert_eq!(r.scale, [2.0; 3]);
    }

    #[test]
    fn blend_vector_mixed_lengths() {
        let a = vector(vec![1.0, 2.0]);
        let b = vector(vec![3.0, 4.0, 5.0]);
        let r = blend_values(&a, &b, 0.5);
        assert_eq!(as_vector(&r), Some(&[2.0, 3.0, 2.5][..]));
    }

    #[test]
    fn blend_vector_with_scalar_broadcasts() {
        let a = float(1.0);
        let b = vector(vec![3.0, 5.0]);
        let r = blend_values(&a, &b, 0.5);
        assert_eq!(as_vector(&r), Some(&[2.0, 3.0][..]));
    }

    #[test]
    fn blend_records_by_field() {
        let a = record([("x", float(0.0)), ("only_a", float(7.0))]);
        let b = record([("x", float(1.0)), ("only_b", float(9.0))]);
        let low = blend_values(&a, &b, 0.25);
        let entries = as_record(&low).expect("record");
        assert_eq!(entries.len(), 2);
        assert_eq!(
            as_float(entries.iter().find(|(n, _)| *n == "x").unwrap().1),
            Some(0.25)
        );
        assert!(entries.iter().any(|(n, _)| *n == "only_a"));

        let high = blend_values(&a, &b, 0.75);
        let entries = as_record(&high).expect("record");
        assert_eq!(
            as_float(entries.iter().find(|(n, _)| *n == "x").unwrap().1),
            Some(0.75)
        );
        assert!(entries.iter().any(|(n, _)| *n == "only_b"));
    }

    #[test]
    fn blend_arrays_indexwise() {
        let a = array(vec![float(0.0), float(10.0)]);
        let b = array(vec![float(1.0)]);
        let r = blend_values(&a, &b, 0.25);
        let items = as_array(&r).expect("array");
        assert_eq!(items.len(), 2);
        assert_eq!(as_float(&items[0]), Some(0.25));
        assert_eq!(as_float(&items[1]), Some(10.0));
    }

    #[test]
    fn step_bool_text() {
        let a = bool_(false);
        let b = bool_(true);
        assert_eq!(as_bool(&step_blend(&a, &b, 0.25)), Some(false));
        assert_eq!(as_bool(&step_blend(&a, &b, 0.75)), Some(true));
        let ta = text("left");
        let tb = text("right");
        assert_eq!(step_blend(&ta, &tb, 0.9), tb);
    }
}
