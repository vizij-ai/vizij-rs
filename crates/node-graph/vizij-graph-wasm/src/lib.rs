use hashbrown::HashMap;
use vizij_api_core::{coercion, Shape, TypedPath, Value};
use vizij_graph_core::{evaluate_all, GraphRuntime, GraphSpec, PortValue};
use wasm_bindgen::prelude::*;

fn normalize_value_json(value: serde_json::Value) -> serde_json::Value {
    use serde_json::{json, Map, Value as JsonValue};

    match value {
        JsonValue::Number(n) => json!({ "type": "float", "data": n }),
        JsonValue::Bool(b) => json!({ "type": "bool", "data": b }),
        JsonValue::String(s) => json!({ "type": "text", "data": s }),
        JsonValue::Array(arr) => {
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

fn value_to_legacy_json(value: &Value) -> serde_json::Value {
    use serde_json::json;

    match value {
        Value::Float(f) => json!({ "float": *f }),
        Value::Bool(b) => json!({ "bool": *b }),
        Value::Vec2(a) => json!({ "vec2": [a[0], a[1]] }),
        Value::Vec3(a) => json!({ "vec3": [a[0], a[1], a[2]] }),
        Value::Vec4(a) => json!({ "vec4": [a[0], a[1], a[2], a[3]] }),
        Value::Quat(a) => json!({ "quat": [a[0], a[1], a[2], a[3]] }),
        Value::ColorRgba(a) => json!({ "color": [a[0], a[1], a[2], a[3]] }),
        Value::Transform { pos, rot, scale } => {
            json!({ "transform": { "pos": pos, "rot": rot, "scale": scale } })
        }
        Value::Vector(a) => json!({ "vector": a }),
        Value::Enum(tag, boxed) => {
            json!({ "enum": { "tag": tag, "value": value_to_legacy_json(boxed) } })
        }
        Value::Text(s) => json!({ "text": s }),
        Value::Record(map) => {
            let mut obj = serde_json::Map::new();
            for (key, val) in map.iter() {
                obj.insert(key.clone(), value_to_legacy_json(val));
            }
            json!({ "record": serde_json::Value::Object(obj) })
        }
        Value::Array(items) => {
            let data: Vec<_> = items.iter().map(value_to_legacy_json).collect();
            json!({ "array": data })
        }
        Value::List(items) => {
            let data: Vec<_> = items.iter().map(value_to_legacy_json).collect();
            json!({ "list": data })
        }
        Value::Tuple(items) => {
            let data: Vec<_> = items.iter().map(value_to_legacy_json).collect();
            json!({ "tuple": data })
        }
    }
}

fn normalize_value_json_for_staging(value: serde_json::Value) -> serde_json::Value {
    use serde_json::{json, Map, Value as JsonValue};

    match value {
        JsonValue::Number(n) => json!({ "type": "float", "data": n }),
        JsonValue::Bool(b) => json!({ "type": "bool", "data": b }),
        JsonValue::String(s) => json!({ "type": "text", "data": s }),
        JsonValue::Array(arr) => {
            let all_numbers = arr.iter().all(|x| x.is_number());
            if all_numbers {
                // For staging, avoid auto-Vec2/3/4; default numeric arrays to Vector unless explicitly tagged
                json!({ "type": "vector", "data": arr })
            } else {
                let data: Vec<JsonValue> = arr
                    .into_iter()
                    .map(normalize_value_json_for_staging)
                    .collect();
                json!({ "type": "list", "data": data })
            }
        }
        JsonValue::Object(obj) => {
            if obj.contains_key("type") && obj.contains_key("data") {
                return JsonValue::Object(obj);
            }

            if let Some(f) = obj.get("float").and_then(|x| x.as_f64()) {
                return json!({ "type": "float", "data": f });
            }
            if let Some(b) = obj.get("bool").and_then(|x| x.as_bool()) {
                return json!({ "type": "bool", "data": b });
            }
            if let Some(text) = obj.get("text").and_then(|x| x.as_str()) {
                return json!({ "type": "text", "data": text });
            }
            // Honor explicit aliases only (no auto detection)
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
                let normalized_payload = normalize_value_json_for_staging(payload);
                return json!({ "type": "enum", "data": [tag, normalized_payload] });
            }
            if let Some(record) = obj.get("record").and_then(|x| x.as_object()) {
                let mut data = Map::new();
                for (key, val) in record.iter() {
                    data.insert(key.clone(), normalize_value_json_for_staging(val.clone()));
                }
                return json!({ "type": "record", "data": JsonValue::Object(data) });
            }
            if let Some(array_items) = obj.get("array").and_then(|x| x.as_array()) {
                let data: Vec<JsonValue> = array_items
                    .iter()
                    .cloned()
                    .map(normalize_value_json_for_staging)
                    .collect();
                return json!({ "type": "array", "data": data });
            }
            if let Some(list_items) = obj.get("list").and_then(|x| x.as_array()) {
                let data: Vec<JsonValue> = list_items
                    .iter()
                    .cloned()
                    .map(normalize_value_json_for_staging)
                    .collect();
                return json!({ "type": "list", "data": data });
            }
            if let Some(tuple_items) = obj.get("tuple").and_then(|x| x.as_array()) {
                let data: Vec<JsonValue> = tuple_items
                    .iter()
                    .cloned()
                    .map(normalize_value_json_for_staging)
                    .collect();
                return json!({ "type": "tuple", "data": data });
            }

            JsonValue::Object(obj)
        }
        other => other,
    }
}

fn normalize_graph_spec_value(json_str: &str) -> Result<serde_json::Value, JsValue> {
    let mut root: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| JsValue::from_str(&e.to_string()))?;

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

#[wasm_bindgen]
pub fn normalize_graph_spec_json(json: &str) -> Result<String, JsValue> {
    let normalized = normalize_graph_spec_value(json)?;
    serde_json::to_string(&normalized).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn it_should_normalize_value_and_path_shorthand() {
        let input = r#"{
            "nodes": [
                {
                    "id": "const",
                    "type": "constant",
                    "params": { "value": 1 },
                    "inputs": {},
                    "output_shapes": {}
                },
                {
                    "id": "out",
                    "type": "output",
                    "params": { "path": { "path": "robot/Arm.Joint" } },
                    "inputs": {},
                    "output_shapes": {}
                }
            ]
        }"#;

        let normalized = normalize_graph_spec_value(input).expect("normalize");
        let nodes = normalized["nodes"].as_array().expect("nodes");
        let constant = nodes[0]["params"]["value"].clone();
        assert_eq!(constant, serde_json::json!({ "type": "float", "data": 1 }));

        let path = nodes[1]["params"]["path"].as_str().expect("path");
        assert_eq!(path, "robot/Arm.Joint");
    }

    #[test]
    fn it_should_normalize_numeric_sizes() {
        let input = r#"{
            "nodes": [
                {
                    "id": "split",
                    "type": "split",
                    "params": { "sizes": ["2", 3, 4.5] },
                    "inputs": {},
                    "output_shapes": {}
                }
            ]
        }"#;

        let normalized = normalize_graph_spec_value(input).expect("normalize");
        let sizes = normalized["nodes"][0]["params"]["sizes"]
            .as_array()
            .unwrap();
        let values: Vec<f64> = sizes.iter().map(|v| v.as_f64().unwrap()).collect();
        assert_eq!(values, vec![2.0, 3.0, 4.5]);
    }

    #[test]
    fn registry_exposes_urdf_nodes() {
        let raw = get_node_schemas_json();
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("valid registry json");
        let nodes = parsed
            .get("nodes")
            .and_then(|v| v.as_array())
            .expect("registry contains nodes array");
        let present: HashSet<String> = nodes
            .iter()
            .filter_map(|entry| entry.get("type_id").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .collect();
        for expected in ["urdfikposition", "urdfikpose", "urdffk"] {
            assert!(
                present.contains(expected),
                "registry missing {expected}; available: {:?}",
                present
            );
        }
    }
}

/// Holds a persistent runtime so transition nodes can accumulate state across
/// evaluations without copying it through the wasm boundary each frame.
#[wasm_bindgen]
pub struct WasmGraph {
    spec: GraphSpec,
    t: f64,
    runtime: GraphRuntime,
}

impl Default for WasmGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WasmGraph {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmGraph {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();
        WasmGraph {
            spec: GraphSpec { nodes: vec![] },
            t: 0.0,
            runtime: GraphRuntime::default(),
        }
    }

    #[wasm_bindgen]
    pub fn load_graph(&mut self, json_str: &str) -> Result<(), JsValue> {
        let normalized = normalize_graph_spec_value(json_str)?;
        // Now deserialize into the typed GraphSpec
        self.spec =
            serde_json::from_value(normalized).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.runtime = GraphRuntime::default();
        self.runtime.t = self.t as f32;
        self.runtime.dt = 0.0;
        Ok(())
    }

    #[wasm_bindgen]
    pub fn stage_input(
        &mut self,
        path: &str,
        value_json: &str,
        declared_shape_json: Option<String>,
    ) -> Result<(), JsValue> {
        let typed_path = TypedPath::parse(path)
            .map_err(|e| JsValue::from_str(&format!("invalid path: {}", e)))?;
        let raw: serde_json::Value =
            serde_json::from_str(value_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let normalized = normalize_value_json_for_staging(raw);
        let value: Value =
            serde_json::from_value(normalized).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let declared: Option<Shape> = match declared_shape_json {
            Some(s) => {
                if s.trim().is_empty() {
                    None
                } else {
                    Some(serde_json::from_str(&s).map_err(|e| JsValue::from_str(&e.to_string()))?)
                }
            }
            None => None,
        };
        self.runtime.set_input(typed_path, value, declared);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn set_time(&mut self, t: f64) {
        self.t = t;
    }

    #[wasm_bindgen]
    pub fn step(&mut self, dt: f64) {
        self.t += dt;
    }

    /// Evaluate the entire graph and return all outputs as JSON.
    /// Returned JSON shape:
    /// {
    ///   "nodes": { [nodeId]: { [outputKey]: { "value": ValueJSON, "shape": ShapeJSON } } },
    ///   "writes": [ { "path": string, "value": ValueJSON, "shape": ShapeJSON }, ... ]
    /// }
    #[wasm_bindgen]
    pub fn eval_all(&mut self) -> Result<String, JsValue> {
        let new_time = self.t as f32;
        let mut dt = new_time - self.runtime.t;
        if !dt.is_finite() || dt < 0.0 {
            dt = 0.0;
        }
        self.runtime.dt = dt;
        self.runtime.t = new_time;
        evaluate_all(&mut self.runtime, &self.spec).map_err(|e| JsValue::from_str(&e))?;

        // Build per-node outputs JSON (for tooling) and collect WriteOps for Output nodes that have a path.
        let mut nodes_map: HashMap<String, serde_json::Value> = HashMap::new();
        let mut writes: Vec<serde_json::Value> = Vec::new();

        for (node_id, outputs) in self.runtime.outputs.iter() {
            let outputs_json: HashMap<String, serde_json::Value> = outputs
                .iter()
                .map(|(key, port)| {
                    let value_json = value_to_legacy_json(&port.value);
                    let shape_json = serde_json::to_value(&port.shape).unwrap();
                    (
                        key.clone(),
                        serde_json::json!({ "value": value_json, "shape": shape_json }),
                    )
                })
                .collect();
            nodes_map.insert(node_id.clone(), serde_json::to_value(outputs_json).unwrap());
        }

        for op in self.runtime.writes.iter() {
            let jv = value_to_legacy_json(&op.value);
            let shape_json = if let Some(shape) = &op.shape {
                serde_json::to_value(shape).unwrap()
            } else {
                let inferred_shape = PortValue::new(op.value.clone()).shape;
                serde_json::to_value(&inferred_shape).unwrap()
            };
            writes.push(serde_json::json!({
                "path": op.path.to_string(),
                "value": jv,
                "shape": shape_json
            }));
        }

        let out_obj = serde_json::json!({
            "nodes": nodes_map,
            "writes": writes,
        });

        Ok(serde_json::to_string(&out_obj).unwrap())
    }

    /// Set a param on a node (e.g., key="value" with float/bool/vec3 JSON)
    #[wasm_bindgen]
    pub fn set_param(&mut self, node_id: &str, key: &str, json_value: &str) -> Result<(), JsValue> {
        let raw: serde_json::Value =
            serde_json::from_str(json_value).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let normalized = normalize_value_json(raw);
        let val: Value =
            serde_json::from_value(normalized).map_err(|e| JsValue::from_str(&e.to_string()))?;

        fn expect_float(node_id: &str, key: &str, v: &Value) -> Result<f32, JsValue> {
            if let Value::Float(f) = v {
                Ok(*f)
            } else {
                Err(JsValue::from_str(&format!(
                    "set_param: node '{}' key '{}' expects Float",
                    node_id, key
                )))
            }
        }
        fn expect_text<'a>(node_id: &str, key: &str, v: &'a Value) -> Result<&'a str, JsValue> {
            if let Value::Text(s) = v {
                Ok(s.as_str())
            } else {
                Err(JsValue::from_str(&format!(
                    "set_param: node '{}' key '{}' expects Text",
                    node_id, key
                )))
            }
        }
        fn parse_u32(node_id: &str, key: &str, v: &Value) -> Result<u32, JsValue> {
            let f = expect_float(node_id, key, v)?;
            if f.is_finite() && f >= 0.0 {
                Ok(f.floor() as u32)
            } else {
                Err(JsValue::from_str(&format!(
                    "set_param: node '{}' key '{}' expects non-negative finite Float",
                    node_id, key
                )))
            }
        }
        fn parse_pairs(node_id: &str, key: &str, v: &Value) -> Result<Vec<(String, f32)>, JsValue> {
            let items: Vec<Value> = match v {
                Value::List(xs) => xs.clone(),
                Value::Array(xs) => xs.clone(),
                _ => {
                    return Err(JsValue::from_str(&format!(
                        "set_param: node '{}' key '{}' expects Array/List of [Text, Float] tuples",
                        node_id, key
                    )))
                }
            };
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::Tuple(elems) if elems.len() >= 2 => {
                        let name = match &elems[0] {
                            Value::Text(s) => s.clone(),
                            _ => {
                                return Err(JsValue::from_str(&format!(
                                    "set_param: node '{}' key '{}' tuple[0] expects Text",
                                    node_id, key
                                )))
                            }
                        };
                        let val = match &elems[1] {
                            Value::Float(f) => *f,
                            _ => {
                                return Err(JsValue::from_str(&format!(
                                    "set_param: node '{}' key '{}' tuple[1] expects Float",
                                    node_id, key
                                )))
                            }
                        };
                        out.push((name, val));
                    }
                    _ => {
                        return Err(JsValue::from_str(&format!(
                        "set_param: node '{}' key '{}' expects Array/List of [Text, Float] tuples",
                        node_id, key
                    )))
                    }
                }
            }
            Ok(out)
        }

        fn parse_string_list(node_id: &str, key: &str, v: &Value) -> Result<Vec<String>, JsValue> {
            match v {
                Value::List(items) | Value::Array(items) | Value::Tuple(items) => {
                    let mut out = Vec::with_capacity(items.len());
                    for item in items {
                        if let Value::Text(s) = item {
                            out.push(s.clone());
                        } else {
                            return Err(JsValue::from_str(&format!(
                                "set_param: node '{}' key '{}' expects list of Text",
                                node_id, key
                            )));
                        }
                    }
                    Ok(out)
                }
                Value::Text(s) => Ok(vec![s.clone()]),
                _ => Err(JsValue::from_str(&format!(
                    "set_param: node '{}' key '{}' expects Text or list of Text",
                    node_id, key
                ))),
            }
        }

        if let Some(node) = self.spec.nodes.iter_mut().find(|n| n.id == node_id) {
            match key {
                "value" => {
                    node.params.value = Some(val);
                }

                // Scalars / options (strict float)
                "frequency" => node.params.frequency = Some(expect_float(node_id, key, &val)?),
                "phase" => node.params.phase = Some(expect_float(node_id, key, &val)?),
                "min" => node.params.min = expect_float(node_id, key, &val)?,
                "max" => node.params.max = expect_float(node_id, key, &val)?,
                "in_min" => node.params.in_min = Some(expect_float(node_id, key, &val)?),
                "in_max" => node.params.in_max = Some(expect_float(node_id, key, &val)?),
                "out_min" => node.params.out_min = Some(expect_float(node_id, key, &val)?),
                "out_max" => node.params.out_max = Some(expect_float(node_id, key, &val)?),
                "x" => node.params.x = Some(expect_float(node_id, key, &val)?),
                "y" => node.params.y = Some(expect_float(node_id, key, &val)?),
                "z" => node.params.z = Some(expect_float(node_id, key, &val)?),
                "bone1" => node.params.bone1 = Some(expect_float(node_id, key, &val)?),
                "bone2" => node.params.bone2 = Some(expect_float(node_id, key, &val)?),
                "bone3" => node.params.bone3 = Some(expect_float(node_id, key, &val)?),
                "index" => node.params.index = Some(expect_float(node_id, key, &val)?),
                "stiffness" => node.params.stiffness = Some(expect_float(node_id, key, &val)?),
                "damping" => node.params.damping = Some(expect_float(node_id, key, &val)?),
                "mass" => node.params.mass = Some(expect_float(node_id, key, &val)?),
                "half_life" => node.params.half_life = Some(expect_float(node_id, key, &val)?),
                "max_rate" => node.params.max_rate = Some(expect_float(node_id, key, &val)?),

                // Vectors / numeric lists
                "sizes" => {
                    node.params.sizes = Some(coercion::to_vector(&val));
                }

                // Paths
                "path" => {
                    let s = expect_text(node_id, key, &val)?;
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        node.params.path = None;
                    } else {
                        let parsed =
                            TypedPath::parse(trimmed).map_err(|e| JsValue::from_str(&e))?;
                        node.params.path = Some(parsed);
                    }
                }

                // URDF / IK configuration
                "urdf_xml" => {
                    node.params.urdf_xml = Some(expect_text(node_id, key, &val)?.to_string());
                }
                "root_link" => {
                    node.params.root_link = Some(expect_text(node_id, key, &val)?.to_string());
                }
                "tip_link" => {
                    node.params.tip_link = Some(expect_text(node_id, key, &val)?.to_string());
                }
                "seed" => {
                    node.params.seed = Some(coercion::to_vector(&val));
                }
                "weights" => {
                    node.params.weights = Some(coercion::to_vector(&val));
                }
                "max_iters" => {
                    node.params.max_iters = Some(parse_u32(node_id, key, &val)?);
                }
                "tol_pos" => {
                    node.params.tol_pos = Some(expect_float(node_id, key, &val)?);
                }
                "tol_rot" => {
                    node.params.tol_rot = Some(expect_float(node_id, key, &val)?);
                }
                "joint_defaults" => {
                    node.params.joint_defaults = Some(parse_pairs(node_id, key, &val)?);
                }
                "case_labels" => {
                    node.params.case_labels = Some(parse_string_list(node_id, key, &val)?);
                }

                _ => {
                    return Err(JsValue::from_str(&format!(
                        "set_param: node '{}' unknown key '{}'",
                        node_id, key
                    )))
                }
            }
            Ok(())
        } else {
            Err(JsValue::from_str("unknown node"))
        }
    }
}

/// Expose the node schema registry as JSON for tooling/UI.
#[wasm_bindgen]
pub fn get_node_schemas_json() -> String {
    let reg = vizij_graph_core::registry();
    serde_json::to_string(&reg).unwrap()
}
