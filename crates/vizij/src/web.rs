//! The browser module: the view and the face's Arora in one wasm module, one App
//! per page, faces as viewports.
//!
//! JavaScript [`mount`]s the page's canvas once — one Bevy [`App`] for the
//! page's lifetime — then [`load_face`]s as many faces as it shows: each is a
//! device of its own ([`FaceRuntime`]), JS-paced (the page calls `step(dt)`
//! each frame, or hands the device its own `run` loop), drawn by the App into
//! the rectangle of the canvas the page [`place_face`]s it in. The view reads
//! each device's pose on its own frame, so a step's writes draw one frame
//! later. The page owns the pacing: Bevy's frames run on
//! `requestAnimationFrame`, which a hidden tab halts, while a device under
//! `run` keeps stepping on its timer.
//!
//! Values cross the boundary as JSON in the Arora `Value` vocabulary
//! (`{"f32": 0.75}`), calls as Arora `Call`s, and a task run as the
//! interpreter's `TaskHandle`; the wrapper package `@vizij/runtime` gives it
//! all idiomatic JS types.
//!
//! What the App holds is reachable only through this module's statics: on
//! the web `App::run` hands the App to winit and returns at once, so the
//! page's requests travel as [`ViewEvent`]s and the App's answers (which
//! faces are ready, what was picked) are copied out each frame.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Mutex;

use arora::{Caller, LocalCaller};
use arora_behavior::{interpreter_module, RunPolicy, TaskHandle};
use arora_types::call::Call;
use arora_web::AroraWeb;
use bevy::prelude::*;
use uuid::Uuid;
use vizij_arora_behavior::{encode_edit_call, encode_load_call, parse_spec, parse_spec_diff};
use vizij_arora_hal::RigHal;
use vizij_arora_host::ProgramSelect;
use vizij_arora_store::BlackboardStore;
use wasm_bindgen::prelude::*;

use crate::face::{self, FaceConfig, LoadedFace};
use crate::modules::animation;
use crate::view::meta::FaceMeta;
use crate::view::{self, FaceAssets, Fit, Picked, Picks, ViewEvent, ViewEvents, ViewOptions};

/// The mounted page: the way into the App.
struct Page {
    events: Sender<ViewEvent>,
}

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
}

/// What the App reports back each frame: the faces whose scene is indexed,
/// and the picks since the page last drained them.
static READY: Mutex<Vec<String>> = Mutex::new(Vec::new());
static PICKS: Mutex<Vec<Picked>> = Mutex::new(Vec::new());

/// The App's report to the page.
fn report(faces: Query<&view::Face>, mut picks: ResMut<Picks>) {
    if let Ok(mut ready) = READY.lock() {
        ready.clear();
        ready.extend(
            faces
                .iter()
                .filter(|face| face.bindings.ready)
                .map(|face| face.id.clone()),
        );
    }
    if !picks.0.is_empty() {
        if let Ok(mut out) = PICKS.lock() {
            out.append(&mut picks.0);
        }
    }
}

/// Create the page's one App over `canvas` (a CSS selector): the view, its
/// events, no face yet. `options_json`, every field optional:
/// - `background`: `RRGGBB`; absent, the canvas stays transparent wherever
///   nothing is drawn, and each face's rectangle too;
/// - `fit`: `contain` (default), `cover` or `stretch`;
/// - `zoom`: one factor, or `[x, y]`;
/// - `ambient`: the three.js-style ambient intensity (default π/2);
/// - `unlit`: render pure albedo.
///
/// Mounting twice is an error; a page has one App.
#[wasm_bindgen]
pub fn mount(canvas: String, options_json: Option<String>) -> Result<(), JsValue> {
    if PAGE.with(|page| page.borrow().is_some()) {
        return Err(JsValue::from_str("already mounted: a page has one App"));
    }
    let _ = console_log::init_with_level(log::Level::Info);
    let options = parse_view_options(options_json.as_deref())?;
    let (events_tx, events_rx) = std::sync::mpsc::channel();

    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Vizij".into(),
                    canvas: Some(canvas),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: false,
                    transparent: true,
                    ..default()
                }),
                ..default()
            })
            // The faces are served from memory ([`FaceAssets`]); no `.meta`
            // file exists to probe for, and a probe would be a fetch.
            .set(bevy::asset::AssetPlugin {
                meta_check: bevy::asset::AssetMetaCheck::Never,
                ..default()
            })
            // `log` carries Bevy's own events to the console (`tracing`'s
            // `log` feature); the page owns the sink.
            .disable::<bevy::log::LogPlugin>(),
    )
    // Wherever no face's rectangle draws, the canvas shows the page.
    .insert_resource(ClearColor(Color::NONE))
    .insert_resource(options)
    .insert_resource(ViewEvents(Mutex::new(events_rx)))
    .add_plugins(view::ViewPlugin)
    .add_systems(PostUpdate, report);
    PAGE.with(|page| *page.borrow_mut() = Some(Page { events: events_tx }));
    // On the web the winit runner spawns the loop and returns at once.
    app.run();
    Ok(())
}

