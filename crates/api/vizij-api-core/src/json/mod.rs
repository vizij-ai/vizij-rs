//! JSON normalization helpers shared by core and wasm crates.
//!
//! Most helpers convert shorthand inputs (numbers, `{ "vec3": [...] }`, etc.)
//! into the canonical `{ "type": "...", "data": ... }` format used by [`Value`].

use serde::de::Error as _;
use serde_json::{json, Map, Value as JsonValue};
use std::collections::HashMap;
use thiserror::Error;

use crate::{TypedPath, Value, WriteBatch, WriteOp};

/// Policy describing how purely numeric arrays should be normalized when
/// converting shorthand JSON into the canonical `{ "type": ..., "data": ... }`
/// representation used by [`Value`](crate::Value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericArrayPolicy {
    /// Match the historical behaviour used in production builds where arrays of
    /// length 2/3/4 become Vec2/Vec3/Vec4 respectively and any other numeric
    /// array becomes `Vector`.
    AutoVectorKinds,
    /// Treat all numeric arrays as `Vector` regardless of length. This matches
    /// the staging normalizer that avoids guessing the intended vector arity.
    AlwaysVector,
}

/// Errors produced while normalizing graph JSON blobs.
#[derive(Debug, Error)]
pub enum JsonError {
    /// The graph JSON string could not be parsed.
    #[error("graph json parse error: {0}")]
    GraphParse(String),
    /// The normalized graph JSON could not be serialized.
    #[error("serialize normalized graph: {0}")]
    GraphSerialize(String),
    /// A deprecated `links` field is present; use `edges` instead.
    #[error("graph spec uses deprecated 'links' field; rename to 'edges'")]
    LegacyLinksField,
}

/// Normalize shorthand `Value` JSON into the canonical `{ "type": ..., "data": ... }`
/// representation understood by the serde derives on [`Value`].
///
/// This helper accepts shorthand objects such as `{ "vec3": [1, 2, 3] }` and
/// primitive aliases like `1.0` or `[0, 1, 0]`.
///
/// # Examples
///
/// ```rust
/// use serde_json::json;
/// use vizij_api_core::json::normalize_value_json;
///
/// let raw = json!({ "vec3": [0.0, 1.0, 0.0] });
/// let normalized = normalize_value_json(raw);
/// assert_eq!(normalized["type"], "vec3");
/// ```
pub fn normalize_value_json(value: JsonValue) -> JsonValue {
    normalize_value_json_with_policy(value, NumericArrayPolicy::AutoVectorKinds)
}

/// Variant of [`normalize_value_json`] that mirrors the staging normalizer used by
/// the graph wasm crate. Numeric arrays are always treated as `Vector` unless they
/// are explicitly tagged with a `vec*` alias.
///
/// # Examples
///
/// ```rust
/// use serde_json::json;
/// use vizij_api_core::json::normalize_value_json_staging;
///
/// let raw = json!([1, 2, 3]);
/// let normalized = normalize_value_json_staging(raw);
/// assert_eq!(normalized["type"], "vector");
/// ```
pub fn normalize_value_json_staging(value: JsonValue) -> JsonValue {
    normalize_value_json_with_policy(value, NumericArrayPolicy::AlwaysVector)
}

