use hashbrown::HashMap;
use vizij_api_core::{coercion, json, Shape, TypedPath, Value};
use vizij_graph_core::{evaluate_all, GraphRuntime, GraphSpec, PortValue};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn normalize_graph_spec_json(json: &str) -> Result<String, JsValue> {
    json::normalize_graph_spec_json_string(json).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// ABI version for compatibility checks with npm wrappers.
#[wasm_bindgen]
pub fn abi_version() -> u32 {
    2
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use vizij_api_core::json;

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

        let normalized = json::normalize_graph_spec_json(input).expect("normalize");
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

        let normalized = json::normalize_graph_spec_json(input).expect("normalize");
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

    #[test]
    fn abi_version_matches_expected() {
        assert_eq!(super::abi_version(), 2);
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
        let normalized = json::normalize_graph_spec_json(json_str)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
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
        let normalized = json::normalize_value_json_staging(raw);
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
                    let value_json = json::value_to_legacy_json(&port.value);
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
            let jv = json::value_to_legacy_json(&op.value);
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
        let normalized = json::normalize_value_json(raw);
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
