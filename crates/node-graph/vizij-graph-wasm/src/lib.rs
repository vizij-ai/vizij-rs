//! `wasm-bindgen` bridge for the Vizij node-graph runtime.
//!
//! This module exposes graph normalization, evaluation, staged-input handling, registry
//! metadata, and delta-friendly output snapshots to JavaScript consumers.

use arora_dataflow_nodes::tree_runtime::{Evaluation, TreeRuntime};
use arora_dataflow_nodes::types::RoundMode;
use arora_dataflow_nodes::{GraphSpec, PortValue};
use arora_shape::shape::ShapeId;
use arora_shape::value::{
    as_array, as_bool, as_float, as_quat, as_text, as_transform, as_vec2, as_vec3, as_vec4,
    as_vector, float, kind, vector, ShapeKind,
};
use arora_shape::{coercion, json, Shape, TypedPath, Value};
use hashbrown::HashMap;
use js_sys::{Float32Array, Uint32Array, JSON};
use serde_wasm_bindgen as swb;
use wasm_bindgen::prelude::*;

/// Normalize a graph-spec JSON string into the canonical serde shape expected by the runtime.
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
    use arora_shape::json;
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

        let normalized = json::normalize_graph_spec_json(input).expect("normalize");
        let nodes = normalized["nodes"].as_array().expect("nodes");
        let constant = nodes[0]["params"]["value"].clone();
        assert_eq!(constant, serde_json::json!({ "f32": 1.0 }));

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

    /// A changed split arity reaches the graph at the next eval: the spec is
    /// loaded again, and the split has the ports its new sizes give it.
    #[test]
    fn a_changed_split_arity_reloads_with_its_new_ports() {
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
        let initial_ports = graph.runtime.outputs["split"].len();

        graph
            .set_param("split", "sizes", "[1, 1, 1, 1]")
            .expect("set_param succeeds");

        assert!(
            !graph.plan_ready,
            "the next eval loads the spec again after a param change"
        );

        graph.eval_all().expect("re-eval loads the spec");
        assert!(graph.plan_ready);
        assert_eq!(graph.runtime.loads(), 2);

        let rebuilt_ports = graph.runtime.outputs["split"].len();
        assert!(
            rebuilt_ports > initial_ports,
            "the reload reflects the new Split arity"
        );
    }

    /// A changed value reaches the graph at the next eval with the tree
    /// kept: on the tree, a parameter is a literal argument of its node's
    /// call, set in place.
    #[test]
    fn a_changed_value_reaches_the_output_at_the_next_eval() {
        let mut graph = WasmGraph::new();
        let spec = r#"{
            "nodes": [
                { "id": "c", "type": "constant", "params": { "value": 1.0 }, "inputs": {}, "output_shapes": {} },
                { "id": "out", "type": "output", "params": { "path": "demo/out" }, "inputs": {}, "output_shapes": {} }
            ],
            "edges": [
                { "from": { "node_id": "c", "output": "out" }, "to": { "node_id": "out", "input": "in" } }
            ]
        }"#;

        graph.load_graph(spec).expect("graph loads");
        graph.eval_all().expect("initial eval");
        assert!(graph.plan_ready, "the spec is loaded after the first eval");

        graph
            .set_param("c", "value", "2.0")
            .expect("set_param succeeds");
        assert!(graph.plan_ready, "the value is set in place");

        graph.eval_all().expect("eval succeeds after a value edit");
        assert_eq!(graph.runtime.loads(), 1, "the tree is kept");
        assert_eq!(
            graph.runtime.outputs["out"]["out"].value,
            Value::F32(2.0),
            "the new value reaches the output"
        );
    }

    #[test]
    fn set_param_on_a_constant_read_through_a_selector_reaches_the_selection() {
        let mut graph = WasmGraph::new();
        let spec = r#"{
            "nodes": [
                { "id": "rec", "type": "constant", "params": { "value": { "record": { "x": { "f32": 1.0 } } } }, "inputs": {}, "output_shapes": {} },
                { "id": "out", "type": "output", "params": { "path": "demo/out" }, "inputs": {}, "output_shapes": {} }
            ],
            "edges": [
                { "from": { "node_id": "rec", "output": "out" }, "to": { "node_id": "out", "input": "in" }, "selector": [{ "field": "x" }] }
            ]
        }"#;
        let out = |graph: &WasmGraph| graph.runtime.outputs["out"]["out"].value.clone();
        graph.load_graph(spec).expect("graph loads");
        graph.eval_all().expect("initial eval");
        assert_eq!(out(&graph), Value::F32(1.0));

        graph
            .set_param("rec", "value", r#"{ "record": { "x": { "f32": 2.0 } } }"#)
            .expect("set_param succeeds");
        assert!(
            graph.plan_ready,
            "the selection reads the constant's value as it changes"
        );
        graph.eval_all().expect("re-eval");
        assert_eq!(out(&graph), Value::F32(2.0));
    }

    #[test]
    fn set_param_supports_noise_params() {
        let mut graph = WasmGraph::new();
        let spec = r#"{
            "nodes": [
                {
                    "id": "noise",
                    "type": "simplenoise",
                    "params": {
                        "noise_seed": 0.0,
                        "frequency": 1.0,
                        "octaves": 1.0,
                        "lacunarity": 2.0,
                        "persistence": 0.5
                    },
                    "inputs": {},
                    "output_shapes": {}
                }
            ]
        }"#;

        graph.load_graph(spec).expect("graph loads");
        graph.eval_all().expect("initial eval");
        assert!(graph.plan_ready, "the spec is loaded after the first eval");

        graph
            .set_param("noise", "noise_seed", "42.0")
            .expect("noise_seed set_param succeeds");
        graph
            .set_param("noise", "octaves", "4.0")
            .expect("octaves set_param succeeds");
        graph
            .set_param("noise", "lacunarity", "2.5")
            .expect("lacunarity set_param succeeds");
        graph
            .set_param("noise", "persistence", "0.4")
            .expect("persistence set_param succeeds");

        assert!(graph.plan_ready, "the noise parameters are set in place");
        graph.eval_all().expect("eval after the noise edits");

        let noise = graph
            .spec
            .nodes
            .iter()
            .find(|n| n.id == "noise")
            .expect("noise node present");
        assert_eq!(noise.params.noise_seed, Some(42.0));
        assert_eq!(noise.params.octaves, Some(4.0));
        assert_eq!(noise.params.lacunarity, Some(2.5));
        assert_eq!(noise.params.persistence, Some(0.4));
    }

    #[test]
    fn delta_since_greater_than_output_version_forces_full_resync() {
        let mut graph = WasmGraph::new();
        let spec = r#"{
            "nodes": [
                { "id": "c", "type": "constant", "params": { "value": 1.0 }, "inputs": {}, "output_shapes": {} },
                { "id": "out", "type": "output", "params": { "path": "demo/out" }, "inputs": {}, "output_shapes": {} }
            ],
            "edges": [
                { "from": { "node_id": "c", "output": "out" }, "to": { "node_id": "out", "input": "in" } }
            ]
        }"#;

        graph.load_graph(spec).expect("graph loads");
        graph.eval_all().expect("initial eval bumps version");

        let version_before_reset = graph.output_version;
        assert!(
            version_before_reset > 0,
            "version should advance after eval"
        );

        // Reset the runtime (and output_version) but keep an old version token.
        graph.load_graph(spec).expect("graph reloads");
        assert_eq!(graph.output_version, 0, "load_graph resets output_version");

        // A "future" token must force a full snapshot so hosts can resync immediately.
        let delta = graph.serialize_delta(version_before_reset);
        assert_eq!(delta["full"], serde_json::json!(true));
        assert_eq!(delta["version"], serde_json::json!(0));
        assert!(
            delta["nodes"].as_object().is_some(),
            "full snapshot should include nodes map"
        );
    }
}