/// Normalizes value JSON with policy.
fn normalize_value_json_with_policy(value: JsonValue, policy: NumericArrayPolicy) -> JsonValue {
    match value {
        JsonValue::Number(n) => json!({ "type": "float", "data": n }),
        JsonValue::Bool(b) => json!({ "type": "bool", "data": b }),
        JsonValue::String(s) => json!({ "type": "text", "data": s }),
        JsonValue::Array(arr) => {
            let all_numbers = arr.iter().all(|x| x.is_number());
            if all_numbers {
                match policy {
                    NumericArrayPolicy::AutoVectorKinds => match arr.len() {
                        2 => json!({ "type": "vec2", "data": arr }),
                        3 => json!({ "type": "vec3", "data": arr }),
                        4 => json!({ "type": "vec4", "data": arr }),
                        _ => json!({ "type": "vector", "data": arr }),
                    },
                    NumericArrayPolicy::AlwaysVector => {
                        json!({ "type": "vector", "data": arr })
                    }
                }
            } else {
                let data: Vec<JsonValue> = arr
                    .into_iter()
                    .map(|item| normalize_value_json_with_policy(item, policy))
                    .collect();
                json!({ "type": "list", "data": data })
            }
        }
        JsonValue::Object(obj) => {
            if obj.contains_key("type") && obj.contains_key("data") {
                return JsonValue::Object(obj);
            }
            if let Some(text) = obj.get("text").and_then(|x| x.as_str()) {
                return json!({ "type": "text", "data": text });
            }
            if let Some(f) = obj.get("float").and_then(|x| x.as_f64()) {
                return json!({ "type": "float", "data": f });
            }
            if let Some(b) = obj.get("bool").and_then(|x| x.as_bool()) {
                return json!({ "type": "bool", "data": b });
            }
            if let Some(arr) = obj.get("vec2").and_then(|x| x.as_array()) {
                return json!({ "type": "vec2", "data": arr });
            }
            if let Some(arr) = obj.get("vec3").and_then(|x| x.as_array()) {
                return json!({ "type": "vec3", "data": arr });
            }
            if let Some(arr) = obj.get("vec4").and_then(|x| x.as_array()) {
                return json!({ "type": "vec4", "data": arr });
            }
            if let Some(arr) = obj.get("quat").and_then(|x| x.as_array()) {
                return json!({ "type": "quat", "data": arr });
            }
            if let Some(arr) = obj.get("color").and_then(|x| x.as_array()) {
                return json!({ "type": "colorrgba", "data": arr });
            }
            if let Some(arr) = obj.get("vector").and_then(|x| x.as_array()) {
                return json!({ "type": "vector", "data": arr });
            }
            if let Some(transform) = obj.get("transform").and_then(|x| x.as_object()) {
                let translation = transform
                    .get("translation")
                    .cloned()
                    .unwrap_or(JsonValue::Null);
                let rotation = transform
                    .get("rotation")
                    .cloned()
                    .unwrap_or(JsonValue::Null);
                let scale = transform.get("scale").cloned().unwrap_or(JsonValue::Null);
                return json!({
                    "type": "transform",
                    "data": { "translation": translation, "rotation": rotation, "scale": scale }
                });
            }
            if let Some(enum_obj) = obj.get("enum").and_then(|x| x.as_object()) {
                let tag = enum_obj
                    .get("tag")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .to_string();
                let payload = enum_obj.get("value").cloned().unwrap_or(JsonValue::Null);
                let normalized_payload = normalize_value_json_with_policy(payload, policy);
                return json!({ "type": "enum", "data": [tag, normalized_payload] });
            }
            if let Some(record) = obj.get("record").and_then(|x| x.as_object()) {
                let mut data = Map::new();
                for (key, val) in record.iter() {
                    data.insert(
                        key.clone(),
                        normalize_value_json_with_policy(val.clone(), policy),
                    );
                }
                return json!({ "type": "record", "data": JsonValue::Object(data) });
            }
            if let Some(array_items) = obj.get("array").and_then(|x| x.as_array()) {
                let data: Vec<JsonValue> = array_items
                    .iter()
                    .cloned()
                    .map(|v| normalize_value_json_with_policy(v, policy))
                    .collect();
                return json!({ "type": "array", "data": data });
            }
            if let Some(list_items) = obj.get("list").and_then(|x| x.as_array()) {
                let data: Vec<JsonValue> = list_items
                    .iter()
                    .cloned()
                    .map(|v| normalize_value_json_with_policy(v, policy))
                    .collect();
                return json!({ "type": "list", "data": data });
            }
            if let Some(tuple_items) = obj.get("tuple").and_then(|x| x.as_array()) {
                let data: Vec<JsonValue> = tuple_items
                    .iter()
                    .cloned()
                    .map(|v| normalize_value_json_with_policy(v, policy))
                    .collect();
                return json!({ "type": "tuple", "data": data });
            }

            JsonValue::Object(obj)
        }
        other => other,
    }
}

/// Normalizes shape JSON.
fn normalize_shape_json(shape: JsonValue) -> JsonValue {
    match shape {
        JsonValue::String(id) => json!({ "id": id }),
        JsonValue::Object(obj) => JsonValue::Object(obj),
        other => other,
    }
}

/// Normalizes input default entry.
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

/// Normalizes operand key.
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

