//! Animation-specific value types layered on top of `vizij-api-core`.
//!
//! Tracks store keyframe values as [`TrackValue`]: the plain-Rust (POD) form
//! of the vizij vocabulary, decoded once when an animation is parsed or
//! constructed. Sampling, interpolation, and accumulation compute on these
//! PODs; the dynamic [`Value`] appears only at the output boundary (engine
//! changes, write batches, baked artifacts), encoded through the vocabulary
//! constructors.

use serde::{Deserialize, Serialize};
use vizij_api_core::value as vocab;

pub use vizij_api_core::{Transform, Value};

/// Keyframe value in plain-Rust form.
///
/// Numeric variants interpolate and blend; `Bool`, `Text`, and `Step` hold
/// their value (step semantics). `Vector` is a variable-length numeric
/// vector (`ArrayF32` on the wire); `NumericArray` is an all-scalar
/// `ArrayValue` sequence, kept distinct so it re-encodes as `ArrayValue`.
/// `Step` carries any other [`Value`] untouched.
///
/// Serialization goes through the wire-form [`Value`] in both directions,
/// so a `TrackValue` field serializes exactly like the vocabulary encoding
/// it decodes from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(into = "Value", from = "Value")]
pub enum TrackValue {
    /// Scalar float.
    Float(f32),
    /// 2D vector.
    Vec2([f32; 2]),
    /// 3D vector.
    Vec3([f32; 3]),
    /// 4D vector.
    Vec4([f32; 4]),
    /// Quaternion as `[x, y, z, w]`.
    Quat([f32; 4]),
    /// RGBA color as `[r, g, b, a]`.
    ColorRgba([f32; 4]),
    /// TRS transform.
    Transform(Transform),
    /// Variable-length numeric vector (`ArrayF32`).
    Vector(Vec<f32>),
    /// All-scalar sequence (`ArrayValue` of floats).
    NumericArray(Vec<f32>),
    /// Boolean; sampled with step (hold) semantics.
    Bool(bool),
    /// UTF-8 text; sampled with step (hold) semantics.
    Text(String),
    /// Any other value, held as-is with step semantics.
    Step(Value),
}

impl TrackValue {
    /// Encode into the wire-form [`Value`] through the vocabulary
    /// constructors.
    pub fn to_value(&self) -> Value {
        Value::from(self.clone())
    }
}

impl From<Value> for TrackValue {
    fn from(value: Value) -> Self {
        match value {
            Value::F32(f) => TrackValue::Float(f),
            Value::F64(f) => TrackValue::Float(f as f32),
            Value::Boolean(b) => TrackValue::Bool(b),
            Value::String(s) => TrackValue::Text(s),
            Value::ArrayF32(xs) => TrackValue::Vector(xs),
            Value::Structure(_) => {
                if let Some(a) = vocab::as_vec2(&value) {
                    TrackValue::Vec2(a)
                } else if let Some(a) = vocab::as_vec3(&value) {
                    TrackValue::Vec3(a)
                } else if let Some(a) = vocab::as_vec4(&value) {
                    TrackValue::Vec4(a)
                } else if let Some(a) = vocab::as_quat(&value) {
                    TrackValue::Quat(a)
                } else if let Some(a) = vocab::as_color_rgba(&value) {
                    TrackValue::ColorRgba(a)
                } else if let Some(t) = vocab::as_transform(&value) {
                    TrackValue::Transform(t)
                } else {
                    TrackValue::Step(value)
                }
            }
            Value::ArrayValue(ref items) => {
                if let Some(floats) = items
                    .iter()
                    .map(vocab::as_float)
                    .collect::<Option<Vec<f32>>>()
                {
                    TrackValue::NumericArray(floats)
                } else {
                    TrackValue::Step(value)
                }
            }
            other => TrackValue::Step(other),
        }
    }
}