/// WASM-facing graph runtime with staging and output delta caches.
///
/// The graph runs on the behavior tree ([`TreeRuntime`]), every node at
/// every evaluation so each port can be shown; node state lives in the tree.
/// This wrapper tracks input staging and output snapshots to avoid crossing
/// the wasm boundary when values are unchanged.
#[wasm_bindgen]
pub struct WasmGraph {
    spec: GraphSpec,
    t: f64,
    runtime: TreeRuntime,
    input_paths: Vec<TypedPath>,
    input_slots: Vec<Option<SlotStaging>>,
    /// Whether the runtime holds the current spec: false after a load, or
    /// after an edit the tree cannot take in place, until the next eval loads
    /// it, every node keeping its state.
    plan_ready: bool,
    /// The evaluations so far: the epoch the next one stages for.
    epoch: u64,
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
    /// Construct an empty graph wrapper with no loaded spec and cleared caches.
    pub fn new() -> WasmGraph {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();
        WasmGraph {
            spec: GraphSpec {
                nodes: vec![],
                ..Default::default()
            },
            t: 0.0,
            runtime: TreeRuntime::new(Evaluation::Every),
            input_paths: Vec::new(),
            input_slots: Vec::new(),
            plan_ready: false,
            epoch: 0,
            output_version: 0,
            last_outputs: HashMap::new(),
            last_outputs_version: 0,
            input_last_values: HashMap::new(),
            input_last_shapes: HashMap::new(),
            input_touched: HashMap::new(),
        }
    }

