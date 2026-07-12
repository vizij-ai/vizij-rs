//! Helpers for flattening structured values into contiguous numeric buffers.
//!
//! This is the kernel seam for numeric node math: [`flatten_numeric`] decodes
//! a value once (through the [`GraphValue`] accessors) into a [`FlatValue`] —
//! a plain `Vec<f32>` plus a [`ValueLayout`] — operators compute on the flat
//! data, and [`ValueLayout::reconstruct`] re-encodes the result through the
//! [`GraphValue`] constructors.

use vizij_api_core::{Shape, Value};

use crate::graph_value::{GraphValue, Transform, VizijKind};

use super::shape_helpers::infer_shape;

/// Evaluated output captured alongside its inferred shape.
#[derive(Clone, Debug)]
pub struct PortValue<V: GraphValue = Value> {
    pub value: V,
    pub shape: Shape,
}

impl<V: GraphValue> PortValue<V> {
    /// Construct a `PortValue`, inferring the [`Shape`] from the value.
    pub fn new(value: V) -> Self {
        let shape = infer_shape(&value);
        PortValue { value, shape }
    }

    /// Construct a `PortValue` with an explicit [`Shape`], bypassing inference.
    pub fn with_shape(value: V, shape: Shape) -> Self {
        PortValue { value, shape }
    }

    /// Overwrite the cached [`Shape`] while leaving the value untouched.
    pub fn set_shape(&mut self, shape: Shape) {
        self.shape = shape;
    }
}

impl<V: GraphValue> Default for PortValue<V> {
    fn default() -> Self {
        PortValue::new(V::float(0.0))
    }
}

/// Describes how a value is laid out when flattened.
///
/// Sequences flatten to a single `Array` layout: the wire value is one
/// sequence kind (`ArrayValue`), so any declared array/list/tuple distinction
/// lives in the path's [`Shape`], not here.
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
            ValueLayout::Array(items) => items.iter().map(|layout| layout.scalar_len()).sum(),
        }
    }

    fn is_scalar(&self) -> bool {
        matches!(self, ValueLayout::Scalar)
    }

    /// Reconstruct a structured value from flattened scalar data.
    pub fn reconstruct<V: GraphValue>(&self, data: &[f32]) -> V {
        match self {
            ValueLayout::Scalar => V::float(data.first().copied().unwrap_or(f32::NAN)),
            ValueLayout::Vec2 => V::vec2(read_array(data, 0)),
            ValueLayout::Vec3 => V::vec3(read_array(data, 0)),
            ValueLayout::Vec4 => V::vec4(read_array(data, 0)),
            ValueLayout::Quat => V::quat(read_array(data, 0)),
            ValueLayout::ColorRgba => V::color_rgba(read_array(data, 0)),
            ValueLayout::Transform => V::transform(Transform {
                translation: read_array(data, 0),
                rotation: read_array(data, 3),
                scale: read_array(data, 7),
            }),
            ValueLayout::Vector(len) => {
                let mut out = Vec::with_capacity(*len);
                out.extend((0..*len).map(|i| *data.get(i).unwrap_or(&f32::NAN)));
                V::vector(out)
            }
            ValueLayout::Record(fields) => {
                let mut offset = 0usize;
                V::record(fields.iter().map(|(key, layout)| {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    (key.as_str(), layout.reconstruct::<V>(slice))
                }))
            }
            ValueLayout::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut offset = 0usize;
                for layout in items.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    out.push(layout.reconstruct::<V>(slice));
                }
                V::array(out)
            }
        }
    }

    pub fn fill_with<V: GraphValue>(&self, value: f32) -> V {
        let len = self.scalar_len();
        let data = vec![value; len];
        self.reconstruct(&data)
    }
}

fn read_array<const N: usize>(data: &[f32], offset: usize) -> [f32; N] {
    let mut arr = [f32::NAN; N];
    for (i, slot) in arr.iter_mut().enumerate() {
        if let Some(v) = data.get(offset + i) {
            *slot = *v;
        }
    }
    arr
}

/// Attempt to flatten a value that contains only numeric content.
pub fn flatten_numeric<V: GraphValue>(value: &V) -> Option<FlatValue> {
    match value.kind() {
        VizijKind::Float => Some(FlatValue {
            layout: ValueLayout::Scalar,
            data: vec![value.as_float()?],
        }),
        VizijKind::Vec2 => Some(FlatValue {
            layout: ValueLayout::Vec2,
            data: value.as_vec2()?.to_vec(),
        }),
        VizijKind::Vec3 => Some(FlatValue {
            layout: ValueLayout::Vec3,
            data: value.as_vec3()?.to_vec(),
        }),
        VizijKind::Vec4 => Some(FlatValue {
            layout: ValueLayout::Vec4,
            data: value.as_vec4()?.to_vec(),
        }),
        VizijKind::Quat => Some(FlatValue {
            layout: ValueLayout::Quat,
            data: value.as_quat()?.to_vec(),
        }),
        VizijKind::ColorRgba => Some(FlatValue {
            layout: ValueLayout::ColorRgba,
            data: value.as_color_rgba()?.to_vec(),
        }),
        VizijKind::Transform => {
            let t = value.as_transform()?;
            let mut data = Vec::with_capacity(10);
            data.extend_from_slice(&t.translation);
            data.extend_from_slice(&t.rotation);
            data.extend_from_slice(&t.scale);
            Some(FlatValue {
                layout: ValueLayout::Transform,
                data,
            })
        }
        VizijKind::Vector => Some(FlatValue {
            layout: ValueLayout::Vector(value.as_vector()?.len()),
            data: value.as_vector()?.to_vec(),
        }),
        VizijKind::Record => {
            // `as_record` yields entries sorted by name, keeping the flat
            // ordering deterministic.
            let entries = value.as_record()?;
            let mut layouts = Vec::with_capacity(entries.len());
            let mut data = Vec::new();
            for (key, val) in entries {
                let flat = flatten_numeric(val)?;
                data.extend(&flat.data);
                layouts.push((key.to_string(), flat.layout));
            }
            Some(FlatValue {
                layout: ValueLayout::Record(layouts),
                data,
            })
        }
        VizijKind::Array => {
            let items = value.as_array()?;
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