/// Send a request to the mounted App.
fn send(event: ViewEvent) -> Result<(), JsValue> {
    PAGE.with(|page| match page.borrow().as_ref() {
        Some(page) => page
            .events
            .send(event)
            .map_err(|_| JsValue::from_str("the App is gone")),
        None => Err(JsValue::from_str("not mounted: call mount(canvas) first")),
    })
}

/// Show a face under `face_id` and start its device: the GLB's bindings and
/// bundle are read, its graphs composed (`options_json` as
/// [`compose_face`]'s: `graphs`, `program`, `ros4hri`, plus `stageNeutral`,
/// default `true`), the device built over `RigHal` + `BlackboardStore` with
/// the animation, gaze and viseme modules, and the scene queued for the App.
/// A face already shown under `face_id` is replaced. The device comes back
/// JS-owned: step it, or `run` it, and `free` it after [`unload_face`].
#[wasm_bindgen(js_name = loadFace)]
pub fn load_face(
    face_id: String,
    glb: Vec<u8>,
    options_json: Option<String>,
) -> Result<FaceRuntime, JsValue> {
    let (config, stage_neutral) = parse_face_config(options_json.as_deref())?;
    let LoadedFace { meta, spec } =
        face::load_face(&glb, &config).map_err(|e| JsValue::from_str(&format!("{e:#}")))?;
    let rig = RigHal::new();
    let store = BlackboardStore::new();
    if stage_neutral {
        face::stage_neutral_pose(&store, &meta);
    }
    let builder = face::builder_for(&spec, rig.clone(), store, &meta.bundle.skills)
        .ok_or_else(|| JsValue::from_str("the composed graph does not encode (see the console)"))?;
    let arora = builder
        .build()
        .map_err(|e| JsValue::from_str(&format!("arora build failed: {e:?}")))?;
    let caller = arora.caller();
    let rig_prefix = meta.bundle.rig_prefix();
    // The bytes move into the App's asset source; the copy wasm-bindgen made
    // from the page's buffer is the only one.
    send(ViewEvent::LoadFace {
        face_id: face_id.clone(),
        meta: Box::new(meta),
        glb,
        rig,
    })?;
    Ok(FaceRuntime {
        face_id,
        rig_prefix,
        inner: AroraWeb::from(arora),
        caller,
        function_modules: animation::function_modules(),
    })
}

/// Take the face down: its scene, its camera, its GLB. The device the page
/// holds keeps stepping until the page frees it.
#[wasm_bindgen(js_name = unloadFace)]
pub fn unload_face(face_id: String) -> Result<(), JsValue> {
    send(ViewEvent::UnloadFace { face_id })
}

/// Confine the face's camera to a rectangle of the canvas, in CSS pixels
/// relative to the canvas (`x`, `y` from its top-left corner) — how several
/// faces share one canvas. The rectangle is kept across the face's reloads.
#[wasm_bindgen(js_name = placeFace)]
pub fn place_face(face_id: String, x: f64, y: f64, width: f64, height: f64) -> Result<(), JsValue> {
    let scale = device_pixel_ratio();
    let px = |v: f64| (v * scale).round().max(0.0) as u32;
    send(ViewEvent::PlaceFace {
        face_id,
        rect: Some([px(x), px(y), px(width), px(height)]),
    })
}

