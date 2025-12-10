use hashbrown::HashMap;
use js_sys::{Float32Array, Uint32Array, JSON};
use serde_wasm_bindgen as swb;
use vizij_api_core::shape::ShapeId;
use vizij_api_core::{coercion, json, Shape, TypedPath, Value};
use vizij_graph_core::types::RoundMode;
use vizij_graph_core::{evaluate_all, evaluate_all_cached, GraphRuntime, GraphSpec, PortValue};
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

    #[test]
    fn set_param_invalidates_plan_cache_for_structural_changes() {
        let mut graph = WasmGraph::new();
        let spec = r#"{
            "nodes": [
                { "id": "vec", "type": "constant", "params": { "value": [1, 2, 3, 4] }, "inputs": {}, "output_shapes": {} },
                { "id": "split", "type": "split", "params": { "sizes": [2, 2] }, "inputs": {}, "output_shapes": {} }
            ],
            "edges": [
                { "from": { "node_id": "vec", "output": "out" }, "to": { "node_id": "split", "input": "in" } }
            ]
        }"#;

        graph.load_graph(spec).expect("graph loads");
        graph.eval_all().expect("initial eval");

        assert!(graph.plan_ready);
        let split_idx = *graph
            .runtime
            .plan
            .node_index
            .get("split")
            .expect("split present");
        let initial_slots = graph.runtime.plan.layouts[split_idx].outputs.slots.len();

        graph
            .set_param("split", "sizes", "[1, 1, 1, 1]")
            .expect("set_param succeeds");

        assert!(
            !graph.plan_ready,
            "plan cache should be invalidated after structural param change"
        );
        assert!(
            graph.runtime.plan.layouts.is_empty(),
            "plan cache should be cleared"
        );

        graph.eval_all().expect("re-eval rebuilds plan");
        assert!(graph.plan_ready);

        let rebuilt_idx = *graph
            .runtime
            .plan
            .node_index
            .get("split")
            .expect("split present");
        let rebuilt_slots = graph.runtime.plan.layouts[rebuilt_idx].outputs.slots.len();

        assert!(
            rebuilt_slots > initial_slots,
            "plan rebuild should reflect new Split arity"
        );
    }
}

/// Holds a persistent runtime so transition nodes can accumulate state across
/// evaluations without copying it through the wasm boundary each frame.
#[wasm_bindgen]
pub struct WasmGraph {
    spec: GraphSpec,
    t: f64,
    runtime: GraphRuntime,
    input_paths: Vec<TypedPath>,
    input_slots: Vec<Option<SlotStaging>>,
    plan_ready: bool,
    output_version: u64,
    last_outputs: HashMap<String, HashMap<String, PortValue>>,
    last_outputs_version: u64,
    input_last_values: HashMap<usize, Value>,
    input_last_shapes: HashMap<usize, Option<Shape>>,
    input_touched: HashMap<usize, u64>,
}

#[derive(Clone)]
struct SlotStaging {
    path_idx: u32,
    declared: Option<Shape>,
}

