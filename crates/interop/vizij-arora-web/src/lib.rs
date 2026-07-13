//! Run a Vizij runtime *as* an Arora device in the browser.
//!
//! This is a thin wasm-bindgen wrapper: it assembles the Vizij interop pieces
//! and hands them to [`arora_web::BrowserRuntime`], the reusable browser-device
//! primitive, then forwards the JS-facing methods. All the runtime scaffolding
//! (the arora step loop, the golden clock, the Value↔JSON store accessors)
//! lives in `arora-web`; here we only choose the backends:
//!
//! - the store is a [`BlackboardStore`] (a Vizij `Blackboard` as an Arora
//!   `DataStore`);
//! - the HAL is a [`RigHal`] (a Vizij rig as an Arora HAL);
//! - the bridge is the in-process [`FakeBridge`];
//! - the behavior is a [`ProcessingGraph`] (a Vizij node graph driven as the
//!   device's `BehaviorInterpreter`), built from the caller's graph spec.
//!
//! JavaScript constructs it with [`VizijArora::start`] (passing a Vizij graph
//! spec as JSON, in any accepted form — the spec normalizer runs here), drives
//! it one tick at a time with [`VizijArora::step`] from `requestAnimationFrame`
//! timestamps, and reads or writes keys on the injected store through the
//! forwarded accessors. Values cross the JS boundary as JSON in the Arora
//! `Value` vocabulary (e.g. `{"f32": 0.75}`).
//!
//! This crate only carries content when built for `wasm32`; on the host it is
//! an empty shim so it can participate in `cargo build`/`cargo test` on the
//! host.

#![cfg(target_arch = "wasm32")]

use std::time::Duration;

use arora_bridge::FakeBridge;
use arora_web::BrowserRuntime;
use vizij_api_core::TypedPath;
use vizij_arora_behavior::{BehaviorGraphSpec as GraphSpec, ProcessingGraph};
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;
use vizij_graph_core::types::NodeType;
use wasm_bindgen::prelude::*;

/// The store path the default proof graph reads from.
const GRAPH_INPUT: &str = "sensor/x";
/// The store path the default proof graph writes to.
const GRAPH_OUTPUT: &str = "actuator/y";

/// A running Vizij-on-Arora device, JS-callable.
///
/// Holds an [`arora_web::BrowserRuntime`] assembled over the Vizij interop
/// pieces; every method forwards to it.
#[wasm_bindgen]
pub struct VizijArora {
    inner: BrowserRuntime,
}

#[wasm_bindgen]
impl VizijArora {
    /// Start the device in the browser: inject a [`BlackboardStore`] +
    /// [`RigHal`] + [`FakeBridge`] into a [`BrowserRuntime`] with a
    /// [`ProcessingGraph`] as its behavior.
    ///
    /// `graph_json` is a Vizij graph spec (any form the spec normalizer
    /// accepts); its `input` nodes' paths become the store keys the graph
    /// reads each tick. Pass nothing to get the passthrough proof graph
    /// (`sensor/x` → `actuator/y`). Drive with [`step`](Self::step).
    pub async fn start(graph_json: Option<String>) -> Result<VizijArora, JsValue> {
        let spec = match graph_json {
            Some(json) => parse_graph(&json)?,
            None => parse_graph(&passthrough_json(GRAPH_INPUT, GRAPH_OUTPUT))?,
        };
        let inputs = input_paths(&spec);
        let graph = ProcessingGraph::from_spec(spec, inputs);

        let inner = BrowserRuntime::start(
            Box::new(RigHal::new()),
            Box::new(FakeBridge::new()),
            Box::new(BlackboardStore::new()),
            Box::new(graph),
        )
        .await?;

        Ok(VizijArora { inner })
    }

    /// Advance the device one tick. `dt_ms` is the wall time elapsed since the
    /// previous step, in milliseconds — the difference of two
    /// `requestAnimationFrame` timestamps. Returns `true` while live, `false`
    /// once the device is unregistered (stop stepping then).
    pub fn step(&mut self, dt_ms: f64) -> Result<bool, JsValue> {
        self.inner
            .step(Duration::from_secs_f64((dt_ms / 1000.0).max(0.0)))
    }

    /// Write one key into the store. `value_json` is an Arora `Value` as JSON,
    /// e.g. `{"f32": 0.75}`.
    #[wasm_bindgen(js_name = setValue)]
    pub fn set_value(&self, path: &str, value_json: &str) -> Result<(), JsValue> {
        self.inner.set_value(path, value_json)
    }

    /// Write several keys at once, as one store change. `values_json` is a JSON
    /// object mapping each key path to an Arora `Value`.
    #[wasm_bindgen(js_name = writeValues)]
    pub fn write_values(&self, values_json: &str) -> Result<(), JsValue> {
        self.inner.write_values(values_json)
    }

    /// Read keys from the store. `paths` is a JS `string[]`; the result maps
    /// each path to its Arora `Value` (or `null` if absent).
    #[wasm_bindgen(js_name = readValues)]
    pub fn read_values(&self, paths: JsValue) -> Result<JsValue, JsValue> {
        self.inner.read_values(paths)
    }

    /// A snapshot of every key currently in the store, as a path → `Value`
    /// object.
    pub fn snapshot(&self) -> Result<JsValue, JsValue> {
        self.inner.snapshot()
    }

    /// Drain the keys that changed since the last call, as a path → `Value`
    /// object (or `null` for a cleared key). Call it right after
    /// [`step`](Self::step).
    #[wasm_bindgen(js_name = drainChanges)]
    pub fn drain_changes(&self) -> Result<JsValue, JsValue> {
        self.inner.drain_changes()
    }
}

/// Normalize and deserialize a Vizij graph spec from JSON.
fn parse_graph(json: &str) -> Result<GraphSpec, JsValue> {
    let mut spec: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| JsValue::from_str(&format!("graph spec is not JSON: {e}")))?;
    vizij_api_core::json::normalize_graph_spec_value(&mut spec)
        .map_err(|e| JsValue::from_str(&format!("normalize graph spec failed: {e}")))?;
    serde_json::from_value(spec).map_err(|e| JsValue::from_str(&format!("invalid graph spec: {e}")))
}

/// The store paths the spec's `input` nodes read — what the graph subscribes
/// to on the device's store.
fn input_paths(spec: &GraphSpec) -> Vec<TypedPath> {
    spec.nodes
        .iter()
        .filter(|node| matches!(node.kind, NodeType::Input))
        .filter_map(|node| node.params.path.clone())
        .collect()
}

/// The passthrough proof graph (`input` path → `output` path), as spec JSON.
fn passthrough_json(input: &str, output: &str) -> String {
    serde_json::json!({
        "nodes": [
            { "id": "in",  "type": "input",  "params": { "path": input } },
            { "id": "out", "type": "output", "params": { "path": output } }
        ],
        "edges": [
            { "from": { "node_id": "in" }, "to": { "node_id": "out", "input": "in" } }
        ]
    })
    .to_string()
}