/// Give the face's camera the whole canvas again.
#[wasm_bindgen(js_name = fillCanvas)]
pub fn fill_canvas(face_id: String) -> Result<(), JsValue> {
    send(ViewEvent::PlaceFace {
        face_id,
        rect: None,
    })
}

/// Whether the face's scene has spawned and its bindings are joined — from
/// then on its device's pose shows.
#[wasm_bindgen]
pub fn ready(face_id: String) -> bool {
    READY
        .lock()
        .map(|ready| ready.contains(&face_id))
        .unwrap_or(false)
}

/// The pointer presses on faces since the last drain, oldest first, as
/// `{ faceId, elementId }` — the ids the face's RobotData declares.
#[wasm_bindgen(js_name = drainPicks)]
pub fn drain_picks() -> Result<JsValue, JsValue> {
    let picks: Vec<Picked> = PICKS
        .lock()
        .map(|mut p| p.drain(..).collect())
        .unwrap_or_default();
    let json = serde_json::Value::Array(
        picks
            .into_iter()
            .map(|pick| serde_json::json!({ "faceId": pick.face_id, "elementId": pick.element_id }))
            .collect(),
    );
    js_sys::JSON::parse(&json.to_string())
}

/// The module's linear memory, in bytes — what a page watches across face
/// loads and unloads. Linear memory never shrinks; a flat reading over many
/// cycles is what "nothing leaks" looks like.
#[wasm_bindgen(js_name = memoryBytes)]
pub fn memory_bytes() -> f64 {
    (core::arch::wasm32::memory_size(0) * 65536) as f64
}

/// What a GLB declares, without loading it: `{ faceId, rigPrefix, rootBounds,
/// elements: [{ id, name, kind, material, morphTargets }], animatables:
/// { <uuid>: { node, feature } }, graphs: [{ kind }], programs: [id],
/// activeProgramId, neutralInputs }` — what a page needs to build its
/// controls and name the face's paths.
#[wasm_bindgen]
pub fn describe(glb: &[u8]) -> Result<JsValue, JsValue> {
    let meta = FaceMeta::from_glb_bytes(glb).map_err(|e| JsValue::from_str(&format!("{e:#}")))?;
    js_sys::JSON::parse(&describe_json(&meta).to_string())
}

/// [`describe`]'s document.
fn describe_json(meta: &FaceMeta) -> serde_json::Value {
    let mut animatables: Vec<(String, serde_json::Value)> = meta
        .animatables
        .iter()
        .map(|(uuid, binding)| {
            (
                uuid.to_string(),
                serde_json::json!({ "node": binding.node_name, "feature": binding.feature.name() }),
            )
        })
        .collect();
    animatables.sort_by(|a, b| a.0.cmp(&b.0));
    let mut neutral: Vec<(String, serde_json::Value)> = meta
        .bundle
        .neutral_inputs
        .iter()
        .map(|(name, value)| (name.clone(), serde_json::json!(value)))
        .collect();
    neutral.sort_by(|a, b| a.0.cmp(&b.0));
    serde_json::json!({
        "faceId": meta.bundle.face_id,
        "rigPrefix": meta.bundle.rig_prefix(),
        "rootBounds": meta.root_bounds.map(|(cx, cy, w, h)| serde_json::json!({
            "center": { "x": cx, "y": cy }, "size": { "x": w, "y": h }
        })),
        "elements": meta.elements.iter().map(|e| serde_json::json!({
            "id": e.id,
            "name": e.node_name,
            "kind": e.kind,
            "material": e.material,
            "morphTargets": e.morph_targets,
        })).collect::<Vec<_>>(),
        "animatables": animatables.into_iter().collect::<serde_json::Map<String, serde_json::Value>>(),
        "graphs": meta.bundle.graphs.iter().map(|(kind, _)| serde_json::json!({ "kind": kind })).collect::<Vec<_>>(),
        "programs": meta.bundle.programs.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        "activeProgramId": meta.bundle.active_program_id,
        "neutralInputs": neutral.into_iter().collect::<serde_json::Map<String, serde_json::Value>>(),
    })
}

