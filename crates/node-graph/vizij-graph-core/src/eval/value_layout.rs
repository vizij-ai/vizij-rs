//! Helpers for flattening structured values into contiguous numeric buffers.
//!
//! This is the kernel seam for numeric node math: [`flatten_numeric`] decodes
//! a [`Value`] once (through the vocabulary accessors) into a [`FlatValue`] —
//! a plain `Vec<f32>` plus a [`ValueLayout`] — operators compute on the flat
//! data, and [`ValueLayout::reconstruct`] re-encodes the result through the
//! vocabulary constructors.

use vizij_api_core::value as vocab;
use vizij_api_core::value::VizijKind;
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
        PortValue::new(vocab::float(0.0))
    }
}

/// Describes how a [`Value`] is laid out when flattened.
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

    /// Reconstruct a structured [`Value`] from flattened scalar data.
    pub fn reconstruct(&self, data: &[f32]) -> Value {
        match self {
            ValueLayout::Scalar => vocab::float(data.first().copied().unwrap_or(f32::NAN)),
            ValueLayout::Vec2 => vocab::vec2(read_array(data, 0)),
            ValueLayout::Vec3 => vocab::vec3(read_array(data, 0)),
            ValueLayout::Vec4 => vocab::vec4(read_array(data, 0)),
            ValueLayout::Quat => vocab::quat(read_array(data, 0)),
            ValueLayout::ColorRgba => vocab::color_rgba(read_array(data, 0)),
            ValueLayout::Transform => vocab::transform(vocab::Transform {
                translation: read_array(data, 0),
                rotation: read_array(data, 3),
                scale: read_array(data, 7),
            }),
            ValueLayout::Vector(len) => {
                let mut out = Vec::with_capacity(*len);
                out.extend((0..*len).map(|i| *data.get(i).unwrap_or(&f32::NAN)));
                vocab::vector(out)
            }
            ValueLayout::Record(fields) => {
                let mut offset = 0usize;
                vocab::record(fields.iter().map(|(key, layout)| {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    (key.as_str(), layout.reconstruct(slice))
                }))
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
                vocab::array(out)
            }
        }
    }

    pub fn fill_with(&self, value: f32) -> Value {
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

/// Attempt to flatten a [`Value`] that contains only numeric content.
pub fn flatten_numeric(value: &Value) -> Option<FlatValue> {
    match vocab::kind(value) {
        VizijKind::Float => Some(FlatValue {
            layout: ValueLayout::Scalar,
            data: vec![vocab::as_float(value)?],
        }),
        VizijKind::Vec2 => Some(FlatValue {
            layout: ValueLayout::Vec2,
            data: vocab::as_vec2(value)?.to_vec(),
        }),
        VizijKind::Vec3 => Some(FlatValue {
            layout: ValueLayout::Vec3,
            data: vocab::as_vec3(value)?.to_vec(),
        }),
        VizijKind::Vec4 => Some(FlatValue {
            layout: ValueLayout::Vec4,
            data: vocab::as_vec4(value)?.to_vec(),
        }),
        VizijKind::Quat => Some(FlatValue {
            layout: ValueLayout::Quat,
            data: vocab::as_quat(value)?.to_vec(),
        }),
        VizijKind::ColorRgba => Some(FlatValue {
            layout: ValueLayout::ColorRgba,
            data: vocab::as_color_rgba(value)?.to_vec(),
        }),
        VizijKind::Transform => {
            let t = vocab::as_transform(value)?;
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
            layout: ValueLayout::Vector(vocab::as_vector(value)?.len()),
            data: vocab::as_vector(value)?.to_vec(),
        }),
        VizijKind::Record => {
            // `as_record` yields entries sorted by name, keeping the flat
            // ordering deterministic.
            let entries = vocab::as_record(value)?;
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
            let items = vocab::as_array(value)?;
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
