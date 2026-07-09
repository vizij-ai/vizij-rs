//! Vizij/Arora value interop.
//!
//! Vizij and Arora share one runtime value type: `arora_types::value::Value`,
//! re-exported (with its vizij vocabulary of type ids, constructors, and
//! accessors) by [`vizij_api_core::value`]. There is nothing to convert
//! between the two sides, so this crate holds only:
//!
//! - [`to_arora`] / [`from_arora`]: identity passthroughs kept so existing
//!   call sites keep compiling; new code uses the shared [`Value`] directly.
//! - The `Shape.meta` **sidecar** helpers ([`meta_key`], [`encode_shape_meta`],
//!   [`decode_shape_meta`]): `Value` carries no metadata, so a path's
//!   unit/space/range/color_space hints ride the same store under a reserved
//!   `"meta/"` namespace.

use std::collections::HashMap;

use vizij_api_core::{Shape, Value};

// ---- passthroughs ---------------------------------------------------------------

/// Identity passthrough: the Vizij and Arora value types are the same
/// [`Value`], so this returns a clone of its input. Kept only for source
/// compatibility; call sites can use the value directly instead.
pub fn to_arora(value: &Value) -> Value {
    value.clone()
}

/// Identity passthrough: the Vizij and Arora value types are the same
/// [`Value`], so this returns a clone of its input. Kept only for source
/// compatibility; call sites can use the value directly instead.
pub fn from_arora(value: &Value) -> Value {
    value.clone()
}

// ---- /meta sidecar ------------------------------------------------------------

/// The sidecar key carrying the metadata for the value stored at `data_path`.
///
/// Arora's `Value` has no place for `Shape.meta` (unit/space/range/color_space),
/// so it travels the same store under a reserved `"meta/"` namespace. (No leading slash: `TypedPath` rejects an empty first segment, and the sidecar must be storable in a `BlackboardStore`.)
pub fn meta_key(data_path: &str) -> String {
    format!("meta/{}", data_path.trim_start_matches('/'))
}

/// Encode a value's shape metadata for the sidecar key, if any is present.
pub fn encode_shape_meta(shape: &Shape) -> Option<Value> {
    if shape.meta.is_empty() {
        return None;
    }
    serde_json::to_string(&shape.meta).ok().map(Value::String)
}

/// Decode shape metadata previously written to a sidecar key.
pub fn decode_shape_meta(value: &Value) -> Option<HashMap<String, String>> {
    match value {
        Value::String(s) => serde_json::from_str(s).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vizij_api_core::value::{
        array, bool_, color_rgba, enumeration, float, quat, record, text, transform, vec2, vec3,
        vec4, vector,
    };
    use vizij_api_core::{ShapeId, Transform};

    #[test]
    fn passthroughs_are_identity_across_the_vocabulary() {
        let samples = vec![
            float(1.5),
            bool_(true),
            text("hi"),
            vector(vec![1.0, 2.0, 3.0]),
            vec2([1.0, 2.0]),
            vec3([1.0, 2.0, 3.0]),
            vec4([1.0, 2.0, 3.0, 4.0]),
            quat([0.0, 0.0, 0.0, 1.0]),
            color_rgba([0.1, 0.2, 0.3, 1.0]),
            transform(Transform {
                translation: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
            }),
            record([
                (
                    "left_arm",
                    record([("shoulder", float(0.4)), ("elbow", float(-1.2))]),
                ),
                ("confidence", float(0.9)),
            ]),
            array(vec![float(1.0), vec3([1.0, 2.0, 3.0]), text("mixed")]),
            enumeration("grasp", record([("force", float(0.5))])),
        ];
        for value in samples {
            assert_eq!(to_arora(&value), value);
            assert_eq!(from_arora(&value), value);
        }
    }

    #[test]
    fn values_outside_the_vocabulary_pass_through_unchanged() {
        // The passthroughs are total: values the vizij vocabulary gives no
        // reading to (integers, unit, ...) flow through untouched.
        for value in [Value::U32(3), Value::Unit] {
            assert_eq!(to_arora(&value), value);
            assert_eq!(from_arora(&value), value);
        }
    }

    #[test]
    fn meta_sidecar_round_trips() {
        assert_eq!(
            meta_key("standard/semio/mouth.x"),
            "meta/standard/semio/mouth.x"
        );
        let shape = Shape::new(ShapeId::Vec3)
            .with_meta("unit", "radians")
            .with_meta("space", "head");
        let encoded = encode_shape_meta(&shape).expect("some meta");
        let decoded = decode_shape_meta(&encoded).expect("decoded");
        assert_eq!(decoded.get("unit").map(String::as_str), Some("radians"));
        assert_eq!(decoded.get("space").map(String::as_str), Some("head"));
        assert!(encode_shape_meta(&Shape::new(ShapeId::Scalar)).is_none());
    }
}