/// A face's device, JS-owned: the composed [`Arora`] behind
/// [`arora_web::AroraWeb`]'s surface (`step`, `run`, `stop`, the store
/// accessors), the in-process caller behind `call`, `loadGraph`,
/// `applyGraphEdits`, `spawn` and `halt`.
#[wasm_bindgen]
pub struct FaceRuntime {
    face_id: String,
    rig_prefix: String,
    inner: AroraWeb,
    /// Dispatches in-process `Call`s into the device — enqueued at once,
    /// applied at the next step, resolved on that step's reply. Usable while
    /// `run` owns the device.
    caller: LocalCaller,
    /// function id -> module id over the host modules, so a call can name
    /// just the function.
    function_modules: HashMap<Uuid, Uuid>,
}

#[wasm_bindgen]
impl FaceRuntime {
    /// A device with no face: `graph_json` (any form the spec normalizer
    /// accepts) as its behavior over a fresh store and rig, the animation
    /// module host-linked. Nothing to draw; a test bench, a graph run in
    /// Node. Omit the graph for the passthrough proof graph
    /// (`sensor/x` → `actuator/y`).
    #[wasm_bindgen(js_name = fromGraph)]
    pub fn from_graph(graph_json: Option<String>) -> Result<FaceRuntime, JsValue> {
        let spec = match graph_json {
            Some(json) => json,
            None => passthrough_json("sensor/x", "actuator/y"),
        };
        parse_spec(&spec).map_err(|e| JsValue::from_str(&e))?;
        let builder = face::builder_for(&spec, RigHal::new(), BlackboardStore::new(), &[])
            .ok_or_else(|| JsValue::from_str("the graph does not encode (see the console)"))?;
        let arora = builder
            .build()
            .map_err(|e| JsValue::from_str(&format!("arora build failed: {e:?}")))?;
        let caller = arora.caller();
        Ok(FaceRuntime {
            face_id: String::new(),
            rig_prefix: String::new(),
            inner: AroraWeb::from(arora),
            caller,
            function_modules: animation::function_modules(),
        })
    }

    /// The slot the face is shown under (empty for a device with no face).
    #[wasm_bindgen(getter, js_name = faceId)]
    pub fn face_id(&self) -> String {
        self.face_id.clone()
    }

    /// The prefix the face's own paths live under (`rig/<faceId>/`), empty
    /// when the GLB names no face.
    #[wasm_bindgen(getter, js_name = rigPrefix)]
    pub fn rig_prefix(&self) -> String {
        self.rig_prefix.clone()
    }

    /// Advance the device one step. `dt_ms` is the wall time elapsed since
    /// the previous step, in milliseconds. Unavailable while `run` owns the
    /// device.
    pub fn step(&self, dt_ms: f64) -> Result<(), JsValue> {
        self.inner.step(dt_ms)
    }

    /// Hand the device its own loop at `period_ms` (default ~100 Hz) until
    /// `stop`. The rest of the surface keeps working meanwhile. A failing
    /// behavior tick does not end the loop — it stands as `behaviorError`
    /// until a tick recovers; the promise rejects only if the runtime itself
    /// fails.
    pub fn run(&self, period_ms: Option<f64>) -> js_sys::Promise {
        self.inner.run(period_ms)
    }

    /// Reclaim the device from `run`: the loop ends at its next step
    /// boundary and `step` works again.
    pub fn stop(&self) {
        self.inner.stop()
    }

    /// Whether `run` currently owns the device.
    #[wasm_bindgen(getter)]
    pub fn running(&self) -> bool {
        self.inner.running()
    }

    /// The behavior's standing error — the message of its latest failed
    /// tick, `undefined` while it is healthy.
    #[wasm_bindgen(getter, js_name = behaviorError)]
    pub fn behavior_error(&self) -> Option<String> {
        self.inner.behavior_error()
    }

    /// Resolves on the next change of the standing error, with the new
    /// reading; one await may be pending at a time.
    #[wasm_bindgen(js_name = behaviorErrorChanged)]
    pub fn behavior_error_changed(&self) -> js_sys::Promise {
        self.inner.behavior_error_changed()
    }

