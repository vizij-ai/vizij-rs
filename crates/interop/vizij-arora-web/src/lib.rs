//! Run a Vizij runtime *as* an Arora device in the browser.
//!
//! This is a thin wasm-bindgen wrapper: it composes an [`arora::Arora`] from
//! the Vizij interop pieces with [`arora::AroraBuilder`], wraps it with
//! [`arora_web::AroraWeb`] (the shared browser JS surface: `step`, the
//! self-pacing `run`, the Value↔JSON store accessors), and forwards the
//! JS-facing methods. Here we only choose the backends:
//!
//! - the store is a [`BlackboardStore`] (a Vizij `Blackboard` as an Arora
//!   `DataStore`);
//! - the HAL is a [`RigHal`] (a Vizij rig as an Arora HAL);
//! - the behavior is a [`ProcessingGraph`] (a Vizij node graph driven as the
//!   device's `BehaviorInterpreter`), built from the caller's graph spec.
//!
//! JavaScript constructs it with [`VizijArora::start`] (passing a Vizij graph
//! spec as JSON, in any accepted form — the spec normalizer runs device-side —
//! plus optionally the Arora wasm modules to load into the device's engine).
//! It then either hands the device to its own loop with
//! [`run`](VizijArora::run) or drives it one tick at a time with
//! [`step`](VizijArora::step); the rest of the surface stays live either way,
//! because none of it touches the stepping device. Values cross the JS
//! boundary as JSON in the Arora `Value` vocabulary (e.g. `{"f32": 0.75}`).
//!
//! Module functions are reached with [`call`](VizijArora::call); the running
//! graph is swapped whole with [`loadGraph`](VizijArora::load_graph) or edited
//! node-by-node with [`applyGraphEdits`](VizijArora::apply_graph_edits) (VIZ-79).
//! All dispatch through the device's in-process [`arora::Caller`], applied at
//! the next step like a remote's command, so none rebuilds the device (VIZ-57).
//!
//! This crate only carries content when built for `wasm32`; on the host it is
//! an empty shim so it can participate in `cargo build`/`cargo test` on the
//! host.

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;

use arora::{Arora, Caller, LocalCaller};
use arora_types::call::Call;
use arora_types::module::low::Header;
use arora_web::AroraWeb;
use uuid::Uuid;
use vizij_arora_behavior::{
    encode_edit_call, encode_load_call, parse_spec, parse_spec_diff, ProcessingGraph,
};
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// The store path the default proof graph reads from.
const GRAPH_INPUT: &str = "sensor/x";
/// The store path the default proof graph writes to.
const GRAPH_OUTPUT: &str = "actuator/y";

/// A Vizij-on-Arora device, JS-callable.
///
/// Wraps the composed [`Arora`] with [`arora_web::AroraWeb`] and keeps two
/// Vizij-side extras: the device's in-process [`LocalCaller`] (behind
/// [`call`](Self::call) and [`loadGraph`](Self::load_graph)) and the
/// `function id -> module id` map that routes a bare function reference to
/// the engine's by-module dispatch.
#[wasm_bindgen]
pub struct VizijArora {
    inner: AroraWeb,
    /// Dispatches in-process `Call`s into the device — enqueued at once,
    /// applied at the next step, resolved on that step's reply. Usable while
    /// [`run`](Self::run) owns the device.
    caller: LocalCaller,
    /// function id -> module id over every loaded module's exports, so a call
    /// (or a graph `ExternalFunction` node) can name just the function.
    function_modules: HashMap<Uuid, Uuid>,
}

#[wasm_bindgen]
impl VizijArora {
    /// Build the device: a [`BlackboardStore`] + [`RigHal`] + a
    /// [`ProcessingGraph`] behavior, composed with [`arora::AroraBuilder`].
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
    /// `ExternalFunction` nodes. The module set is fixed at build.
    ///
    /// Drive the device with [`run`](Self::run) (self-paced) or
    /// [`step`](Self::step) (your own clock).
    pub async fn start(
        graph_json: Option<String>,
        modules: Option<js_sys::Array>,
    ) -> Result<VizijArora, JsValue> {
        let spec = match graph_json {
            Some(json) => parse_spec(&json),
            None => parse_spec(&passthrough_json(GRAPH_INPUT, GRAPH_OUTPUT)),
        }
        .map_err(|e| JsValue::from_str(&e))?;
        let modules = parse_modules(modules)?;
        let function_modules = function_modules(&modules);

        let mut graph = ProcessingGraph::from_spec(spec).map_err(|e| JsValue::from_str(&e))?;
        graph.set_function_modules(function_modules.clone());

        let mut builder = Arora::builder()
            .with_hal(Box::new(RigHal::new()))
            .with_data_store(Box::new(BlackboardStore::new()))
            .with_behavior_interpreter(Box::new(graph));
        for (header, wasm) in modules {
            builder = builder.with_module(header, wasm);
        }
        let arora = builder
            .build()
            .map_err(|e| JsValue::from_str(&format!("arora build failed: {e:?}")))?;
        let caller = arora.caller();

        Ok(VizijArora {
            inner: AroraWeb::from(arora),
            caller,
            function_modules,
        })
    }

    /// Advance the device one step. `dt_ms` is the wall time elapsed since
    /// the previous step, in milliseconds — the difference of two
    /// `requestAnimationFrame` timestamps. Unavailable once
    /// [`run`](Self::run) has taken the device.
    pub fn step(&self, dt_ms: f64) -> Result<(), JsValue> {
        self.inner.step(dt_ms)
    }

