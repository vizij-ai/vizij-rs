//! JSON parsing and normalization for [`Value`] payloads and graph specs.
//!
//! The canonical JSON form of a value is Arora `Value`'s own serde
//! (externally tagged: `{"f32": 1.0}`, `{"str": "hi"}`, `{"f32s": [...]}`,
//! `{"struct": {"id": ..., "fields": [...]}}`, ...). [`parse_value`] and
//! [`normalize_value_json`] additionally accept every payload form vizij
//! hosts have emitted, and produce the canonical form:
//!
//! - bare JSON primitives (`1.0`, `true`, `"hi"`) and numeric arrays
//!   (`[1, 2, 3]`, arity-mapped per [`NumericArrayPolicy`]);
//! - legacy shorthand objects (`{"vec3": [1, 2, 3]}`, `{"float": 1.0}`,
//!   `{"transform": {...}}`, `{"enum": {"tag": ..., "value": ...}}`,
//!   `{"record": {...}}`, `{"array"|"list"|"tuple": [...]}`);
//! - tagged objects (`{"type": "vec3", "data": [1, 2, 3]}`);
//! - raw component objects (`{"x": ..., "y": ...[, "z"[, "w"]]}` — two or
//!   three components read as vec2/vec3, four as a quaternion — and
//!   `{"r": ..., "g": ..., "b": ...[, "a": ...]}`, read as a color with
//!   alpha defaulting to 1);
//! - canonical Arora serde, passed through unchanged.
//!
//! This normalizer is the single entry point for migrating persisted
//! documents (e.g. Value-bearing JSON embedded in `.glb` face bundles) to
//! the canonical form. Values serialize back to JSON with plain `serde_json`;
//! there is no producer of the legacy forms.
//!
//! Graph-spec normalization (`inputs` -> `edges`, operand aliasing, shape
//! shorthand) also lives here; value payloads inside specs normalize through
//! the same rules.

use serde::de::Error as _;
use serde_json::{json, Map, Value as JsonValue};
use std::collections::HashMap;
use thiserror::Error;

use crate::value::{
    array, bool_, color_rgba, enumeration, float, quat, record, text, transform, vec2, vec3, vec4,
    vector, Transform,
};
use crate::{TypedPath, Value, WriteBatch, WriteOp};

/// Policy describing how purely numeric JSON arrays are read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericArrayPolicy {
    /// Arrays of length 2/3/4 become vec2/vec3/vec4 structures; any other
    /// numeric array becomes a generic vector (`ArrayF32`). This matches the
    /// historical behaviour used in production builds.
    AutoVectorKinds,
    /// All numeric arrays become generic vectors regardless of length. This
    /// matches the staging normalizer that avoids guessing vector arity.
    AlwaysVector,
}

/// Errors produced while normalizing graph JSON blobs.
#[derive(Debug, Error)]
pub enum JsonError {
    #[error("graph json parse error: {0}")]
    GraphParse(String),
    #[error("serialize normalized graph: {0}")]
    GraphSerialize(String),
    #[error("graph spec uses deprecated 'links' field; rename to 'edges'")]
    LegacyLinksField,
}

/// Parse any accepted JSON payload form (see the module header) into a
/// [`Value`], using the [`NumericArrayPolicy::AutoVectorKinds`] policy.
pub fn parse_value(value: JsonValue) -> Result<Value, serde_json::Error> {
    parse_value_with_policy(value, NumericArrayPolicy::AutoVectorKinds)
}

/// Variant of [`parse_value`] using [`NumericArrayPolicy::AlwaysVector`],
/// mirroring the staging graph normalizer.
pub fn parse_value_staging(value: JsonValue) -> Result<Value, serde_json::Error> {
    parse_value_with_policy(value, NumericArrayPolicy::AlwaysVector)
}

/// Normalize any accepted JSON payload form into canonical Arora `Value`
/// serde JSON. Unrecognized payloads pass through unchanged (deserializing
/// them into [`Value`] then reports the error).
pub fn normalize_value_json(value: JsonValue) -> JsonValue {
    normalize_value_json_with_policy(value, NumericArrayPolicy::AutoVectorKinds)
}

/// Variant of [`normalize_value_json`] using
/// [`NumericArrayPolicy::AlwaysVector`].
pub fn normalize_value_json_staging(value: JsonValue) -> JsonValue {
    normalize_value_json_with_policy(value, NumericArrayPolicy::AlwaysVector)
}

fn normalize_value_json_with_policy(value: JsonValue, policy: NumericArrayPolicy) -> JsonValue {
    match parse_value_with_policy(value.clone(), policy) {
        Ok(parsed) => serde_json::to_value(&parsed).unwrap_or(value),
        Err(_) => value,
    }
}

fn parse_value_with_policy(
    value: JsonValue,
    policy: NumericArrayPolicy,
) -> Result<Value, serde_json::Error> {
    match value {
        JsonValue::Number(n) => Ok(float(
            n.as_f64()
                .ok_or_else(|| serde_json::Error::custom("non-finite number"))? as f32,
        )),
        JsonValue::Bool(b) => Ok(bool_(b)),
        JsonValue::String(s) => Ok(text(&s)),
        JsonValue::Array(items) => parse_array(items, policy),
        JsonValue::Object(obj) => parse_object(obj, policy),
        JsonValue::Null => Err(serde_json::Error::custom("null is not a value")),
    }
}

