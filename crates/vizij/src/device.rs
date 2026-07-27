//! The arora device behind the view: `RigHal` + `BlackboardStore` + the
//! face's composed graph as the behavior, run on a worker thread.

use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use arora::tui::{commands_frontend, TuiCommand, TuiCommandEvent};
use futures::StreamExt;
use serde_json::{json, Value as Json};
use vizij_arora_behavior::{parse_spec, ProcessingGraph};
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;

use crate::meta::FaceMeta;

/// A running face device. Both handles share storage with the device's own
/// (they are sibling clones), so the view reads the rig and the store live.
pub struct Device {
    pub rig: RigHal,
    /// Sibling store handle; unused until input staging/UI lands (VIZ-47 UI stage).
    #[allow(dead_code)]
    pub store: BlackboardStore,
    /// The face the device booted on.
    pub meta: FaceMeta,
    /// Its GLB, canonicalized — what the view loads.
    pub glb_path: String,
    /// Runtime changes the operator makes through the terminal UI; the view
    /// applies them.
    pub events: Receiver<DeviceEvent>,
    _thread: thread::JoinHandle<()>,
}

/// A runtime change the operator made through the terminal UI.
pub enum DeviceEvent {
    /// A different face was loaded (`g`): the device restarted on its graphs;
    /// the view swaps over to the new meta and rig feed.
    FaceLoaded {
        glb_path: String,
        meta: Box<FaceMeta>,
        rig: RigHal,
    },
    /// A new background color (`b`), as sRGB bytes.
    Background([u8; 3]),
}

/// How the device fronts the operator.
pub enum Mode {
    /// The standard arora operator flow (`AroraBuilder::run`): the terminal UI
    /// on an interactive terminal (headless front end otherwise) with the
    /// vizij commands installed, the open local bridge auto-attached
    /// (`ws://127.0.0.1:9000`), logging owned by the front end's sink.
    Operator,
    /// Build and step quietly: no bridge, no front end — the snapshot
    /// harness, where the process' own logger stays in charge and a
    /// port conflict with a running window instance cannot occur.
    Quiet,
}

/// Compose bundle graphs into the one spec the device runs — the native
/// equivalent of the web host's `composeGraphSpecs`. Node ids are namespaced
/// per source (`{source}::{id}`) so sources cannot collide; **store paths stay
/// shared** — that is the cross-source contract.
///
/// Each graph is normalized first (legacy input-connection forms become
/// edges), so id rewriting sees the canonical `nodes`/`edges` shape.
pub fn compose(graphs: &[(String, Json)]) -> Result<Json> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for (index, (kind, spec)) in graphs.iter().enumerate() {
        let mut normalized = spec.clone();
        vizij_api_core::json::normalize_graph_spec_value(&mut normalized)
            .map_err(|e| anyhow!("graph {index} ({kind}) does not normalize: {e:?}"))?;
        let prefix = format!("{kind}{index}::");
        for node in normalized
            .get("nodes")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            let mut node = node.clone();
            if let Some(id) = node.get("id").and_then(Json::as_str) {
                node["id"] = json!(format!("{prefix}{id}"));
            }
            nodes.push(node);
        }
        for edge in normalized
            .get("edges")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            let mut edge = edge.clone();
            for end in ["from", "to"] {
                if let Some(id) = edge
                    .get(end)
                    .and_then(|e| e.get("node_id"))
                    .and_then(Json::as_str)
                {
                    edge[end]["node_id"] = json!(format!("{prefix}{id}"));
                }
            }
            edges.push(edge);
        }
    }
    Ok(json!({ "nodes": nodes, "edges": edges }))
}

