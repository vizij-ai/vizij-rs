//! Blending utilities for Value types.
//! Minimal implementations required for animation/node-graph blending semantics.
//! - f32 linear interpolation for floats and vector components
//! - quaternion slerp (shortest-arc)
//! - transform TRS blending (pos/scale lerp, rot slerp)
//! - elementwise blending for generic Vector
//! - step blending for Bool/Text/Enum (choose left or right by t < 0.5)

use crate::coercion;
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

/// Blend two Values according to their kinds and shape semantics.
/// For mismatched kinds we attempt reasonable coercions:
/// - Float <-> Vector/VecN: broadcast scalar to vector
/// - VecN <-> Vector: convert VecN to Vector and blend elementwise
///   Step types (Bool/Text/Enum) are chosen based on t < 0.5 -> a else b.
pub fn blend_values(a: &Value, b: &Value, t: f32) -> Value {
    match (a, b) {
        (Value::Float(af), Value::Float(bf)) => Value::Float(lerp_f(*af, *bf, t)),

        (Value::Vec2(aa), Value::Vec2(bb)) => Value::Vec2(lerp_array(aa, bb, t)),
        (Value::Vec3(aa), Value::Vec3(bb)) => Value::Vec3(lerp_array(aa, bb, t)),
        (Value::Vec4(aa), Value::Vec4(bb)) => Value::Vec4(lerp_array(aa, bb, t)),

        (Value::ColorRgba(ac), Value::ColorRgba(bc)) => Value::ColorRgba(lerp_array(ac, bc, t)),

        (Value::Quat(aq), Value::Quat(bq)) => Value::Quat(slerp(*aq, *bq, t)),

        (
            Value::Transform {
                pos: ap,
                rot: ar,
                scale: ascale,
            },
            Value::Transform {
                pos: bp,
                rot: br,
                scale: bscale,
            },
        ) => {
            let pos = lerp_array(ap, bp, t);
            let scale = lerp_array(ascale, bscale, t);
            let rot = slerp(*ar, *br, t);
            Value::Transform { pos, rot, scale }
        }

        // Vector and VecN mixes
        (Value::Vector(va), Value::Vector(vb)) => Value::Vector(blend_vector(va, vb, t)),

        (Value::Float(af), Value::Vector(vb)) => {
            let a_vec = vec![*af; vb.len()];
            Value::Vector(blend_vector(&a_vec, vb, t))
        }
        (Value::Vector(va), Value::Float(bf)) => {
            let b_vec = vec![*bf; va.len()];
            Value::Vector(blend_vector(va, &b_vec, t))
        }

        (Value::Vec3(aa), Value::Vector(vb)) => {
            let a_vec = vec![aa[0], aa[1], aa[2]];
            Value::Vector(blend_vector(&a_vec, vb, t))
        }
        (Value::Vector(va), Value::Vec3(bb)) => {
            let b_vec = vec![bb[0], bb[1], bb[2]];
            Value::Vector(blend_vector(va, &b_vec, t))
        }

        (Value::Vec2(aa), Value::Vector(vb)) => {
            let a_vec = vec![aa[0], aa[1]];
            Value::Vector(blend_vector(&a_vec, vb, t))
        }
        (Value::Vector(va), Value::Vec2(bb)) => {
            let b_vec = vec![bb[0], bb[1]];
            Value::Vector(blend_vector(va, &b_vec, t))
        }

        // Fallback scalar-from-any -> scalar-from-any blending using coercion
        (a_val, b_val) => {
            // If either is a Vector-like, try to blend into Vector
            if let Value::Vector(_) = a_val {
                let va = coercion::to_vector(a_val);
                let vb = coercion::to_vector(b_val);
                return Value::Vector(blend_vector(&va, &vb, t));
            }
            if let Value::Vector(_) = b_val {
                let va = coercion::to_vector(a_val);
                let vb = coercion::to_vector(b_val);
                return Value::Vector(blend_vector(&va, &vb, t));
            }

            // Otherwise blend scalarly
            let fa = coercion::to_float(a_val);
            let fb = coercion::to_float(b_val);
            Value::Float(lerp_f(fa, fb, t))
        }
    }
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
    use crate::Value;

    #[test]
    fn blend_floats() {
        let a = Value::Float(0.0);
        let b = Value::Float(1.0);
        let r = blend_values(&a, &b, 0.5);
        assert_eq!(r, Value::Float(0.5));
    }

    #[test]
    fn blend_vec3() {
        let a = Value::Vec3([0.0, 0.0, 0.0]);
        let b = Value::Vec3([1.0, 2.0, 3.0]);
        let r = blend_values(&a, &b, 0.5);
        assert_eq!(r, Value::Vec3([0.5, 1.0, 1.5]));
    }

    #[test]
    fn blend_vector_mixed() {
        let a = Value::Vector(vec![1.0, 2.0]);
        let b = Value::Vector(vec![3.0, 4.0, 5.0]);
        let r = blend_values(&a, &b, 0.5);
        assert_eq!(r, Value::Vector(vec![2.0, 3.0, 2.5]));
    }

    #[test]
    fn step_bool_text() {
        let a = Value::Bool(false);
        let b = Value::Bool(true);
        assert_eq!(step_blend(&a, &b, 0.25), a);
        assert_eq!(step_blend(&a, &b, 0.75), b);
    }
}