    /// Load a new graph spec from JSON and reset all staged inputs, output snapshots, and caches.
    #[wasm_bindgen]
    pub fn load_graph(&mut self, json_str: &str) -> Result<(), JsValue> {
        let normalized = json::normalize_graph_spec_json(json_str)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        // Now deserialize into the typed GraphSpec
        // The runtime loads the spec whenever the plan is not ready, so the
        // spec carries no cache key of its own.
        self.spec = serde_json::from_value::<GraphSpec>(normalized)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        self.runtime = TreeRuntime::new(Evaluation::Every);
        self.epoch = 0;
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

    /// Stage one input by typed path using a JSON string payload.
    ///
    /// This path-based staging route bypasses the slot/index cache helpers.
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

    /// Stage one input by typed path using a JS value payload.
    ///
    /// This path-based staging route bypasses the slot/index cache helpers.
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
        self.runtime.clear_input(&typed_path);
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

    /// Have the next eval load the spec again, and drop the delta snapshot.
    fn invalidate_plan_cache(&mut self) {
        self.plan_ready = false;
        self.last_outputs.clear();
        self.last_outputs_version = 0;
    }

    fn eval_internal(&mut self) -> Result<(), JsValue> {
        let res = if self.plan_ready {
            Ok(())
        } else {
            self.runtime.load(&self.spec)
        }
        .and_then(|()| self.runtime.evaluate());
        self.epoch = self.epoch.saturating_add(1);
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
        let next_epoch = self.epoch.saturating_add(1);
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
        let next_epoch = self.epoch.saturating_add(1);
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

    fn serialize_full_with_flag(
        &mut self,
        snapshot: &HashMap<String, HashMap<String, PortValue>>,
    ) -> serde_json::Value {
        let mut obj = self.serialize_full_from_snapshot(snapshot);
        obj.as_object_mut()
            .expect("full snapshot is an object")
            .insert("full".to_string(), serde_json::json!(true));
        obj
    }

    fn refresh_snapshot_and_full(&mut self) -> serde_json::Value {
        let snapshot = self.snapshot_outputs();
        let full = self.serialize_full_with_flag(&snapshot);
        self.last_outputs = snapshot;
        self.last_outputs_version = self.output_version;
        full
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
                    let value_json = serde_json::to_value(&port.value).unwrap();
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
            let jv = serde_json::to_value(&op.value).unwrap();
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
        // Fast path: caller is exactly up-to-date.
        if since == self.output_version {
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

        // If the caller's token is "from the future" relative to our current version, treat it
        // as a baseline mismatch and force a full resync snapshot.
        //
        // This can happen when the runtime resets `output_version` (e.g., after `load_graph`) but
        // a host still holds a previous version token. Returning an empty delta would keep the
        // host out-of-sync until the version catches back up.
        if since > self.output_version {
            return self.refresh_snapshot_and_full();
        }

        // If the caller's baseline does not match our cached snapshot, resync by returning
        // a full snapshot for the current version.
        if since != self.last_outputs_version {
            return self.refresh_snapshot_and_full();
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
                    let value_json = serde_json::to_value(&port.value).unwrap();
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
            let jv = serde_json::to_value(&op.value).unwrap();
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

    fn serialize_delta_js(&mut self, since_version: u64) -> Result<JsValue, JsValue> {
        let out_obj = self.serialize_delta(since_version);
        let s = serde_json::to_string(&out_obj).map_err(|e| JsValue::from_str(&e.to_string()))?;
        JSON::parse(&s)
    }

    /// Set the external graph clock in seconds.
    #[wasm_bindgen]
    pub fn set_time(&mut self, t: f64) {
        self.t = t;
    }

    /// Advance the external graph clock by `dt` seconds without evaluating the graph.
    #[wasm_bindgen]
    pub fn step(&mut self, dt: f64) {
        self.t += dt;
    }

    /// Stage one numeric vector without JSON, using a `Float32Array`.
    ///
    /// This path-based staging route bypasses the slot/index cache helpers.
    #[wasm_bindgen(js_name = "stage_input_f32")]
    pub fn stage_input_f32(&mut self, path: &str, data: &Float32Array) -> Result<(), JsValue> {
        let typed_path = TypedPath::parse(path)
            .map_err(|e| JsValue::from_str(&format!("invalid path: {}", e)))?;
        let mut buf = vec![0.0f32; data.length() as usize];
        data.copy_to(&mut buf);
        let value = vector(buf);
        let declared = Some(Shape::new(ShapeId::Vector { len: None }));
        // Path-based staging bypasses slot cache; no diffing here.
        self.runtime.set_input(typed_path, value, declared);
        Ok(())
    }

    /// Batch-stage many scalar inputs in one call (`paths[i] -> values[i]`).
    ///
    /// This path-based staging route bypasses the slot/index cache helpers.
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
            let value = float(val);
            let declared = Some(Shape::new(ShapeId::Scalar));
            // Batch by path bypasses slot cache; no diffing here.
            self.runtime.set_input(tp, value, declared);
        }
        Ok(())
    }

    /// Register typed paths once and reuse their indices for faster staging.
    ///
    /// Returned indices are stable for the lifetime of the loaded graph wrapper unless
    /// `load_graph()` is called again.
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

    /// Stage scalar inputs by path index using previously registered paths.
    ///
    /// This route participates in the cached fast path, so unchanged values can be restaged
    /// without re-sending them from JS every frame.
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
            let value = float(val);
            let declared = Some(Shape::new(ShapeId::Scalar));
            // Use path index as slot key for caching.
            self.stage_cached(*idx as usize, tp.clone(), value, declared)?;
        }
        Ok(())
    }

    /// Attach declared-shape metadata to previously registered indices so slot staging can reuse it.
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

    /// Stage scalar inputs by pre-prepared slots with no path parsing and reused declared shapes.
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
            let value = float(val);
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
        self.runtime.clear_input(&tp);
        self.input_last_values.remove(&idx);
        self.input_last_shapes.remove(&idx);
        self.input_touched.remove(&idx);
        Ok(())
    }

