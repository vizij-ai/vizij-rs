use serde_json::{json, Map, Value as JsonValue};

/// Normalize Value shorthand used in GraphSpec node params into the full
/// { "type": "...", "data": ... } representation. This mirrors the logic
/// from `vizij-graph-wasm` but returns Rust-friendly Result types.
fn normalize_value_json(value: serde_json::Value) -> serde_json::Value {
    use serde_json::Value as V;
    match value {
        V::Number(n) => json!({ "type": "float", "data": n }),
        V::Bool(b) => json!({ "type": "bool", "data": b }),
        V::String(s) => json!({ "type": "text", "data": s }),
        V::Array(arr) => {
            let all_numbers = arr.iter().all(|x| x.is_number());
            if all_numbers {
                if arr.len() == 2 {
                    json!({ "type": "vec2", "data": arr })
                } else if arr.len() == 3 {
                    json!({ "type": "vec3", "data": arr })
                } else if arr.len() == 4 {
                    json!({ "type": "vec4", "data": arr })
                } else {
                    json!({ "type": "vector", "data": arr })
                }
            } else {
                let data: Vec<JsonValue> = arr.into_iter().map(normalize_value_json).collect();
                json!({ "type": "list", "data": data })
            }
        }
        V::Object(obj) => {
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
                let pos = transform.get("pos").cloned().unwrap_or(JsonValue::Null);
                let rot = transform.get("rot").cloned().unwrap_or(JsonValue::Null);
                let scale = transform.get("scale").cloned().unwrap_or(JsonValue::Null);
                return json!({ "type": "transform", "data": { "pos": pos, "rot": rot, "scale": scale } });
            }
            if let Some(enum_obj) = obj.get("enum").and_then(|x| x.as_object()) {
                let tag = enum_obj
                    .get("tag")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .to_string();
                let payload = enum_obj.get("value").cloned().unwrap_or(JsonValue::Null);
                let normalized_payload = normalize_value_json(payload);
                return json!({ "type": "enum", "data": [tag, normalized_payload] });
            }
            if let Some(record) = obj.get("record").and_then(|x| x.as_object()) {
                let mut data = Map::new();
                for (key, val) in record.iter() {
                    data.insert(key.clone(), normalize_value_json(val.clone()));
                }
                return json!({ "type": "record", "data": JsonValue::Object(data) });
            }
            if let Some(array_items) = obj.get("array").and_then(|x| x.as_array()) {
                let data: Vec<JsonValue> = array_items
                    .iter()
                    .cloned()
                    .map(normalize_value_json)
                    .collect();
                return json!({ "type": "array", "data": data });
            }
            if let Some(list_items) = obj.get("list").and_then(|x| x.as_array()) {
                let data: Vec<JsonValue> = list_items
                    .iter()
                    .cloned()
                    .map(normalize_value_json)
                    .collect();
                return json!({ "type": "list", "data": data });
            }
            if let Some(tuple_items) = obj.get("tuple").and_then(|x| x.as_array()) {
                let data: Vec<JsonValue> = tuple_items
                    .iter()
                    .cloned()
                    .map(normalize_value_json)
                    .collect();
                return json!({ "type": "tuple", "data": data });
            }

            JsonValue::Object(obj)
        }
        other => other,
    }
}

/// Normalize a full GraphSpec JSON string into a serde_json::Value with
/// all shorthand normalized. This mirrors `normalize_graph_spec_value` from node-graph wasm.
pub fn normalize_graph_spec_value(json_str: &str) -> Result<serde_json::Value, String> {
    let mut root: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("graph json parse error: {}", e))?;

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
                node["type"] = serde_json::Value::String(ty);
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
                                *path_val = serde_json::Value::String(s);
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
                            normalized.push(serde_json::Value::from(num));
                        }
                        *sizes_val = serde_json::Value::Array(normalized);
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
                            *shape = serde_json::json!({ "id": id });
                        }
                    }
                }
            }
        }
    }

    Ok(root)
}

/// Convenience: return a JSON string of the normalized spec.
pub fn normalize_graph_spec_json(json_str: &str) -> Result<String, String> {
    match normalize_graph_spec_value(json_str) {
        Ok(v) => {
            serde_json::to_string(&v).map_err(|e| format!("serialize normalized graph: {}", e))
        }
        Err(e) => Err(e),
    }
}
