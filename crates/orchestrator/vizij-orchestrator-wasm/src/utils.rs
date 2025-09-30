use serde_json::json;
use serde_json::Value as JsonValue;

use vizij_api_core::{Value as ApiValue, WriteBatch, WriteOp};

/// Convert a core `Value` into the "legacy" JSON shape used by the wasm graph/animation outputs.
///
/// This mirrors the logic in `vizij-graph-wasm` so tooling that expects shapes like
/// `{ "vec3": [...] }`, `{ "float": 1.0 }`, `{ "text": "..." }`, or the object forms
/// `{ "type": "...", "data": ... }` continues to work.
pub fn value_to_legacy_json(value: &ApiValue) -> JsonValue {
    match value {
        ApiValue::Float(f) => json!({ "float": *f }),
        ApiValue::Bool(b) => json!({ "bool": *b }),
        ApiValue::Vec2(a) => json!({ "vec2": [a[0], a[1]] }),
        ApiValue::Vec3(a) => json!({ "vec3": [a[0], a[1], a[2]] }),
        ApiValue::Vec4(a) => json!({ "vec4": [a[0], a[1], a[2], a[3]] }),
        ApiValue::Quat(a) => json!({ "quat": [a[0], a[1], a[2], a[3]] }),
        ApiValue::ColorRgba(a) => json!({ "color": [a[0], a[1], a[2], a[3]] }),
        ApiValue::Transform { pos, rot, scale } => {
            json!({ "transform": { "pos": pos, "rot": rot, "scale": scale } })
        }
        ApiValue::Vector(a) => json!({ "vector": a }),
        ApiValue::Enum(tag, boxed) => {
            json!({ "enum": { "tag": tag, "value": value_to_legacy_json(boxed) } })
        }
        ApiValue::Text(s) => json!({ "text": s }),
        ApiValue::Record(map) => {
            let mut obj = serde_json::Map::new();
            for (key, val) in map.iter() {
                obj.insert(key.clone(), value_to_legacy_json(val));
            }
            json!({ "record": JsonValue::Object(obj) })
        }
        ApiValue::Array(items) => {
            let data: Vec<JsonValue> = items.iter().map(value_to_legacy_json).collect();
            json!({ "array": data })
        }
        ApiValue::List(items) => {
            let data: Vec<JsonValue> = items.iter().map(value_to_legacy_json).collect();
            json!({ "list": data })
        }
        ApiValue::Tuple(items) => {
            let data: Vec<JsonValue> = items.iter().map(value_to_legacy_json).collect();
            json!({ "tuple": data })
        }
    }
}

/// Serialize a WriteBatch into a JSON array of objects:
/// [ { "path": "<str>", "value": <legacy Value JSON>, "shape": <shape JSON?> }, ... ]
/// This intentionally mirrors the `WriteBatch` serializer shape but uses the legacy
/// value representation for compatibility with existing tooling.
pub fn writebatch_to_legacy_json(batch: &WriteBatch) -> JsonValue {
    let mut arr: Vec<JsonValue> = Vec::new();
    for op in batch.iter() {
        let value_json = value_to_legacy_json(&op.value);
        let mut map = serde_json::Map::new();
        map.insert("path".to_string(), JsonValue::String(op.path.to_string()));
        map.insert("value".to_string(), value_json);
        if let Some(shape) = &op.shape {
            map.insert(
                "shape".to_string(),
                serde_json::to_value(shape).unwrap_or(JsonValue::Null),
            );
        }
        arr.push(JsonValue::Object(map));
    }
    JsonValue::Array(arr)
}

/// Convert a single WriteOp into a legacy JSON object.
pub fn writeop_to_legacy_json(op: &WriteOp) -> JsonValue {
    let mut map = serde_json::Map::new();
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
