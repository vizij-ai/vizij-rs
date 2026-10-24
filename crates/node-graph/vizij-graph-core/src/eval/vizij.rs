//! What belongs to Vizij's graph format: an `input` node fitting what it
//! reads to its declared shape, the check of a node's declared output
//! shapes, and the writes of a path-less `output` node's keyed batch. The
//! functions of [`crate::vizij_functions`] run the first two. A node's port
//! is reported with its shape ([`PortValue`]).

use uuid::Uuid;
use vizij_api_core::{Shape, TypedPath, Value, WriteOp};

use super::shape_helpers::{
    coerce_numeric_to_shape, infer_shape, is_numeric_like, null_of_shape_numeric,
    value_matches_shape,
};

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
        PortValue::new(Value::F32(0.0))
    }
}

/// The value an `input` node gives: what its store key holds (`staged`),
/// else its `default`, fitted to its `declared` shape when it has one.
///
/// Without a staged value or a default, a numeric declared shape gives its
/// `NaN`-filled value, and anything else is an error.
pub(crate) fn input(
    id: &str,
    path: Option<&TypedPath>,
    declared: Option<&Shape>,
    staged: Option<Value>,
    default: Option<&Value>,
) -> Result<Value, String> {
    let path =
        path.ok_or_else(|| format!("Input node '{id}' missing required 'path' parameter"))?;
    let Some(declared) = declared else {
        return staged.or_else(|| default.cloned()).ok_or_else(|| {
            format!("Input node '{id}' missing staged value for '{path}' and no default provided")
        });
    };
    if let Some(staged) = staged {
        return fit(id, path, "staged input", declared, staged);
    }
    if let Some(default) = default {
        return fit(id, path, "default value", declared, default.clone());
    }
    if is_numeric_like(&declared.id) {
        return Ok(null_of_shape_numeric(&declared.id));
    }
    Err(format!(
        "Input node '{id}' missing staged value for '{path}' and declared non-numeric shape {:?}",
        declared.id
    ))
}

/// `value`, which an `input` node read from `source`, fitted to `declared`:
/// as it is when it matches, its numbers in that shape when it is numeric,
/// else the shape's `NaN`-filled value.
fn fit(
    id: &str,
    path: &TypedPath,
    source: &str,
    declared: &Shape,
    value: Value,
) -> Result<Value, String> {
    if value_matches_shape(&declared.id, &value) {
        return Ok(value);
    }
    if !is_numeric_like(&declared.id) {
        return Err(format!(
            "Input node '{id}' cannot coerce {source} for '{path}' into declared shape {:?}",
            declared.id
        ));
    }
    Ok(coerce_numeric_to_shape(&declared.id, &value)
        .unwrap_or_else(|| null_of_shape_numeric(&declared.id)))
}

/// Check that `value`, node `id`'s port `port`, has the shape `shape` it
/// declares.
pub(crate) fn check_shape(
    id: &str,
    port: &str,
    shape: &Shape,
    value: &Value,
) -> Result<(), String> {
    if value_matches_shape(&shape.id, value) {
        Ok(())
    } else {
        Err(format!(
            "node '{id}' output '{port}' does not match declared shape {:?}",
            shape.id
        ))
    }
}

/// The writes a keyed record batch makes, in batch order: how a
/// **path-less** [`NodeType::Output`](crate::types::NodeType::Output) applies
/// a value that names its own keys (e.g. a module call's "what changed") onto
/// them.
///
/// The batch is a `Value::ArrayStructure`; in every record, `key_field` holds
/// the target path as a string and `value_field` the value to write. A record
/// missing either (or keying on a non-string) is an **evaluation error** — a
/// miswired batch, not data — while an empty batch simply writes nothing.
/// Records repeating a key produce writes in batch order, and the per-tick
/// flush keeps the last one; explicit combination of concurrent publishers is
/// VIZ-76's ground (https://linear.app/semio-ai/issue/VIZ-76).
pub(crate) fn keyed_batch_writes(
    batch: &Value,
    key_field: Uuid,
    value_field: Uuid,
) -> Result<Vec<WriteOp>, String> {
    let Value::ArrayStructure { elements, .. } = batch else {
        return Err(
            "a path-less Output applies a keyed record batch (an array of structures); \
             set `path` to write a single value"
                .to_string(),
        );
    };
    let mut writes = Vec::with_capacity(elements.len());
    for (index, record) in elements.iter().enumerate() {
        let mut key: Option<&str> = None;
        let mut value: Option<&Value> = None;
        for field in &record.fields {
            if field.id == key_field {
                key = match field.value.as_ref() {
                    Value::String(s) => Some(s.as_str()),
                    other => {
                        return Err(format!(
                            "record {index}: key field must be a string path, got {other:?}"
                        ));
                    }
                };
            } else if field.id == value_field {
                value = Some(field.value.as_ref());
            }
        }
        let (Some(key), Some(value)) = (key, value) else {
            return Err(format!(
                "record {index}: missing key field {key_field} or value field {value_field}"
            ));
        };
        let path = TypedPath::parse(key)
            .map_err(|e| format!("record {index}: key {key:?} is not a path: {e}"))?;
        writes.push(WriteOp::new(path, value.clone()));
    }
    Ok(writes)
}
