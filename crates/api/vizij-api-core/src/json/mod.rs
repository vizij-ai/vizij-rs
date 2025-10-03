use serde::de::Error as _;
use serde_json::{json, Map, Value as JsonValue};
use thiserror::Error;

use crate::{TypedPath, Value, WriteBatch, WriteOp};

/// Policy describing how purely numeric arrays should be normalized when
/// converting shorthand JSON into the canonical `{ "type": ..., "data": ... }`
/// representation used by `vizij_api_core::Value`.
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

/// Errors produced while normalizing orchestrator graph JSON blobs.
#[derive(Debug, Error)]
pub enum JsonError {
    #[error("graph json parse error: {0}")]
    GraphParse(String),
    #[error("serialize normalized graph: {0}")]
    GraphSerialize(String),
}

/// Normalize shorthand `Value` JSON into the canonical `{ "type": ..., "data": ... }`
/// representation understood by the serde derives on [`Value`]. This helper accepts
/// both shorthand objects such as `{ "vec3": [1, 2, 3] }` and primitive aliases
/// like `1.0` or `[0, 1, 0]`.
pub fn normalize_value_json(value: JsonValue) -> JsonValue {
    normalize_value_json_with_policy(value, NumericArrayPolicy::AutoVectorKinds)
}

/// Variant of [`normalize_value_json`] that mirrors the staging normalizer used by
/// the graph wasm crate. Numeric arrays are always treated as `Vector` unless they
/// are explicitly tagged with a `vec*` alias.
pub fn normalize_value_json_staging(value: JsonValue) -> JsonValue {
    normalize_value_json_with_policy(value, NumericArrayPolicy::AlwaysVector)
}

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

/// Convenience helper that normalizes Value JSON then deserializes it into the
/// strongly typed [`Value`] enum. This keeps JSON shorthands consistent across
/// call-sites (blackboard, wasm wrappers, tests).
pub fn parse_value(value: JsonValue) -> Result<Value, serde_json::Error> {
    let normalized = normalize_value_json(value);
    serde_json::from_value(normalized)
}

/// Convert the legacy shorthand `Value` JSON into a strongly typed [`Value`]
/// while using the staging numeric policy. This mirrors the staging graph wasm
/// normalizer behaviour.
pub fn parse_value_staging(value: JsonValue) -> Result<Value, serde_json::Error> {
    let normalized = normalize_value_json_staging(value);
    serde_json::from_value(normalized)
}

/// Normalize a graph specification JSON value in-place. This mirrors the wasm
/// helpers previously implemented in individual crates.
pub fn normalize_graph_spec_value(root: &mut JsonValue) {
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
        }
    }
}

/// Convenience wrapper that parses a JSON string, normalizes it, and returns
/// the normalized [`serde_json::Value`].
pub fn normalize_graph_spec_json(json_str: &str) -> Result<JsonValue, JsonError> {
    let mut root: JsonValue =
        serde_json::from_str(json_str).map_err(|e| JsonError::GraphParse(e.to_string()))?;
    normalize_graph_spec_value(&mut root);
    Ok(root)
}

/// Convenience wrapper that returns a JSON string with all shorthand normalized.
pub fn normalize_graph_spec_json_string(json_str: &str) -> Result<String, JsonError> {
    let value = normalize_graph_spec_json(json_str)?;
    serde_json::to_string(&value).map_err(|e| JsonError::GraphSerialize(e.to_string()))
}

/// Convert a core [`Value`] into the legacy JSON structure used by existing wasm
/// consumers (objects like `{ "vec3": [...] }`).
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
pub fn writebatch_to_legacy_json(batch: &WriteBatch) -> JsonValue {
    let arr: Vec<JsonValue> = batch.iter().map(writeop_to_legacy_json).collect();
    JsonValue::Array(arr)
}

/// Deserialize a legacy JSON value (e.g. `{ "vec3": [...] }`) into a [`Value`].
/// This helper is primarily intended for wasm consumers that still emit the legacy
/// structure; it normalizes and then deserializes just like [`parse_value`].
pub fn value_from_legacy_json(value: JsonValue) -> Result<Value, serde_json::Error> {
    parse_value(value)
}

/// Deserialize a legacy JSON write batch array back into a strongly typed [`WriteBatch`].
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

/// Helper to create a [`WriteBatch`] from a list of changes expressed as `(TypedPath, Value)`
/// pairs. This is primarily used in tests where ergonomic builders are desirable.
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
    fn normalize_numeric_arrays_auto_vectors() {
        let value = json!([1, 2, 3]);
        let normalized = normalize_value_json(value);
        assert_eq!(normalized["type"], "vec3");
    }

    #[test]
    fn normalize_numeric_arrays_staging() {
        let value = json!([1, 2, 3]);
        let normalized = normalize_value_json_staging(value);
        assert_eq!(normalized["type"], "vector");
    }

    #[test]
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
    fn writebatch_legacy_roundtrip() {
        let tp = TypedPath::parse("robot/Arm/Joint.angle").unwrap();
        let batch = writebatch_from_pairs(vec![(tp, Value::Vec3([1.0, 2.0, 3.0]))]);
        let json = writebatch_to_legacy_json(&batch);
        let parsed = writebatch_from_legacy_json(json).expect("deserialize");
        assert_eq!(batch, parsed);
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
        normalize_graph_spec_value(&mut root);
        assert_eq!(root["nodes"][0]["type"], "node");
        assert_eq!(root["nodes"][0]["params"]["value"]["type"], "float");
        assert_eq!(root["nodes"][0]["output_shapes"]["value"]["id"], "vec3");
    }
}
