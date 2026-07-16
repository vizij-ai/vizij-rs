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
//! - the bridge is a [`JsBridge`], an in-process endpoint whose "remote" is the
//!   embedding JavaScript — [`VizijArora::call`] feeds it;
//! - the behavior is a [`ProcessingGraph`] (a Vizij node graph driven as the
//!   device's `BehaviorInterpreter`), built from the caller's graph spec.
//!
//! JavaScript constructs it with [`VizijArora::start`] (passing a Vizij graph
//! spec as JSON, in any accepted form — the spec normalizer runs here, plus
//! optionally the Arora wasm modules to load into the device's engine), drives
//! it one tick at a time with [`VizijArora::step`] from `requestAnimationFrame`
//! timestamps, and reads or writes keys on the injected store through the
//! forwarded accessors. Values cross the JS boundary as JSON in the Arora
//! `Value` vocabulary (e.g. `{"f32": 0.75}`). Module functions are reached with
//! [`VizijArora::call`], which dispatches through the device's step like any
//! bridge command.
//!
//! This crate only carries content when built for `wasm32`; on the host it is
//! an empty shim so it can participate in `cargo build`/`cargo test` on the
//! host.

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;
use std::time::Duration;

use arora_bridge::{
    Bridge, BridgeCommand, BridgeOp, BridgeResult, DeviceInfo, Inbound, InboundStream,
};
use arora_types::call::Call;
use arora_types::module::low::Header;
use arora_web::BrowserRuntime;
use async_trait::async_trait;
use futures_channel::{mpsc, oneshot};
use uuid::Uuid;
use vizij_api_core::TypedPath;
use vizij_arora_behavior::ProcessingGraph;
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;
use vizij_graph_core::types::{GraphSpec, NodeType};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// The store path the default proof graph reads from.
const GRAPH_INPUT: &str = "sensor/x";
/// The store path the default proof graph writes to.
const GRAPH_OUTPUT: &str = "actuator/y";

/// An in-process bridge endpoint whose "remote" is the embedding JavaScript.
///
/// Commands enqueued by [`VizijArora::call`] arrive on the device's inbound
/// stream and dispatch during the next step's sweep — the same seam a remote
/// (Studio) bridge command takes, so a JS call behaves exactly like a bridged
/// one: it executes inside `step`, through the engine's `CallBridge`, and
/// replies on its one-shot channel. Outbound state changes are dropped (this
/// endpoint has no remote store).
struct JsBridge {
    /// Handed over (once) by [`Bridge::take_inbound`].
    inbound: Option<mpsc::UnboundedReceiver<Inbound>>,
}

impl JsBridge {
    /// The endpoint plus the sender JS-side methods enqueue commands on.
    fn new() -> (Self, mpsc::UnboundedSender<Inbound>) {
        let (tx, rx) = mpsc::unbounded();
        (Self { inbound: Some(rx) }, tx)
    }
}

#[async_trait]
impl Bridge for JsBridge {
    fn take_inbound(&mut self) -> InboundStream {
        Box::pin(self.inbound.take().expect("inbound stream already taken"))
    }

    fn try_send(&mut self, _change: &arora_types::data::StateChange) {}

    async fn get_device_info(&self) -> BridgeResult<Option<DeviceInfo>> {
        Ok(None)
    }

    async fn update_device_info(
        &self,
        info: Option<DeviceInfo>,
    ) -> BridgeResult<Option<DeviceInfo>> {
        Ok(info)
    }
}

/// A running Vizij-on-Arora device, JS-callable.
///
/// Holds an [`arora_web::BrowserRuntime`] assembled over the Vizij interop
/// pieces; every method forwards to it.
#[wasm_bindgen]
pub struct VizijArora {
    inner: BrowserRuntime,
    /// Where [`call`](Self::call) enqueues commands for the device's
    /// [`JsBridge`]; the next step dispatches them.
    commands: mpsc::UnboundedSender<Inbound>,
    /// function id -> module id over every loaded module's exports, so a call
    /// (or a graph `ExternalFunction` node) can name just the function.
    function_modules: HashMap<Uuid, Uuid>,
}