fn parse_array(
    items: Vec<JsonValue>,
    policy: NumericArrayPolicy,
) -> Result<Value, serde_json::Error> {
    if items.iter().all(|x| x.is_number()) {
        let xs: Vec<f32> = items
            .iter()
            .map(|x| x.as_f64().unwrap_or(0.0) as f32)
            .collect();
        return Ok(match policy {
            NumericArrayPolicy::AutoVectorKinds => match xs.len() {
                2 => vec2([xs[0], xs[1]]),
                3 => vec3([xs[0], xs[1], xs[2]]),
                4 => vec4([xs[0], xs[1], xs[2], xs[3]]),
                _ => vector(xs),
            },
            NumericArrayPolicy::AlwaysVector => vector(xs),
        });
    }
    Ok(array(
        items
            .into_iter()
            .map(|item| parse_value_with_policy(item, policy))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn parse_object(
    obj: Map<String, JsonValue>,
    policy: NumericArrayPolicy,
) -> Result<Value, serde_json::Error> {
    // Tagged form: { "type": ..., "data": ... }.
    if obj.contains_key("type") && obj.contains_key("data") {
        return parse_tagged(&obj, policy);
    }

    // Canonical Arora serde passes through. (Legacy shorthands never parse
    // here: their keys are not Arora tags, except `bool` — identical in both
    // vocabularies — and `enum`, whose legacy payload lacks the
    // `id`/`variant_id` fields Arora requires and thus falls through.)
    if let Ok(parsed) = serde_json::from_value::<Value>(JsonValue::Object(obj.clone())) {
        return Ok(parsed);
    }

    // Legacy shorthand objects, in the historical priority order.
    if let Some(s) = obj.get("text").and_then(|x| x.as_str()) {
        return Ok(text(s));
    }
    if let Some(f) = obj.get("float").and_then(|x| x.as_f64()) {
        return Ok(float(f as f32));
    }
    if let Some(b) = obj.get("bool").and_then(|x| x.as_bool()) {
        return Ok(bool_(b));
    }
    if let Some(v) = obj.get("vec2") {
        return parse_components::<2>(v, "vec2").map(vec2);
    }
    if let Some(v) = obj.get("vec3") {
        return parse_components::<3>(v, "vec3").map(vec3);
    }
    if let Some(v) = obj.get("vec4") {
        return parse_components::<4>(v, "vec4").map(vec4);
    }
    if let Some(v) = obj.get("quat") {
        return parse_components::<4>(v, "quat").map(quat);
    }
    if let Some(v) = obj.get("color") {
        return parse_components::<4>(v, "color").map(color_rgba);
    }
    if let Some(items) = obj.get("vector").and_then(|x| x.as_array()) {
        let xs: Vec<f32> = items
            .iter()
            .map(|x| x.as_f64().unwrap_or(0.0) as f32)
            .collect();
        return Ok(vector(xs));
    }
    if let Some(t) = obj.get("transform").and_then(|x| x.as_object()) {
        return parse_transform(t);
    }
    if let Some(e) = obj.get("enum").and_then(|x| x.as_object()) {
        let tag = e.get("tag").and_then(|x| x.as_str()).unwrap_or_default();
        let payload = e.get("value").cloned().unwrap_or(JsonValue::Null);
        return Ok(enumeration(tag, parse_value_with_policy(payload, policy)?));
    }
    if let Some(entries) = obj.get("record").and_then(|x| x.as_object()) {
        return parse_record(entries, policy);
    }
    for key in ["array", "list", "tuple"] {
        if let Some(items) = obj.get(key).and_then(|x| x.as_array()) {
            return Ok(array(
                items
                    .iter()
                    .cloned()
                    .map(|item| parse_value_with_policy(item, policy))
                    .collect::<Result<Vec<_>, _>>()?,
            ));
        }
    }

    // Raw component objects: {x, y}, {x, y, z}, {x, y, z, w}.
    if obj.contains_key("x") && obj.contains_key("y") {
        let raw = JsonValue::Object(obj.clone());
        match obj.len() {
            2 => {
                if let Ok(a) = parse_components::<2>(&raw, "raw vec2") {
                    return Ok(vec2(a));
                }
            }
            3 => {
                if let Ok(a) = parse_components::<3>(&raw, "raw vec3") {
                    return Ok(vec3(a));
                }
            }
            4 => {
                if let Ok(a) = parse_components::<4>(&raw, "raw quat") {
                    return Ok(quat(a));
                }
            }
            _ => {}
        }
    }

    // Raw color component objects: {r, g, b} and {r, g, b, a} (alpha
    // defaults to 1).
    if let (Some(r), Some(g), Some(b)) = (
        obj.get("r").and_then(|x| x.as_f64()),
        obj.get("g").and_then(|x| x.as_f64()),
        obj.get("b").and_then(|x| x.as_f64()),
    ) {
        let a = obj.get("a").and_then(|x| x.as_f64());
        let expected_len = if a.is_some() { 4 } else { 3 };
        if obj.len() == expected_len {
            return Ok(color_rgba([
                r as f32,
                g as f32,
                b as f32,
                a.unwrap_or(1.0) as f32,
            ]));
        }
    }

    Err(serde_json::Error::custom(format!(
        "unrecognized value payload with keys [{}]",
        obj.keys().cloned().collect::<Vec<_>>().join(", ")
    )))
}

fn parse_tagged(
    obj: &Map<String, JsonValue>,
    policy: NumericArrayPolicy,
) -> Result<Value, serde_json::Error> {
    let ty = obj
        .get("type")
        .and_then(|x| x.as_str())
        .ok_or_else(|| serde_json::Error::custom("'type' must be a string"))?;
    let data = obj
        .get("data")
        .cloned()
        .ok_or_else(|| serde_json::Error::custom("missing 'data'"))?;
    match ty {
        "float" => Ok(float(
            data.as_f64()
                .ok_or_else(|| serde_json::Error::custom("float data must be a number"))?
                as f32,
        )),
        "bool" => Ok(bool_(data.as_bool().ok_or_else(|| {
            serde_json::Error::custom("bool data must be a boolean")
        })?)),
        "text" => Ok(text(data.as_str().ok_or_else(|| {
            serde_json::Error::custom("text data must be a string")
        })?)),
        "vec2" => parse_components::<2>(&data, "vec2").map(vec2),
        "vec3" => parse_components::<3>(&data, "vec3").map(vec3),
        "vec4" => parse_components::<4>(&data, "vec4").map(vec4),
        "quat" => parse_components::<4>(&data, "quat").map(quat),
        "colorrgba" => parse_components::<4>(&data, "colorrgba").map(color_rgba),
        "vector" => {
            let items = data
                .as_array()
                .ok_or_else(|| serde_json::Error::custom("vector data must be an array"))?;
            Ok(vector(
                items
                    .iter()
                    .map(|x| x.as_f64().unwrap_or(0.0) as f32)
                    .collect(),
            ))
        }
        "transform" => {
            let t = data
                .as_object()
                .ok_or_else(|| serde_json::Error::custom("transform data must be an object"))?;
            parse_transform(t)
        }
        "enum" => {
            let parts = data
                .as_array()
                .ok_or_else(|| serde_json::Error::custom("enum data must be [tag, value]"))?;
            let tag = parts
                .first()
                .and_then(|x| x.as_str())
                .ok_or_else(|| serde_json::Error::custom("enum tag must be a string"))?;
            let payload = parts
                .get(1)
                .cloned()
                .ok_or_else(|| serde_json::Error::custom("enum data must be [tag, value]"))?;
            Ok(enumeration(tag, parse_value_with_policy(payload, policy)?))
        }
        "record" => {
            let entries = data
                .as_object()
                .ok_or_else(|| serde_json::Error::custom("record data must be an object"))?;
            parse_record(entries, policy)
        }
        "array" | "list" | "tuple" => {
            let items = data
                .as_array()
                .ok_or_else(|| serde_json::Error::custom("sequence data must be an array"))?;
            Ok(array(
                items
                    .iter()
                    .cloned()
                    .map(|item| parse_value_with_policy(item, policy))
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        other => Err(serde_json::Error::custom(format!(
            "unknown value type tag '{other}'"
        ))),
    }
}

fn parse_record(
    entries: &Map<String, JsonValue>,
    policy: NumericArrayPolicy,
) -> Result<Value, serde_json::Error> {
    let fields = entries
        .iter()
        .map(|(key, val)| Ok((key.as_str(), parse_value_with_policy(val.clone(), policy)?)))
        .collect::<Result<Vec<_>, serde_json::Error>>()?;
    Ok(record(fields))
}

fn parse_transform(t: &Map<String, JsonValue>) -> Result<Value, serde_json::Error> {
    let component = |key: &str| {
        t.get(key)
            .ok_or_else(|| serde_json::Error::custom(format!("transform missing '{key}'")))
    };
    Ok(transform(Transform {
        translation: parse_components::<3>(component("translation")?, "translation")?,
        rotation: parse_components::<4>(component("rotation")?, "rotation")?,
        scale: parse_components::<3>(component("scale")?, "scale")?,
    }))
}

/// Read `N` float components from a JSON array of numbers or from an object
/// with `x`/`y`/`z`/`w` keys.
fn parse_components<const N: usize>(
    v: &JsonValue,
    what: &str,
) -> Result<[f32; N], serde_json::Error> {
    const KEYS: [&str; 4] = ["x", "y", "z", "w"];
    let mut out = [0.0f32; N];
    if let Some(items) = v.as_array() {
        if items.len() != N {
            return Err(serde_json::Error::custom(format!(
                "{what} expects {N} components, got {}",
                items.len()
            )));
        }
        for (slot, item) in out.iter_mut().zip(items) {
            *slot = item.as_f64().ok_or_else(|| {
                serde_json::Error::custom(format!("{what} component not a number"))
            })? as f32;
        }
        return Ok(out);
    }
    if let Some(obj) = v.as_object() {
        for (slot, key) in out.iter_mut().zip(KEYS.iter().take(N)) {
            *slot = obj.get(*key).and_then(|x| x.as_f64()).ok_or_else(|| {
                serde_json::Error::custom(format!("{what} missing numeric '{key}'"))
            })? as f32;
        }
        return Ok(out);
    }
    Err(serde_json::Error::custom(format!(
        "{what} must be an array or component object"
    )))
}

// ---- write batches ---------------------------------------------------------------

/// Deserialize a JSON write-batch array (`[{ "path": ..., "value": ...,
/// "shape"? }]`) into a [`WriteBatch`], accepting values in any form
/// [`parse_value`] accepts. Canonical batches also deserialize directly with
/// serde; this helper exists for payloads carrying legacy value forms.
pub fn writebatch_from_json(value: JsonValue) -> Result<WriteBatch, serde_json::Error> {
    let Some(items) = value.as_array() else {
        return Ok(WriteBatch::new());
    };

    let mut batch = WriteBatch::new();
    for item in items {
        let path_str = item
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| serde_json::Error::custom("missing 'path' field"))?;
        let path = TypedPath::parse(path_str).map_err(serde_json::Error::custom)?;

        let value_field = item
            .get("value")
            .cloned()
            .ok_or_else(|| serde_json::Error::custom("missing 'value' field"))?;
        let value = parse_value(value_field)?;

        let shape = match item.get("shape") {
            Some(shape_value) => Some(serde_json::from_value(shape_value.clone())?),
            None => None,
        };

        batch.push(WriteOp::new_with_shape(path, value, shape));
    }

    Ok(batch)
}

/// Helper to create a [`WriteBatch`] from a list of changes expressed as `(TypedPath, Value)`
/// pairs. This is primarily used in tests where ergonomic builders are desirable.
pub fn writebatch_from_pairs(pairs: impl IntoIterator<Item = (TypedPath, Value)>) -> WriteBatch {
    let mut batch = WriteBatch::new();
    for (path, value) in pairs {
        batch.push(WriteOp::new(path, value));
    }
    batch
}

// ---- graph specs ----------------------------------------------------------------

fn normalize_shape_json(shape: JsonValue) -> JsonValue {
    match shape {
        JsonValue::String(id) => json!({ "id": id }),
        JsonValue::Object(obj) => JsonValue::Object(obj),
        other => other,
    }
}

fn normalize_input_default_entry(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(mut map) => {
            let payload = if let Some(val) = map.remove("value") {
                normalize_value_json(val)
            } else if let Some(val) = map.remove("default") {
                normalize_value_json(val)
            } else {
                normalize_value_json(JsonValue::Object(map.clone()))
            };

            let mut normalized = Map::new();
            normalized.insert("value".to_string(), payload);

            if let Some(shape_val) = map.remove("shape").or_else(|| map.remove("default_shape")) {
                normalized.insert("shape".to_string(), normalize_shape_json(shape_val));
            }

            JsonValue::Object(normalized)
        }
        other => {
            let mut normalized = Map::new();
            normalized.insert("value".to_string(), normalize_value_json(other));
            JsonValue::Object(normalized)
        }
    }
}

fn normalize_operand_key(
    original: &str,
    aliases: &mut HashMap<String, String>,
    next_index: &mut usize,
) -> String {
    if let Some(existing) = aliases.get(original) {
        return existing.clone();
    }

    if original.starts_with("operand_") {
        let key = original.to_string();
        aliases.insert(original.to_string(), key.clone());
        return key;
    }

    let key = format!("operand_{}", *next_index);
    *next_index += 1;
    aliases.insert(original.to_string(), key.clone());
    key
}

/// Normalize a graph specification JSON value in-place. This mirrors the wasm
/// helpers previously implemented in individual crates.
pub fn normalize_graph_spec_value(root: &mut JsonValue) -> Result<(), JsonError> {
    if root.get("links").is_some() {
        return Err(JsonError::LegacyLinksField);
    }

    let mut converted_edges: Vec<JsonValue> = Vec::new();
    let mut node_variadic_aliases: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut node_operand_counters: HashMap<String, usize> = HashMap::new();

    if let Some(nodes) = root.get_mut("nodes").and_then(|n| n.as_array_mut()) {
        for node in nodes.iter_mut() {
            if let Some(kind) = node.get("kind") {
                if node.get("type").is_none() {
                    node["type"] = kind.clone();
                }
            }

            if let Some(ty) = node
                .get_mut("type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase())
            {
                node["type"] = JsonValue::String(ty);
            }

            if let Some(params) = node.get_mut("params").and_then(|p| p.as_object_mut()) {
                if let Some(value) = params.get_mut("value") {
                    let taken = std::mem::take(value);
                    *value = normalize_value_json(taken);
                }

                if let Some(path_val) = params.get_mut("path") {
                    if !path_val.is_string() {
                        if let Some(obj) = path_val.as_object() {
                            if let Some(s) = obj
                                .get("path")
                                .and_then(|inner| inner.as_str())
                                .map(|s| s.to_string())
                            {
                                *path_val = JsonValue::String(s);
                            }
                        }
                    }
                }

                if let Some(sizes_val) = params.get_mut("sizes") {
                    if let Some(arr) = sizes_val.as_array() {
                        let mut normalized = Vec::with_capacity(arr.len());
                        for item in arr {
                            let num = if let Some(f) = item.as_f64() {
                                f
                            } else if let Some(s) = item.as_str() {
                                s.parse::<f64>().unwrap_or(0.0)
                            } else {
                                0.0
                            };
                            normalized.push(JsonValue::from(num));
                        }
                        *sizes_val = JsonValue::Array(normalized);
                    }
                }
            }

            if let Some(outputs) = node
                .get_mut("output_shapes")
                .and_then(|o| o.as_object_mut())
            {
                let keys: Vec<String> = outputs.keys().cloned().collect();
                for key in keys {
                    if let Some(shape) = outputs.get_mut(&key) {
                        if shape.is_string() {
                            let id = shape.as_str().unwrap().to_string();
                            *shape = json!({ "id": id });
                        }
                    }
                }
            }

            if let Some(node_map) = node.as_object_mut() {
                let node_id = node_map
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let node_type = node_map
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let needs_operand = matches!(
                    node_type.as_str(),
                    "add" | "multiply" | "join" | "case" | "default_blend"
                );
                let mut operand_aliases: HashMap<String, String> = HashMap::new();
                let mut next_operand_index: usize = 1;

                let mut input_defaults_map =
                    if let Some(existing_defaults) = node_map.remove("input_defaults") {
                        if let Some(obj) = existing_defaults.as_object() {
                            let mut normalized = Map::new();
                            for (input, default_value) in obj {
                                normalized.insert(
                                    input.clone(),
                                    normalize_input_default_entry(default_value.clone()),
                                );
                            }
                            normalized
                        } else {
                            Map::new()
                        }
                    } else {
                        Map::new()
                    };

                if let Some(inputs_value) = node_map.remove("inputs") {
                    if let Some(inputs_obj) = inputs_value.as_object() {
                        for (input_key, conn_value) in inputs_obj.iter() {
                            let normalized_input_key = if needs_operand {
                                normalize_operand_key(
                                    input_key,
                                    &mut operand_aliases,
                                    &mut next_operand_index,
                                )
                            } else {
                                input_key.clone()
                            };

                            let mut from_node: Option<String> = None;
                            let mut output_key: Option<String> = None;
                            let mut selector: Option<JsonValue> = None;
                            let mut default_payload: Option<JsonValue> = None;
                            let mut default_shape_payload: Option<JsonValue> = None;

                            match conn_value {
                                JsonValue::String(node_id) => {
                                    from_node = Some(node_id.clone());
                                }
                                JsonValue::Object(map) => {
                                    if let Some(id_val) = map
                                        .get("node_id")
                                        .or_else(|| map.get("node"))
                                        .and_then(|v| v.as_str())
                                    {
                                        from_node = Some(id_val.to_string());
                                    }
                                    output_key = map
                                        .get("output_key")
                                        .or_else(|| map.get("output"))
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string());
                                    if let Some(sel_val) = map.get("selector") {
                                        selector = Some(sel_val.clone());
                                    }
                                    if let Some(default_val) =
                                        map.get("default").or_else(|| map.get("value"))
                                    {
                                        default_payload = Some(default_val.clone());
                                    }
                                    if let Some(shape_val) =
                                        map.get("default_shape").or_else(|| map.get("shape"))
                                    {
                                        default_shape_payload = Some(shape_val.clone());
                                    }
                                    if from_node.is_none() && default_payload.is_none() {
                                        default_payload = Some(JsonValue::Object(map.clone()));
                                    }
                                }
                                other => {
                                    default_payload = Some(other.clone());
                                }
                            }

                            if let Some(payload) = default_payload {
                                let default_entry = if let Some(shape_payload) =
                                    default_shape_payload
                                {
                                    let mut default_obj = Map::new();
                                    default_obj.insert("value".to_string(), payload);
                                    default_obj.insert("shape".to_string(), shape_payload);
                                    normalize_input_default_entry(JsonValue::Object(default_obj))
                                } else {
                                    normalize_input_default_entry(payload)
                                };
                                input_defaults_map
                                    .insert(normalized_input_key.clone(), default_entry);
                            }

                            if let Some(source_id) = from_node {
                                let mut edge = serde_json::Map::new();
                                let mut from_obj = serde_json::Map::new();
                                from_obj
                                    .insert("node_id".to_string(), JsonValue::String(source_id));
                                if let Some(output) = output_key.clone() {
                                    from_obj
                                        .insert("output".to_string(), JsonValue::String(output));
                                }
                                let mut to_obj = serde_json::Map::new();
                                to_obj.insert(
                                    "node_id".to_string(),
                                    JsonValue::String(node_id.clone()),
                                );
                                to_obj.insert(
                                    "input".to_string(),
                                    JsonValue::String(normalized_input_key.clone()),
                                );
                                edge.insert("from".to_string(), JsonValue::Object(from_obj));
                                edge.insert("to".to_string(), JsonValue::Object(to_obj));
                                if let Some(sel) = selector.clone() {
                                    edge.insert("selector".to_string(), sel);
                                }
                                converted_edges.push(JsonValue::Object(edge));
                            }
                        }
                    }
                }

                if needs_operand && !input_defaults_map.is_empty() {
                    let mut remapped = Map::new();
                    for (key, value) in input_defaults_map.into_iter() {
                        let normalized = normalize_operand_key(
                            &key,
                            &mut operand_aliases,
                            &mut next_operand_index,
                        );
                        remapped.insert(normalized, value);
                    }
                    input_defaults_map = remapped;
                }

                if !input_defaults_map.is_empty() {
                    node_map.insert(
                        "input_defaults".to_string(),
                        JsonValue::Object(input_defaults_map),
                    );
                }

                if needs_operand {
                    node_variadic_aliases.insert(node_id.clone(), operand_aliases);
                    node_operand_counters.insert(node_id, next_operand_index);
                }
            }
        }
    }

    if let Some(edges_value) = root.get_mut("edges").and_then(|v| v.as_array_mut()) {
        for edge in edges_value.iter_mut() {
            if let Some(to_obj) = edge.get_mut("to").and_then(|v| v.as_object_mut()) {
                let node_id = to_obj
                    .get("node_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                if let (Some(alias_map), Some(counter)) = (
                    node_variadic_aliases.get_mut(&node_id),
                    node_operand_counters.get_mut(&node_id),
                ) {
                    if let Some(input_value) = to_obj.get_mut("input") {
                        if let Some(current) = input_value.as_str() {
                            let normalized = normalize_operand_key(current, alias_map, counter);
                            *input_value = JsonValue::String(normalized);
                        }
                    }
                }
            }
        }
    }

    if !converted_edges.is_empty() {
        match root.get_mut("edges") {
            Some(existing) => {
                if let Some(array) = existing.as_array_mut() {
                    array.extend(converted_edges);
                } else {
                    *existing = JsonValue::Array(converted_edges);
                }
            }
            None => {
                root["edges"] = JsonValue::Array(converted_edges);
            }
        }
    } else if root.get("edges").is_none() {
        root["edges"] = JsonValue::Array(Vec::new());
    }

    Ok(())
}

/// Convenience wrapper that parses a JSON string, normalizes it, and returns
/// the normalized [`serde_json::Value`].
pub fn normalize_graph_spec_json(json_str: &str) -> Result<JsonValue, JsonError> {
    let mut root: JsonValue =
        serde_json::from_str(json_str).map_err(|e| JsonError::GraphParse(e.to_string()))?;
    normalize_graph_spec_value(&mut root)?;
    Ok(root)
}

/// Convenience wrapper that returns a JSON string with all shorthand normalized.
pub fn normalize_graph_spec_json_string(json_str: &str) -> Result<String, JsonError> {
    let value = normalize_graph_spec_json(json_str)?;
    serde_json::to_string(&value).map_err(|e| JsonError::GraphSerialize(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{
        as_array, as_bool, as_color_rgba, as_enumeration, as_float, as_quat, as_record, as_text,
        as_transform, as_vec2, as_vec3, as_vec4, as_vector, variant_id, VEC3_TYPE,
    };

    #[test]
    fn bare_primitives_normalize_to_arora_serde() {
        assert_eq!(normalize_value_json(json!(1.5)), json!({ "f32": 1.5 }));
        assert_eq!(normalize_value_json(json!(true)), json!({ "bool": true }));
        assert_eq!(normalize_value_json(json!("hi")), json!({ "str": "hi" }));
    }

    #[test]
    fn numeric_arrays_follow_policy() {
        assert_eq!(
            as_vec2(&parse_value(json!([1, 2])).unwrap()),
            Some([1.0, 2.0])
        );
        assert_eq!(
            as_vec3(&parse_value(json!([1, 2, 3])).unwrap()),
            Some([1.0, 2.0, 3.0])
        );
        assert_eq!(
            as_vec4(&parse_value(json!([1, 2, 3, 4])).unwrap()),
            Some([1.0, 2.0, 3.0, 4.0])
        );
        assert_eq!(
            as_vector(&parse_value(json!([1, 2, 3, 4, 5])).unwrap()),
            Some(&[1.0, 2.0, 3.0, 4.0, 5.0][..])
        );

        // Staging: never guess arity.
        assert_eq!(
            as_vector(&parse_value_staging(json!([1, 2, 3])).unwrap()),
            Some(&[1.0, 2.0, 3.0][..])
        );
        assert_eq!(
            normalize_value_json_staging(json!([1, 2, 3])),
            json!({ "f32s": [1.0, 2.0, 3.0] })
        );
    }

    #[test]
    fn vec3_normalizes_to_structure_with_vizij_id() {
        let normalized = normalize_value_json(json!({ "vec3": [1, 2, 3] }));
        assert_eq!(normalized["struct"]["id"], VEC3_TYPE.to_string());
        let parsed: Value = serde_json::from_value(normalized).unwrap();
        assert_eq!(as_vec3(&parsed), Some([1.0, 2.0, 3.0]));
    }

    #[test]
    fn legacy_shorthand_objects_parse() {
        assert_eq!(
            as_float(&parse_value(json!({ "float": 1.5 })).unwrap()),
            Some(1.5)
        );
        assert_eq!(
            as_bool(&parse_value(json!({ "bool": true })).unwrap()),
            Some(true)
        );
        assert_eq!(
            as_text(&parse_value(json!({ "text": "hi" })).unwrap()),
            Some("hi")
        );
        assert_eq!(
            as_vec2(&parse_value(json!({ "vec2": [1, 2] })).unwrap()),
            Some([1.0, 2.0])
        );
        assert_eq!(
            as_vec4(&parse_value(json!({ "vec4": [1, 2, 3, 4] })).unwrap()),
            Some([1.0, 2.0, 3.0, 4.0])
        );
        assert_eq!(
            as_quat(&parse_value(json!({ "quat": [0, 0, 0, 1] })).unwrap()),
            Some([0.0, 0.0, 0.0, 1.0])
        );
        assert_eq!(
            as_color_rgba(&parse_value(json!({ "color": [0.1, 0.2, 0.3, 1.0] })).unwrap()),
            Some([0.1, 0.2, 0.3, 1.0])
        );
        assert_eq!(
            as_vector(&parse_value(json!({ "vector": [9, 8] })).unwrap()),
            Some(&[9.0, 8.0][..])
        );
    }

    #[test]
    fn legacy_transform_parses() {
        let t = parse_value(json!({
            "transform": {
                "translation": [1, 2, 3],
                "rotation": [0, 0, 0, 1],
                "scale": [1, 1, 1]
            }
        }))
        .unwrap();
        let pod = as_transform(&t).expect("transform");
        assert_eq!(pod.translation, [1.0, 2.0, 3.0]);
        assert_eq!(pod.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(pod.scale, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn legacy_enum_becomes_native_enumeration() {
        let v = parse_value(json!({
            "enum": { "tag": "Option", "value": { "float": 1.0 } }
        }))
        .unwrap();
        let (variant, payload) = as_enumeration(&v).expect("enumeration");
        assert_eq!(variant, variant_id("Option"));
        assert_eq!(as_float(payload), Some(1.0));

        // Normalized JSON is the native Arora form.
        let normalized = normalize_value_json(json!({
            "enum": { "tag": "Option", "value": { "float": 1.0 } }
        }));
        assert_eq!(
            normalized["enum"]["variant_id"],
            variant_id("Option").to_string()
        );
    }

    #[test]
    fn legacy_record_becomes_keyvalue() {
        let v = parse_value(json!({
            "record": { "angle": { "float": 0.4 }, "nested": { "record": { "x": 1.0 } } }
        }))
        .unwrap();
        let entries = as_record(&v).expect("record");
        assert_eq!(entries[0].0, "angle");
        assert_eq!(as_float(entries[0].1), Some(0.4));
        let nested = as_record(entries[1].1).expect("nested");
        assert_eq!(as_float(nested[0].1), Some(1.0));
    }

    #[test]
    fn legacy_sequences_collapse_to_array_value() {
        for key in ["array", "list", "tuple"] {
            let v = parse_value(json!({ key: [1.0, true] })).unwrap();
            let items = as_array(&v).expect("array value");
            assert_eq!(as_float(&items[0]), Some(1.0));
            assert_eq!(as_bool(&items[1]), Some(true));
        }
    }

    #[test]
    fn tagged_forms_parse() {
        assert_eq!(
            as_float(&parse_value(json!({ "type": "float", "data": 1.0 })).unwrap()),
            Some(1.0)
        );
        assert_eq!(
            as_vec3(&parse_value(json!({ "type": "vec3", "data": [1, 2, 3] })).unwrap()),
            Some([1.0, 2.0, 3.0])
        );
        assert_eq!(
            as_color_rgba(
                &parse_value(json!({ "type": "colorrgba", "data": [0.0, 0.5, 1.0, 1.0] })).unwrap()
            ),
            Some([0.0, 0.5, 1.0, 1.0])
        );
        let e = parse_value(json!({
            "type": "enum",
            "data": ["grasp", { "type": "float", "data": 0.5 }]
        }))
        .unwrap();
        let (variant, payload) = as_enumeration(&e).expect("enumeration");
        assert_eq!(variant, variant_id("grasp"));
        assert_eq!(as_float(payload), Some(0.5));
        let r = parse_value(json!({ "type": "record", "data": { "a": 1.0 } })).unwrap();
        assert_eq!(as_float(as_record(&r).unwrap()[0].1), Some(1.0));
        let t = parse_value(json!({
            "type": "transform",
            "data": {
                "translation": [1, 2, 3],
                "rotation": [0, 0, 0, 1],
                "scale": [1, 1, 1]
            }
        }))
        .unwrap();
        assert!(as_transform(&t).is_some());
    }

    #[test]
    fn raw_component_objects_parse() {
        assert_eq!(
            as_vec2(&parse_value(json!({ "x": 1.0, "y": 2.0 })).unwrap()),
            Some([1.0, 2.0])
        );
        assert_eq!(
            as_vec3(&parse_value(json!({ "x": 1.0, "y": 2.0, "z": 3.0 })).unwrap()),
            Some([1.0, 2.0, 3.0])
        );
        // Four components read as a quaternion (the historical raw form).
        assert_eq!(
            as_quat(&parse_value(json!({ "x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0 })).unwrap()),
            Some([0.0, 0.0, 0.0, 1.0])
        );
        // Transforms with raw components in their fields also parse.
        let t = parse_value(json!({
            "transform": {
                "translation": { "x": 1.0, "y": 2.0, "z": 3.0 },
                "rotation": { "x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0 },
                "scale": { "x": 1.0, "y": 1.0, "z": 1.0 }
            }
        }))
        .unwrap();
        assert_eq!(as_transform(&t).unwrap().translation, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn raw_color_component_objects_parse() {
        assert_eq!(
            as_color_rgba(&parse_value(json!({ "r": 0.25, "g": 0.5, "b": 0.75 })).unwrap()),
            Some([0.25, 0.5, 0.75, 1.0])
        );
        assert_eq!(
            as_color_rgba(
                &parse_value(json!({ "r": 0.25, "g": 0.5, "b": 0.75, "a": 0.5 })).unwrap()
            ),
            Some([0.25, 0.5, 0.75, 0.5])
        );
        // Extra keys make the payload ambiguous; it is not a color.
        assert!(parse_value(json!({ "r": 1.0, "g": 1.0, "b": 1.0, "extra": 1.0 })).is_err());
    }

    #[test]
    fn canonical_arora_serde_passes_through() {
        let forms = [
            json!({ "f32": 1.5 }),
            json!({ "str": "hi" }),
            json!({ "f32s": [1.0, 2.0] }),
            normalize_value_json(json!({ "vec3": [1, 2, 3] })),
            normalize_value_json(json!({ "record": { "a": 1.0 } })),
            normalize_value_json(json!({ "enum": { "tag": "t", "value": 1.0 } })),
        ];
        for form in forms {
            assert_eq!(
                normalize_value_json(form.clone()),
                form,
                "must be idempotent"
            );
        }
    }

    #[test]
    fn unrecognized_payloads_pass_through_and_fail_parse() {
        let unknown = json!({ "mystery": 1 });
        assert_eq!(normalize_value_json(unknown.clone()), unknown);
        assert!(parse_value(unknown).is_err());
        assert!(parse_value(json!(null)).is_err());
    }

    #[test]
    fn writebatch_accepts_legacy_value_forms() {
        let tp = TypedPath::parse("robot/Arm/Joint.angle").unwrap();
        let expected = writebatch_from_pairs(vec![(tp, crate::value::vec3([1.0, 2.0, 3.0]))]);
        let parsed = writebatch_from_json(json!([
            { "path": "robot/Arm/Joint.angle", "value": { "vec3": [1, 2, 3] } }
        ]))
        .expect("deserialize");
        assert_eq!(expected, parsed);

        // Canonical batches round-trip through the same helper.
        let canonical = serde_json::to_value(&expected).unwrap();
        assert_eq!(writebatch_from_json(canonical).unwrap(), expected);
    }

    #[test]
    fn graph_spec_normalization_inserts_type() {
        let mut root = json!({
            "nodes": [
                {
                    "kind": "Node",
                    "params": {
                        "value": { "float": 1.0 }
                    },
                    "output_shapes": {
                        "value": "vec3"
                    }
                }
            ]
        });
        normalize_graph_spec_value(&mut root).expect("normalize graph spec");
        assert_eq!(root["nodes"][0]["type"], "node");
        assert_eq!(root["nodes"][0]["params"]["value"], json!({ "f32": 1.0 }));
        assert_eq!(root["nodes"][0]["output_shapes"]["value"]["id"], "vec3");
    }

    #[test]
    fn graph_spec_normalization_converts_inputs_to_edges() {
        let mut root = json!({
            "nodes": [
                { "id": "constant", "type": "constant" },
                {
                    "id": "adder",
                    "type": "add",
                    "inputs": {
                        "lhs": { "node_id": "constant", "output_key": "value" },
                        "rhs": { "node_id": "constant" }
                    }
                }
            ]
        });

        normalize_graph_spec_value(&mut root).expect("normalize graph spec");

        assert!(
            root["nodes"][1].get("inputs").is_none(),
            "inputs should be removed"
        );

        let edges = root["edges"].as_array().expect("edges array");
        assert_eq!(edges.len(), 2, "expected two generated edges");

        let lhs = &edges[0];
        assert_eq!(lhs["from"]["node_id"], "constant");
        assert_eq!(lhs["from"]["output"], "value");
        assert_eq!(lhs["to"]["node_id"], "adder");
        assert_eq!(lhs["to"]["input"], "operand_1");

        let rhs = &edges[1];
        assert_eq!(rhs["from"]["node_id"], "constant");
        assert!(
            rhs["from"].get("output").is_none(),
            "default output key omitted"
        );
        assert_eq!(rhs["to"]["node_id"], "adder");
        assert_eq!(rhs["to"]["input"], "operand_2");
    }

    #[test]
    fn graph_spec_normalization_injects_empty_edges_array() {
        let mut root = json!({
            "nodes": [
                { "id": "constant", "type": "constant" }
            ]
        });

        normalize_graph_spec_value(&mut root).expect("normalize graph spec");

        assert_eq!(
            root["edges"].as_array().map(|a| a.len()),
            Some(0),
            "normalizer should emit an empty edges array"
        );
    }

    #[test]
    fn graph_spec_normalization_preserves_default_only_inputs() {
        let mut root = json!({
            "nodes": [
                { "id": "num", "type": "constant" },
                {
                    "id": "div",
                    "type": "divide",
                    "inputs": {
                        "lhs": "num",
                        "rhs": 2.0
                    }
                }
            ]
        });

        normalize_graph_spec_value(&mut root).expect("normalize graph spec");

        let div = &root["nodes"][1];
        assert!(div.get("inputs").is_none(), "inputs should be stripped");

        let defaults = div["input_defaults"]
            .as_object()
            .expect("defaults map present");
        let rhs_default = defaults.get("rhs").expect("rhs default retained");
        assert_eq!(rhs_default["value"], json!({ "f32": 2.0 }));

        let edges = root["edges"].as_array().expect("edges array");
        assert_eq!(edges.len(), 1, "only the lhs connection becomes an edge");
        let lhs_link = &edges[0];
        assert_eq!(lhs_link["to"]["node_id"], "div");
        assert_eq!(lhs_link["to"]["input"], "lhs");
    }

    #[test]
    fn graph_spec_normalization_extracts_defaults_from_connections() {
        let mut root = json!({
            "nodes": [
                { "id": "config", "type": "constant" },
                {
                    "id": "div",
                    "type": "divide",
                    "inputs": {
                        "rhs": {
                            "node_id": "config",
                            "output_key": "gain",
                            "selector": [ { "index": 0 } ],
                            "default": { "float": 0.5 },
                            "default_shape": "Scalar"
                        }
                    }
                }
            ]
        });

        normalize_graph_spec_value(&mut root).expect("normalize graph spec");

        let div = &root["nodes"][1];
        let defaults = div["input_defaults"]
            .as_object()
            .expect("defaults map present");
        let rhs_default = defaults.get("rhs").expect("rhs default retained");
        assert_eq!(rhs_default["value"], json!({ "f32": 0.5 }));
        assert_eq!(rhs_default["shape"]["id"], "Scalar");

        let edges = root["edges"].as_array().expect("edges array");
        assert_eq!(edges.len(), 1, "single edge retained");
        let edge = &edges[0];
        assert_eq!(edge["from"]["node_id"], "config");
        assert_eq!(edge["from"]["output"], "gain");
        let selector = edge["selector"]
            .as_array()
            .expect("selector preserved for edge");
        assert_eq!(selector.len(), 1);
        assert_eq!(selector[0]["index"], 0);
    }

    #[test]
    fn graph_spec_normalization_rejects_legacy_links_field() {
        let mut root = json!({
            "nodes": [],
            "links": []
        });

        let err = normalize_graph_spec_value(&mut root)
            .expect_err("legacy links field should be rejected");
        assert!(matches!(err, JsonError::LegacyLinksField));
    }
}
