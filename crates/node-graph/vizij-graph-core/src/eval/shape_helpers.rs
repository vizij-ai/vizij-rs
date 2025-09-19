//! Shape inference and validation helpers for node outputs.

use crate::types::{NodeSpec, SelectorSegment};
use hashbrown::HashMap;
use vizij_api_core::shape::Field;
use vizij_api_core::{Shape, ShapeId, Value};

use super::value_layout::{flatten_numeric, PortValue};

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
        ShapeId::Scalar => Value::Float(f32::NAN),
        ShapeId::Vec2 => Value::Vec2([f32::NAN; 2]),
        ShapeId::Vec3 => Value::Vec3([f32::NAN; 3]),
        ShapeId::Vec4 => Value::Vec4([f32::NAN; 4]),
        ShapeId::Quat => Value::Quat([f32::NAN; 4]),
        ShapeId::ColorRgba => Value::ColorRgba([f32::NAN; 4]),
        ShapeId::Transform => Value::Transform {
            pos: [f32::NAN; 3],
            rot: [f32::NAN; 4],
            scale: [f32::NAN; 3],
        },
        ShapeId::Vector { len } => Value::Vector(match len {
            Some(expected) => vec![f32::NAN; *expected],
            None => Vec::new(),
        }),
        ShapeId::Record(fields) => {
            let mut map = HashMap::with_capacity(fields.len());
            for field in fields {
                map.insert(field.name.clone(), null_of_shape_numeric(&field.shape));
            }
            Value::Record(map)
        }
        ShapeId::Array(inner, len) => {
            Value::Array((0..*len).map(|_| null_of_shape_numeric(inner)).collect())
        }
        ShapeId::List(inner) => {
            // Length is unspecified; surface an empty list of the correct element type.
            let _ = inner;
            Value::List(Vec::new())
        }
        ShapeId::Tuple(entries) => {
            Value::Tuple(entries.iter().map(null_of_shape_numeric).collect())
        }
        ShapeId::Enum(variants) => {
            if let Some((tag, variant_shape)) = variants.first() {
                Value::Enum(tag.clone(), Box::new(null_of_shape_numeric(variant_shape)))
            } else {
                Value::Enum(String::new(), Box::new(Value::Float(f32::NAN)))
            }
        }
        _ => Value::Float(f32::NAN),
    }
}

/// Apply selector segments to a value and optional shape metadata, returning the projected value.
pub fn project_by_selector(
    value: &Value,
    shape: Option<&ShapeId>,
    selector: &[SelectorSegment],
) -> Result<(Value, Option<ShapeId>), String> {
    if selector.is_empty() {
        return Ok((value.clone(), shape.cloned()));
    }

    let mut current_value = value.clone();
    let mut current_shape = shape.cloned();

    for segment in selector {
        match segment {
            SelectorSegment::Field(field) => {
                let next_value = match &current_value {
                    Value::Record(map) => map
                        .get(field)
                        .cloned()
                        .ok_or_else(|| format!("selector field '{}' missing in record", field))?,
                    Value::Transform { pos, rot, scale } => {
                        transform_field_value(field, pos, rot, scale).ok_or_else(|| {
                            format!("selector field '{}' invalid for transform", field)
                        })?
                    }
                    Value::Enum(tag, payload) => {
                        if tag == field {
                            (**payload).clone()
                        } else {
                            return Err(format!(
                                "selector field '{}' does not match enum variant '{}'",
                                field, tag
                            ));
                        }
                    }
                    _ => {
                        return Err(format!(
                            "selector field '{}' unsupported for value {:?}",
                            field,
                            infer_shape_id(&current_value)
                        ))
                    }
                };

                let mut next_shape = None;
                if let Some(shape_id) = current_shape.as_ref() {
                    next_shape = match shape_id {
                        ShapeId::Record(fields) => fields
                            .iter()
                            .find(|f| f.name == *field)
                            .map(|f| f.shape.clone()),
                        ShapeId::Transform => transform_field_shape(field),
                        ShapeId::Enum(variants) => variants
                            .iter()
                            .find(|(tag, _)| tag == field)
                            .map(|(_, shape)| shape.clone()),
                        _ => None,
                    };
                }

                current_value = next_value;
                current_shape = next_shape.or_else(|| Some(infer_shape_id(&current_value)));
            }
            SelectorSegment::Index(index) => {
                let idx = *index;
                let next_value = match &current_value {
                    Value::Vector(vec) => {
                        vec.get(idx).map(|v| Value::Float(*v)).ok_or_else(|| {
                            format!(
                                "selector index {} out of bounds for vector of len {}",
                                idx,
                                vec.len()
                            )
                        })?
                    }
                    Value::Array(items) => items.get(idx).cloned().ok_or_else(|| {
                        format!(
                            "selector index {} out of bounds for array of len {}",
                            idx,
                            items.len()
                        )
                    })?,
                    Value::List(items) => items.get(idx).cloned().ok_or_else(|| {
                        format!(
                            "selector index {} out of bounds for list of len {}",
                            idx,
                            items.len()
                        )
                    })?,
                    Value::Tuple(items) => items.get(idx).cloned().ok_or_else(|| {
                        format!(
                            "selector index {} out of bounds for tuple of len {}",
                            idx,
                            items.len()
                        )
                    })?,
                    Value::Vec2(arr) => arr
                        .get(idx)
                        .map(|v| Value::Float(*v))
                        .ok_or_else(|| format!("selector index {} out of bounds for vec2", idx))?,
                    Value::Vec3(arr) => arr
                        .get(idx)
                        .map(|v| Value::Float(*v))
                        .ok_or_else(|| format!("selector index {} out of bounds for vec3", idx))?,
                    Value::Vec4(arr) => arr
                        .get(idx)
                        .map(|v| Value::Float(*v))
                        .ok_or_else(|| format!("selector index {} out of bounds for vec4", idx))?,
                    Value::Quat(arr) => arr
                        .get(idx)
                        .map(|v| Value::Float(*v))
                        .ok_or_else(|| format!("selector index {} out of bounds for quat", idx))?,
                    Value::ColorRgba(arr) => arr
                        .get(idx)
                        .map(|v| Value::Float(*v))
                        .ok_or_else(|| format!("selector index {} out of bounds for color", idx))?,
                    _ => {
                        return Err(format!(
                            "selector index {} unsupported for value {:?}",
                            idx,
                            infer_shape_id(&current_value)
                        ))
                    }
                };

                let mut next_shape = None;
                if let Some(shape_id) = current_shape.as_ref() {
                    next_shape = match shape_id {
                        ShapeId::Vector { .. }
                        | ShapeId::Vec2
                        | ShapeId::Vec3
                        | ShapeId::Vec4
                        | ShapeId::Quat
                        | ShapeId::ColorRgba => Some(ShapeId::Scalar),
                        ShapeId::Array(inner, _) | ShapeId::List(inner) => Some((**inner).clone()),
                        ShapeId::Tuple(entries) => entries.get(idx).cloned(),
                        _ => None,
                    };
                }

                current_value = next_value;
                current_shape = next_shape.or_else(|| Some(infer_shape_id(&current_value)));
            }
        }
    }

    Ok((current_value, current_shape))
}