#[wasm_bindgen]
impl VizijArora {
    /// Start the device in the browser: inject a [`BlackboardStore`] +
    /// [`RigHal`] + a [`JsBridge`] into a [`BrowserRuntime`] with a
    /// [`ProcessingGraph`] as its behavior.
    ///
    /// `graph_json` is a Vizij graph spec (any form the spec normalizer
    /// accepts); its `input` nodes' paths become the store keys the graph
    /// reads each tick. Pass nothing to get the passthrough proof graph
    /// (`sensor/x` → `actuator/y`).
    ///
    /// `modules` optionally loads Arora wasm modules into the device's engine:
    /// a JS array of `{ headerJson, wasmBytes }` (the module's header as JSON
    /// and its `.wasm` bytes as a `Uint8Array`). Their functions are then
    /// reachable with [`call`](Self::call) and from the graph's
    /// `ExternalFunction` nodes. Drive with [`step`](Self::step).
    pub async fn start(
        graph_json: Option<String>,
        modules: Option<js_sys::Array>,
    ) -> Result<VizijArora, JsValue> {
        let spec = match graph_json {
            Some(json) => parse_graph(&json)?,
            None => parse_graph(&passthrough_json(GRAPH_INPUT, GRAPH_OUTPUT))?,
        };
        let modules = parse_modules(modules)?;
        let function_modules = function_modules(&modules);

        let inputs = input_paths(&spec);
        let mut graph = ProcessingGraph::from_spec(spec, inputs);
        graph.set_function_modules(function_modules.clone());

        let (bridge, commands) = JsBridge::new();
        let mut builder = BrowserRuntime::builder()
            .with_hal(Box::new(RigHal::new()))
            .with_bridge(Box::new(bridge))
            .with_data_store(Box::new(BlackboardStore::new()))
            .with_behavior_interpreter(Box::new(graph));
        for (header, wasm) in modules {
            builder = builder.with_module(header, wasm);
        }
        let inner = builder.build()?;

        Ok(VizijArora {
            inner,
            commands,
            function_modules,
        })
    }

    /// Call a loaded module's function through the device. `call_json` is an
    /// Arora `Call` as JSON (`{ "id": <function uuid>, "args": [...] }`, args
    /// and result in the Arora `Value` vocabulary); `module_id` may be omitted
    /// when the function belongs to a module loaded at
    /// [`start`](Self::start).
    ///
    /// The call dispatches inside the device's **next** [`step`](Self::step)
    /// — the same phase a remote bridge command executes in — so the returned
    /// promise (of the `CallResult` as JSON) resolves only after that step
    /// runs. Under a `requestAnimationFrame` loop just `await` it; a direct
    /// driver calls `step` in between.
    pub fn call(&self, call_json: &str) -> Result<js_sys::Promise, JsValue> {
        let mut call: Call = serde_json::from_str(call_json)
            .map_err(|e| JsValue::from_str(&format!("invalid call json: {e}")))?;
        if call.module_id.is_none() {
            call.module_id = self.function_modules.get(&call.id).copied();
        }
        if call.module_id.is_none() {
            return Err(JsValue::from_str(&format!(
                "no loaded module exports function {}; pass module_id explicitly",
                call.id
            )));
        }

        let (reply, response) = oneshot::channel();
        self.commands
            .unbounded_send(Inbound::Command(BridgeCommand::new(
                BridgeOp::Call(call),
                reply,
            )))
            .map_err(|_| JsValue::from_str("the device no longer accepts commands"))?;

        Ok(wasm_bindgen_futures::future_to_promise(async move {
            let result = response
                .await
                .map_err(|_| JsValue::from_str("the device dropped the call unanswered"))?
                .map_err(|e| JsValue::from_str(&e))?;
            serde_json::to_string(&result)
                .map(|json| JsValue::from_str(&json))
                .map_err(|e| JsValue::from_str(&format!("serialize call result: {e}")))
        }))
    }

    /// Advance the device one tick. `dt_ms` is the wall time elapsed since the
    /// previous step, in milliseconds — the difference of two
    /// `requestAnimationFrame` timestamps. Returns `true` while live, `false`
    /// once the device is unregistered (stop stepping then).
    pub fn step(&mut self, dt_ms: f64) -> Result<bool, JsValue> {
        self.inner
            .step(Duration::from_secs_f64((dt_ms / 1000.0).max(0.0)))
    }

    /// Write one key into the store. `value_json` is a value in any accepted
    /// vizij payload form — the canonical Arora `Value` serde (`{"f32": 0.75}`)
    /// or a vizij shorthand (`{"float": 0.75}`, `{"vec3": [1, 2, 3]}`, …) — which
    /// is normalized to the canonical form the store deserializes.
    #[wasm_bindgen(js_name = setValue)]
    pub fn set_value(&self, path: &str, value_json: &str) -> Result<(), JsValue> {
        self.inner
            .set_value(path, &normalize_value_str(value_json)?)
    }

