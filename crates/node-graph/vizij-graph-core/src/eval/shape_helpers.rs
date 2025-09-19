//! Shape inference and validation helpers for node outputs.

use crate::types::NodeSpec;
use hashbrown::HashMap;
use vizij_api_core::shape::Field;
use vizij_api_core::{Shape, ShapeId, Value};

use super::value_layout::PortValue;

/// Infer the [`Shape`] for a [`Value`].
pub fn infer_shape(value: &Value) -> Shape {
    Shape::new(infer_shape_id(value))
}

/// Infer the [`ShapeId`] for a [`Value`].
pub fn infer_shape_id(value: &Value) -> ShapeId {
    match value {
        Value::Float(_) => ShapeId::Scalar,
        Value::Bool(_) => ShapeId::Bool,
        Value::Vec2(_) => ShapeId::Vec2,
        Value::Vec3(_) => ShapeId::Vec3,
        Value::Vec4(_) => ShapeId::Vec4,
        Value::Quat(_) => ShapeId::Quat,
        Value::ColorRgba(_) => ShapeId::ColorRgba,
        Value::Transform { .. } => ShapeId::Transform,
        Value::Vector(vec) => ShapeId::Vector {
            len: if vec.is_empty() {
                None
            } else {
                Some(vec.len())
            },
        },
        Value::Text(_) => ShapeId::Text,
        Value::Enum(tag, boxed) => ShapeId::Enum(vec![(tag.clone(), infer_shape_id(boxed))]),
        Value::Record(map) => {
            let mut fields: Vec<Field> = map
                .iter()
                .map(|(name, value)| Field {
                    name: name.clone(),
                    shape: infer_shape_id(value),
                })
                .collect();
            fields.sort_by(|a, b| a.name.cmp(&b.name));
            ShapeId::Record(fields)
        }
        Value::Array(items) => {
            if let Some(first) = items.first() {
                let first_shape = infer_shape_id(first);
                let consistent = items.iter().all(|item| infer_shape_id(item) == first_shape);
                let inner = if consistent {
                    first_shape
                } else {
                    ShapeId::Scalar
                };
                ShapeId::Array(Box::new(inner), items.len())
            } else {
                ShapeId::Array(Box::new(ShapeId::Scalar), 0)
            }
        }
        Value::List(items) => {
            if let Some(first) = items.first() {
                let first_shape = infer_shape_id(first);
                let consistent = items.iter().all(|item| infer_shape_id(item) == first_shape);
                let inner = if consistent {
                    first_shape
                } else {
                    ShapeId::Scalar
                };
                ShapeId::List(Box::new(inner))
            } else {
                ShapeId::List(Box::new(ShapeId::Scalar))
            }
        }
        Value::Tuple(items) => {
            let shapes = items.iter().map(infer_shape_id).collect();
            ShapeId::Tuple(shapes)
        }
    }
}

/// Ensure outputs match their declared shapes, updating cached shapes in-place.
pub fn enforce_output_shapes(
    spec: &NodeSpec,
    outputs: &mut HashMap<String, PortValue>,
) -> Result<(), String> {
    if spec.output_shapes.is_empty() {
        return Ok(());
    }

    for (key, declared) in spec.output_shapes.iter() {
        let port = outputs.get_mut(key).ok_or_else(|| {
            format!(
                "node '{}' missing declared output '{}' during evaluation",
                spec.id, key
            )
        })?;

        if !value_matches_shape(&declared.id, &port.value) {
            return Err(format!(
                "node '{}' output '{}' does not match declared shape {:?}",
                spec.id, key, declared.id
            ));
        }

        port.shape = declared.clone();
    }

    Ok(())
}

/// Check whether `value` conforms to the expected `shape`.
pub fn value_matches_shape(shape: &ShapeId, value: &Value) -> bool {
    match shape {
        ShapeId::Scalar => matches!(value, Value::Float(_)),
        ShapeId::Bool => matches!(value, Value::Bool(_)),
        ShapeId::Vec2 => matches!(value, Value::Vec2(_)),
        ShapeId::Vec3 => matches!(value, Value::Vec3(_)),
        ShapeId::Vec4 => matches!(value, Value::Vec4(_)),
        ShapeId::Quat => matches!(value, Value::Quat(_)),
        ShapeId::ColorRgba => matches!(value, Value::ColorRgba(_)),
        ShapeId::Transform => matches!(value, Value::Transform { .. }),
        ShapeId::Text => matches!(value, Value::Text(_)),
        ShapeId::Vector { len } => match value {
            Value::Vector(items) => match len {
                Some(expected) => items.len() == *expected,
                None => true,
            },
            _ => false,
        },
        ShapeId::Record(fields) => match value {
            Value::Record(map) => fields.iter().all(|field| {
                map.get(&field.name)
                    .map(|v| value_matches_shape(&field.shape, v))
                    .unwrap_or(false)
            }),
            _ => false,
        },
        ShapeId::Array(inner, len) => match value {
            Value::Array(items) => {
                items.len() == *len && items.iter().all(|item| value_matches_shape(inner, item))
            }
            _ => false,
        },
        ShapeId::List(inner) => match value {
            Value::List(items) => items.iter().all(|item| value_matches_shape(inner, item)),
            _ => false,
        },
        ShapeId::Tuple(entries) => match value {
            Value::Tuple(items) => {
                items.len() == entries.len()
                    && items
                        .iter()
                        .zip(entries.iter())
                        .all(|(item, shape)| value_matches_shape(shape, item))
            }
            _ => false,
        },
        ShapeId::Enum(variants) => match value {
            Value::Enum(tag, boxed) => variants
                .iter()
                .find(|(variant, _)| variant == tag)
                .is_some_and(|(_, shape)| value_matches_shape(shape, boxed)),
            _ => false,
        },
    }
}
