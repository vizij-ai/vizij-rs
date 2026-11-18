//! Reads mesh data out of a glTF document whose buffer is a GLB's binary
//! chunk, and locates index data so it can be rewritten in place.
//!
//! Only what the import passes need is supported: float `VEC3` attributes
//! (dense or sparse) and unsigned-integer indices stored densely in the
//! binary chunk. Anything else is reported as [`Unsupported`] so a pass can
//! leave that mesh alone instead of failing the whole file.

use serde_json::Value;

const FLOAT: u64 = 5126;
const UNSIGNED_BYTE: u64 = 5121;
const UNSIGNED_SHORT: u64 = 5123;
const UNSIGNED_INT: u64 = 5125;

/// Why a piece of mesh data cannot be read or written by these passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Unsupported {
    /// The data lives in an external or embedded-URI buffer rather than the
    /// GLB's binary chunk.
    ExternalBuffer,
    /// Positions or morph deltas are not 32-bit floats (quantized meshes).
    NotFloat,
    /// Indices are stored sparsely, so they have no dense range to rewrite.
    SparseIndices,
    /// The document does not describe the data consistently.
    Malformed(String),
}

fn malformed(what: impl Into<String>) -> Unsupported {
    Unsupported::Malformed(what.into())
}

/// The glTF JSON document and the GLB binary chunk it addresses as buffer 0.
pub(crate) struct Document<'a> {
    pub json: &'a Value,
    pub bin: &'a [u8],
}

/// Where a bufferView's bytes sit in the binary chunk.
struct View {
    offset: usize,
    length: usize,
    stride: Option<usize>,
}

