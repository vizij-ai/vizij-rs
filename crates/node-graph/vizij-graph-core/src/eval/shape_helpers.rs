//! Shape inference and validation helpers for node outputs.
//!
//! Values are classified through the vizij vocabulary ([`vocab::kind`] and
//! the `as_*` accessors); the [`ShapeId`] is declared metadata layered on
//! top. Because the wire value has a single sequence kind (`ArrayValue`),
//! the declared `Array`/`List`/`Tuple` shapes all validate against it — the
//! distinction lives in the shape alone.

use vizij_api_core::shape::Field;
use vizij_api_core::value as vocab;
use vizij_api_core::value::ShapeKind;
use vizij_api_core::{Shape, ShapeId, Value};

use super::value_layout::flatten_numeric;

/// Infer the [`Shape`] for a [`Value`].
pub fn infer_shape(value: &Value) -> Shape {
    Shape::new(infer_shape_id(value))
}

/// Infer the [`ShapeId`] for a [`Value`].
///
/// Sequences infer as `Array` when their items share one shape and as
/// `Tuple` otherwise. Enumerations carry no variant name on the wire, so
/// their inferred shape tags the variant with its id string. Values outside
/// the vizij vocabulary (integers, unit, unknown structures, ...) infer as
/// `Scalar`, matching their scalar numeric coercion.
pub fn infer_shape_id(value: &Value) -> ShapeId {
    match vocab::kind(value) {
        ShapeKind::Float => ShapeId::Scalar,
        ShapeKind::Bool => ShapeId::Bool,
        ShapeKind::Text => ShapeId::Text,
        ShapeKind::Vec2 => ShapeId::Vec2,
        ShapeKind::Vec3 => ShapeId::Vec3,
        ShapeKind::Vec4 => ShapeId::Vec4,
        ShapeKind::Quat => ShapeId::Quat,
        ShapeKind::ColorRgba => ShapeId::ColorRgba,
        ShapeKind::Transform => ShapeId::Transform,
        ShapeKind::Vector => {
            let len = vocab::as_vector(value).map(<[f32]>::len).unwrap_or(0);
            ShapeId::Vector {
                len: if len == 0 { None } else { Some(len) },
            }
        }
        ShapeKind::Record => {
            // `as_record` yields entries sorted by name.
            let fields: Vec<Field> = vocab::as_record(value)
                .unwrap_or_default()
                .into_iter()
                .map(|(name, value)| Field {
                    name: name.to_string(),
                    shape: infer_shape_id(value),
                })
                .collect();
            ShapeId::Record(fields)
        }
        ShapeKind::Array => {
            let items = vocab::as_array(value).unwrap_or_default();
            if let Some(first) = items.first() {
                let first_shape = infer_shape_id(first);
                let consistent = items.iter().all(|item| infer_shape_id(item) == first_shape);
                if consistent {
                    ShapeId::Array(Box::new(first_shape), items.len())
                } else {
                    ShapeId::Tuple(items.iter().map(infer_shape_id).collect())
                }
            } else {
                ShapeId::Array(Box::new(ShapeId::Scalar), 0)
            }
        }
        ShapeKind::Enum => match vocab::as_enumeration(value) {
            Some((variant, payload)) => {
                ShapeId::Enum(vec![(variant.to_string(), infer_shape_id(payload))])
            }
            None => ShapeId::Scalar,
        },
        ShapeKind::Other => ShapeId::Scalar,
    }
}

