//! Helpers for flattening structured values into contiguous numeric buffers.

use hashbrown::HashMap;
use vizij_api_core::{Shape, Value};

use super::shape_helpers::infer_shape;

/// Evaluated output captured alongside its inferred shape.
#[derive(Clone, Debug)]
pub struct PortValue {
    pub value: Value,
    pub shape: Shape,
}

impl PortValue {
    /// Construct a `PortValue`, inferring the [`Shape`] from the [`Value`].
    pub fn new(value: Value) -> Self {
        let shape = infer_shape(&value);
        PortValue { value, shape }
    }

    /// Construct a `PortValue` with an explicit [`Shape`], bypassing inference.
    pub fn with_shape(value: Value, shape: Shape) -> Self {
        PortValue { value, shape }
    }

    /// Overwrite the cached [`Shape`] while leaving the [`Value`] untouched.
    pub fn set_shape(&mut self, shape: Shape) {
        self.shape = shape;
    }
}

impl Default for PortValue {
    fn default() -> Self {
        PortValue::new(Value::Float(0.0))
    }
}

/// Describes how a [`Value`] is laid out when flattened.
#[derive(Clone, Debug, PartialEq)]
pub enum ValueLayout {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    ColorRgba,
    Transform,
    Vector(usize),
    Record(Vec<(String, ValueLayout)>),
    Array(Vec<ValueLayout>),
    List(Vec<ValueLayout>),
    Tuple(Vec<ValueLayout>),
}

/// Numeric data flattened into row-major storage with an associated layout description.
#[derive(Clone, Debug)]
pub struct FlatValue {
    pub layout: ValueLayout,
    pub data: Vec<f32>,
}

impl ValueLayout {
    /// Number of scalar slots required to store this layout.
    pub fn scalar_len(&self) -> usize {
        match self {
            ValueLayout::Scalar => 1,
            ValueLayout::Vec2 => 2,
            ValueLayout::Vec3 => 3,
            ValueLayout::Vec4 => 4,
            ValueLayout::Quat => 4,
            ValueLayout::ColorRgba => 4,
            ValueLayout::Transform => 10,
            ValueLayout::Vector(len) => *len,
            ValueLayout::Record(fields) => {
                fields.iter().map(|(_, layout)| layout.scalar_len()).sum()
            }
            ValueLayout::Array(items) | ValueLayout::List(items) | ValueLayout::Tuple(items) => {
                items.iter().map(|layout| layout.scalar_len()).sum()
            }
        }
    }

    fn is_scalar(&self) -> bool {
        matches!(self, ValueLayout::Scalar)
    }

    /// Reconstruct a structured [`Value`] from flattened scalar data.
    pub fn reconstruct(&self, data: &[f32]) -> Value {
        match self {
            ValueLayout::Scalar => Value::Float(data.first().copied().unwrap_or(f32::NAN)),
            ValueLayout::Vec2 => {
                let mut arr = [0.0; 2];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::Vec2(arr)
            }
            ValueLayout::Vec3 => {
                let mut arr = [0.0; 3];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::Vec3(arr)
            }
            ValueLayout::Vec4 => {
                let mut arr = [0.0; 4];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::Vec4(arr)
            }
            ValueLayout::Quat => {
                let mut arr = [0.0; 4];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::Quat(arr)
            }
            ValueLayout::ColorRgba => {
                let mut arr = [0.0; 4];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::ColorRgba(arr)
            }
            ValueLayout::Transform => {
                let mut pos = [0.0; 3];
                let mut rot = [0.0; 4];
                let mut scale = [0.0; 3];
                for (i, slot) in pos.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                for (i, slot) in rot.iter_mut().enumerate() {
                    *slot = *data.get(3 + i).unwrap_or(&f32::NAN);
                }
                for (i, slot) in scale.iter_mut().enumerate() {
                    *slot = *data.get(7 + i).unwrap_or(&f32::NAN);
                }
                Value::Transform { pos, rot, scale }
            }
            ValueLayout::Vector(len) => {
                let mut out = Vec::with_capacity(*len);
                out.extend((0..*len).map(|i| *data.get(i).unwrap_or(&f32::NAN)));
                Value::Vector(out)
            }
            ValueLayout::Record(fields) => {
                let mut map = HashMap::new();
                let mut offset = 0usize;
                for (key, layout) in fields.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    map.insert(key.clone(), layout.reconstruct(slice));
                }
                Value::Record(map)
            }
            ValueLayout::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut offset = 0usize;
                for layout in items.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    out.push(layout.reconstruct(slice));
                }
                Value::Array(out)
            }
            ValueLayout::List(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut offset = 0usize;
                for layout in items.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    out.push(layout.reconstruct(slice));
                }
                Value::List(out)
            }
            ValueLayout::Tuple(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut offset = 0usize;
                for layout in items.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    out.push(layout.reconstruct(slice));
                }
                Value::Tuple(out)
            }
        }
    }

    pub fn fill_with(&self, value: f32) -> Value {
        let len = self.scalar_len();
        let data = vec![value; len];
        self.reconstruct(&data)
    }
}