/// Attempt to coerce a numeric value into a declared numeric-like shape.
pub fn coerce_numeric_to_shape(target: &ShapeId, value: &Value) -> Option<Value> {
    let flat = flatten_numeric(value)?;

    match target {
        ShapeId::Vector { len: None } => Some(Value::Vector(flat.data)),
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
        ShapeId::Scalar => {
            let value = *scalars.get(*offset)?;
            *offset += 1;
            Some(Value::Float(value))
        }
        ShapeId::Vec2 => {
            let mut arr = [0.0; 2];
            for slot in arr.iter_mut() {
                *slot = *scalars.get(*offset)?;
                *offset += 1;
            }
            Some(Value::Vec2(arr))
        }
        ShapeId::Vec3 => {
            let mut arr = [0.0; 3];
            for slot in arr.iter_mut() {
                *slot = *scalars.get(*offset)?;
                *offset += 1;
            }
            Some(Value::Vec3(arr))
        }
        ShapeId::Vec4 => {
            let mut arr = [0.0; 4];
            for slot in arr.iter_mut() {
                *slot = *scalars.get(*offset)?;
                *offset += 1;
            }
            Some(Value::Vec4(arr))
        }
        ShapeId::Quat => {
            let mut arr = [0.0; 4];
            for slot in arr.iter_mut() {
                *slot = *scalars.get(*offset)?;
                *offset += 1;
            }
            Some(Value::Quat(arr))
        }
        ShapeId::ColorRgba => {
            let mut arr = [0.0; 4];
            for slot in arr.iter_mut() {
                *slot = *scalars.get(*offset)?;
                *offset += 1;
            }
            Some(Value::ColorRgba(arr))
        }
        ShapeId::Transform => {
            let mut pos = [0.0; 3];
            let mut rot = [0.0; 4];
            let mut scale = [0.0; 3];
            for slot in pos.iter_mut() {
                *slot = *scalars.get(*offset)?;
                *offset += 1;
            }
            for slot in rot.iter_mut() {
                *slot = *scalars.get(*offset)?;
                *offset += 1;
            }
            for slot in scale.iter_mut() {
                *slot = *scalars.get(*offset)?;
                *offset += 1;
            }
            Some(Value::Transform { pos, rot, scale })
        }
        ShapeId::Vector {
            len: Some(expected),
        } => {
            let mut vec = Vec::with_capacity(*expected);
            for _ in 0..*expected {
                vec.push(*scalars.get(*offset)?);
                *offset += 1;
            }
            Some(Value::Vector(vec))
        }
        ShapeId::Record(fields) => {
            let mut map = HashMap::with_capacity(fields.len());
            for field in fields {
                let entry = build_numeric_value(&field.shape, scalars, offset)?;
                map.insert(field.name.clone(), entry);
            }
            Some(Value::Record(map))
        }
        ShapeId::Array(inner, len) => {
            let mut items = Vec::with_capacity(*len);
            for _ in 0..*len {
                items.push(build_numeric_value(inner, scalars, offset)?);
            }
            Some(Value::Array(items))
        }
        ShapeId::Tuple(entries) => {
            let mut items = Vec::with_capacity(entries.len());
            for entry in entries {
                items.push(build_numeric_value(entry, scalars, offset)?);
            }
            Some(Value::Tuple(items))
        }
        ShapeId::List(_)
        | ShapeId::Vector { len: None }
        | ShapeId::Enum(_)
        | ShapeId::Bool
        | ShapeId::Text => None,
    }
}

fn transform_field_value(
    field: &str,
    pos: &[f32; 3],
    rot: &[f32; 4],
    scale: &[f32; 3],
) -> Option<Value> {
    match field {
        "pos" | "position" => Some(Value::Vec3(*pos)),
        "rot" | "rotation" => Some(Value::Quat(*rot)),
        "scale" => Some(Value::Vec3(*scale)),
        _ => None,
    }
}

fn transform_field_shape(field: &str) -> Option<ShapeId> {
    match field {
        "pos" | "position" => Some(ShapeId::Vec3),
        "rot" | "rotation" => Some(ShapeId::Quat),
        "scale" => Some(ShapeId::Vec3),
        _ => None,
    }
}