/// Normalize `Value` JSON then deserialize it into the strongly typed [`Value`] enum.
///
/// This keeps JSON shorthands consistent across call-sites (blackboard, wasm
/// wrappers, tests).
///
/// # Errors
///
/// Returns a [`serde_json::Error`] if the normalized payload does not match the
/// [`Value`](crate::Value) schema.
///
/// # Examples
///
/// ```rust
/// use serde_json::json;
/// use vizij_api_core::json::parse_value;
/// use vizij_api_core::Value;
///
/// let value = parse_value(json!({ "float": 2.0 }))?;
/// assert_eq!(value, Value::Float(2.0));
/// # Ok::<(), serde_json::Error>(())
/// ```
pub fn parse_value(value: JsonValue) -> Result<Value, serde_json::Error> {
    let normalized = normalize_value_json(value);
    serde_json::from_value(normalized)
}

/// Convert legacy shorthand `Value` JSON into a strongly typed [`Value`]
/// while using the staging numeric policy.
///
/// This mirrors the staging graph wasm normalizer behaviour.
///
/// # Errors
///
/// Returns a [`serde_json::Error`] if the normalized payload does not match the
/// [`Value`](crate::Value) schema.
///
/// # Examples
///
/// ```rust
/// use serde_json::json;
/// use vizij_api_core::json::parse_value_staging;
///
/// let value = parse_value_staging(json!([1, 2, 3]))?;
/// assert_eq!(value.kind(), vizij_api_core::ValueKind::Vector);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub fn parse_value_staging(value: JsonValue) -> Result<Value, serde_json::Error> {
    let normalized = normalize_value_json_staging(value);
    serde_json::from_value(normalized)
}

/// Normalize a graph specification JSON value in-place.
///
/// This mirrors the wasm helpers previously implemented in individual crates.
/// It lowercases node types, rewrites legacy `kind` fields, expands `inputs`
/// into `edges`, and normalizes inline defaults into `input_defaults`.
///
/// # Errors
///
/// Returns [`JsonError::LegacyLinksField`] when the deprecated `links` field
/// is present.
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

/// Parse a JSON string, normalize it, and return the normalized
/// [`serde_json::Value`].
///
/// # Errors
///
/// Returns [`JsonError::GraphParse`] when the input is not valid JSON and
/// [`JsonError::LegacyLinksField`] when the deprecated `links` field appears.
///
/// # Examples
///
/// ```rust
/// use vizij_api_core::json::normalize_graph_spec_json;
///
/// let raw = r#"{"nodes":[{"id":"a","type":"constant"}]}"#;
/// let normalized = normalize_graph_spec_json(raw)?;
/// assert!(normalized.get("edges").is_some());
/// # Ok::<(), vizij_api_core::json::JsonError>(())
/// ```
pub fn normalize_graph_spec_json(json_str: &str) -> Result<JsonValue, JsonError> {
    let mut root: JsonValue =
        serde_json::from_str(json_str).map_err(|e| JsonError::GraphParse(e.to_string()))?;
    normalize_graph_spec_value(&mut root)?;
    Ok(root)
}

/// Return a JSON string with all shorthand normalized.
///
/// # Errors
///
/// Returns [`JsonError::GraphParse`] when the input is not valid JSON,
/// [`JsonError::LegacyLinksField`] when the deprecated `links` field appears,
/// and [`JsonError::GraphSerialize`] when the normalized JSON cannot be stringified.
///
/// # Examples
///
/// ```rust
/// use vizij_api_core::json::normalize_graph_spec_json_string;
///
/// let raw = r#"{"nodes":[{"id":"a","type":"constant"}]}"#;
/// let normalized = normalize_graph_spec_json_string(raw)?;
/// assert!(normalized.contains("\"edges\""));
/// # Ok::<(), vizij_api_core::json::JsonError>(())
/// ```
pub fn normalize_graph_spec_json_string(json_str: &str) -> Result<String, JsonError> {
    let value = normalize_graph_spec_json(json_str)?;
    serde_json::to_string(&value).map_err(|e| JsonError::GraphSerialize(e.to_string()))
}