    /// Write several keys at once, as one store change. `values_json` is a JSON
    /// object mapping each key path to a value (canonical Arora `Value` serde or
    /// a vizij shorthand — each is normalized to the canonical form).
    #[wasm_bindgen(js_name = writeValues)]
    pub fn write_values(&self, values_json: &str) -> Result<(), JsValue> {
        self.inner
            .write_values(&normalize_values_map_str(values_json)?)
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

/// Decode the `modules` constructor argument: a JS array of
/// `{ headerJson: string, wasmBytes: Uint8Array }` into `(Header, bytes)`
/// pairs ready for `with_module`. `None` means no modules.
fn parse_modules(modules: Option<js_sys::Array>) -> Result<Vec<(Header, Vec<u8>)>, JsValue> {
    let Some(modules) = modules else {
        return Ok(Vec::new());
    };
    modules
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let header_json = js_sys::Reflect::get(&entry, &JsValue::from_str("headerJson"))?
                .as_string()
                .ok_or_else(|| {
                    JsValue::from_str(&format!("modules[{i}].headerJson must be a string"))
                })?;
            let header: Header = serde_json::from_str(&header_json).map_err(|e| {
                JsValue::from_str(&format!("modules[{i}].headerJson is not a module header: {e}"))
            })?;
            let bytes: js_sys::Uint8Array =
                js_sys::Reflect::get(&entry, &JsValue::from_str("wasmBytes"))?
                    .dyn_into()
                    .map_err(|_| {
                        JsValue::from_str(&format!("modules[{i}].wasmBytes must be a Uint8Array"))
                    })?;
            Ok((header, bytes.to_vec()))
        })
        .collect()
}

/// function id -> module id over every module's exports — how a bare function
/// reference (a JS call without `module_id`, a graph `ExternalFunction` node)
/// is routed to `arora_call`'s by-module dispatch.
fn function_modules(modules: &[(Header, Vec<u8>)]) -> HashMap<Uuid, Uuid> {
    modules
        .iter()
        .flat_map(|(header, _)| {
            header
                .exports
                .iter()
                .map(move |export| (*export.id(), header.id))
        })
        .collect()
}

/// Normalize and deserialize a Vizij graph spec from JSON.
fn parse_graph(json: &str) -> Result<GraphSpec, JsValue> {
    let mut spec: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| JsValue::from_str(&format!("graph spec is not JSON: {e}")))?;
    vizij_api_core::json::normalize_graph_spec_value(&mut spec)
        .map_err(|e| JsValue::from_str(&format!("normalize graph spec failed: {e}")))?;
    serde_json::from_value(spec).map_err(|e| JsValue::from_str(&format!("invalid graph spec: {e}")))
}

/// Normalize one value's JSON from any accepted vizij payload form (canonical
/// Arora `Value` serde, or a vizij shorthand like `{"float": 0.5}` /
/// `{"vec3": [1, 2, 3]}`) to the canonical form the store deserializes. Values
/// the normalizer doesn't recognize are passed through unchanged.
fn normalize_value_str(value_json: &str) -> Result<String, JsValue> {
    let value: serde_json::Value = serde_json::from_str(value_json)
        .map_err(|e| JsValue::from_str(&format!("value is not JSON: {e}")))?;
    let normalized = vizij_api_core::json::normalize_value_json(value);
    serde_json::to_string(&normalized)
        .map_err(|e| JsValue::from_str(&format!("serialize value: {e}")))
}

/// Normalize every value in a `{path: value}` write batch to the canonical Arora
/// `Value` serde (see [`normalize_value_str`]). Keys are left untouched.
fn normalize_values_map_str(values_json: &str) -> Result<String, JsValue> {
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(values_json)
        .map_err(|e| JsValue::from_str(&format!("values are not a JSON object: {e}")))?;
    let normalized: serde_json::Map<String, serde_json::Value> = map
        .into_iter()
        .map(|(path, value)| (path, vizij_api_core::json::normalize_value_json(value)))
        .collect();
    serde_json::to_string(&normalized)
        .map_err(|e| JsValue::from_str(&format!("serialize values: {e}")))
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