    /// Call a module function through the device. `call_json` is an Arora
    /// `Call` as JSON; `module_id` may be omitted for the animation module's
    /// functions. Dispatched inside the **next** step, so the promise (of the
    /// `CallResult` as JSON) resolves once that step has run.
    pub fn call(&self, call_json: &str) -> Result<js_sys::Promise, JsValue> {
        let mut call: Call = serde_json::from_str(call_json)
            .map_err(|e| JsValue::from_str(&format!("invalid call json: {e}")))?;
        if call.module_id.is_none() {
            call.module_id = self.function_modules.get(&call.id).copied();
        }
        if call.module_id.is_none() {
            return Err(JsValue::from_str(&format!(
                "no host module exports function {}; pass module_id explicitly",
                call.id
            )));
        }
        Ok(self.dispatch(call))
    }

    /// Spawn `call_json` (an Arora `Call` naming a described function, `say`
    /// or `look_at` or `play_viseme`) as a task run on the device's
    /// interpreter — what a bridge does for a remote. Resolves to the run's
    /// `TaskHandle` as JSON: its `status` key to watch, its `feedback` keys,
    /// and the `stop` call [`halt`](Self::halt) issues.
    pub fn spawn(&self, call_json: &str) -> Result<js_sys::Promise, JsValue> {
        let call: Call = serde_json::from_str(call_json)
            .map_err(|e| JsValue::from_str(&format!("invalid call json: {e}")))?;
        let spawn = interpreter_module::encode_spawn(&call, RunPolicy::Concurrent);
        let caller = self.caller.clone();
        Ok(dispatch_with(caller, spawn, |result| {
            let handle = interpreter_module::decode_spawn_result(&result.ret)?;
            serde_json::to_string(&handle).map_err(|e| format!("serialize the handle: {e}"))
        }))
    }

    /// Halt a run: `handle_json` is the `TaskHandle` `spawn` resolved to.
    /// Resolves once the halt has been applied (the run's status key then
    /// reads terminal).
    pub fn halt(&self, handle_json: &str) -> Result<js_sys::Promise, JsValue> {
        let handle: TaskHandle = serde_json::from_str(handle_json)
            .map_err(|e| JsValue::from_str(&format!("invalid task handle: {e}")))?;
        Ok(self.dispatch(handle.stop))
    }

    /// Replace the device's running graph **in place**: the store, the
    /// modules and the device survive the swap. Applied at the next step.
    #[wasm_bindgen(js_name = loadGraph)]
    pub fn load_graph(&self, graph_json: &str) -> js_sys::Promise {
        let call = match parse_spec(graph_json).and_then(|spec| encode_load_call(&spec)) {
            Ok(call) => call,
            Err(e) => return js_sys::Promise::reject(&JsValue::from_str(&e)),
        };
        self.dispatch(call)
    }

    /// Edit the running graph **in place**: `edits_json` is a spec-level
    /// diff (`upsert_nodes`, `remove_nodes`, `upsert_edges`, `remove_edges`).
    /// Unchanged nodes keep their state. Applied at the next step.
    #[wasm_bindgen(js_name = applyGraphEdits)]
    pub fn apply_graph_edits(&self, edits_json: &str) -> js_sys::Promise {
        let call = match parse_spec_diff(edits_json).and_then(|diff| encode_edit_call(&diff)) {
            Ok(call) => call,
            Err(e) => return js_sys::Promise::reject(&JsValue::from_str(&e)),
        };
        self.dispatch(call)
    }

    /// Dispatch `call` through the in-process caller; the promise resolves
    /// to the `CallResult` as JSON after the step that applies it.
    fn dispatch(&self, call: Call) -> js_sys::Promise {
        dispatch_with(self.caller.clone(), call, |result| {
            serde_json::to_string(&result).map_err(|e| format!("serialize call result: {e}"))
        })
    }

    /// Write one key. `value_json` is any accepted vizij payload form (the
    /// canonical Arora `Value` serde or a vizij shorthand), normalized here.
    #[wasm_bindgen(js_name = setValue)]
    pub fn set_value(&self, path: &str, value_json: &str) -> Result<(), JsValue> {
        self.inner
            .set_value(path, &normalize_value_str(value_json)?)
    }