/// Load a face for the device: parse the GLB's metadata, compose the bundle
/// graphs whose kind is in `wanted` into the device's one behavior graph, and
/// validate it. Returns the canonicalized GLB path (what the view loads), the
/// metadata, and the composed spec.
pub fn load_face(glb: &Path, wanted: &[String]) -> Result<(String, FaceMeta, String)> {
    let canonical = glb
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", glb.display()))?;
    let meta = FaceMeta::from_glb_file(&canonical)?;
    let graphs: Vec<(String, Json)> = meta
        .bundle_graphs
        .iter()
        .filter(|(kind, _)| wanted.iter().any(|w| w == kind))
        .cloned()
        .collect();
    if graphs.is_empty() {
        log::warn!(
            "no bundle graphs matched {wanted:?} in {}; the face will hold its authored pose",
            glb.display()
        );
    }
    let spec = compose(&graphs)?.to_string();
    parse_spec(&spec).map_err(|e| anyhow!("composed spec does not parse: {e}"))?;
    Ok((canonical.to_string_lossy().into_owned(), meta, spec))
}

/// `RRGGBB` hex (leading `#` allowed) to sRGB bytes.
pub fn parse_rgb(hex: &str) -> Result<[u8; 3]> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return Err(anyhow!("expected RRGGBB, got {hex:?}"));
    }
    Ok([
        u8::from_str_radix(&hex[0..2], 16)?,
        u8::from_str_radix(&hex[2..4], 16)?,
        u8::from_str_radix(&hex[4..6], 16)?,
    ])
}

/// Load the face at `glb` (its graph kinds filtered by `wanted`) and run its
/// device on a worker thread.
///
/// The `Arora` is constructed **inside** the worker thread — it is not `Send`
/// (single-owner by design); only the spec JSON and the sibling rig/store
/// handles cross the thread boundary. The face is loaded here first so
/// composition errors surface to the caller, not in a log.
pub fn start(glb: &Path, wanted: Vec<String>, mode: Mode) -> Result<Device> {
    let (glb_path, meta, spec) = load_face(glb, &wanted)?;
    let rig = RigHal::new();
    let store = BlackboardStore::new();
    let (events_tx, events_rx) = std::sync::mpsc::channel();

    let thread = {
        let rig = rig.clone();
        let store = store.clone();
        thread::Builder::new()
            .name("arora".into())
            .spawn(move || match mode {
                Mode::Operator => supervise(spec, wanted, rig, store, events_tx),
                Mode::Quiet => {
                    let Some(builder) = builder_for(&spec, rig, store) else {
                        return;
                    };
                    match builder.build() {
                        Ok(mut arora) => step_forever(&mut arora),
                        Err(e) => log::error!("building the arora device: {e:?}"),
                    }
                }
            })?
    };

    Ok(Device {
        rig,
        store,
        meta,
        glb_path,
        events: events_rx,
        _thread: thread,
    })
}

/// The device builder over the Vizij seams; `None` (logged) when the spec
/// does not encode.
fn builder_for(spec: &str, rig: RigHal, store: BlackboardStore) -> Option<arora::AroraBuilder> {
    let spec = match parse_spec(spec) {
        Ok(spec) => spec,
        Err(e) => {
            log::error!("spec re-parse failed: {e}");
            return None;
        }
    };
    let graph = match ProcessingGraph::from_spec(spec) {
        Ok(graph) => graph,
        Err(e) => {
            log::error!("graph encode failed: {e}");
            return None;
        }
    };
    Some(
        arora::Arora::builder()
            .with_hal(Box::new(rig))
            .with_data_store(Box::new(store))
            .with_behavior_interpreter(Box::new(graph)),
    )
}