fn parse_shape_json(declared_shape_json: Option<String>) -> Result<Option<Shape>, JsValue> {
    match declared_shape_json {
        Some(s) => {
            if s.trim().is_empty() {
                Ok(None)
            } else {
                serde_json::from_str(&s)
                    .map(Some)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
        }
        None => Ok(None),
    }
}

fn parse_shape_js(declared: &JsValue) -> Result<Option<Shape>, JsValue> {
    if declared.is_undefined() || declared.is_null() {
        Ok(None)
    } else {
        swb::from_value(declared.clone())
            .map(Some)
            .map_err(|e| JsValue::from_str(&format!("invalid declared shape: {}", e)))
    }
}

fn parse_value_js(
    value: JsValue,
    normalize: fn(serde_json::Value) -> serde_json::Value,
    ctx: &str,
) -> Result<Value, JsValue> {
    let raw: serde_json::Value = swb::from_value(value)
        .map_err(|e| JsValue::from_str(&format!("{ctx} parse error: {e}")))?;
    let normalized = normalize(raw);
    serde_json::from_value(normalized)
        .map_err(|e| JsValue::from_str(&format!("{ctx} convert error: {e}")))
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
            spec: GraphSpec {
                nodes: vec![],
                ..Default::default()
            },
            t: 0.0,
            runtime: GraphRuntime::default(),
            input_paths: Vec::new(),
            input_slots: Vec::new(),
            plan_ready: false,
            output_version: 0,
            last_outputs: HashMap::new(),
            last_outputs_version: 0,
            input_last_values: HashMap::new(),
            input_last_shapes: HashMap::new(),
            input_touched: HashMap::new(),
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
        self.input_paths.clear();
        self.input_slots.clear();
        self.plan_ready = false;
        self.output_version = 0;
        self.last_outputs.clear();
        self.last_outputs_version = 0;
        self.input_last_values.clear();
        self.input_last_shapes.clear();
        self.input_touched.clear();
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
        let declared = parse_shape_json(declared_shape_json)?;
        // Path-based staging bypasses slot cache; no diffing here.
        self.runtime.set_input(typed_path, value, declared);
        Ok(())
    }

    #[wasm_bindgen(js_name = "stage_input_value")]
    pub fn stage_input_value(
        &mut self,
        path: &str,
        value: JsValue,
        declared_shape: JsValue,
    ) -> Result<(), JsValue> {
        let typed_path = TypedPath::parse(path)
            .map_err(|e| JsValue::from_str(&format!("invalid path: {}", e)))?;
        let value = parse_value_js(value, json::normalize_value_json_staging, "stage_input")?;
        let declared = parse_shape_js(&declared_shape)?;
        // Path-based staging bypasses slot cache; no diffing here.
        self.runtime.set_input(typed_path, value, declared);
        Ok(())
    }

    /// Clear a staged input by path, removing it from caches and staged inputs.
    #[wasm_bindgen(js_name = "clear_input_path")]
    pub fn clear_input_path(&mut self, path: &str) -> Result<(), JsValue> {
        let typed_path = TypedPath::parse(path)
            .map_err(|e| JsValue::from_str(&format!("invalid path: {}", e)))?;
        self.runtime.staged_inputs.remove(&typed_path);
        // Drop slot caches if this path was registered.
        if let Some((slot_idx, _)) = self
            .input_paths
            .iter()
            .enumerate()
            .find(|(_, p)| **p == typed_path)
        {
            self.input_last_values.remove(&slot_idx);
            self.input_last_shapes.remove(&slot_idx);
            self.input_touched.remove(&slot_idx);
        }
        Ok(())
    }

    /// Drop the cached execution plan and delta snapshot so the next eval rebuilds layouts.
    fn invalidate_plan_cache(&mut self) {
        self.plan_ready = false;
        self.runtime.plan = Default::default();
        self.last_outputs.clear();
        self.last_outputs_version = 0;
    }

    fn eval_internal(&mut self) -> Result<(), JsValue> {
        let res = if self.plan_ready {
            evaluate_all_cached(&mut self.runtime, &self.spec)
        } else {
            evaluate_all(&mut self.runtime, &self.spec)
        };
        match res {
            Ok(_) => {
                self.plan_ready = true;
                self.output_version = self.output_version.saturating_add(1);
                Ok(())
            }
            Err(e) => {
                self.plan_ready = false;
                Err(JsValue::from_str(&e))
            }
        }
    }

    /// Restage cached slots so the core sees fresh inputs each frame while JS can skip resending.
    fn restage_cached_inputs(&mut self) -> Result<(), JsValue> {
        let next_epoch = self.runtime.input_epoch.saturating_add(1);
        for (slot_idx, value) in self.input_last_values.iter() {
            let tp = self
                .input_paths
                .get(*slot_idx)
                .ok_or_else(|| {
                    JsValue::from_str("restage_cached_inputs: path index out of bounds")
                })?
                .clone();
            let declared = self
                .input_last_shapes
                .get(slot_idx)
                .cloned()
                .unwrap_or(None);
            self.runtime.set_input(tp, value.clone(), declared);
            self.input_touched.insert(*slot_idx, next_epoch);
        }
        Ok(())
    }

    fn stage_cached(
        &mut self,
        slot_idx: usize,
        tp: TypedPath,
        value: Value,
        declared: Option<Shape>,
    ) -> Result<(), JsValue> {
        let next_epoch = self.runtime.input_epoch.saturating_add(1);
        if let Some(prev) = self.input_last_values.get(&slot_idx) {
            let same_value = *prev == value;
            let same_shape = self
                .input_last_shapes
                .get(&slot_idx)
                .map(|s| s == &declared)
                .unwrap_or(false);
            if same_value && same_shape {
                self.input_touched.insert(slot_idx, next_epoch);
                return Ok(());
            }
        }
        self.runtime.set_input(tp, value.clone(), declared.clone());
        self.input_last_values.insert(slot_idx, value);
        self.input_last_shapes.insert(slot_idx, declared);
        self.input_touched.insert(slot_idx, next_epoch);
        Ok(())
    }

    fn snapshot_outputs(&self) -> HashMap<String, HashMap<String, PortValue>> {
        self.runtime
            .outputs
            .iter()
            .map(|(id, ports)| {
                let cloned = ports
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<HashMap<_, _>>();
                (id.clone(), cloned)
            })
            .collect()
    }

    fn serialize_full(&mut self) -> serde_json::Value {
        let snapshot = self.snapshot_outputs();
        self.last_outputs = snapshot.clone();
        self.last_outputs_version = self.output_version;
        self.serialize_full_from_snapshot(&snapshot)
    }

    fn serialize_full_from_snapshot(
        &self,
        snapshot: &HashMap<String, HashMap<String, PortValue>>,
    ) -> serde_json::Value {
        let mut nodes_map: HashMap<String, serde_json::Value> = HashMap::new();
        for (node_id, outputs) in snapshot.iter() {
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

        let mut writes: Vec<serde_json::Value> = Vec::new();
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

        serde_json::json!({
            "version": self.output_version,
            "nodes": nodes_map,
            "writes": writes,
        })
    }

    fn serialize_delta(&mut self, since: u64) -> serde_json::Value {
        if since >= self.output_version {
            let snapshot = self.snapshot_outputs();
            self.last_outputs = snapshot;
            self.last_outputs_version = self.output_version;
            return serde_json::json!({
                "version": self.output_version,
                "nodes": {},
                "writes": [],
                "full": false,
            });
        }

        // If the caller's baseline does not match our cached snapshot, resync by returning
        // a full snapshot for the current version.
        if since != self.last_outputs_version {
            let snapshot = self.snapshot_outputs();
            let full = self.serialize_full_from_snapshot(&snapshot);
            self.last_outputs = snapshot;
            self.last_outputs_version = self.output_version;
            let mut obj = full;
            obj.as_object_mut()
                .expect("full snapshot is an object")
                .insert("full".to_string(), serde_json::json!(true));
            return obj;
        }

        let mut delta_nodes: HashMap<String, serde_json::Value> = HashMap::new();

        for (node_id, outputs) in self.runtime.outputs.iter() {
            let mut changed_ports: HashMap<String, serde_json::Value> = HashMap::new();
            let prev_node = self.last_outputs.get(node_id);
            for (key, port) in outputs {
                let changed = match prev_node.and_then(|m| m.get(key)) {
                    Some(prev) => prev.value != port.value || prev.shape != port.shape,
                    None => true,
                };
                if changed {
                    let value_json = json::value_to_legacy_json(&port.value);
                    let shape_json = serde_json::to_value(&port.shape).unwrap();
                    changed_ports.insert(
                        key.clone(),
                        serde_json::json!({ "value": value_json, "shape": shape_json }),
                    );
                }
            }

            if let Some(prev) = prev_node {
                for key in prev.keys() {
                    if !outputs.contains_key(key) {
                        changed_ports.insert(
                            key.clone(),
                            serde_json::json!({ "value": serde_json::Value::Null, "shape": serde_json::Value::Null }),
                        );
                    }
                }
            }

            if !changed_ports.is_empty() {
                delta_nodes.insert(
                    node_id.clone(),
                    serde_json::to_value(changed_ports).unwrap(),
                );
            }
        }

        let mut writes: Vec<serde_json::Value> = Vec::new();
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

        self.last_outputs = self.snapshot_outputs();
        self.last_outputs_version = self.output_version;

        serde_json::json!({
            "version": self.output_version,
            "nodes": delta_nodes,
            "writes": writes,
            "full": false,
        })
    }

    #[wasm_bindgen]
    pub fn set_time(&mut self, t: f64) {
        self.t = t;
    }

    #[wasm_bindgen]
    pub fn step(&mut self, dt: f64) {
        self.t += dt;
    }

    /// Stage a float32 vector without JSON, using a shared buffer view.
    #[wasm_bindgen(js_name = "stage_input_f32")]
    pub fn stage_input_f32(&mut self, path: &str, data: &Float32Array) -> Result<(), JsValue> {
        let typed_path = TypedPath::parse(path)
            .map_err(|e| JsValue::from_str(&format!("invalid path: {}", e)))?;
        let mut buf = vec![0.0f32; data.length() as usize];
        data.copy_to(&mut buf);
        let value = Value::Vector(buf);
        let declared = Some(Shape::new(ShapeId::Vector { len: None }));
        // Path-based staging bypasses slot cache; no diffing here.
        self.runtime.set_input(typed_path, value, declared);
        Ok(())
    }

    /// Batch-stage many scalar/vector inputs in one call (paths[i] -> values[i]).
    #[wasm_bindgen(js_name = "stage_inputs_batch")]
    pub fn stage_inputs_batch(
        &mut self,
        paths: JsValue,
        values: &Float32Array,
    ) -> Result<(), JsValue> {
        let paths: Vec<String> = swb::from_value(paths)
            .map_err(|e| JsValue::from_str(&format!("stage_inputs_batch paths: {}", e)))?;
        if paths.len() != values.length() as usize {
            return Err(JsValue::from_str(
                "stage_inputs_batch: paths and values length mismatch",
            ));
        }
        for (i, path) in paths.iter().enumerate() {
            let tp = TypedPath::parse(path)
                .map_err(|e| JsValue::from_str(&format!("invalid path '{}': {}", path, e)))?;
            let val = values.get_index(i as u32);
            let value = Value::Float(val);
            let declared = Some(Shape::new(ShapeId::Scalar));
            // Batch by path bypasses slot cache; no diffing here.
            self.runtime.set_input(tp, value, declared);
        }
        Ok(())
    }

    /// Register paths once and reuse their indices for faster staging.
    #[wasm_bindgen(js_name = "register_input_paths")]
    pub fn register_input_paths(&mut self, paths: JsValue) -> Result<Uint32Array, JsValue> {
        let new_paths: Vec<String> = swb::from_value(paths)
            .map_err(|e| JsValue::from_str(&format!("register_input_paths: {}", e)))?;
        let mut indices = Vec::with_capacity(new_paths.len());
        for p in new_paths {
            let tp = TypedPath::parse(&p)
                .map_err(|e| JsValue::from_str(&format!("invalid path '{}': {}", p, e)))?;
            let idx = self.input_paths.len() as u32;
            self.input_paths.push(tp);
            self.input_slots.push(None);
            indices.push(idx);
        }
        Ok(Uint32Array::from(indices.as_slice()))
    }

    /// Stage inputs by index using previously registered paths.
    #[wasm_bindgen(js_name = "stage_inputs_indices")]
    pub fn stage_inputs_indices(
        &mut self,
        indices: &Uint32Array,
        values: &Float32Array,
    ) -> Result<(), JsValue> {
        let len = indices.length();
        if len != values.length() {
            return Err(JsValue::from_str(
                "stage_inputs_indices: indices and values length mismatch",
            ));
        }
        let mut idx_buf = vec![0u32; len as usize];
        indices.copy_to(&mut idx_buf);
        for (i, idx) in idx_buf.iter().enumerate() {
            let tp = self
                .input_paths
                .get(*idx as usize)
                .ok_or_else(|| JsValue::from_str("stage_inputs_indices: index out of bounds"))?;
            let val = values.get_index(i as u32);
            let value = Value::Float(val);
            let declared = Some(Shape::new(ShapeId::Scalar));
            // Use path index as slot key for caching.
            self.stage_cached(*idx as usize, tp.clone(), value, declared)?;
        }
        Ok(())
    }

    /// Pre-allocate slots with declared shapes to enable slot staging.
    #[wasm_bindgen(js_name = "prepare_input_slots")]
    pub fn prepare_input_slots(
        &mut self,
        indices: &Uint32Array,
        declared: JsValue,
    ) -> Result<(), JsValue> {
        let decls: Vec<Option<Shape>> = swb::from_value(declared)
            .map_err(|e| JsValue::from_str(&format!("prepare_input_slots declared: {}", e)))?;
        if indices.length() as usize != decls.len() {
            return Err(JsValue::from_str(
                "prepare_input_slots: indices and declared length mismatch",
            ));
        }
        let mut idx_buf = vec![0u32; indices.length() as usize];
        indices.copy_to(&mut idx_buf);
        for (i, idx) in idx_buf.iter().enumerate() {
            let slot = self
                .input_slots
                .get_mut(*idx as usize)
                .ok_or_else(|| JsValue::from_str("prepare_input_slots: index out of bounds"))?;
            *slot = Some(SlotStaging {
                path_idx: *idx,
                declared: decls[i].clone(),
            });
        }
        Ok(())
    }

    /// Stage inputs by pre-prepared slots (no path parse, reuse declared).
    #[wasm_bindgen(js_name = "stage_inputs_slots")]
    pub fn stage_inputs_slots(
        &mut self,
        indices: &Uint32Array,
        values: &Float32Array,
    ) -> Result<(), JsValue> {
        let len = indices.length();
        if len != values.length() {
            return Err(JsValue::from_str(
                "stage_inputs_slots: indices and values length mismatch",
            ));
        }
        let mut idx_buf = vec![0u32; len as usize];
        indices.copy_to(&mut idx_buf);
        for (i, idx) in idx_buf.iter().enumerate() {
            let slot = self
                .input_slots
                .get(*idx as usize)
                .and_then(|s| s.as_ref())
                .ok_or_else(|| JsValue::from_str("stage_inputs_slots: slot not prepared"))?;
            let tp = self
                .input_paths
                .get(slot.path_idx as usize)
                .ok_or_else(|| JsValue::from_str("stage_inputs_slots: path index out of bounds"))?;
            let val = values.get_index(i as u32);
            let value = Value::Float(val);
            self.stage_cached(
                slot.path_idx as usize,
                tp.clone(),
                value,
                slot.declared.clone(),
            )?;
        }
        Ok(())
    }

    /// Clear a staged input by slot index (registered path).
    #[wasm_bindgen(js_name = "clear_input_slot")]
    pub fn clear_input_slot(&mut self, slot_idx: u32) -> Result<(), JsValue> {
        let idx = slot_idx as usize;
        let tp = self
            .input_paths
            .get(idx)
            .ok_or_else(|| JsValue::from_str("clear_input_slot: slot index out of bounds"))?
            .clone();
        self.runtime.staged_inputs.remove(&tp);
        self.input_last_values.remove(&idx);
        self.input_last_shapes.remove(&idx);
        self.input_touched.remove(&idx);
        Ok(())
    }

    /// Fetch a float/vec/array output directly as Float32Array (if numeric).
    #[wasm_bindgen(js_name = "get_output_f32")]
    pub fn get_output_f32(&self, node_id: &str, output_key: &str) -> Option<Float32Array> {
        let port = self.runtime.outputs.get(node_id)?.get(output_key)?;
        match &port.value {
            Value::Float(f) => Some(Float32Array::from(&[*f][..])),
            Value::Vector(v) => Some(Float32Array::from(v.as_slice())),
            Value::Vec2(arr) => Some(Float32Array::from(arr.as_slice())),
            Value::Vec3(arr) => Some(Float32Array::from(arr.as_slice())),
            Value::Vec4(arr) => Some(Float32Array::from(arr.as_slice())),
            Value::Quat(arr) => Some(Float32Array::from(arr.as_slice())),
            Value::Transform {
                translation,
                rotation,
                scale,
            } => {
                let mut tmp = Vec::with_capacity(10);
                tmp.extend_from_slice(translation);
                tmp.extend_from_slice(rotation);
                tmp.extend_from_slice(scale);
                Some(Float32Array::from(tmp.as_slice()))
            }
            _ => None,
        }
    }

    /// Batch fetch the default "out" port for many nodes as a Float32Array (scalars only; non-scalars -> NaN).
    #[wasm_bindgen(js_name = "get_outputs_batch")]
    pub fn get_outputs_batch(&self, nodes: JsValue) -> Result<Float32Array, JsValue> {
        let ids: Vec<String> = swb::from_value(nodes)
            .map_err(|e| JsValue::from_str(&format!("get_outputs_batch nodes: {}", e)))?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(port) = self.runtime.outputs.get(&id).and_then(|m| m.get("out")) {
                match &port.value {
                    Value::Float(f) => out.push(*f),
                    Value::Vector(v) if !v.is_empty() => out.push(v[0]),
                    Value::Vec2(v) => out.push(v[0]),
                    Value::Vec3(v) => out.push(v[0]),
                    Value::Vec4(v) => out.push(v[0]),
                    Value::Quat(v) => out.push(v[0]),
                    Value::Transform { translation, .. } => out.push(translation[0]),
                    _ => out.push(f32::NAN),
                }
            } else {
                out.push(f32::NAN);
            }
        }
        Ok(Float32Array::from(out.as_slice()))
    }

    /// Internal helper: run multiple steps and return the final frame outputs.
    fn eval_steps_json(&mut self, steps: u32, dt: f64) -> Result<serde_json::Value, JsValue> {
        if !dt.is_finite() || dt < 0.0 {
            return Err(JsValue::from_str(
                "eval_steps: dt must be finite and non-negative",
            ));
        }
        let iterations = steps.max(1);
        let mut last = None;
        for _ in 0..iterations {
            self.step(dt);
            last = Some(self.eval_all_json()?);
        }
        last.ok_or_else(|| JsValue::from_str("eval_steps: no steps executed"))
    }

    fn eval_all_json(&mut self) -> Result<serde_json::Value, JsValue> {
        let new_time = self.t as f32;
        let mut dt = new_time - self.runtime.t;
        if !dt.is_finite() || dt < 0.0 {
            dt = 0.0;
        }
        self.runtime.dt = dt;
        self.runtime.t = new_time;
        self.restage_cached_inputs()?;
        self.eval_internal()?;
        Ok(self.serialize_full())
    }

    /// Evaluate the entire graph and return a JS object (avoids JSON stringify/parse).
    #[wasm_bindgen(js_name = "eval_all_js")]
    pub fn eval_all_js(&mut self) -> Result<JsValue, JsValue> {
        let out_obj = self.eval_all_json()?;
        let s = serde_json::to_string(&out_obj).map_err(|e| JsValue::from_str(&e.to_string()))?;
        JSON::parse(&s)
    }

    /// Evaluate the entire graph and return all outputs as JSON.
    /// Returned JSON shape:
    /// {
    ///   "nodes": { [nodeId]: { [outputKey]: { "value": ValueJSON, "shape": ShapeJSON } } },
    ///   "writes": [ { "path": string, "value": ValueJSON, "shape": ShapeJSON }, ... ]
    /// }
    #[wasm_bindgen]
    pub fn eval_all(&mut self) -> Result<String, JsValue> {
        let out_obj = self.eval_all_json()?;
        serde_json::to_string(&out_obj).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Evaluate without serializing to JSON; returns a monotonic output version.
    #[wasm_bindgen(js_name = "eval_all_slots")]
    pub fn eval_all_slots(&mut self) -> Result<u64, JsValue> {
        let new_time = self.t as f32;
        let mut dt = new_time - self.runtime.t;
        if !dt.is_finite() || dt < 0.0 {
            dt = 0.0;
        }
        self.runtime.dt = dt;
        self.runtime.t = new_time;
        self.restage_cached_inputs()?;
        self.eval_internal()?;
        Ok(self.output_version)
    }

    /// Return a full snapshot of outputs/writes (JSON) without re-evaluating.
    #[wasm_bindgen(js_name = "get_outputs_full")]
    pub fn get_outputs_full(&mut self) -> Result<JsValue, JsValue> {
        let out_obj = self.serialize_full();
        let s = serde_json::to_string(&out_obj).map_err(|e| JsValue::from_str(&e.to_string()))?;
        JSON::parse(&s)
    }

    /// Return only outputs that changed since the provided version token.
    #[wasm_bindgen(js_name = "get_outputs_delta")]
    pub fn get_outputs_delta(&mut self, since_version: u64) -> Result<JsValue, JsValue> {
        let out_obj = self.serialize_delta(since_version);
        let s = serde_json::to_string(&out_obj).map_err(|e| JsValue::from_str(&e.to_string()))?;
        JSON::parse(&s)
    }

    /// Evaluate and return only outputs that changed since the provided version token in a single crossing.
    #[wasm_bindgen(js_name = "eval_all_slots_delta")]
    pub fn eval_all_slots_delta(&mut self, since_version: u64) -> Result<JsValue, JsValue> {
        let new_time = self.t as f32;
        let mut dt = new_time - self.runtime.t;
        if !dt.is_finite() || dt < 0.0 {
            dt = 0.0;
        }
        self.runtime.dt = dt;
        self.runtime.t = new_time;
        self.restage_cached_inputs()?;
        self.eval_internal()?;
        let out_obj = self.serialize_delta(since_version);
        let s = serde_json::to_string(&out_obj).map_err(|e| JsValue::from_str(&e.to_string()))?;
        JSON::parse(&s)
    }

    /// Step forward multiple times and return only the final outputs/writes.
    /// Useful for amortizing JS/WASM boundary cost when ticking many frames.
    #[wasm_bindgen(js_name = "eval_steps_js")]
    pub fn eval_steps_js(&mut self, steps: u32, dt: f64) -> Result<JsValue, JsValue> {
        let out_obj = self.eval_steps_json(steps, dt)?;
        let s = serde_json::to_string(&out_obj).map_err(|e| JsValue::from_str(&e.to_string()))?;
        JSON::parse(&s)
    }

    #[wasm_bindgen(js_name = "eval_steps")]
    pub fn eval_steps(&mut self, steps: u32, dt: f64) -> Result<String, JsValue> {
        let out_obj = self.eval_steps_json(steps, dt)?;
        serde_json::to_string(&out_obj).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Set a param on a node (e.g., key="value" with float/bool/vec3 JSON).
    ///
    /// Structural parameter edits (e.g., adjusting Split sizes) invalidate the
    /// cached execution plan so the next evaluation rebuilds layouts safely.
    #[wasm_bindgen]
    pub fn set_param(&mut self, node_id: &str, key: &str, json_value: &str) -> Result<(), JsValue> {
        let raw: serde_json::Value =
            serde_json::from_str(json_value).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let normalized = json::normalize_value_json(raw);
        let val: Value =
            serde_json::from_value(normalized).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.set_param_inner(node_id, key, val)
    }

    #[wasm_bindgen(js_name = "set_param_value")]
    pub fn set_param_value(
        &mut self,
        node_id: &str,
        key: &str,
        value: JsValue,
    ) -> Result<(), JsValue> {
        let val = parse_value_js(value, json::normalize_value_json, "set_param")?;
        self.set_param_inner(node_id, key, val)
    }

    fn set_param_inner(&mut self, node_id: &str, key: &str, val: Value) -> Result<(), JsValue> {
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
        fn expect_bool(node_id: &str, key: &str, v: &Value) -> Result<bool, JsValue> {
            if let Value::Bool(b) = v {
                Ok(*b)
            } else {
                Err(JsValue::from_str(&format!(
                    "set_param: node '{}' key '{}' expects Bool",
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
        fn parse_round_mode(node_id: &str, key: &str, v: &Value) -> Result<RoundMode, JsValue> {
            let raw = expect_text(node_id, key, v)?;
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "floor" => Ok(RoundMode::Floor),
                "ceil" => Ok(RoundMode::Ceil),
                "trunc" => Ok(RoundMode::Trunc),
                other => Err(JsValue::from_str(&format!(
                    "set_param: node '{}' key '{}' expects \"floor\", \"ceil\", or \"trunc\" (got '{}')",
                    node_id, key, other
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
                "clamp" => node.params.clamp = Some(expect_bool(node_id, key, &val)?),
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
                "round_mode" => {
                    node.params.round_mode = Some(parse_round_mode(node_id, key, &val)?);
                }

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
            self.invalidate_plan_cache();
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