/// Convert a core [`Value`] into the legacy JSON structure used by existing wasm
/// consumers (objects like `{ "vec3": [...] }`).
///
/// # Examples
///
/// ```rust
/// use vizij_api_core::{Value, json::value_to_legacy_json};
///
/// let value = Value::Vec3([1.0, 2.0, 3.0]);
/// let legacy = value_to_legacy_json(&value);
/// assert_eq!(legacy["vec3"], serde_json::json!([1.0, 2.0, 3.0]));
/// ```
pub fn value_to_legacy_json(value: &Value) -> JsonValue {
    match value {
        Value::Float(f) => json!({ "float": *f }),
        Value::Bool(b) => json!({ "bool": *b }),
        Value::Vec2(a) => json!({ "vec2": [a[0], a[1]] }),
        Value::Vec3(a) => json!({ "vec3": [a[0], a[1], a[2]] }),
        Value::Vec4(a) => json!({ "vec4": [a[0], a[1], a[2], a[3]] }),
        Value::Quat(a) => json!({ "quat": [a[0], a[1], a[2], a[3]] }),
        Value::ColorRgba(a) => json!({ "color": [a[0], a[1], a[2], a[3]] }),
        Value::Transform {
            translation,
            rotation,
            scale,
        } => {
            json!({
                "transform": {
                    "translation": translation,
                    "rotation": rotation,
                    "scale": scale
                }
            })
        }
        Value::Vector(a) => json!({ "vector": a }),
        Value::Enum(tag, boxed) => {
            json!({ "enum": { "tag": tag, "value": value_to_legacy_json(boxed) } })
        }
        Value::Text(s) => json!({ "text": s }),
        Value::Record(map) => {
            let mut obj = Map::new();
            for (key, val) in map.iter() {
                obj.insert(key.clone(), value_to_legacy_json(val));
            }
            json!({ "record": JsonValue::Object(obj) })
        }
        Value::Array(items) => {
            let data: Vec<JsonValue> = items.iter().map(value_to_legacy_json).collect();
            json!({ "array": data })
        }
        Value::List(items) => {
            let data: Vec<JsonValue> = items.iter().map(value_to_legacy_json).collect();
            json!({ "list": data })
        }
        Value::Tuple(items) => {
            let data: Vec<JsonValue> = items.iter().map(value_to_legacy_json).collect();
            json!({ "tuple": data })
        }
    }
}

/// Convert a single [`WriteOp`] into the legacy JSON representation used by wasm wrappers.
///
/// # Examples
///
/// ```rust
/// use serde_json::json;
/// use vizij_api_core::{TypedPath, Value, WriteOp};
/// use vizij_api_core::json::writeop_to_legacy_json;
///
/// let op = WriteOp::new(TypedPath::parse("robot/Arm/Joint.angle")?, Value::Float(1.0));
/// let legacy = writeop_to_legacy_json(&op);
/// assert_eq!(legacy["path"], json!("robot/Arm/Joint.angle"));
/// # Ok::<(), String>(())
/// ```
pub fn writeop_to_legacy_json(op: &WriteOp) -> JsonValue {
    let mut map = Map::new();
    map.insert("path".to_string(), JsonValue::String(op.path.to_string()));
    map.insert("value".to_string(), value_to_legacy_json(&op.value));
    if let Some(shape) = &op.shape {
        map.insert(
            "shape".to_string(),
            serde_json::to_value(shape).unwrap_or(JsonValue::Null),
        );
    }
    JsonValue::Object(map)
}

/// Convert a [`WriteBatch`] into the legacy JSON array structure used by wasm wrappers.
///
/// # Examples
///
/// ```rust
/// use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};
/// use vizij_api_core::json::writebatch_to_legacy_json;
///
/// let mut batch = WriteBatch::new();
/// batch.push(WriteOp::new(
///     TypedPath::parse("robot/Arm/Joint.angle")?,
///     Value::Float(1.0),
/// ));
/// let legacy = writebatch_to_legacy_json(&batch);
/// assert!(legacy.as_array().is_some());
/// # Ok::<(), String>(())
/// ```
pub fn writebatch_to_legacy_json(batch: &WriteBatch) -> JsonValue {
    let arr: Vec<JsonValue> = batch.iter().map(writeop_to_legacy_json).collect();
    JsonValue::Array(arr)
}

/// Deserialize a legacy JSON value (e.g. `{ "vec3": [...] }`) into a [`Value`].
///
/// This helper is primarily intended for wasm consumers that still emit the
/// legacy structure; it normalizes and then deserializes just like [`parse_value`].
///
/// # Examples
///
/// ```rust
/// use serde_json::json;
/// use vizij_api_core::json::value_from_legacy_json;
/// use vizij_api_core::Value;
///
/// let legacy = json!({ "vec2": [0.0, 1.0] });
/// let value = value_from_legacy_json(legacy)?;
/// assert_eq!(value, Value::Vec2([0.0, 1.0]));
/// # Ok::<(), serde_json::Error>(())
/// ```
pub fn value_from_legacy_json(value: JsonValue) -> Result<Value, serde_json::Error> {
    parse_value(value)
}