/// The operator flow, generation by generation: each `g` (load GLB) stops the
/// running device and rebuilds it — graphs, rig, store — on the new face, and
/// the view follows through [`DeviceEvent::FaceLoaded`]. Runs until a
/// generation ends without a reload (a device error; the terminal UI's quit
/// exits the process).
fn supervise(
    mut spec: String,
    wanted: Vec<String>,
    rig: RigHal,
    store: BlackboardStore,
    events: Sender<DeviceEvent>,
) {
    let tokio_rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return log::error!("tokio runtime: {e}"),
    };
    // The first generation runs on the handles the view already holds; a
    // reload's face swap is announced only after the new front end is up, so
    // the view's swap (and everything it logs) lands in the live pane.
    let mut fresh = Some((rig, store));
    let mut pending_face: Option<(String, FaceMeta)> = None;
    loop {
        let (rig, store) = fresh
            .take()
            .unwrap_or_else(|| (RigHal::new(), BlackboardStore::new()));
        let (frontend, commands) = commands_frontend(vec![
            TuiCommand {
                key: 'g',
                label: "load GLB".into(),
                prompt: Some("GLB path".into()),
            },
            TuiCommand {
                key: 'b',
                label: "background".into(),
                prompt: Some("background RRGGBB".into()),
            },
        ]);
        if let Some((glb_path, meta)) = pending_face.take() {
            let _ = events.send(DeviceEvent::FaceLoaded {
                glb_path,
                meta: Box::new(meta),
                rig: rig.clone(),
            });
        }
        let Some(builder) = builder_for(&spec, rig, store) else {
            return;
        };
        let (stop_tx, stop_rx) = futures::channel::oneshot::channel();
        let (reload_tx, reload_rx) = std::sync::mpsc::channel();
        tokio_rt.block_on(async {
            tokio::spawn(pump(
                commands,
                wanted.clone(),
                events.clone(),
                reload_tx,
                stop_tx,
            ));
            // A fired stop drops the run future — arora's stop story: the
            // teardown is complete and synchronous (front end released, local
            // bridge's port freed) before the next generation starts. The
            // pump merely ending (headless: no commands will ever come) must
            // not stop a healthy run.
            let stop = async move {
                if stop_rx.await.is_err() {
                    futures::future::pending::<()>().await
                }
            };
            tokio::select! {
                result = builder.with_frontend(frontend).run() => {
                    if let Err(e) = result {
                        log::error!("arora device stopped: {e:?}");
                    }
                }
                _ = stop => {}
            }
        });
        let Ok((glb_path, meta, new_spec)) = reload_rx.try_recv() else {
            break;
        };
        spec = new_spec;
        pending_face = Some((glb_path, meta));
    }
}

/// Serve the terminal UI's command events for one device generation: `b`
/// forwards a parsed background to the view; `g` validates the new face first
/// and only then asks the run to stop, handing the supervisor what it needs to
/// rebuild — a bad path is an error in the log pane, not a dead device.
async fn pump(
    mut commands: futures::channel::mpsc::UnboundedReceiver<TuiCommandEvent>,
    wanted: Vec<String>,
    events: Sender<DeviceEvent>,
    reload: Sender<(String, FaceMeta, String)>,
    stop: futures::channel::oneshot::Sender<()>,
) {
    while let Some(event) = commands.next().await {
        match (event.key, event.input) {
            ('g', Some(path)) => match load_face(Path::new(&path), &wanted) {
                Ok(loaded) => {
                    let _ = reload.send(loaded);
                    let _ = stop.send(());
                    return;
                }
                Err(e) => log::error!("cannot load {path}: {e:#}"),
            },
            ('b', Some(hex)) => match parse_rgb(&hex) {
                Ok(rgb) => {
                    let _ = events.send(DeviceEvent::Background(rgb));
                }
                Err(e) => log::error!("background: {e}"),
            },
            _ => {}
        }
    }
}

/// The ~100 Hz step loop with measured dt — the quiet mode's drive; the
/// operator flow's loop lives in `AroraBuilder::run`.
fn step_forever(arora: &mut arora::Arora) {
    let period = Duration::from_millis(10);
    let mut last = Instant::now();
    loop {
        let now = Instant::now();
        let dt = now.duration_since(last);
        last = now;
        if let Err(e) = arora.step(dt) {
            log::error!("arora device stopped: {e:?}");
            break;
        }
        thread::sleep(period);
    }
}
