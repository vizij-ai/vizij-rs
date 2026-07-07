//! Run a Vizij runtime *as* an Arora device in the browser.
//!
//! This is a thin wasm-bindgen wrapper: it assembles the Vizij interop pieces
//! and hands them to [`arora_web::BrowserRuntime`], the reusable browser-runtime
//! primitive, then forwards the JS-facing methods. All the runtime scaffolding
//! (the `arora::Runtime`, the io pump, the Value↔JSON store accessors) lives in
//! `arora-web`; here we only choose the backends:
//!
//! - the store is a [`BlackboardStore`] (a Vizij `Blackboard` as an Arora
//!   `DataStore`);
//! - the HAL is a [`RigHal`] (a Vizij rig as an Arora HAL);
//! - the bridge is the in-process [`FakeBridge`];
//! - the queued behavior is a [`ProcessingGraph`] (a Vizij node graph driven as
//!   an Arora `Behavior`).
//!
//! JavaScript constructs it with [`VizijArora::start`], drives it one tick at a
//! time with [`VizijArora::step`] (e.g. from `requestAnimationFrame`), and reads
//! or writes keys on the injected store through the forwarded accessors. Values
//! cross the JS boundary as JSON in the Arora `Value` vocabulary (e.g.
//! `{"f32": 0.75}`).
//!
//! This crate only carries content when built for `wasm32`; on the host it is an
//! empty shim so it can participate in `cargo build`/`cargo test` on the host.

#![cfg(target_arch = "wasm32")]

use std::sync::Arc;

use arora_bridge::FakeBridge;
use arora_web::BrowserRuntime;
use vizij_api_core::TypedPath;
use vizij_arora_behavior::ProcessingGraph;
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;
use vizij_graph_core::types::GraphSpec;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn _start() {
    console_error_panic_hook::set_once();
}

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
    /// Start the device in the browser: inject a [`BlackboardStore`] + [`RigHal`]
    /// + [`FakeBridge`] into a [`BrowserRuntime`], queue a passthrough
    /// [`ProcessingGraph`] (`sensor/x` → `actuator/y`), and let the primitive spawn
    /// the async io pump. Drive it by calling [`step`](Self::step).
    pub async fn start() -> Result<VizijArora, JsValue> {
        let store = Arc::new(BlackboardStore::new());
        let hal = Arc::new(RigHal::new());
        let bridge = Arc::new(FakeBridge::new());

        let mut inner = BrowserRuntime::start(hal, bridge, store).await?;

        let graph = passthrough_graph(GRAPH_INPUT, GRAPH_OUTPUT)?;
        inner.queue_behavior(Box::new(graph));

        Ok(VizijArora { inner })
    }

    /// Advance the device one tick. Returns `true` while live, `false` once it is
    /// unregistered (stop stepping then).
    pub fn step(&mut self) -> Result<bool, JsValue> {
        self.inner.step()
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

    /// Read keys from the store. `paths` is a JS `string[]`; the result maps each
    /// path to its Arora `Value` (or `null` if absent).
    #[wasm_bindgen(js_name = readValues)]
    pub fn read_values(&self, paths: JsValue) -> Result<JsValue, JsValue> {
        self.inner.read_values(paths)
    }

    /// A snapshot of every key currently in the store, as a path → `Value` object.
    pub fn snapshot(&self) -> Result<JsValue, JsValue> {
        self.inner.snapshot()
    }

    /// Drain the keys that changed since the last call, as a path → `Value`
    /// object (or `null` for a cleared key). Call it right after [`step`](Self::step).
    #[wasm_bindgen(js_name = drainChanges)]
    pub fn drain_changes(&self) -> Result<JsValue, JsValue> {
        self.inner.drain_changes()
    }
}

/// Build a passthrough Vizij node graph (`input` path → `output` path) driven as
/// an Arora behavior. Mirrors the minimal construction the Vizij graph crates use
/// for JS: author the spec as JSON, normalize it, deserialize a [`GraphSpec`].
fn passthrough_graph(input: &str, output: &str) -> Result<ProcessingGraph, JsValue> {
    let mut spec = serde_json::json!({
        "nodes": [
            { "id": "in",  "type": "input",  "params": { "path": input } },
            { "id": "out", "type": "output", "params": { "path": output } }
        ],
        "edges": [
            { "from": { "node_id": "in" }, "to": { "node_id": "out", "input": "in" } }
        ]
    });
    vizij_api_core::json::normalize_graph_spec_value(&mut spec)
        .map_err(|e| JsValue::from_str(&format!("normalize graph spec failed: {e}")))?;
    let spec: GraphSpec = serde_json::from_value(spec)
        .map_err(|e| JsValue::from_str(&format!("invalid graph spec: {e}")))?;
    let inputs =
        vec![TypedPath::parse(input)
            .map_err(|e| JsValue::from_str(&format!("bad input path: {e}")))?];
    Ok(ProcessingGraph::from_spec(spec, inputs))
}