    /// Hand the device to its own loop: a self-paced run at `period_ms`
    /// (default: the runtime's ~100 Hz). While it runs, [`step`](Self::step)
    /// is unavailable and the rest of the surface keeps working: it never
    /// touches the stepping device. A failing behavior tick does **not** end
    /// the loop — it stands as [`behavior_error`](Self::behavior_error) until
    /// a tick recovers; the promise rejects only if the runtime itself fails,
    /// and the device stays usable (steppable, runnable again) afterwards.
    pub fn run(&self, period_ms: Option<f64>) -> js_sys::Promise {
        self.inner.run(period_ms)
    }

    /// The behavior's standing error — the message of its latest failed
    /// tick, `undefined` while the behavior is healthy or none is installed.
    /// A failing tick does not stop the device or its [`run`](Self::run)
    /// loop; the reading stays available throughout.
    #[wasm_bindgen(getter, js_name = behaviorError)]
    pub fn behavior_error(&self) -> Option<String> {
        self.inner.behavior_error()
    }

    /// Resolves on the next change of the behavior's standing error, with
    /// the new reading: a message when a distinct failure appears,
    /// `undefined` when a tick recovers. Sequential awaits share one cursor,
    /// so no change is missed between them; one await may be pending at a
    /// time. Rejects when the device is gone.
    #[wasm_bindgen(js_name = behaviorErrorChanged)]
    pub fn behavior_error_changed(&self) -> js_sys::Promise {
        self.inner.behavior_error_changed()
    }

    /// Call a loaded module's function through the device. `call_json` is an
    /// Arora `Call` as JSON (`{ "id": <function uuid>, "args": [...] }`, args
    /// and result in the Arora `Value` vocabulary); `module_id` may be
    /// omitted when the function belongs to a module loaded at
    /// [`start`](Self::start).
    ///
    /// The call dispatches through the device's in-process caller inside the
    /// **next** step — the same phase a remote bridge command executes in —
    /// so the returned promise (of the `CallResult` as JSON) resolves only
    /// after that step runs. The device must be stepping (a [`run`](Self::run)
    /// loop, or your own [`step`](Self::step) calls) for it to land.
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
        Ok(self.dispatch(call))
    }

    /// Replace the device's running Vizij graph **in place** (VIZ-57).
    /// `graph_json` is a Vizij graph spec in any accepted form; it is encoded to
    /// the shared model's structural form and reaches the device's interpreter
    /// as the engine's LOAD call, so the store, the loaded modules, and the
    /// device itself all survive the swap.
    ///
    /// Applied — like any call — at the next step; the promise resolves once
    /// the new graph is installed and rejects if the spec does not parse.
    #[wasm_bindgen(js_name = loadGraph)]
    pub fn load_graph(&self, graph_json: &str) -> js_sys::Promise {
        let call = match parse_spec(graph_json).and_then(|spec| encode_load_call(&spec)) {
            Ok(call) => call,
            Err(e) => return js_sys::Promise::reject(&JsValue::from_str(&e)),
        };
        self.dispatch(call)
    }

    /// Edit the device's running Vizij graph **in place** (VIZ-79). `edits_json`
    /// is a spec-level graph diff — `upsert_nodes`, `remove_nodes`,
    /// `upsert_edges`, `remove_edges`, in the Vizij spec vocabulary — the editor
    /// computes from its change. It reaches the interpreter as the engine's EDIT
    /// call, applied at the next step, so unchanged nodes keep their runtime
    /// state (the edit patches the graph rather than reloading it) and the
    /// device survives.
    ///
    /// Every edge incident to an upserted node must be present in `upsert_edges`
    /// (an upserted node is removed then re-added). The promise resolves once the
    /// edit is applied and rejects if the diff does not parse.
    #[wasm_bindgen(js_name = applyGraphEdits)]
    pub fn apply_graph_edits(&self, edits_json: &str) -> js_sys::Promise {
        let call = match parse_spec_diff(edits_json).and_then(|diff| encode_edit_call(&diff)) {
            Ok(call) => call,
            Err(e) => return js_sys::Promise::reject(&JsValue::from_str(&e)),
        };
        self.dispatch(call)
    }

    /// Dispatch `call` through the in-process caller; the promise resolves to
    /// the `CallResult` as JSON after the step that applies it.
    ///
    /// `LocalCaller::call` enqueues the call synchronously inside `call()` —
    /// its future is only the reply — but `call()` itself runs at the
    /// composed future's first poll, and the promise machinery first polls in
    /// a microtask, after the current JS turn. The one poll here runs it now,
    /// so the call is enqueued **before this returns** and a manual driver
    /// can dispatch and step in the same turn
    /// (`device.loadGraph(spec); device.step(0);`). Parking on the throwaway
    /// waker loses no wakeup: the reply channel re-registers its waker on
    /// every poll.
    fn dispatch(&self, call: Call) -> js_sys::Promise {
        use std::future::Future;
        use std::task::{Context, Poll, Waker};

        let caller = self.caller.clone();
        let mut pending = Box::pin(async move { caller.call(call).await });
        // Ready on the first poll = the device is gone (the enqueue failed);
        // an applied call can only resolve through a later step.
        let first = pending
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()));
        wasm_bindgen_futures::future_to_promise(async move {
            let result = match first {
                Poll::Ready(result) => result,
                Poll::Pending => pending.await,
            }
            .map_err(|e| JsValue::from_str(&format!("call failed: {e}")))?;
            serde_json::to_string(&result)
                .map(|json| JsValue::from_str(&json))
                .map_err(|e| JsValue::from_str(&format!("serialize call result: {e}")))
        })
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
    /// object (or `null` for a cleared key). The first drain returns the
    /// store's whole current state — the subscription opens on it.
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
                JsValue::from_str(&format!(
                    "modules[{i}].headerJson is not a module header: {e}"
                ))
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
/// is routed to the engine's by-module dispatch.
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