/// Attempt to flatten a [`Value`] that contains only numeric content.
pub fn flatten_numeric(value: &Value) -> Option<FlatValue> {
    match value {
        Value::Float(f) => Some(FlatValue {
            layout: ValueLayout::Scalar,
            data: vec![*f],
        }),
        Value::Vec2(a) => Some(FlatValue {
            layout: ValueLayout::Vec2,
            data: a.to_vec(),
        }),
        Value::Vec3(a) => Some(FlatValue {
            layout: ValueLayout::Vec3,
            data: a.to_vec(),
        }),
        Value::Vec4(a) => Some(FlatValue {
            layout: ValueLayout::Vec4,
            data: a.to_vec(),
        }),
        Value::Quat(a) => Some(FlatValue {
            layout: ValueLayout::Quat,
            data: a.to_vec(),
        }),
        Value::ColorRgba(a) => Some(FlatValue {
            layout: ValueLayout::ColorRgba,
            data: a.to_vec(),
        }),
        Value::Transform { pos, rot, scale } => {
            let mut data = Vec::with_capacity(10);
            data.extend_from_slice(pos);
            data.extend_from_slice(rot);
            data.extend_from_slice(scale);
            Some(FlatValue {
                layout: ValueLayout::Transform,
                data,
            })
        }
        Value::Vector(vec) => Some(FlatValue {
            layout: ValueLayout::Vector(vec.len()),
            data: vec.clone(),
        }),
        Value::Record(map) => {
            let mut fields: Vec<_> = map.iter().collect();
            fields.sort_by(|a, b| a.0.cmp(b.0));
            let mut layouts = Vec::with_capacity(fields.len());
            let mut data = Vec::new();
            for (key, val) in fields {
                let flat = flatten_numeric(val)?;
                data.extend(&flat.data);
                layouts.push((key.clone(), flat.layout));
            }
            Some(FlatValue {
                layout: ValueLayout::Record(layouts),
                data,
            })
        }
        Value::Array(items) => {
            let mut layouts = Vec::with_capacity(items.len());
            let mut data = Vec::new();
            for item in items.iter() {
                let flat = flatten_numeric(item)?;
                data.extend(&flat.data);
                layouts.push(flat.layout);
            }
            Some(FlatValue {
                layout: ValueLayout::Array(layouts),
                data,
            })
        }
        Value::List(items) => {
            let mut layouts = Vec::with_capacity(items.len());
            let mut data = Vec::new();
            for item in items.iter() {
                let flat = flatten_numeric(item)?;
                data.extend(&flat.data);
                layouts.push(flat.layout);
            }
            Some(FlatValue {
                layout: ValueLayout::List(layouts),
                data,
            })
        }
        Value::Tuple(items) => {
            let mut layouts = Vec::with_capacity(items.len());
            let mut data = Vec::new();
            for item in items.iter() {
                let flat = flatten_numeric(item)?;
                data.extend(&flat.data);
                layouts.push(flat.layout);
            }
            Some(FlatValue {
                layout: ValueLayout::Tuple(layouts),
                data,
            })
        }
        _ => None,
    }
}

/// Align two flattened values for point-wise operations, broadcasting scalars when possible.
pub fn align_flattened(
    a: &FlatValue,
    b: &FlatValue,
) -> Result<(ValueLayout, Vec<f32>, Vec<f32>), ValueLayout> {
    if a.layout == b.layout {
        return Ok((a.layout.clone(), a.data.clone(), b.data.clone()));
    }
    if a.layout.is_scalar() {
        let layout = b.layout.clone();
        let len = layout.scalar_len();
        let repeated = vec![a.data.first().copied().unwrap_or(f32::NAN); len];
        return Ok((layout, repeated, b.data.clone()));
    }
    if b.layout.is_scalar() {
        let layout = a.layout.clone();
        let len = layout.scalar_len();
        let repeated = vec![b.data.first().copied().unwrap_or(f32::NAN); len];
        return Ok((layout, a.data.clone(), repeated));
    }

    let len_a = a.layout.scalar_len();
    let len_b = b.layout.scalar_len();
    if len_a >= len_b {
        Err(a.layout.clone())
    } else {
        Err(b.layout.clone())
    }
}