    /// Write several keys as one store change (`{ path: value }`).
    #[wasm_bindgen(js_name = writeValues)]
    pub fn write_values(&self, values_json: &str) -> Result<(), JsValue> {
        self.inner
            .write_values(&normalize_values_map_str(values_json)?)
    }

    /// Read keys (`string[]`); the result maps each path to its `Value` or
    /// `null`.
    #[wasm_bindgen(js_name = readValues)]
    pub fn read_values(&self, paths: JsValue) -> Result<JsValue, JsValue> {
        self.inner.read_values(paths)
    }

    /// Every key currently in the store, as a path → `Value` object.
    pub fn snapshot(&self) -> Result<JsValue, JsValue> {
        self.inner.snapshot()
    }

    /// The keys that changed since the last drain (`null` = cleared); the
    /// first drain returns the whole current state.
    #[wasm_bindgen(js_name = drainChanges)]
    pub fn drain_changes(&self) -> Result<JsValue, JsValue> {
        self.inner.drain_changes()
    }
}

/// Dispatch `call` through `caller`; `finish` turns the reply into the JSON
/// the promise resolves to.
///
/// `LocalCaller::call` enqueues the call synchronously inside `call()` — its
/// future is only the reply — but `call()` itself runs at the composed
/// future's first poll, and the promise machinery first polls in a
/// microtask, after the current JS turn. The one poll here runs it now, so
/// the call is enqueued **before this returns** and a manual driver can
/// dispatch and step in the same turn. Parking on the throwaway waker loses
/// no wakeup: the reply channel re-registers its waker on every poll.
fn dispatch_with(
    caller: LocalCaller,
    call: Call,
    finish: impl FnOnce(arora_types::call::CallResult) -> Result<String, String> + 'static,
) -> js_sys::Promise {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

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
        finish(result)
            .map(|json| JsValue::from_str(&json))
            .map_err(|e| JsValue::from_str(&e))
    })
}

/// `window.devicePixelRatio`, 1 where there is no window.
fn device_pixel_ratio() -> f64 {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("devicePixelRatio"))
        .ok()
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0)
}

/// [`mount`]'s options.
fn parse_view_options(json: Option<&str>) -> Result<ViewOptions, JsValue> {
    let options: serde_json::Value = match json {
        None | Some("") => serde_json::Value::Null,
        Some(json) => serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("options JSON: {e}")))?,
    };
    let background = match options
        .get("background")
        .and_then(serde_json::Value::as_str)
    {
        Some(hex) => {
            let [r, g, b] = face::parse_rgb(hex).map_err(|e| JsValue::from_str(&e.to_string()))?;
            Color::srgb_u8(r, g, b)
        }
        None => Color::NONE,
    };
    let fit = match options.get("fit").and_then(serde_json::Value::as_str) {
        None | Some("contain") => Fit::Contain,
        Some("cover") => Fit::Cover,
        Some("stretch") => Fit::Stretch,
        Some(other) => {
            return Err(JsValue::from_str(&format!(
                "fit must be contain, cover or stretch, got {other:?}"
            )))
        }
    };
    let zoom = match options.get("zoom") {
        None | Some(serde_json::Value::Null) => Vec2::ONE,
        Some(serde_json::Value::Number(n)) => Vec2::splat(n.as_f64().unwrap_or(1.0) as f32),
        Some(serde_json::Value::Array(xy)) if xy.len() == 2 => Vec2::new(
            xy[0].as_f64().unwrap_or(1.0) as f32,
            xy[1].as_f64().unwrap_or(1.0) as f32,
        ),
        Some(other) => {
            return Err(JsValue::from_str(&format!(
                "zoom must be a number or [x, y], got {other}"
            )))
        }
    };
    if !(zoom.x > 0.0 && zoom.y > 0.0) {
        return Err(JsValue::from_str("zoom factors must be positive"));
    }
    Ok(ViewOptions {
        background,
        fit,
        zoom,
        ambient: options
            .get("ambient")
            .and_then(serde_json::Value::as_f64)
            .map(|a| a as f32)
            .unwrap_or(std::f32::consts::FRAC_PI_2),
        unlit: options
            .get("unlit")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

/// [`load_face`]'s options: the composition ([`compose_face`]'s fields) and
/// whether the neutral pose is staged.
fn parse_face_config(json: Option<&str>) -> Result<(FaceConfig, bool), JsValue> {
    let options: serde_json::Value = match json {
        None | Some("") => serde_json::Value::Null,
        Some(json) => serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("options JSON: {e}")))?,
    };
    let wanted = match options.get("graphs").and_then(serde_json::Value::as_array) {
        Some(kinds) => kinds
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect(),
        None => ["rig", "pose-driver", "pose", "standard-adaptation"]
            .map(str::to_string)
            .to_vec(),
    };
    let program = match options.get("program").and_then(serde_json::Value::as_str) {
        None | Some("auto") => ProgramSelect::Auto,
        Some("none") => ProgramSelect::None,
        Some(id) => ProgramSelect::Id(id.to_string()),
    };
    let flag = |name: &str| {
        options
            .get(name)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true)
    };
    Ok((
        FaceConfig {
            wanted,
            program,
            stage_neutral: flag("stageNeutral"),
            ros4hri: flag("ros4hri"),
        },
        flag("stageNeutral"),
    ))
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

