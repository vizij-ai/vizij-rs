use hashbrown::HashMap;
use vizij_api_core::Value;
use vizij_graph_core::{evaluate_all, GraphRuntime, GraphSpec};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmGraph {
    spec: GraphSpec,
    t: f64,
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
        }
    }

    #[wasm_bindgen]
    pub fn load_graph(&mut self, json_str: &str) -> Result<(), JsValue> {
        // Parse into a serde_json::Value first so we can normalize user-friendly
        // shorthand for `params.value` (e.g. plain numbers) into the adjacently-tagged
        // Value JSON shape expected by the Rust `Value` enum deserializer.
        let mut v: serde_json::Value =
            serde_json::from_str(json_str).map_err(|e| JsValue::from_str(&e.to_string()))?;

        if let Some(nodes) = v.get_mut("nodes").and_then(|n| n.as_array_mut()) {
            for node in nodes.iter_mut() {
                if let Some(params) = node.get_mut("params") {
                    if let Some(val) = params.get_mut("value") {
                        match val {
                            serde_json::Value::Number(n) => {
                                let f = n.as_f64().unwrap_or(0.0);
                                // Convert shorthand number into adjacently-tagged Value JSON expected by serde:
                                // { "type": "Float", "data": f }
                                *val = serde_json::json!({ "type": "Float", "data": f });
                            }
                            serde_json::Value::Bool(b) => {
                                let b2 = *b;
                                *val = serde_json::json!({ "type": "Bool", "data": b2 });
                            }
                            serde_json::Value::String(s) => {
                                let s2 = s.clone();
                                *val = serde_json::json!({ "type": "Text", "data": s2 });
                            }
                            serde_json::Value::Array(arr) => {
                                let all_numbers = arr.iter().all(|x| x.is_number());
                                if arr.len() == 3 && all_numbers {
                                    *val =
                                        serde_json::json!({ "type": "Vec3", "data": arr.clone() });
                                } else if all_numbers {
                                    *val = serde_json::json!({ "type": "Vector", "data": arr.clone() });
                                } else {
                                    // leave complex arrays untouched (could be enums etc)
                                }
                            }
                            _ => {
                                // If object is in legacy ValueJSON form, convert to adjacently-tagged {"type": "...", "data": ...}
                                if let serde_json::Value::Object(obj) = val {
                                    if let Some(f) = obj.get("float").and_then(|x| x.as_f64()) {
                                        *val = serde_json::json!({ "type": "Float", "data": f });
                                    } else if let Some(b) =
                                        obj.get("bool").and_then(|x| x.as_bool())
                                    {
                                        *val = serde_json::json!({ "type": "Bool", "data": b });
                                    } else if let Some(arr) =
                                        obj.get("vec2").and_then(|x| x.as_array())
                                    {
                                        *val = serde_json::json!({ "type": "Vec2", "data": arr.clone() });
                                    } else if let Some(arr) =
                                        obj.get("vec3").and_then(|x| x.as_array())
                                    {
                                        *val = serde_json::json!({ "type": "Vec3", "data": arr.clone() });
                                    } else if let Some(arr) =
                                        obj.get("vec4").and_then(|x| x.as_array())
                                    {
                                        *val = serde_json::json!({ "type": "Vec4", "data": arr.clone() });
                                    } else if let Some(arr) =
                                        obj.get("quat").and_then(|x| x.as_array())
                                    {
                                        *val = serde_json::json!({ "type": "Quat", "data": arr.clone() });
                                    } else if let Some(arr) =
                                        obj.get("color").and_then(|x| x.as_array())
                                    {
                                        *val = serde_json::json!({ "type": "ColorRgba", "data": arr.clone() });
                                    } else if let Some(arr) =
                                        obj.get("vector").and_then(|x| x.as_array())
                                    {
                                        *val = serde_json::json!({ "type": "Vector", "data": arr.clone() });
                                    } else if let Some(s) = obj.get("text").and_then(|x| x.as_str())
                                    {
                                        *val = serde_json::json!({ "type": "Text", "data": s });
                                    } else if let Some(tr) =
                                        obj.get("transform").and_then(|x| x.as_object())
                                    {
                                        if tr.get("pos").is_some()
                                            && tr.get("rot").is_some()
                                            && tr.get("scale").is_some()
                                        {
                                            *val = serde_json::json!({ "type": "Transform", "data": tr.clone() });
                                        }
                                    } else if let Some(en) =
                                        obj.get("enum").and_then(|x| x.as_object())
                                    {
                                        // Expect { tag, value }, where value may itself be a legacy form; normalize common inner types.
                                        if let (Some(tag), Some(value)) = (
                                            en.get("tag").and_then(|x| x.as_str()),
                                            en.get("value"),
                                        ) {
                                            let inner = if let Some(f) =
                                                value.get("float").and_then(|x| x.as_f64())
                                            {
                                                serde_json::json!({ "type": "Float", "data": f })
                                            } else if let Some(b) =
                                                value.get("bool").and_then(|x| x.as_bool())
                                            {
                                                serde_json::json!({ "type": "Bool", "data": b })
                                            } else if let Some(arr) =
                                                value.get("vec3").and_then(|x| x.as_array())
                                            {
                                                serde_json::json!({ "type": "Vec3", "data": arr.clone() })
                                            } else if let Some(arr) =
                                                value.get("vector").and_then(|x| x.as_array())
                                            {
                                                serde_json::json!({ "type": "Vector", "data": arr.clone() })
                                            } else if let Some(s) =
                                                value.get("text").and_then(|x| x.as_str())
                                            {
                                                serde_json::json!({ "type": "Text", "data": s })
                                            } else {
                                                value.clone()
                                            };
                                            *val = serde_json::json!({ "type": "Enum", "data": [tag, inner] });
                                        }
                                    } else {
                                        // already adjacently-tagged or null — leave as-is
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Now deserialize into the typed GraphSpec
        self.spec = serde_json::from_value(v).map_err(|e| JsValue::from_str(&e.to_string()))?;
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
    ///   "nodes": { [nodeId]: { [outputKey]: ValueJSON } },
    ///   "writes": [ { path: string, value: ValueJSON }, ... ]
    /// }
    #[wasm_bindgen]
    pub fn eval_all(&self) -> Result<String, JsValue> {
        // GraphRuntime.t is f32 in core; cast from f64
        let mut rt = GraphRuntime {
            t: self.t as f32,
            outputs: HashMap::new(),
        };
        evaluate_all(&mut rt, &self.spec).map_err(|e| JsValue::from_str(&e))?;

        // Build per-node outputs JSON (for tooling) and collect WriteOps for Output nodes that have a path.
        let mut nodes_map: HashMap<String, serde_json::Value> = HashMap::new();
        let mut writes: Vec<serde_json::Value> = Vec::new();

        for (node_id, outputs) in rt.outputs.iter() {
            let outputs_json: HashMap<String, serde_json::Value> = outputs
                .iter()
                .map(|(key, val)| {
                    let jv = match val {
                        Value::Float(f) => serde_json::json!({ "float": *f }),
                        Value::Bool(b) => serde_json::json!({ "bool": *b }),
                        Value::Vec2(a) => serde_json::json!({ "vec2": [a[0], a[1]] }),
                        Value::Vec3(a) => serde_json::json!({ "vec3": [a[0], a[1], a[2]] }),
                        Value::Vec4(a) => serde_json::json!({ "vec4": [a[0], a[1], a[2], a[3]] }),
                        Value::Quat(a) => serde_json::json!({ "quat": [a[0], a[1], a[2], a[3]] }),
                        Value::ColorRgba(a) => serde_json::json!({ "color": [a[0], a[1], a[2], a[3]] }),
                        Value::Transform { pos, rot, scale } => serde_json::json!({ "transform": { "pos": pos, "rot": rot, "scale": scale } }),
                        Value::Vector(a) => serde_json::json!({ "vector": a }),
                        Value::Enum(tag, boxed) => {
                            let inner = match boxed.as_ref() {
                                Value::Float(f) => serde_json::json!({ "float": *f }),
                                Value::Bool(b) => serde_json::json!({ "bool": *b }),
                                Value::Vec3(a) => serde_json::json!({ "vec3": [a[0], a[1], a[2]] }),
                                Value::Vector(a) => serde_json::json!({ "vector": a }),
                                Value::Text(s) => serde_json::json!({ "text": s }),
                                _ => serde_json::json!(null),
                            };
                            serde_json::json!({ "enum": { "tag": tag, "value": inner } })
                        }
                        Value::Text(s) => serde_json::json!({ "text": s }),
                    };
                    (key.clone(), jv)
                })
                .collect();
            nodes_map.insert(node_id.clone(), serde_json::to_value(outputs_json).unwrap());
        }

        // Build writes by scanning node specs for Output nodes with a path param
        for node in &self.spec.nodes {
            if let Some(path_str) = node.params.path.as_ref() {
                if let Some(outputs) = rt.outputs.get(&node.id) {
                    if let Some(val) = outputs.get("out") {
                        // convert val to ValueJSON same as above
                        let jv = match val {
                            Value::Float(f) => serde_json::json!({ "float": *f }),
                            Value::Bool(b) => serde_json::json!({ "bool": *b }),
                            Value::Vec2(a) => serde_json::json!({ "vec2": [a[0], a[1]] }),
                            Value::Vec3(a) => serde_json::json!({ "vec3": [a[0], a[1], a[2]] }),
                            Value::Vec4(a) => {
                                serde_json::json!({ "vec4": [a[0], a[1], a[2], a[3]] })
                            }
                            Value::Quat(a) => {
                                serde_json::json!({ "quat": [a[0], a[1], a[2], a[3]] })
                            }
                            Value::ColorRgba(a) => {
                                serde_json::json!({ "color": [a[0], a[1], a[2], a[3]] })
                            }
                            Value::Transform { pos, rot, scale } => {
                                serde_json::json!({ "transform": { "pos": pos, "rot": rot, "scale": scale } })
                            }
                            Value::Vector(a) => serde_json::json!({ "vector": a }),
                            Value::Enum(tag, boxed) => {
                                let inner = match boxed.as_ref() {
                                    Value::Float(f) => serde_json::json!({ "float": *f }),
                                    Value::Bool(b) => serde_json::json!({ "bool": *b }),
                                    Value::Vec3(a) => {
                                        serde_json::json!({ "vec3": [a[0], a[1], a[2]] })
                                    }
                                    Value::Vector(a) => serde_json::json!({ "vector": a }),
                                    Value::Text(s) => serde_json::json!({ "text": s }),
                                    _ => serde_json::json!(null),
                                };
                                serde_json::json!({ "enum": { "tag": tag, "value": inner } })
                            }
                            Value::Text(s) => serde_json::json!({ "text": s }),
                        };
                        writes.push(serde_json::json!({ "path": path_str, "value": jv }));
                    }
                }
            }
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
        let v: serde_json::Value =
            serde_json::from_str(json_value).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let val = if let Some(f) = v.get("float").and_then(|x| x.as_f64()) {
            Value::Float(f as f32)
        } else if let Some(b) = v.get("bool").and_then(|x| x.as_bool()) {
            Value::Bool(b)
        } else if let Some(arr) = v.get("vec3").and_then(|x| x.as_array()) {
            let mut a = [0.0f32; 3];
            for (i, slot) in a.iter_mut().enumerate() {
                *slot = arr.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            }
            Value::Vec3(a)
        } else if let Some(arr) = v.get("vector").and_then(|x| x.as_array()) {
            let vec: Vec<f32> = arr
                .iter()
                .map(|x| x.as_f64().unwrap_or(0.0) as f32)
                .collect();
            Value::Vector(vec)
        } else {
            return Err(JsValue::from_str("unsupported value"));
        };
        if let Some(f) = v.get("float").and_then(|x| x.as_f64()) {
            Value::Float(f as f32)
        } else if let Some(b) = v.get("bool").and_then(|x| x.as_bool()) {
            Value::Bool(b)
        } else if let Some(arr) = v.get("vec3").and_then(|x| x.as_array()) {
            let mut a = [0.0f32; 3];
            for (i, slot) in a.iter_mut().enumerate() {
                *slot = arr.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            }
            Value::Vec3(a)
        } else if let Some(arr) = v.get("vector").and_then(|x| x.as_array()) {
            let vec: Vec<f32> = arr
                .iter()
                .map(|x| x.as_f64().unwrap_or(0.0) as f32)
                .collect();
            Value::Vector(vec)
        } else {
            return Err(JsValue::from_str("unsupported value"));
        };
        if let Some(node) = self.spec.nodes.iter_mut().find(|n| n.id == node_id) {
            match key {
                "value" => node.params.value = Some(val),
                "frequency" => {
                    node.params.frequency = Some(if let Value::Float(f) = val { f } else { 0.0 })
                }
                "phase" => {
                    node.params.phase = Some(if let Value::Float(f) = val { f } else { 0.0 })
                }
                "min" => node.params.min = if let Value::Float(f) = val { f } else { 0.0 },
                "max" => node.params.max = if let Value::Float(f) = val { f } else { 0.0 },
                "in_min" => {
                    node.params.in_min = Some(if let Value::Float(f) = val { f } else { 0.0 })
                }
                "in_max" => {
                    node.params.in_max = Some(if let Value::Float(f) = val { f } else { 0.0 })
                }
                "out_min" => {
                    node.params.out_min = Some(if let Value::Float(f) = val { f } else { 0.0 })
                }
                "out_max" => {
                    node.params.out_max = Some(if let Value::Float(f) = val { f } else { 0.0 })
                }
                "x" => node.params.x = Some(if let Value::Float(f) = val { f } else { 0.0 }),
                "y" => node.params.y = Some(if let Value::Float(f) = val { f } else { 0.0 }),
                "z" => node.params.z = Some(if let Value::Float(f) = val { f } else { 0.0 }),
                "bone1" => {
                    node.params.bone1 = Some(if let Value::Float(f) = val { f } else { 0.0 })
                }
                "bone2" => {
                    node.params.bone2 = Some(if let Value::Float(f) = val { f } else { 0.0 })
                }
                "bone3" => {
                    node.params.bone3 = Some(if let Value::Float(f) = val { f } else { 0.0 })
                }
                "index" => {
                    node.params.index = Some(if let Value::Float(f) = val { f } else { 0.0 })
                }
                "sizes" => {
                    if let Value::Vector(vec) = val {
                        node.params.sizes = Some(vec);
                    }
                }
                _ => {}
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