    /// Fetch a float/vec/array output directly as Float32Array (if numeric).
    #[wasm_bindgen(js_name = "get_output_f32")]
    pub fn get_output_f32(&self, node_id: &str, output_key: &str) -> Option<Float32Array> {
        let port = self.runtime.outputs.get(node_id)?.get(output_key)?;
        let value = &port.value;
        match kind(value) {
            ShapeKind::Float => as_float(value).map(|f| Float32Array::from(&[f][..])),
            ShapeKind::Vector => as_vector(value).map(Float32Array::from),
            ShapeKind::Vec2 => as_vec2(value).map(|a| Float32Array::from(&a[..])),
            ShapeKind::Vec3 => as_vec3(value).map(|a| Float32Array::from(&a[..])),
            ShapeKind::Vec4 => as_vec4(value).map(|a| Float32Array::from(&a[..])),
            ShapeKind::Quat => as_quat(value).map(|a| Float32Array::from(&a[..])),
            ShapeKind::Transform => as_transform(value).map(|t| {
                let mut tmp = Vec::with_capacity(10);
                tmp.extend_from_slice(&t.translation);
                tmp.extend_from_slice(&t.rotation);
                tmp.extend_from_slice(&t.scale);
                Float32Array::from(tmp.as_slice())
            }),
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
            let first = self
                .runtime
                .outputs
                .get(&id)
                .and_then(|m| m.get("out"))
                .and_then(|port| {
                    let value = &port.value;
                    match kind(value) {
                        ShapeKind::Float => as_float(value),
                        ShapeKind::Vector => as_vector(value).and_then(|v| v.first().copied()),
                        ShapeKind::Vec2 => as_vec2(value).map(|a| a[0]),
                        ShapeKind::Vec3 => as_vec3(value).map(|a| a[0]),
                        ShapeKind::Vec4 => as_vec4(value).map(|a| a[0]),
                        ShapeKind::Quat => as_quat(value).map(|a| a[0]),
                        ShapeKind::Transform => as_transform(value).map(|t| t.translation[0]),
                        _ => None,
                    }
                });
            out.push(first.unwrap_or(f32::NAN));
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
    ///
    /// Uses the currently staged inputs, refreshes output snapshots, and bumps the output version.
    #[wasm_bindgen(js_name = "eval_all_js")]
    pub fn eval_all_js(&mut self) -> Result<JsValue, JsValue> {
        let out_obj = self.eval_all_json()?;
        let s = serde_json::to_string(&out_obj).map_err(|e| JsValue::from_str(&e.to_string()))?;
        JSON::parse(&s)
    }

    /// Evaluate the entire graph and return all outputs as JSON.
    /// Returned JSON shape:
    /// {
    ///   `"nodes": { "[nodeId]": { "[outputKey]": { "value": ValueJSON, "shape": ShapeJSON } } },`
    ///   `"writes": [{ "path": string, "value": ValueJSON, "shape": ShapeJSON }, ...]`
    /// }
    /// where `ValueJSON` is the Arora `Value` serde form (externally tagged:
    /// `{"f32": 1.0}`, `{"bool": true}`, `{"str": "hi"}`, `{"f32s": [...]}`,
    /// `{"struct": {...}}`, ...).
    #[wasm_bindgen]
    pub fn eval_all(&mut self) -> Result<String, JsValue> {
        let out_obj = self.eval_all_json()?;
        serde_json::to_string(&out_obj).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Evaluate without serializing to JSON and return the current output version token.
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
    ///
    /// If the caller baseline is stale, missing, or from a future/reset runtime, a full snapshot
    /// is returned so the host can resynchronize immediately.
    #[wasm_bindgen(js_name = "get_outputs_delta")]
    pub fn get_outputs_delta(&mut self, since_version: u64) -> Result<JsValue, JsValue> {
        self.serialize_delta_js(since_version)
    }

    /// Evaluate and return only outputs that changed since the provided version token in a single crossing.
    ///
    /// If the caller baseline is stale, missing, or from a future/reset runtime, a full snapshot
    /// is returned so the host can resynchronize immediately.
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
        self.serialize_delta_js(since_version)
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

    /// Set a param on a node (e.g., key="value" with a JSON payload in Arora
    /// `Value` serde form or any legacy form the arora-shape normalizer accepts).
    ///
    /// A parameter is a literal argument of the node's call on the behavior
    /// tree. A value is set in place: the tree is kept, and the node runs
    /// with it at the next evaluation. An edit that changes the tree (a
    /// split's sizes, a record's keys, an input's or output's path, a
    /// parameter the node did not set) has the next evaluation load the
    /// edited spec, every node keeping its state.
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
            as_float(v).ok_or_else(|| {
                JsValue::from_str(&format!(
                    "set_param: node '{}' key '{}' expects a float",
                    node_id, key
                ))
            })
        }
        fn expect_bool(node_id: &str, key: &str, v: &Value) -> Result<bool, JsValue> {
            as_bool(v).ok_or_else(|| {
                JsValue::from_str(&format!(
                    "set_param: node '{}' key '{}' expects a boolean",
                    node_id, key
                ))
            })
        }
        fn expect_text<'a>(node_id: &str, key: &str, v: &'a Value) -> Result<&'a str, JsValue> {
            as_text(v).ok_or_else(|| {
                JsValue::from_str(&format!(
                    "set_param: node '{}' key '{}' expects text",
                    node_id, key
                ))
            })
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
            let items = as_array(v).ok_or_else(|| {
                JsValue::from_str(&format!(
                    "set_param: node '{}' key '{}' expects a sequence of [text, float] pairs",
                    node_id, key
                ))
            })?;
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let elems = as_array(item)
                    .filter(|elems| elems.len() >= 2)
                    .ok_or_else(|| {
                        JsValue::from_str(&format!(
                        "set_param: node '{}' key '{}' expects a sequence of [text, float] pairs",
                        node_id, key
                    ))
                    })?;
                let name = as_text(&elems[0]).ok_or_else(|| {
                    JsValue::from_str(&format!(
                        "set_param: node '{}' key '{}' pair[0] expects text",
                        node_id, key
                    ))
                })?;
                let val = as_float(&elems[1]).ok_or_else(|| {
                    JsValue::from_str(&format!(
                        "set_param: node '{}' key '{}' pair[1] expects a float",
                        node_id, key
                    ))
                })?;
                out.push((name.to_string(), val));
            }
            Ok(out)
        }

        fn parse_string_list(node_id: &str, key: &str, v: &Value) -> Result<Vec<String>, JsValue> {
            if let Some(items) = as_array(v) {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    let s = as_text(item).ok_or_else(|| {
                        JsValue::from_str(&format!(
                            "set_param: node '{}' key '{}' expects a sequence of text",
                            node_id, key
                        ))
                    })?;
                    out.push(s.to_string());
                }
                return Ok(out);
            }
            if let Some(s) = as_text(v) {
                return Ok(vec![s.to_string()]);
            }
            Err(JsValue::from_str(&format!(
                "set_param: node '{}' key '{}' expects text or a sequence of text",
                node_id, key
            )))
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
                "noise_seed" => node.params.noise_seed = Some(expect_float(node_id, key, &val)?),
                "octaves" => node.params.octaves = Some(expect_float(node_id, key, &val)?),
                "lacunarity" => node.params.lacunarity = Some(expect_float(node_id, key, &val)?),
                "persistence" => node.params.persistence = Some(expect_float(node_id, key, &val)?),
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
                "record_keys" => {
                    node.params.record_keys = Some(parse_string_list(node_id, key, &val)?);
                }
                "keys" => {
                    node.params.keys = match as_text(&val) {
                        Some(s) => Some(s.to_string()),
                        None => {
                            return Err(JsValue::from_str(&format!(
                                "set_param: node '{}' key 'keys' expects a string value",
                                node_id
                            )))
                        }
                    };
                }

                _ => {
                    return Err(JsValue::from_str(&format!(
                        "set_param: node '{}' unknown key '{}'",
                        node_id, key
                    )))
                }
            }
            // An edit the tree refuses, or fails to make, is left to the
            // next eval's load of the edited spec, which reports any error.
            let params = node.params.clone();
            let in_place =
                self.plan_ready && matches!(self.runtime.set_params(node_id, &params), Ok(true));
            if !in_place {
                self.invalidate_plan_cache();
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
    let reg = arora_dataflow_nodes::registry();
    serde_json::to_string(&reg).unwrap()
}