/// The standard mappings Vizij ships (ROS4HRI, …) as a JS array of
/// `{ id, title, description }` — the introspectable list an authoring app
/// offers for opt-in. A *mapping* is a graph that implements one profile in
/// terms of another; the profiles themselves are listed by [`profiles`].
#[wasm_bindgen(js_name = mappings)]
pub fn mappings() -> Result<JsValue, JsValue> {
    let list = vizij_arora_host::mappings::standard_mappings_json();
    let json =
        serde_json::to_string(&list).map_err(|e| JsValue::from_str(&format!("mappings: {e}")))?;
    js_sys::JSON::parse(&json)
}

/// A standard mapping's graph as a JS object, ready to compose or embed —
/// its written control paths prefixed with `rig_prefix` (e.g.
/// `rig/quori_latest/`; empty for the unprefixed graph). `null` for an
/// unknown id (see [`mappings`]).
#[wasm_bindgen(js_name = mapping)]
pub fn mapping(id: &str, rig_prefix: &str) -> Result<JsValue, JsValue> {
    match vizij_arora_host::mappings::standard_mapping_source(id, rig_prefix) {
        Some((_, spec)) => {
            let json = serde_json::to_string(&spec)
                .map_err(|e| JsValue::from_str(&format!("mapping {id}: {e}")))?;
            js_sys::JSON::parse(&json)
        }
        None => Ok(JsValue::NULL),
    }
}

/// The profiles Vizij ships as a JS array of `{ id, version, title,
/// description, scope, keys }` — where a *profile* is an interface: the set
/// of store paths and their types one party exposes to another. The
/// introspectable list an authoring app's import picker offers.
#[wasm_bindgen(js_name = profiles)]
pub fn profiles() -> Result<JsValue, JsValue> {
    let list = vizij_arora_host::profile::profiles_json();
    let json =
        serde_json::to_string(&list).map_err(|e| JsValue::from_str(&format!("profiles: {e}")))?;
    js_sys::JSON::parse(&json)
}

/// One shipped profile in full — every path it declares, with its type,
/// range, default, and standard metadata. `null` for an unknown id (see
/// [`profiles`]).
///
/// `rig_prefix` (e.g. `rig/quori_latest/`) addresses a face-scoped profile
/// to one face's store; a device-scoped profile is returned as is, whatever
/// the prefix. Pass an empty string for the portable form the registry ships.
#[wasm_bindgen(js_name = profile)]
pub fn profile(id: &str, rig_prefix: &str) -> Result<JsValue, JsValue> {
    match vizij_arora_host::profile::profile(id) {
        Some(declared) => {
            let json = serde_json::to_string(&declared.with_rig_prefix(rig_prefix))
                .map_err(|e| JsValue::from_str(&format!("profile {id}: {e}")))?;
            js_sys::JSON::parse(&json)
        }
        None => Ok(JsValue::NULL),
    }
}