/// Deserialize a legacy JSON write batch array back into a strongly typed [`WriteBatch`].
///
/// If the input is not an array, this returns an empty batch.
///
/// # Examples
///
/// ```rust
/// use serde_json::json;
/// use vizij_api_core::json::writebatch_from_legacy_json;
///
/// let legacy = json!([{
///     "path": "robot/Arm/Joint.angle",
///     "value": { "float": 1.0 }
/// }]);
/// let batch = writebatch_from_legacy_json(legacy)?;
/// assert_eq!(batch.iter().count(), 1);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub fn writebatch_from_legacy_json(value: JsonValue) -> Result<WriteBatch, serde_json::Error> {
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
        let value = value_from_legacy_json(value_field)?;

        let shape = match item.get("shape") {
            Some(shape_value) => Some(serde_json::from_value(shape_value.clone())?),
            None => None,
        };

        batch.push(WriteOp::new_with_shape(path, value, shape));
    }

    Ok(batch)
}

/// Create a [`WriteBatch`] from a list of changes expressed as `(TypedPath, Value)` pairs.
///
/// This is primarily used in tests where ergonomic builders are desirable.
///
/// # Examples
///
/// ```rust
/// use vizij_api_core::{TypedPath, Value};
/// use vizij_api_core::json::writebatch_from_pairs;
///
/// let batch = writebatch_from_pairs(vec![
///     (TypedPath::parse("robot/Arm/Joint.angle")?, Value::f(1.0)),
///     (TypedPath::parse("robot/Arm/Joint.enabled")?, Value::Bool(true)),
/// ]);
/// assert_eq!(batch.iter().count(), 2);
/// # Ok::<(), String>(())
/// ```
pub fn writebatch_from_pairs(pairs: impl IntoIterator<Item = (TypedPath, Value)>) -> WriteBatch {
    let mut batch = WriteBatch::new();
    for (path, value) in pairs {
        batch.push(WriteOp::new(path, value));
    }
    batch
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Normalizes numeric arrays auto vectors.
    fn normalize_numeric_arrays_auto_vectors() {
        let value = json!([1, 2, 3]);
        let normalized = normalize_value_json(value);
        assert_eq!(normalized["type"], "vec3");
    }

    #[test]
    /// Normalizes numeric arrays staging.
    fn normalize_numeric_arrays_staging() {
        let value = json!([1, 2, 3]);
        let normalized = normalize_value_json_staging(value);
        assert_eq!(normalized["type"], "vector");
    }

    #[test]
    /// Parses enum payload.
    fn parse_enum_payload() {
        let value = json!({
            "enum": {
                "tag": "Option",
                "value": { "float": 1.0 }
            }
        });
        let parsed = parse_value(value).expect("parse value");
        match parsed {
            Value::Enum(tag, boxed) => {
                assert_eq!(tag, "Option");
                assert!(matches!(*boxed, Value::Float(f) if (f - 1.0).abs() < f32::EPSILON));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    /// Verify writebatch legacy JSON round-trip stability.
    fn writebatch_legacy_roundtrip() {
        let tp = TypedPath::parse("robot/Arm/Joint.angle").unwrap();
        let batch = writebatch_from_pairs(vec![(tp, Value::Vec3([1.0, 2.0, 3.0]))]);
        let json = writebatch_to_legacy_json(&batch);
        let parsed = writebatch_from_legacy_json(json).expect("deserialize");
        assert_eq!(batch, parsed);
    }

    #[test]
    /// Verify normalization injects node `type` fields.
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
        assert_eq!(root["nodes"][0]["params"]["value"]["type"], "float");
        assert_eq!(root["nodes"][0]["output_shapes"]["value"]["id"], "vec3");
    }

    #[test]
    /// Verify normalization converts legacy `inputs` to edges.
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
    /// Verify normalization adds empty edge arrays when absent.
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
    /// Verify defaults without connections remain after normalization.
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
        assert_eq!(rhs_default["value"]["type"], "float");
        assert_eq!(rhs_default["value"]["data"], 2.0);

        let edges = root["edges"].as_array().expect("edges array");
        assert_eq!(edges.len(), 1, "only the lhs connection becomes an edge");
        let lhs_link = &edges[0];
        assert_eq!(lhs_link["to"]["node_id"], "div");
        assert_eq!(lhs_link["to"]["input"], "lhs");
    }

    #[test]
    /// Verify defaults are extracted from legacy connection objects.
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
        assert_eq!(rhs_default["value"]["type"], "float");
        assert_eq!(rhs_default["value"]["data"], 0.5);
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
    /// Verify legacy `links` fields are rejected during normalization.
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