impl<'a> Document<'a> {
    fn get(&self, collection: &str, index: u64) -> Result<&'a Value, Unsupported> {
        self.json
            .get(collection)
            .and_then(|c| c.get(index as usize))
            .ok_or_else(|| malformed(format!("{collection}[{index}] is missing")))
    }

    fn view(&self, index: u64) -> Result<View, Unsupported> {
        let view = self.get("bufferViews", index)?;
        let buffer = view.get("buffer").and_then(Value::as_u64).unwrap_or(0);
        // Only a uri-less buffer refers to the GLB binary chunk.
        if self.get("buffers", buffer)?.get("uri").is_some() {
            return Err(Unsupported::ExternalBuffer);
        }
        let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let length = view
            .get("byteLength")
            .and_then(Value::as_u64)
            .ok_or_else(|| malformed(format!("bufferViews[{index}] has no byteLength")))?
            as usize;
        if offset + length > self.bin.len() {
            return Err(malformed(format!(
                "bufferViews[{index}] runs past the binary chunk"
            )));
        }
        let stride = view
            .get("byteStride")
            .and_then(Value::as_u64)
            .map(|s| s as usize);
        Ok(View {
            offset,
            length,
            stride,
        })
    }

    /// The number of elements an accessor holds.
    pub fn count(&self, accessor: u64) -> Result<usize, Unsupported> {
        self.get("accessors", accessor)?
            .get("count")
            .and_then(Value::as_u64)
            .map(|c| c as usize)
            .ok_or_else(|| malformed(format!("accessors[{accessor}] has no count")))
    }

    /// Reads a float `VEC3` accessor, applying its sparse substitution when
    /// it has one. An accessor without a bufferView starts from zeros, as
    /// glTF specifies; exporters use that for sparse morph deltas.
    pub fn read_vec3(&self, accessor: u64) -> Result<Vec<[f32; 3]>, Unsupported> {
        let acc = self.get("accessors", accessor)?;
        if acc.get("componentType").and_then(Value::as_u64) != Some(FLOAT) {
            return Err(Unsupported::NotFloat);
        }
        if acc.get("type").and_then(Value::as_str) != Some("VEC3") {
            return Err(malformed(format!("accessors[{accessor}] is not VEC3")));
        }
        let count = self.count(accessor)?;
        let mut out = vec![[0.0f32; 3]; count];
        if let Some(view) = acc.get("bufferView").and_then(Value::as_u64) {
            let view = self.view(view)?;
            let base =
                view.offset + acc.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
            let stride = view.stride.unwrap_or(12);
            for (i, slot) in out.iter_mut().enumerate() {
                *slot = self.vec3_at(base + i * stride, &view, accessor)?;
            }
        }
        if let Some(sparse) = acc.get("sparse") {
            let n =
                sparse.get("count").and_then(Value::as_u64).ok_or_else(|| {
                    malformed(format!("accessors[{accessor}].sparse has no count"))
                })? as usize;
            let indices = sparse
                .get("indices")
                .ok_or_else(|| malformed(format!("accessors[{accessor}].sparse has no indices")))?;
            let targets = self.read_uints(indices, n)?;
            let values = sparse
                .get("values")
                .ok_or_else(|| malformed(format!("accessors[{accessor}].sparse has no values")))?;
            let view = self.view(
                values
                    .get("bufferView")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| malformed("sparse values have no bufferView"))?,
            )?;
            let base = view.offset
                + values
                    .get("byteOffset")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;
            for (i, target) in targets.into_iter().enumerate() {
                let slot = out.get_mut(target as usize).ok_or_else(|| {
                    malformed(format!("accessors[{accessor}] sparse index out of range"))
                })?;
                *slot = self.vec3_at(base + i * 12, &view, accessor)?;
            }
        }
        Ok(out)
    }

    fn vec3_at(&self, at: usize, view: &View, accessor: u64) -> Result<[f32; 3], Unsupported> {
        if at + 12 > view.offset + view.length {
            return Err(malformed(format!(
                "accessors[{accessor}] runs past its bufferView"
            )));
        }
        let f = |k: usize| {
            f32::from_le_bytes(
                self.bin[at + 4 * k..at + 4 * k + 4]
                    .try_into()
                    .expect("4 bytes"),
            )
        };
        Ok([f(0), f(1), f(2)])
    }

    /// Reads `count` unsigned integers from a `{bufferView, byteOffset,
    /// componentType}` object: a sparse accessor's `indices`.
    fn read_uints(&self, spec: &Value, count: usize) -> Result<Vec<u32>, Unsupported> {
        let size = uint_size(spec.get("componentType").and_then(Value::as_u64))?;
        let view = self.view(
            spec.get("bufferView")
                .and_then(Value::as_u64)
                .ok_or_else(|| malformed("indices have no bufferView"))?,
        )?;
        let base =
            view.offset + spec.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        if base + count * size > view.offset + view.length {
            return Err(malformed("indices run past their bufferView"));
        }
        Ok((0..count)
            .map(|i| read_uint(&self.bin[base + i * size..], size))
            .collect())
    }

    /// Reads an index accessor and returns its values with the byte range
    /// they occupy in the binary chunk, which [`write_index`] rewrites.
    pub fn read_indices(&self, accessor: u64) -> Result<(Vec<u32>, IndexSlots), Unsupported> {
        let acc = self.get("accessors", accessor)?;
        if acc.get("sparse").is_some() {
            return Err(Unsupported::SparseIndices);
        }
        let size = uint_size(acc.get("componentType").and_then(Value::as_u64))?;
        let count = self.count(accessor)?;
        let view = acc
            .get("bufferView")
            .and_then(Value::as_u64)
            .ok_or_else(|| malformed(format!("accessors[{accessor}] has no bufferView")))?;
        let view = self.view(view)?;
        let offset =
            view.offset + acc.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        if offset + count * size > view.offset + view.length {
            return Err(malformed(format!(
                "accessors[{accessor}] runs past its bufferView"
            )));
        }
        let values = (0..count)
            .map(|i| read_uint(&self.bin[offset + i * size..], size))
            .collect();
        Ok((values, IndexSlots { offset, size }))
    }
}

/// The dense byte range of an index accessor in the binary chunk.
#[derive(Debug, Clone, Copy)]
pub(crate) struct IndexSlots {
    offset: usize,
    size: usize,
}

/// Writes index `value` into slot `i` of `slots`. The value must fit the
/// accessor's component type; a pass only writes indices it read from the
/// same accessor, so it always does.
pub(crate) fn write_index(bin: &mut [u8], slots: IndexSlots, i: usize, value: u32) {
    let at = slots.offset + i * slots.size;
    match slots.size {
        1 => bin[at] = value as u8,
        2 => bin[at..at + 2].copy_from_slice(&(value as u16).to_le_bytes()),
        _ => bin[at..at + 4].copy_from_slice(&value.to_le_bytes()),
    }
}

fn uint_size(component_type: Option<u64>) -> Result<usize, Unsupported> {
    match component_type {
        Some(UNSIGNED_BYTE) => Ok(1),
        Some(UNSIGNED_SHORT) => Ok(2),
        Some(UNSIGNED_INT) => Ok(4),
        other => Err(malformed(format!(
            "indices have component type {other:?}, not an unsigned integer"
        ))),
    }
}

fn read_uint(bytes: &[u8], size: usize) -> u32 {
    match size {
        1 => bytes[0] as u32,
        2 => u16::from_le_bytes([bytes[0], bytes[1]]) as u32,
        _ => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
    }
}
