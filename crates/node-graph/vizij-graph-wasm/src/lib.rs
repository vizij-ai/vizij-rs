use wasm_bindgen::prelude::*;
use hashbrown::HashMap;
use vizij_graph_core::{GraphSpec, GraphRuntime, Value, evaluate_all};

#[wasm_bindgen]
pub struct WasmGraph {
    spec: GraphSpec,
    t: f64,
}

#[wasm_bindgen]
impl WasmGraph {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmGraph {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();
        WasmGraph { spec: GraphSpec { nodes: vec![] }, t: 0.0 }
    }

    #[wasm_bindgen]
    pub fn load_graph(&mut self, json_str: &str) -> Result<(), JsValue> {
        self.spec = serde_json::from_str(json_str).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(())
    }

    #[wasm_bindgen]
    pub fn set_time(&mut self, t: f64) { self.t = t; }

    #[wasm_bindgen]
    pub fn step(&mut self, dt: f64) { self.t += dt; }

    /// Evaluate the entire graph and return all outputs as JSON.
    #[wasm_bindgen]
    pub fn eval_all(&self) -> Result<String, JsValue> {
        let mut rt = GraphRuntime { t: self.t, outputs: HashMap::new() };
        evaluate_all(&mut rt, &self.spec).map_err(|e| JsValue::from_str(&e))?;
        let map: HashMap<String, serde_json::Value> = rt.outputs.into_iter().map(|(k,v)| {
            let j = match v {
                Value::Float(f) => serde_json::json!({"float": f}),
                Value::Bool(b) => serde_json::json!({"bool": b}),
                Value::Vec3(v) => serde_json::json!({"vec3": v}),
            };
            (k, j)
        }).collect();
        Ok(serde_json::to_string(&map).unwrap())
    }

    /// Set a param on a node (e.g., key="value" with float/bool/vec3 JSON)
    #[wasm_bindgen]
    pub fn set_param(&mut self, node_id: &str, key: &str, json_value: &str) -> Result<(), JsValue> {
        let v: serde_json::Value = serde_json::from_str(json_value).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let val = if let Some(f) = v.get("float").and_then(|x| x.as_f64()) { Value::Float(f) }
                  else if let Some(b) = v.get("bool").and_then(|x| x.as_bool()) { Value::Bool(b) }
                  else if let Some(arr) = v.get("vec3").and_then(|x| x.as_array()) {
                      let mut a = [0.0;3];
                      for i in 0..3 { a[i] = arr.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0); }
                      Value::Vec3(a)
                  } else { return Err(JsValue::from_str("unsupported value")); };
        if let Some(node) = self.spec.nodes.iter_mut().find(|n| n.id == node_id) {
            match key {
                "value" => node.params.value = Some(val),
                "frequency" => node.params.frequency = Some(if let Value::Float(f)=val { f } else { 0.0 }),
                "phase" => node.params.phase = Some(if let Value::Float(f)=val { f } else { 0.0 }),
                "min" => node.params.min = if let Value::Float(f)=val { f } else { 0.0 },
                "max" => node.params.max = if let Value::Float(f)=val { f } else { 0.0 },
                "x" => node.params.x = Some(if let Value::Float(f)=val { f } else { 0.0 }),
                "y" => node.params.y = Some(if let Value::Float(f)=val { f } else { 0.0 }),
                "z" => node.params.z = Some(if let Value::Float(f)=val { f } else { 0.0 }),
                _ => {}
            }
            Ok(())
        } else {
            Err(JsValue::from_str("unknown node"))
        }
    }
}
