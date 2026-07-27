//! The arora device behind the view: `RigHal` + `BlackboardStore` + the
//! face's composed graph as the behavior, run on a worker thread.

use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use arora::tui::{commands_frontend, TuiCommand, TuiCommandEvent};
use arora_types::data::{DataStore, Key, StateChange};
use futures::StreamExt;
use vizij_api_core::value::float;
use vizij_arora_behavior::{parse_spec, ProcessingGraph};
use vizij_arora_hal::RigHal;
pub use vizij_arora_host::ProgramSelect;
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

/// How this app composes and stages a face — carried from the CLI through the
/// supervisor into each generation and its reloads.
#[derive(Clone)]
pub struct FaceConfig {
    /// Bundle graph kinds to compose into the base behavior (rig, pose-driver).
    pub wanted: Vec<String>,
    /// Which program to autoplay on top of the rig.
    pub program: ProgramSelect,
    /// Stage the bundle's neutral inputs into the store at boot.
    pub stage_neutral: bool,
}

/// The bridges the device serves beyond the always-on open local bridge — a
/// build/CLI choice, constant for the process. Empty by default; the fields
/// exist only for the bridge features that are compiled in.
#[derive(Clone, Default)]
pub struct BridgeConfig {
    /// `--ros2 [namespace][:domain]`: expose the device's keys over ROS 2 topics.
    #[cfg(feature = "ros2")]
    pub ros2: Option<(String, u16)>,
}

/// Attach the device's bridges to `builder`: always the open local bridge
/// (`ws://127.0.0.1:9000`, the one local editors and apps connect to), plus any
/// the build/CLI adds. `run()` would attach the local bridge itself only if no
/// bridge were injected, so once we add another bridge we attach the local one
/// explicitly too. A bridge that fails to build is logged and skipped, not
/// fatal. (A `--studio` arm belongs here next to the ROS 2 one — see the
/// `studio` note in Cargo.toml.)
#[cfg_attr(not(feature = "ros2"), allow(unused_variables))]
async fn attach_bridges(
    mut builder: arora::AroraBuilder,
    bridges: &BridgeConfig,
) -> arora::AroraBuilder {
    match arora::local_ws_bridge().await {
        Ok(bridge) => builder = builder.with_bridge(bridge),
        Err(e) => log::error!("local bridge: {e:?}"),
    }
    #[cfg(feature = "ros2")]
    if let Some((namespace, domain)) = &bridges.ros2 {
        let config = arora_bridge_ros2::Ros2BridgeConfig::new(namespace.clone(), *domain);
        builder = builder.with_bridge(Box::new(arora_bridge_ros2::Ros2Bridge::new(config).await));
        log::info!("serving the ROS 2 bridge (namespace {namespace:?}, domain {domain})");
    }
    builder
}

/// Load a face for the device: parse the GLB's metadata, compose its bundle
/// graphs (the base kinds plus the chosen program) into the one behavior graph,
/// and validate it. Returns the canonicalized GLB path (what the view loads),
/// the metadata, and the composed spec.
pub fn load_face(glb: &Path, config: &FaceConfig) -> Result<(String, FaceMeta, String)> {
    let canonical = glb
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", glb.display()))?;
    let meta = FaceMeta::from_glb_file(&canonical)?;
    let wanted: Vec<&str> = config.wanted.iter().map(String::as_str).collect();
    let spec = meta.bundle.compose(&wanted, &config.program)?.to_string();
    parse_spec(&spec).map_err(|e| anyhow!("composed spec does not parse: {e}"))?;
    Ok((canonical.to_string_lossy().into_owned(), meta, spec))
}

/// Stage a face's neutral pose into the store before the first tick: the web's
/// `stagePoseNeutral`, ported. The rig's input nodes read these store paths, so
/// pre-seeding them holds the face at its authored neutral even where the
/// inputs' own defaults don't (a no-op when the bundle carries no neutral
/// config, or every input already defaults to its neutral).
fn stage_neutral_pose(store: &BlackboardStore, meta: &FaceMeta) {
    let writes = meta.bundle.neutral_stage_writes();
    if writes.is_empty() {
        return;
    }
    let mut change = StateChange::new();
    for (path, value) in &writes {
        change
            .set
            .insert(Key::from(path.as_str()), Some(float(*value)));
    }
    match store.write(change) {
        Ok(()) => log::info!("staged {} neutral inputs", writes.len()),
        Err(e) => log::warn!("neutral staging failed: {e:?}"),
    }
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
pub fn start(
    glb: &Path,
    config: FaceConfig,
    bridges: BridgeConfig,
    mode: Mode,
) -> Result<Device> {
    let (glb_path, meta, spec) = load_face(glb, &config)?;
    let rig = RigHal::new();
    let store = BlackboardStore::new();
    let (events_tx, events_rx) = std::sync::mpsc::channel();

    let thread = {
        let rig = rig.clone();
        let store = store.clone();
        // The supervisor stages and announces each generation from the meta; the
        // view keeps its own copy for the initial face.
        let meta = meta.clone();
        thread::Builder::new()
            .name("arora".into())
            .spawn(move || match mode {
                Mode::Operator => supervise(spec, meta, config, bridges, rig, store, events_tx),
                Mode::Quiet => {
                    if config.stage_neutral {
                        stage_neutral_pose(&store, &meta);
                    }
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
    mut meta: FaceMeta,
    config: FaceConfig,
    bridges: BridgeConfig,
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
    // Set on a reload so the next generation announces the new face to the view.
    let mut pending_glb: Option<String> = None;
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
        if let Some(glb_path) = pending_glb.take() {
            let _ = events.send(DeviceEvent::FaceLoaded {
                glb_path,
                meta: Box::new(meta.clone()),
                rig: rig.clone(),
            });
        }
        if config.stage_neutral {
            stage_neutral_pose(&store, &meta);
        }
        let Some(builder) = builder_for(&spec, rig, store) else {
            return;
        };
        let (stop_tx, stop_rx) = futures::channel::oneshot::channel();
        let (reload_tx, reload_rx) = std::sync::mpsc::channel();
        tokio_rt.block_on(async {
            tokio::spawn(pump(
                commands,
                config.clone(),
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
            let builder = attach_bridges(builder.with_frontend(frontend), &bridges).await;
            tokio::select! {
                result = builder.run() => {
                    if let Err(e) = result {
                        log::error!("arora device stopped: {e:?}");
                    }
                }
                _ = stop => {}
            }
        });
        let Ok((glb_path, new_meta, new_spec)) = reload_rx.try_recv() else {
            break;
        };
        spec = new_spec;
        meta = new_meta;
        pending_glb = Some(glb_path);
    }
}

/// Serve the terminal UI's command events for one device generation: `b`
/// forwards a parsed background to the view; `g` validates the new face first
/// and only then asks the run to stop, handing the supervisor what it needs to
/// rebuild — a bad path is an error in the log pane, not a dead device.
async fn pump(
    mut commands: futures::channel::mpsc::UnboundedReceiver<TuiCommandEvent>,
    config: FaceConfig,
    events: Sender<DeviceEvent>,
    reload: Sender<(String, FaceMeta, String)>,
    stop: futures::channel::oneshot::Sender<()>,
) {
    while let Some(event) = commands.next().await {
        match (event.key, event.input) {
            ('g', Some(path)) => match load_face(Path::new(&path), &config) {
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
