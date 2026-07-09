//! Lossy numeric coercions from any [`Value`] into scalar/vector PODs.
//!
//! These helpers give every value a numeric reading so mixed-kind blends and
//! adapters always have something sensible to work with: scalars pass
//! through, composites decode via the vocabulary accessors, records and
//! sequences flatten (records in name order, for determinism over the
//! unordered field map), enumerations read through to their payload, and
//! anything without a numeric reading coerces to zero/empty.

use crate::value::{
    as_array, as_enumeration, as_record, as_transform, as_vec2, as_vec3, as_vec4, kind, VizijKind,
};
use crate::Value;

/// Coerce a value into a scalar `f32`.
///
/// Scalars (all numeric widths) cast; booleans read as `1.0`/`0.0`; vectors
/// and composites yield their first component (a transform its
/// `translation.x`); records/sequences recurse into their first element;
/// text and value-less kinds yield `0.0`.
pub fn to_float(v: &Value) -> f32 {
    match v {
        Value::F32(f) => *f,
        Value::F64(f) => *f as f32,
        Value::U8(n) => *n as f32,
        Value::U16(n) => *n as f32,
        Value::U32(n) => *n as f32,
        Value::U64(n) => *n as f32,
        Value::I8(n) => *n as f32,
        Value::I16(n) => *n as f32,
        Value::I32(n) => *n as f32,
        Value::I64(n) => *n as f32,
        Value::Boolean(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::ArrayF32(xs) => xs.first().copied().unwrap_or(0.0),
        Value::ArrayF64(xs) => xs.first().copied().unwrap_or(0.0) as f32,
        Value::ArrayValue(items) => items.first().map(to_float).unwrap_or(0.0),
        Value::KeyValue(_) => as_record(v)
            .and_then(|entries| entries.first().map(|(_, value)| to_float(value)))
            .unwrap_or(0.0),
        Value::Enumeration(_) => as_enumeration(v).map(|(_, p)| to_float(p)).unwrap_or(0.0),
        Value::Option(Some(inner)) => to_float(inner),
        Value::Structure(s) => {
            if let Some(t) = as_transform(v) {
                t.translation[0]
            } else if let Some(a) = as_vec2(v) {
                a[0]
            } else {
                // vec3/vec4/quat/color and unknown structures alike: first field.
                s.fields.first().map(|f| to_float(&f.value)).unwrap_or(0.0)
            }
        }
        _ => 0.0,
    }
}

/// Coerce a value into a generic `Vec<f32>`.
///
/// Scalars/booleans become one-element vectors; `vec2/3/4`, `quat`, and
/// `color-rgba` yield their components; a transform yields its translation;
/// records (name order) and sequences flatten recursively; text and
/// value-less kinds yield an empty vector.
pub fn to_vector(v: &Value) -> Vec<f32> {
    match kind(v) {
        VizijKind::Float | VizijKind::Bool => vec![to_float(v)],
        VizijKind::Text => vec![],
        VizijKind::Vector => match v {
            Value::ArrayF32(xs) => xs.clone(),
            _ => unreachable!("VizijKind::Vector is only ArrayF32"),
        },
        VizijKind::Vec2 => as_vec2(v).map(|a| a.to_vec()).unwrap_or_default(),
        VizijKind::Vec3 => as_vec3(v).map(|a| a.to_vec()).unwrap_or_default(),
        VizijKind::Vec4 => as_vec4(v).map(|a| a.to_vec()).unwrap_or_default(),
        VizijKind::Quat => crate::value::as_quat(v)
            .map(|a| a.to_vec())
            .unwrap_or_default(),
        VizijKind::ColorRgba => crate::value::as_color_rgba(v)
            .map(|a| a.to_vec())
            .unwrap_or_default(),
        VizijKind::Transform => as_transform(v)
            .map(|t| t.translation.to_vec())
            .unwrap_or_default(),
        VizijKind::Record => as_record(v)
            .map(|entries| {
                entries
                    .iter()
                    .flat_map(|(_, value)| to_vector(value))
                    .collect()
            })
            .unwrap_or_default(),
        VizijKind::Array => as_array(v)
            .map(|items| items.iter().flat_map(to_vector).collect())
            .unwrap_or_default(),
        VizijKind::Enum => as_enumeration(v)
            .map(|(_, payload)| to_vector(payload))
            .unwrap_or_default(),
        VizijKind::Other => match v {
            Value::U8(_)
            | Value::U16(_)
            | Value::U32(_)
            | Value::U64(_)
            | Value::I8(_)
            | Value::I16(_)
            | Value::I32(_)
            | Value::I64(_) => vec![to_float(v)],
            Value::ArrayF64(xs) => xs.iter().map(|x| *x as f32).collect(),
            Value::ArrayU8(xs) => xs.iter().map(|x| *x as f32).collect(),
            Value::ArrayU16(xs) => xs.iter().map(|x| *x as f32).collect(),
            Value::ArrayU32(xs) => xs.iter().map(|x| *x as f32).collect(),
            Value::ArrayU64(xs) => xs.iter().map(|x| *x as f32).collect(),
            Value::ArrayI8(xs) => xs.iter().map(|x| *x as f32).collect(),
            Value::ArrayI16(xs) => xs.iter().map(|x| *x as f32).collect(),
            Value::ArrayI32(xs) => xs.iter().map(|x| *x as f32).collect(),
            Value::ArrayI64(xs) => xs.iter().map(|x| *x as f32).collect(),
            Value::ArrayBoolean(xs) => xs.iter().map(|b| if *b { 1.0 } else { 0.0 }).collect(),
            Value::Option(Some(inner)) => to_vector(inner),
            // Unknown structures flatten their fields in declaration order.
            Value::Structure(s) => s.fields.iter().flat_map(|f| to_vector(&f.value)).collect(),
            _ => vec![],
        },
    }
}

/// Coerce a value into a `[f32; 3]`.
///
/// `vec3` passes through; `vec2` zero-extends; `vec4`/`quat` truncate;
/// scalars/booleans broadcast; a transform yields its translation; anything
/// else flattens via [`to_vector`] and takes the first three components
/// (zero-padded).
pub fn to_vec3(v: &Value) -> [f32; 3] {
    match kind(v) {
        VizijKind::Vec3 => as_vec3(v).unwrap_or([0.0; 3]),
        VizijKind::Vec2 => {
            let a = as_vec2(v).unwrap_or([0.0; 2]);
            [a[0], a[1], 0.0]
        }
        VizijKind::Vec4 => {
            let a = as_vec4(v).unwrap_or([0.0; 4]);
            [a[0], a[1], a[2]]
        }
        VizijKind::Float | VizijKind::Bool => {
            let f = to_float(v);
            [f, f, f]
        }
        VizijKind::Transform => as_transform(v).map(|t| t.translation).unwrap_or([0.0; 3]),
        VizijKind::Enum => as_enumeration(v)
            .map(|(_, payload)| to_vec3(payload))
            .unwrap_or([0.0; 3]),
        VizijKind::Text => [0.0; 3],
        _ => {
            let flat = to_vector(v);
            let mut out = [0.0f32; 3];
            for (slot, x) in out.iter_mut().zip(flat) {
                *slot = x;
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{
        array, bool_, enumeration, float, quat, record, text, transform, vec2, vec3, vec4, vector,
        Transform,
    };

    #[test]
    fn floats_from_everything() {
        assert_eq!(to_float(&float(1.5)), 1.5);
        assert_eq!(to_float(&Value::F64(2.0)), 2.0);
        assert_eq!(to_float(&Value::I32(-3)), -3.0);
        assert_eq!(to_float(&bool_(true)), 1.0);
        assert_eq!(to_float(&vec3([7.0, 8.0, 9.0])), 7.0);
        assert_eq!(to_float(&vector(vec![4.0, 5.0])), 4.0);
        assert_eq!(to_float(&vector(vec![])), 0.0);
        assert_eq!(to_float(&text("hi")), 0.0);
        assert_eq!(to_float(&enumeration("v", float(0.5))), 0.5);
        assert_eq!(to_float(&array(vec![float(6.0)])), 6.0);
        // Records read their first entry in name order.
        assert_eq!(
            to_float(&record([("b", float(2.0)), ("a", float(1.0))])),
            1.0
        );
        let t = transform(Transform {
            translation: [3.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        });
        assert_eq!(to_float(&t), 3.0);
    }

    #[test]
    fn vectors_flatten() {
        assert_eq!(to_vector(&float(1.0)), vec![1.0]);
        assert_eq!(to_vector(&vec2([1.0, 2.0])), vec![1.0, 2.0]);
        assert_eq!(
            to_vector(&vec4([1.0, 2.0, 3.0, 4.0])),
            vec![1.0, 2.0, 3.0, 4.0]
        );
        assert_eq!(
            to_vector(&quat([0.0, 0.0, 0.0, 1.0])),
            vec![0.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(to_vector(&vector(vec![9.0])), vec![9.0]);
        assert_eq!(to_vector(&text("hi")), Vec::<f32>::new());
        assert_eq!(
            to_vector(&array(vec![float(1.0), vec2([2.0, 3.0])])),
            vec![1.0, 2.0, 3.0]
        );
        assert_eq!(
            to_vector(&record([("b", float(2.0)), ("a", float(1.0))])),
            vec![1.0, 2.0]
        );
        let t = transform(Transform {
            translation: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        });
        assert_eq!(to_vector(&t), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn vec3_broadcasts_and_truncates() {
        assert_eq!(to_vec3(&vec3([1.0, 2.0, 3.0])), [1.0, 2.0, 3.0]);
        assert_eq!(to_vec3(&vec2([1.0, 2.0])), [1.0, 2.0, 0.0]);
        assert_eq!(to_vec3(&vec4([1.0, 2.0, 3.0, 4.0])), [1.0, 2.0, 3.0]);
        assert_eq!(to_vec3(&float(2.0)), [2.0, 2.0, 2.0]);
        assert_eq!(to_vec3(&bool_(true)), [1.0, 1.0, 1.0]);
        assert_eq!(to_vec3(&vector(vec![5.0])), [5.0, 0.0, 0.0]);
        assert_eq!(to_vec3(&text("hi")), [0.0, 0.0, 0.0]);
    }
}