/// The skills Vizij ships (the look_at gaze skill, …) as a JS array of
/// `{ id, title, description, parameters }` — the introspectable list an
/// authoring app's Skills menu and a device's actions view offer.
#[wasm_bindgen(js_name = skills)]
pub fn skills() -> Result<JsValue, JsValue> {
    let list = vizij_arora_host::skills::skills_json();
    let json =
        serde_json::to_string(&list).map_err(|e| JsValue::from_str(&format!("skills: {e}")))?;
    js_sys::JSON::parse(&json)
}

/// A skill's canonical fragment graph as a JS object, ready to embed as the
/// face's `skill::<id>` override. Face-independent by construction (its
/// placeholder `task/*` paths are rewritten per run at graft time), so unlike
/// a mapping it takes no rig prefix. `null` for an unknown id (see
/// [`skills`]).
#[wasm_bindgen(js_name = skillSource)]
pub fn skill_source(id: &str) -> Result<JsValue, JsValue> {
    match vizij_arora_host::skills::skill_source(id) {
        Some(spec) => {
            let json = serde_json::to_string(&spec)
                .map_err(|e| JsValue::from_str(&format!("skill {id}: {e}")))?;
            js_sys::JSON::parse(&json)
        }
        None => Ok(JsValue::NULL),
    }
}

/// The composed behavior graph of a face bundle — the composition the native
/// `vizij` app deploys and [`load_face`] installs: the bundle's base graphs,
/// its embedded standard mappings (each suppressing the built-in of the same
/// id — an embedded copy is the author's pinned override), the built-in
/// ROS4HRI mapping unless opted out, then the selected program. The returned
/// spec is ready for [`FaceRuntime::from_graph`] or
/// [`loadGraph`](FaceRuntime::load_graph), so an exported GLB can be deployed
/// and verified without a face on screen.
///
/// `gltf_json` is the GLB's glTF JSON document (its `VIZIJ_bundle` read from
/// the root or a node extension). `options_json`, all fields optional:
/// - `graphs`: base kinds to compose — default `rig`, `pose-driver`, `pose`,
///   `standard-adaptation` (the native default);
/// - `program`: `"auto"` (default), `"none"`, or a program id;
/// - `ros4hri`: compose the built-in ROS4HRI mapping — default `true`;
/// - `animations`: compose the animation source — default `false`; a device
///   built by [`load_face`] or [`FaceRuntime::from_graph`] always links the
///   animation module, so the source dispatches there.
#[wasm_bindgen(js_name = composeFace)]
pub fn compose_face(gltf_json: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    use vizij_arora_host::{ros4hri, Bundle};

    let gltf: serde_json::Value = serde_json::from_str(gltf_json)
        .map_err(|e| JsValue::from_str(&format!("glTF JSON: {e}")))?;
    let bundle = Bundle::from_gltf_json(&gltf)
        .ok_or_else(|| JsValue::from_str("the document carries no VIZIJ_bundle"))?;

    let options: serde_json::Value = match options_json.as_deref() {
        None | Some("") => serde_json::Value::Null,
        Some(json) => serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("options JSON: {e}")))?,
    };
    let wanted: Vec<String> = match options.get("graphs").and_then(serde_json::Value::as_array) {
        Some(kinds) => kinds
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect(),
        None => ["rig", "pose-driver", "pose", "standard-adaptation"]
            .map(str::to_string)
            .to_vec(),
    };
    let wanted: Vec<&str> = wanted.iter().map(String::as_str).collect();
    let program = match options.get("program").and_then(serde_json::Value::as_str) {
        None | Some("auto") => ProgramSelect::Auto,
        Some("none") => ProgramSelect::None,
        Some(id) => ProgramSelect::Id(id.to_string()),
    };
    let mut mappings = Vec::new();
    if options
        .get("ros4hri")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
    {
        mappings.push(ros4hri::ros4hri_source(&bundle.rig_prefix()));
    }
    let with_animations = options
        .get("animations")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let spec = bundle
        .compose(&wanted, &program, with_animations, &mappings)
        .map_err(|e| JsValue::from_str(&format!("compose: {e}")))?;
    let json = serde_json::to_string(&spec)
        .map_err(|e| JsValue::from_str(&format!("composed spec: {e}")))?;
    js_sys::JSON::parse(&json)
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