/// Check whether `value` conforms to the expected `shape`.
pub fn value_matches_shape(shape: &ShapeId, value: &Value) -> bool {
    match shape {
        ShapeId::Scalar => vocab::kind(value) == ShapeKind::Float,
        ShapeId::Bool => vocab::kind(value) == ShapeKind::Bool,
        ShapeId::Vec2 => vocab::kind(value) == ShapeKind::Vec2,
        ShapeId::Vec3 => vocab::kind(value) == ShapeKind::Vec3,
        ShapeId::Vec4 => vocab::kind(value) == ShapeKind::Vec4,
        ShapeId::Quat => vocab::kind(value) == ShapeKind::Quat,
        ShapeId::ColorRgba => vocab::kind(value) == ShapeKind::ColorRgba,
        ShapeId::Transform => vocab::kind(value) == ShapeKind::Transform,
        ShapeId::Text => vocab::kind(value) == ShapeKind::Text,
        ShapeId::Vector { len } => match vocab::as_vector(value) {
            Some(items) => match len {
                Some(expected) => items.len() == *expected,
                None => true,
            },
            None => false,
        },
        ShapeId::Record(fields) => match vocab::as_record(value) {
            Some(entries) => fields.iter().all(|field| {
                entries
                    .iter()
                    .find(|(name, _)| *name == field.name)
                    .map(|(_, v)| value_matches_shape(&field.shape, v))
                    .unwrap_or(false)
            }),
            None => false,
        },
        ShapeId::Array(inner, len) => match vocab::as_array(value) {
            Some(items) => {
                items.len() == *len && items.iter().all(|item| value_matches_shape(inner, item))
            }
            None => false,
        },
        ShapeId::List(inner) => match vocab::as_array(value) {
            Some(items) => items.iter().all(|item| value_matches_shape(inner, item)),
            None => false,
        },
        ShapeId::Tuple(entries) => match vocab::as_array(value) {
            Some(items) => {
                items.len() == entries.len()
                    && items
                        .iter()
                        .zip(entries.iter())
                        .all(|(item, shape)| value_matches_shape(shape, item))
            }
            None => false,
        },
        // Enum shape tags are variant names (compared through
        // [`vocab::variant_id`]); a tag equal to the id's string form is also
        // accepted, covering shapes inferred from values.
        ShapeId::Enum(variants) => match vocab::as_enumeration(value) {
            Some((variant, payload)) => variants
                .iter()
                .find(|(tag, _)| vocab::variant_id(tag) == variant || variant.to_string() == *tag)
                .is_some_and(|(_, shape)| value_matches_shape(shape, payload)),
            None => false,
        },
    }
}

/// Determine whether a [`ShapeId`] is composed entirely of numeric leaves.
pub fn is_numeric_like(shape: &ShapeId) -> bool {
    match shape {
        ShapeId::Scalar
        | ShapeId::Vec2
        | ShapeId::Vec3
        | ShapeId::Vec4
        | ShapeId::Quat
        | ShapeId::ColorRgba
        | ShapeId::Transform => true,
        ShapeId::Vector { .. } => true,
        ShapeId::Record(fields) => fields.iter().all(|field| is_numeric_like(&field.shape)),
        ShapeId::Array(inner, _) => is_numeric_like(inner),
        ShapeId::List(inner) => is_numeric_like(inner),
        ShapeId::Tuple(entries) => entries.iter().all(is_numeric_like),
        ShapeId::Enum(variants) => variants.iter().all(|(_, shape)| is_numeric_like(shape)),
        _ => false,
    }
}

/// Produce a NaN-filled value matching the provided numeric-like shape.
pub fn null_of_shape_numeric(shape: &ShapeId) -> Value {
    match shape {
        ShapeId::Scalar => vocab::float(f32::NAN),
        ShapeId::Vec2 => vocab::vec2([f32::NAN; 2]),
        ShapeId::Vec3 => vocab::vec3([f32::NAN; 3]),
        ShapeId::Vec4 => vocab::vec4([f32::NAN; 4]),
        ShapeId::Quat => vocab::quat([f32::NAN; 4]),
        ShapeId::ColorRgba => vocab::color_rgba([f32::NAN; 4]),
        ShapeId::Transform => vocab::transform(vocab::Transform {
            translation: [f32::NAN; 3],
            rotation: [f32::NAN; 4],
            scale: [f32::NAN; 3],
        }),
        ShapeId::Vector { len } => vocab::vector(match len {
            Some(expected) => vec![f32::NAN; *expected],
            None => Vec::new(),
        }),
        ShapeId::Record(fields) => vocab::record(
            fields
                .iter()
                .map(|field| (field.name.as_str(), null_of_shape_numeric(&field.shape))),
        ),
        ShapeId::Array(inner, len) => {
            vocab::array((0..*len).map(|_| null_of_shape_numeric(inner)).collect())
        }
        ShapeId::List(inner) => {
            // Length is unspecified; surface an empty sequence of the correct element type.
            let _ = inner;
            vocab::array(Vec::new())
        }
        ShapeId::Tuple(entries) => {
            vocab::array(entries.iter().map(null_of_shape_numeric).collect())
        }
        ShapeId::Enum(variants) => {
            if let Some((tag, variant_shape)) = variants.first() {
                vocab::enumeration(tag, null_of_shape_numeric(variant_shape))
            } else {
                vocab::enumeration("", vocab::float(f32::NAN))
            }
        }
        _ => vocab::float(f32::NAN),
    }
}