impl From<TrackValue> for Value {
    fn from(tv: TrackValue) -> Self {
        match tv {
            TrackValue::Float(f) => vocab::float(f),
            TrackValue::Vec2(a) => vocab::vec2(a),
            TrackValue::Vec3(a) => vocab::vec3(a),
            TrackValue::Vec4(a) => vocab::vec4(a),
            TrackValue::Quat(a) => vocab::quat(a),
            TrackValue::ColorRgba(a) => vocab::color_rgba(a),
            TrackValue::Transform(t) => vocab::transform(t),
            TrackValue::Vector(xs) => vocab::vector(xs),
            TrackValue::NumericArray(xs) => {
                vocab::array(xs.into_iter().map(vocab::float).collect())
            }
            TrackValue::Bool(b) => vocab::bool_(b),
            TrackValue::Text(s) => Value::String(s),
            TrackValue::Step(v) => v,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vizij_api_core::value::{
        array, bool_, color_rgba, enumeration, float, quat, record, text, transform, vec2, vec3,
        vec4, vector,
    };

    #[test]
    fn decodes_the_vocabulary_into_pods() {
        assert_eq!(TrackValue::from(float(1.5)), TrackValue::Float(1.5));
        assert_eq!(TrackValue::from(Value::F64(2.5)), TrackValue::Float(2.5));
        assert_eq!(TrackValue::from(bool_(true)), TrackValue::Bool(true));
        assert_eq!(
            TrackValue::from(text("hi")),
            TrackValue::Text("hi".to_string())
        );
        assert_eq!(
            TrackValue::from(vector(vec![1.0, 2.0])),
            TrackValue::Vector(vec![1.0, 2.0])
        );
        assert_eq!(
            TrackValue::from(vec2([1.0, 2.0])),
            TrackValue::Vec2([1.0, 2.0])
        );
        assert_eq!(
            TrackValue::from(vec3([1.0, 2.0, 3.0])),
            TrackValue::Vec3([1.0, 2.0, 3.0])
        );
        assert_eq!(
            TrackValue::from(vec4([1.0, 2.0, 3.0, 4.0])),
            TrackValue::Vec4([1.0, 2.0, 3.0, 4.0])
        );
        assert_eq!(
            TrackValue::from(quat([0.0, 0.0, 0.0, 1.0])),
            TrackValue::Quat([0.0, 0.0, 0.0, 1.0])
        );
        assert_eq!(
            TrackValue::from(color_rgba([0.1, 0.2, 0.3, 1.0])),
            TrackValue::ColorRgba([0.1, 0.2, 0.3, 1.0])
        );
        let t = Transform {
            translation: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        };
        assert_eq!(TrackValue::from(transform(t)), TrackValue::Transform(t));
    }

    #[test]
    fn numeric_sequences_decode_as_numeric_arrays() {
        assert_eq!(
            TrackValue::from(array(vec![float(1.0), float(2.0)])),
            TrackValue::NumericArray(vec![1.0, 2.0])
        );
        // Mixed-kind sequences have no numeric reading and hold as-is.
        let mixed = array(vec![float(1.0), text("x")]);
        assert_eq!(TrackValue::from(mixed.clone()), TrackValue::Step(mixed));
    }

    #[test]
    fn values_without_a_pod_reading_hold_as_step() {
        let rec = record([("x", float(1.0))]);
        assert_eq!(TrackValue::from(rec.clone()), TrackValue::Step(rec));
        let en = enumeration("grasp", float(0.5));
        assert_eq!(TrackValue::from(en.clone()), TrackValue::Step(en));
        assert_eq!(
            TrackValue::from(Value::U32(3)),
            TrackValue::Step(Value::U32(3))
        );
    }

    #[test]
    fn encodes_back_through_the_vocabulary() {
        let cases = [
            float(1.5),
            bool_(true),
            text("hi"),
            vector(vec![1.0, 2.0]),
            vec2([1.0, 2.0]),
            vec3([1.0, 2.0, 3.0]),
            vec4([1.0, 2.0, 3.0, 4.0]),
            quat([0.0, 0.0, 0.0, 1.0]),
            color_rgba([0.1, 0.2, 0.3, 1.0]),
            transform(Transform {
                translation: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
            }),
            array(vec![float(1.0), float(2.0)]),
            record([("x", float(1.0))]),
        ];
        for value in cases {
            assert_eq!(TrackValue::from(value.clone()).to_value(), value);
        }
    }

    #[test]
    fn serde_uses_the_wire_form() {
        let tv = TrackValue::Vec3([1.0, 2.0, 3.0]);
        let json = serde_json::to_value(&tv).expect("serialize");
        let as_value = serde_json::to_value(vec3([1.0, 2.0, 3.0])).expect("serialize value");
        assert_eq!(json, as_value);
        let back: TrackValue = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, tv);
    }
}
