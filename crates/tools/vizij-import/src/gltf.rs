//! Reads mesh data out of a glTF document whose buffer is a GLB's binary
//! chunk, and locates index data so it can be rewritten in place.
//!
//! Only what the import passes need is supported: float `VEC3` attributes
//! (dense or sparse) and unsigned-integer indices stored densely in the
//! binary chunk. Anything else is reported as [`Unsupported`] so a pass can
//! leave that mesh alone instead of failing the whole file.

use std::ops::Range;

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
        // In a GLB only buffer 0, without a uri, is the binary chunk. Any
        // other buffer — one with a uri, or a uri-less one such as an
        // extension's fallback buffer — holds bytes this pass cannot see.
        if buffer != 0 || self.get("buffers", buffer)?.get("uri").is_some() {
            return Err(Unsupported::ExternalBuffer);
        }
        let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let length = view
            .get("byteLength")
            .and_then(Value::as_u64)
            .ok_or_else(|| malformed(format!("bufferViews[{index}] has no byteLength")))?
            as usize;
        if offset
            .checked_add(length)
            .is_none_or(|end| end > self.bin.len())
        {
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
    ///
    /// `expected` is the element count the caller requires (a morph
    /// target's count is its mesh's vertex count). The count is checked
    /// against it, or against the bytes the accessor's bufferView holds,
    /// before anything is allocated, so a corrupt count is reported rather
    /// than exhausting memory.
    pub fn read_vec3(
        &self,
        accessor: u64,
        expected: Option<usize>,
    ) -> Result<Vec<[f32; 3]>, Unsupported> {
        let acc = self.get("accessors", accessor)?;
        if acc.get("componentType").and_then(Value::as_u64) != Some(FLOAT) {
            return Err(Unsupported::NotFloat);
        }
        if acc.get("type").and_then(Value::as_str) != Some("VEC3") {
            return Err(malformed(format!("accessors[{accessor}] is not VEC3")));
        }
        let count = self.count(accessor)?;
        if expected.is_some_and(|e| e != count) {
            return Err(malformed(format!(
                "accessors[{accessor}] has {count} elements, not the mesh's {}",
                expected.unwrap_or_default()
            )));
        }
        let dense = match acc.get("bufferView").and_then(Value::as_u64) {
            Some(view) => {
                let view = self.view(view)?;
                let base = byte_offset(acc, &view)?;
                let stride = view.stride.unwrap_or(12);
                if stride < 12 {
                    return Err(malformed(format!(
                        "accessors[{accessor}] has a byteStride shorter than its elements"
                    )));
                }
                fits(base, count, stride, 12, &view)
                    .then_some((view, base, stride))
                    .ok_or_else(|| {
                        malformed(format!("accessors[{accessor}] runs past its bufferView"))
                    })
                    .map(Some)?
            }
            None if expected.is_some() => None,
            None => {
                return Err(malformed(format!(
                    "accessors[{accessor}] has no bufferView to bound its count"
                )))
            }
        };
        let mut out = vec![[0.0f32; 3]; count];
        if let Some((view, base, stride)) = dense {
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
            let base = byte_offset(values, &view)?;
            if !fits(base, targets.len(), 12, 12, &view) {
                return Err(malformed(format!(
                    "accessors[{accessor}] sparse values run past their bufferView"
                )));
            }
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
        let base = byte_offset(spec, &view)?;
        if !fits(base, count, size, size, &view) {
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
        let offset = byte_offset(acc, &view)?;
        if !fits(offset, count, size, size, &view) {
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

impl IndexSlots {
    /// The bytes `count` indices occupy.
    pub fn range(&self, count: usize) -> Range<usize> {
        self.offset..self.offset + count * self.size
    }
}

impl Document<'_> {
    /// The binary-chunk byte ranges the document reads, each tagged with the
    /// accessor reading it (`None` for a bufferView something other than an
    /// accessor references directly, such as an image, read whole). An
    /// accessor's range includes its sparse indices and values; one whose
    /// exact range cannot be worked out is given its whole bufferView, so a
    /// byte it might read is never taken for unused.
    pub fn reader_ranges(&self) -> Vec<(Option<u64>, Range<usize>)> {
        let mut ranges: Vec<(Option<u64>, Range<usize>)> = Vec::new();
        let mut direct = Vec::new();
        for (key, value) in self.json.as_object().into_iter().flatten() {
            if key != "accessors" && key != "bufferViews" {
                collect_view_refs(value, &mut direct);
            }
        }
        for view in direct {
            if let Ok(view) = self.view(view) {
                ranges.push((None, view.offset..view.offset + view.length));
            }
        }
        let accessors = self.json.get("accessors").and_then(Value::as_array);
        for (a, acc) in accessors.into_iter().flatten().enumerate() {
            let a = a as u64;
            let count = acc.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
            let element = element_size(acc);
            let mut push = |spec: &Value, count: usize, element: Option<usize>| {
                let Some(view) = spec
                    .get("bufferView")
                    .and_then(Value::as_u64)
                    .and_then(|v| self.view(v).ok())
                else {
                    return;
                };
                let whole = view.offset..view.offset + view.length;
                let exact = (|| {
                    let element = element?;
                    let start = byte_offset(spec, &view).ok()?;
                    let stride = view.stride.unwrap_or(element);
                    if count == 0 {
                        return Some(start..start);
                    }
                    let end = (count - 1)
                        .checked_mul(stride)?
                        .checked_add(element)?
                        .checked_add(start)?;
                    Some(start..end)
                })();
                ranges.push((Some(a), exact.unwrap_or(whole)));
            };
            push(acc, count, element);
            if let Some(sparse) = acc.get("sparse") {
                let n = sparse.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
                if let Some(indices) = sparse.get("indices") {
                    let size = uint_size(indices.get("componentType").and_then(Value::as_u64));
                    push(indices, n, size.ok());
                }
                if let Some(values) = sparse.get("values") {
                    push(values, n, element);
                }
            }
        }
        ranges
    }
}

/// Every `bufferView` index referenced anywhere in `value`.
fn collect_view_refs(value: &Value, out: &mut Vec<u64>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                match (key.as_str(), inner.as_u64()) {
                    ("bufferView", Some(view)) => out.push(view),
                    _ => collect_view_refs(inner, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_view_refs(v, out)),
        _ => {}
    }
}

/// An accessor's (or sparse part's) `byteOffset` added to its bufferView's
/// start, within the view.
fn byte_offset(spec: &Value, view: &View) -> Result<usize, Unsupported> {
    let offset = spec.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    view.offset
        .checked_add(offset)
        .filter(|&at| at <= view.offset + view.length)
        .ok_or_else(|| malformed("a byteOffset lies past its bufferView"))
}

/// Whether `count` elements of `element` bytes, `stride` apart from
/// `start`, end within `view`. Overflowing arithmetic means they do not.
fn fits(start: usize, count: usize, stride: usize, element: usize, view: &View) -> bool {
    if count == 0 {
        return true;
    }
    (count - 1)
        .checked_mul(stride)
        .and_then(|span| span.checked_add(element))
        .and_then(|span| span.checked_add(start))
        .is_some_and(|end| end <= view.offset + view.length)
}

/// The byte size of one element of an accessor, from its component type
/// and element type; `None` for a combination glTF does not define.
fn element_size(acc: &Value) -> Option<usize> {
    let component = match acc.get("componentType").and_then(Value::as_u64)? {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return None,
    };
    let width = match acc.get("type").and_then(Value::as_str)? {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" | "MAT2" => 4,
        "MAT3" => 9,
        "MAT4" => 16,
        _ => return None,
    };
    Some(component * width)
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