/// Attempt to coerce a numeric value into a declared numeric-like shape.
pub fn coerce_numeric_to_shape(target: &ShapeId, value: &Value) -> Option<Value> {
    let flat = flatten_numeric(value)?;

    match target {
        ShapeId::Vector { len: None } => Some(vocab::vector(flat.data)),
        ShapeId::List(_) | ShapeId::Enum(_) => None,
        _ => {
            let mut offset = 0usize;
            let rebuilt = build_numeric_value(target, &flat.data, &mut offset)?;
            if offset == flat.data.len() {
                Some(rebuilt)
            } else {
                None
            }
        }
    }
}

fn build_numeric_value(shape: &ShapeId, scalars: &[f32], offset: &mut usize) -> Option<Value> {
    match shape {
        ShapeId::Scalar => take_components::<1>(scalars, offset).map(|a| vocab::float(a[0])),
        ShapeId::Vec2 => take_components::<2>(scalars, offset).map(vocab::vec2),
        ShapeId::Vec3 => take_components::<3>(scalars, offset).map(vocab::vec3),
        ShapeId::Vec4 => take_components::<4>(scalars, offset).map(vocab::vec4),
        ShapeId::Quat => take_components::<4>(scalars, offset).map(vocab::quat),
        ShapeId::ColorRgba => take_components::<4>(scalars, offset).map(vocab::color_rgba),
        ShapeId::Transform => {
            let translation = take_components::<3>(scalars, offset)?;
            let rotation = take_components::<4>(scalars, offset)?;
            let scale = take_components::<3>(scalars, offset)?;
            Some(vocab::transform(vocab::Transform {
                translation,
                rotation,
                scale,
            }))
        }
        ShapeId::Vector {
            len: Some(expected),
        } => {
            let mut vec = Vec::with_capacity(*expected);
            for _ in 0..*expected {
                vec.push(*scalars.get(*offset)?);
                *offset += 1;
            }
            Some(vocab::vector(vec))
        }
        ShapeId::Record(fields) => {
            let mut entries = Vec::with_capacity(fields.len());
            for field in fields {
                let entry = build_numeric_value(&field.shape, scalars, offset)?;
                entries.push((field.name.as_str(), entry));
            }
            Some(vocab::record(entries))
        }
        ShapeId::Array(inner, len) => {
            let mut items = Vec::with_capacity(*len);
            for _ in 0..*len {
                items.push(build_numeric_value(inner, scalars, offset)?);
            }
            Some(vocab::array(items))
        }
        ShapeId::Tuple(entries) => {
            let mut items = Vec::with_capacity(entries.len());
            for entry in entries {
                items.push(build_numeric_value(entry, scalars, offset)?);
            }
            Some(vocab::array(items))
        }
        ShapeId::List(_)
        | ShapeId::Vector { len: None }
        | ShapeId::Enum(_)
        | ShapeId::Bool
        | ShapeId::Text => None,
    }
}

fn take_components<const N: usize>(scalars: &[f32], offset: &mut usize) -> Option<[f32; N]> {
    let mut arr = [0.0f32; N];
    for slot in arr.iter_mut() {
        *slot = *scalars.get(*offset)?;
        *offset += 1;
    }
    Some(arr)
}
