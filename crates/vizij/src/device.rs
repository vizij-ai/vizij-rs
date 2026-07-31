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

use crate::animation;
use crate::gaze;
use crate::meta::FaceMeta;
#[cfg(not(feature = "tts-piper"))]
use crate::tts;
#[cfg(feature = "tts-piper")]
use crate::tts_piper;

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
    /// Compose the built-in ROS4HRI profile (`standard/ros4hri/*` keys drive
    /// the face's standard controls). On by default in the binary, opt-out
    /// via `--no-ros4hri`.
    pub ros4hri: bool,
}

/// The bridges the device serves beyond the always-on open local bridge — a
/// build/CLI choice, constant for the process. Empty by default; the fields
/// exist only for the bridge features that are compiled in.
#[derive(Clone, Default)]
pub struct BridgeConfig {
    /// `--ros2 [namespace][:domain]`: expose the device's keys over ROS 2 topics.
    #[cfg(feature = "ros2")]
    pub ros2: Option<(String, u16)>,
    /// `--studio`: attach the Semio Studio bridge (env-configured).
    #[cfg(feature = "studio")]
    pub studio: bool,
}

/// Attach the device's bridges to `builder`: always the open local bridge
/// (`ws://127.0.0.1:9000`, the one local editors and apps connect to), plus any
/// the build/CLI adds. `run()` would attach the local bridge itself only if no
/// bridge were injected, so once we add another bridge we attach the local one
/// explicitly too. A bridge that fails to build is logged and skipped, not fatal.
#[cfg_attr(
    not(any(feature = "ros2", feature = "studio")),
    allow(unused_variables)
)]
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
        // The ROS4HRI exposure profile: typed face topics fanning onto the
        // standard keys, and the `/skill/look_at` action bound to the gaze
        // skill the device describes.
        let config = arora_bridge_ros2::Ros2BridgeConfig::new(namespace.clone(), *domain)
            .with_profile(arora_bridge_ros2::ExposureProfile::ros4hri());
        builder = builder.with_bridge(Box::new(arora_bridge_ros2::Ros2Bridge::new(config).await));
        log::info!("serving the ROS 2 bridge (namespace {namespace:?}, domain {domain})");
    }
    #[cfg(feature = "studio")]
    if bridges.studio {
        match arora::studio::connect().await {
            Ok(bridge) => builder = builder.with_bridge(bridge),
            Err(e) => log::error!("studio bridge: {e:?}"),
        }
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
    // The standard profiles this face composes: ROS4HRI unless opted out. The
    // profile writes the face's namespaced standard controls, so it takes the
    // bundle's rig prefix.
    let mut profiles = Vec::new();
    if config.ros4hri {
        profiles.push(vizij_arora_host::ros4hri::ros4hri_source(
            &meta.bundle.rig_prefix(),
        ));
    }
    // `with_animations`: the device always loads the animation module (see
    // `builder_for`), so the animation source it dispatches to is always
    // composed — inert until a clip plays.
    let spec = meta
        .bundle
        .compose(&wanted, &config.program, true, &profiles)?
        .to_string();
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
pub fn start(glb: &Path, config: FaceConfig, bridges: BridgeConfig, mode: Mode) -> Result<Device> {
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
                    let Some(builder) = builder_for(&spec, rig, store, &meta.bundle.skills) else {
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

/// The device builder over the Vizij seams: the composed graph as the behavior,
/// with the animation module loaded so the composed animation source's
/// `ExternalFunction` nodes dispatch and its transport is callable. `None`
/// (logged) when the spec does not encode or the baked-in module does not load.
pub(crate) fn builder_for(
    spec: &str,
    rig: RigHal,
    store: BlackboardStore,
    embedded_skills: &[(String, serde_json::Value)],
) -> Option<arora::AroraBuilder> {
    let spec = match parse_spec(spec) {
        Ok(spec) => spec,
        Err(e) => {
            log::error!("spec re-parse failed: {e}");
            return None;
        }
    };
    let mut graph = match ProcessingGraph::from_spec(spec) {
        Ok(graph) => graph,
        Err(e) => {
            log::error!("graph encode failed: {e}");
            return None;
        }
    };
    // Route the animation source's `step`/`player_states` handles (and any
    // in-process transport call) to the host module registered below.
    graph.set_function_modules(animation::function_modules());
    // The gaze skill: the described contract rides the gaze module; the
    // behavior is the shipped fragment the interpreter grafts per goal — or
    // the face's embedded override.
    graph.set_task_fragment(
        gaze::look_at_id(),
        gaze::look_at_fragment_from(embedded_skills),
    );
    let builder = arora::Arora::builder()
        .with_hal(Box::new(rig))
        .with_data_store(Box::new(store))
        .with_behavior_interpreter(Box::new(graph))
        .with_host_module(animation::host_module())
        .with_host_module(gaze::host_module());
    // The TTS module: the described `say` action (poll-on-tick, viseme
    // out-param). One provider per build, same contract (`tts_api`): the cloud
    // provider by default, the local Piper provider under `tts-piper`.
    #[cfg(not(feature = "tts-piper"))]
    let builder = builder.with_host_module(tts::host_module());
    #[cfg(feature = "tts-piper")]
    let builder = builder.with_host_module(tts_piper::host_module());
    Some(builder)
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
        let Some(builder) = builder_for(&spec, rig, store, &meta.bundle.skills) else {
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

#[cfg(test)]
mod tests {
    use super::*;
    use vizij_arora_host::{animations_source, compose_sources, ANIMATION_PLAYERS_PATH};

    /// The composed animation source ticks the loaded module: `builder_for`
    /// loads the baked-in wasm and wires `set_function_modules`, so the source's
    /// `player_states` `ExternalFunction` dispatches and its output lands in the
    /// store. A dispatch failure ("no module registered") would abort the graph
    /// tick before any write, so the key's mere presence proves the module runs.
    #[test]
    fn animation_source_ticks_the_loaded_module() {
        let spec = compose_sources(&[animations_source()])
            .expect("compose the animation source")
            .to_string();
        let mut arora = builder_for(&spec, RigHal::new(), BlackboardStore::new(), &[])
            .expect("build the device over the loaded animation module")
            .build()
            .expect("build arora");
        for _ in 0..3 {
            arora.step(Duration::from_millis(16)).expect("step");
        }
        let key = Key::from(ANIMATION_PLAYERS_PATH);
        let players = arora
            .store()
            .read(std::slice::from_ref(&key))
            .into_iter()
            .next()
            .flatten();
        assert!(
            players.is_some(),
            "{ANIMATION_PLAYERS_PATH} absent — the animation module did not dispatch",
        );
    }

    /// A LookAt-style behavior spawned through the device: the SPAWN call
    /// reaches the graph interpreter through the engine, the run grafts into
    /// the running node graph and reports on its status key each step, and the
    /// handle's stop call halts it — all in-process, no bridge involved.
    #[test]
    fn a_look_at_run_advances_and_halts_through_the_device() {
        use arora_behavior::{interpreter_module, RunPolicy};
        use arora_types::call::{Call, CallResult};
        use arora_types::value::{StructureField, Value};
        use uuid::Uuid;
        use vizij_arora_behavior::task;

        const GAZE_MODULE: Uuid = Uuid::from_u128(0x67617a65);
        const LOOK_AT: Uuid = Uuid::from_u128(0x6c6f6f6b);
        const TARGET: Uuid = Uuid::from_u128(0x74617267);

        // A gaze skill that tracks indefinitely, like the real LookAt: every
        // invocation steers toward its target and reports `Running`.
        let gaze = arora::ModuleBuilder::new(GAZE_MODULE)
            .function(LOOK_AT, |_call| {
                Ok(CallResult {
                    ret: task::running(),
                    mutated: Vec::new(),
                })
            })
            .build();

        let mut arora = builder_for(
            r#"{ "nodes": [], "edges": [] }"#,
            RigHal::new(),
            BlackboardStore::new(),
            &[],
        )
        .expect("build the device")
        .with_host_module(gaze)
        .build()
        .expect("build arora");

        let look_at = Call {
            module_id: Some(GAZE_MODULE),
            id: LOOK_AT,
            args: vec![StructureField {
                id: TARGET,
                value: Box::new(Value::F32(0.5)),
            }],
        };
        let spawned = arora
            .call(interpreter_module::encode_spawn(
                &look_at,
                RunPolicy::Concurrent,
            ))
            .expect("SPAWN dispatches through the engine");
        let handle =
            interpreter_module::decode_spawn_result(&spawned.ret).expect("a TaskHandle comes back");

        let status = |arora: &arora::Arora| {
            arora
                .store()
                .read(std::slice::from_ref(&handle.status))
                .into_iter()
                .next()
                .flatten()
        };

        arora.step(Duration::from_millis(16)).expect("step");
        assert_eq!(status(&arora), Some(task::running()));
        arora.step(Duration::from_millis(16)).expect("step");
        assert_eq!(status(&arora), Some(task::running()), "indefinite tracking");

        arora
            .call(handle.stop.clone())
            .expect("the handle's stop call dispatches");
        arora.step(Duration::from_millis(16)).expect("step");
        assert_eq!(status(&arora), Some(task::failure()), "halted");
    }

    /// The production gaze skill through the device: SPAWN on the described
    /// `look_at` grafts the shipped fragment (`builder_for` registers it) —
    /// asset content, no module call — so the goal target lands on the
    /// standard gaze surface and the run reports `Running` until halted.
    #[test]
    fn the_gaze_skill_runs_the_shipped_fragment_through_the_device() {
        use arora_behavior::{interpreter_module, RunPolicy};
        use arora_types::call::Call;
        use arora_types::gen_uuid_from_str;
        use arora_types::value::StructureField;
        use vizij_arora_behavior::task;

        use crate::gaze;

        let mut arora = builder_for(
            r#"{ "nodes": [], "edges": [] }"#,
            RigHal::new(),
            BlackboardStore::new(),
            &[],
        )
        .expect("build the device")
        .build()
        .expect("build arora");

        let look_at = Call {
            module_id: Some(gaze::module_id()),
            id: gaze::look_at_id(),
            args: vec![
                StructureField {
                    id: gen_uuid_from_str("policy"),
                    value: Box::new(Value::String(String::new())),
                },
                StructureField {
                    id: gen_uuid_from_str("target"),
                    value: Box::new(Value::ArrayF32(vec![1.0, 2.0, 3.0])),
                },
                StructureField {
                    id: gen_uuid_from_str("frame"),
                    value: Box::new(Value::String("sellion_link".to_string())),
                },
            ],
        };
        let spawned = arora
            .call(interpreter_module::encode_spawn(
                &look_at,
                RunPolicy::Concurrent,
            ))
            .expect("SPAWN dispatches through the engine");
        let handle =
            interpreter_module::decode_spawn_result(&spawned.ret).expect("a TaskHandle comes back");

        let read = |arora: &arora::Arora, key: &arora_types::data::Key| {
            arora
                .store()
                .read(std::slice::from_ref(key))
                .into_iter()
                .next()
                .flatten()
        };

        arora.step(Duration::from_millis(16)).expect("step");
        assert_eq!(
            read(&arora, &Key::from(ros4hri::GAZE_TARGET_KEY)),
            Some(Value::ArrayF32(vec![1.0, 2.0, 3.0])),
            "the fragment writes the goal onto the gaze surface"
        );
        assert_eq!(
            read(&arora, &Key::from(ros4hri::GAZE_FRAME_KEY)),
            Some(Value::String("sellion_link".to_string())),
        );
        assert_eq!(read(&arora, &handle.status), Some(task::running()));

        arora
            .call(handle.stop.clone())
            .expect("the handle's stop call dispatches");
        arora.step(Duration::from_millis(16)).expect("step");
        assert_eq!(
            read(&arora, &handle.status),
            Some(task::failure()),
            "halted"
        );
    }

    /// A face's bundle-embedded `skill::look_at` fragment overrides the
    /// shipped behavior — the `standard::<id>` precedence, applied to the
    /// skill plane. The override here redirects the gaze write to a marker
    /// key, proving the embedded copy (not the built-in) serves the run.
    #[test]
    fn the_face_embedded_skill_fragment_overrides_the_built_in() {
        use arora_behavior::{interpreter_module, RunPolicy};
        use arora_types::call::Call;
        use arora_types::gen_uuid_from_str;
        use arora_types::value::StructureField;
        use vizij_arora_host::skills;

        use crate::gaze;

        let edited =
            skills::LOOK_AT_JSON.replace(ros4hri::GAZE_TARGET_KEY, "test/edited/gaze/target");
        let embedded = vec![(
            "look_at".to_string(),
            serde_json::from_str(&edited).expect("the edited fragment parses"),
        )];

        let mut arora = builder_for(
            r#"{ "nodes": [], "edges": [] }"#,
            RigHal::new(),
            BlackboardStore::new(),
            &embedded,
        )
        .expect("build the device")
        .build()
        .expect("build arora");

        let look_at = Call {
            module_id: Some(gaze::module_id()),
            id: gaze::look_at_id(),
            args: vec![StructureField {
                id: gen_uuid_from_str("target"),
                value: Box::new(Value::ArrayF32(vec![7.0, 8.0, 9.0])),
            }],
        };
        arora
            .call(interpreter_module::encode_spawn(
                &look_at,
                RunPolicy::Concurrent,
            ))
            .expect("SPAWN dispatches through the engine");
        arora.step(Duration::from_millis(16)).expect("step");

        let read = |key: &str| {
            arora
                .store()
                .read(std::slice::from_ref(&Key::from(key)))
                .into_iter()
                .next()
                .flatten()
        };
        assert_eq!(
            read("test/edited/gaze/target"),
            Some(Value::ArrayF32(vec![7.0, 8.0, 9.0])),
            "the embedded fragment's redirected write serves"
        );
        assert_eq!(
            read(ros4hri::GAZE_TARGET_KEY),
            None,
            "the built-in fragment's write does not"
        );
    }

    use vizij_api_core::value::{as_float, text, vec3, Value};
    use vizij_arora_host::ros4hri::{self, ros4hri_source};
    use vizij_arora_host::standard;

    /// A device running only the ROS4HRI profile (unprefixed controls) — the
    /// headless harness for the profile's mapping math: stage `standard/
    /// ros4hri/*` keys, tick, read `standard/vizij/*` controls back.
    fn ros4hri_device() -> arora::Arora {
        let spec = compose_sources(&[ros4hri_source("")])
            .expect("compose the ros4hri profile")
            .to_string();
        builder_for(&spec, RigHal::new(), BlackboardStore::new(), &[])
            .expect("build the device over the profile")
            .build()
            .expect("build arora")
    }

    fn stage(arora: &arora::Arora, path: &str, value: Value) {
        let mut change = StateChange::new();
        change.set.insert(Key::from(path), Some(value));
        arora.store().write(change).expect("stage");
    }

    fn read_f32(arora: &arora::Arora, path: &str) -> f32 {
        let key = Key::from(path);
        let value = arora
            .store()
            .read(std::slice::from_ref(&key))
            .into_iter()
            .next()
            .flatten()
            .unwrap_or_else(|| panic!("{path} absent"));
        as_float(&value).unwrap_or_else(|| panic!("{path} not a float"))
    }

    /// Settle the ~200 ms smoothers: 2 s of 16 ms ticks.
    fn settle(arora: &mut arora::Arora) {
        for _ in 0..125 {
            arora.step(Duration::from_millis(16)).expect("step");
        }
    }

    /// A face that embeds its own modified `ros4hri` copy runs that copy
    /// INSTEAD of the built-in — VIZ-92's precedence: an embedded profile is
    /// the author's pinned override of the shipped mapping. The embedded graph
    /// here maps valence verbatim onto the happy weight (no smoothing, no
    /// blending, name ignored), which the built-in never produces.
    #[test]
    fn embedded_profile_wins_over_the_built_in() {
        let bundle = vizij_arora_host::Bundle::from_bundle_json(&serde_json::json!({
            "graphs": [{
                "id": "standard::ros4hri",
                "kind": "standard-profile",
                "spec": {
                    "nodes": [
                        { "id": "v", "type": "input",
                          "params": { "path": ros4hri::EXPRESSION_VALENCE_KEY, "value": 0.0 } },
                        { "id": "o", "type": "output",
                          "params": { "path": standard::expression_path("happy") } },
                    ],
                    "edges": [
                        { "from": { "node_id": "v" }, "to": { "node_id": "o", "input": "in" } },
                    ],
                },
            }]
        }));
        let spec = bundle
            .compose(
                &["rig"],
                &vizij_arora_host::ProgramSelect::None,
                false,
                &[ros4hri_source("")],
            )
            .expect("compose the face with its embedded profile")
            .to_string();
        let mut arora = builder_for(&spec, RigHal::new(), BlackboardStore::new(), &[])
            .expect("build the device over the composed face")
            .build()
            .expect("build arora");
        // The built-in would one-hot "sad" (happy ≈ 0, smoothed); the embedded
        // verbatim mapping ignores the name and rides valence straight through.
        stage(&arora, ros4hri::EXPRESSION_NAME_KEY, text("sad"));
        stage(&arora, ros4hri::EXPRESSION_VALENCE_KEY, float(0.8));
        settle(&mut arora);
        let happy = read_f32(&arora, &standard::expression_path("happy"));
        assert!(
            (happy - 0.8).abs() < 1e-6,
            "embedded mapping must win verbatim, got happy={happy}"
        );
    }

    #[test]
    fn ros4hri_profile_rests_neutral() {
        let mut arora = ros4hri_device();
        settle(&mut arora);
        assert!(read_f32(&arora, &standard::expression_path("neutral")) > 0.95);
        assert!(read_f32(&arora, &standard::expression_path("happy")) < 0.01);
        assert!(read_f32(&arora, &standard::expression_path("skeptical")) < 0.01);
        // Eyes at their default forward target: centered.
        assert!(read_f32(&arora, standard::LEFT_EYE_POS_X).abs() < 0.01);
    }

    #[test]
    fn ros4hri_expression_name_one_hots() {
        let mut arora = ros4hri_device();
        stage(&arora, ros4hri::EXPRESSION_NAME_KEY, text("happy"));
        settle(&mut arora);
        assert!(read_f32(&arora, &standard::expression_path("happy")) > 0.95);
        assert!(read_f32(&arora, &standard::expression_path("neutral")) < 0.01);
        assert!(read_f32(&arora, &standard::expression_path("sad")) < 0.01);
    }

    #[test]
    fn ros4hri_valence_arousal_blends_anchors() {
        let mut arora = ros4hri_device();
        stage(&arora, ros4hri::EXPRESSION_VALENCE_KEY, float(0.8));
        stage(&arora, ros4hri::EXPRESSION_AROUSAL_KEY, float(0.4));
        settle(&mut arora);
        // The happy anchor sits at (0.8, 0.4): dominant, blended with its
        // positive-affect neighbors, nothing negative.
        let happy = read_f32(&arora, &standard::expression_path("happy"));
        assert!(happy > 0.4, "happy = {happy}");
        assert!(read_f32(&arora, &standard::expression_path("angry")) < 0.01);
        assert!(read_f32(&arora, &standard::expression_path("neutral")) < 0.05);
    }

    #[test]
    fn ros4hri_gaze_maps_eyes_with_vergence() {
        let mut arora = ros4hri_device();
        stage(&arora, ros4hri::GAZE_TARGET_KEY, vec3([1.0, 0.3, 0.2]));
        settle(&mut arora);
        let left_x = read_f32(&arora, standard::LEFT_EYE_POS_X);
        let right_x = read_f32(&arora, standard::RIGHT_EYE_POS_X);
        let left_y = read_f32(&arora, standard::LEFT_EYE_POS_Y);
        // atan(0.33)/0.78 ≈ 0.41 (left, verged outward), atan(0.27)/0.78 ≈ 0.34.
        assert!((0.35..=0.46).contains(&left_x), "left_x = {left_x}");
        assert!((0.29..=0.40).contains(&right_x), "right_x = {right_x}");
        assert!(left_x > right_x, "vergence: {left_x} <= {right_x}");
        // atan(0.2)/0.78 ≈ 0.25.
        assert!((0.20..=0.31).contains(&left_y), "left_y = {left_y}");

        // Targets at or behind the face plane recenter the eyes.
        stage(&arora, ros4hri::GAZE_TARGET_KEY, vec3([0.05, 0.5, 0.0]));
        settle(&mut arora);
        assert!(read_f32(&arora, standard::LEFT_EYE_POS_X).abs() < 0.01);
    }

    #[test]
    fn ros4hri_action_units_route_to_muscles() {
        let mut arora = ros4hri_device();
        stage(&arora, &ros4hri::au_key(12), float(1.0));
        stage(&arora, &ros4hri::au_key(26), float(0.8));
        settle(&mut arora);
        // AU 12 (lip corner puller) drives the lateralized smile pair.
        assert!(read_f32(&arora, &standard::face_path("mouth_smile_left")) > 0.95);
        assert!(read_f32(&arora, &standard::face_path("mouth_smile_right")) > 0.95);
        assert!(read_f32(&arora, &standard::face_path("mouth_frown_left")) < 0.01);
        // AU 26 (jaw drop) drives the muscle control and the de-facto mouth morph.
        let jaw = read_f32(&arora, &standard::face_path("jaw_open"));
        assert!((0.75..=0.85).contains(&jaw), "jaw = {jaw}");
        let defacto = read_f32(&arora, "standard/vizij/mouth/morph/jaw_open");
        assert!((0.75..=0.85).contains(&defacto), "de-facto jaw = {defacto}");
    }

    #[test]
    fn ros4hri_visemes_pass_through() {
        let mut arora = ros4hri_device();
        stage(&arora, &ros4hri::viseme_key("aa"), float(1.0));
        settle(&mut arora);
        assert!(read_f32(&arora, &standard::viseme_path("aa")) > 0.95);
        assert!(read_f32(&arora, &standard::viseme_path("oh")) < 0.01);
    }

    #[test]
    fn ros4hri_blink_pulses_and_commanded_close_wins() {
        let mut arora = ros4hri_device();
        // Over 10 s the jittered 8 s cycle blinks at least once; between
        // blinks the lids rest open.
        let mut min_lid = f32::MAX;
        let mut max_lid = f32::MIN;
        for _ in 0..625 {
            arora.step(Duration::from_millis(16)).expect("step");
            let lid = read_f32(&arora, standard::LEFT_EYE_TOP_EYELID_POS_Y);
            min_lid = min_lid.min(lid);
            max_lid = max_lid.max(lid);
        }
        assert!(max_lid > 0.5, "no blink in 10 s (max lid {max_lid})");
        assert!(min_lid < 0.05, "lids never rest open (min lid {min_lid})");

        // Commanded eyes-closed holds the lids shut and inhibits the pulse.
        stage(&arora, &ros4hri::au_key(43), float(1.0));
        settle(&mut arora);
        for _ in 0..300 {
            arora.step(Duration::from_millis(16)).expect("step");
            let lid = read_f32(&arora, standard::LEFT_EYE_TOP_EYELID_POS_Y);
            assert!(lid > 0.9, "lid opened while commanded closed ({lid})");
        }
    }

    /// End-to-end over the real demo face: graft Quori's standard-adaptation
    /// sidecar into `Quori_Current_Extended.glb` with the bundler, compose the
    /// ROS4HRI profile, and drive the ROS4HRI keys through to Quori's pose
    /// plane. Needs the GLB: set `VIZIJ_FIXTURES` to a directory holding it
    /// (the snapshot-regression convention); skipped otherwise.
    #[test]
    fn ros4hri_drives_the_adapted_quori() {
        let Ok(fixtures) = std::env::var("VIZIJ_FIXTURES") else {
            eprintln!("VIZIJ_FIXTURES unset — skipping the Quori integration test");
            return;
        };
        let glb = std::path::Path::new(&fixtures).join("Quori_Current_Extended.glb");
        let bytes = std::fs::read(&glb).expect("read the Quori GLB");

        // The bundler grafts the committed adaptation sidecar into the bundle.
        let sidecar = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/faces/quori/standard-adaptation.json"
        );
        let spec = vizij_bundle::from_sidecar(
            &std::fs::read_to_string(sidecar).expect("read the adaptation sidecar"),
        )
        .expect("parse the adaptation sidecar");
        let mut face = vizij_bundle::Face::parse(&bytes).expect("parse the Quori GLB");
        face.add_graph("standard-adaptation", "quori_standard_adaptation", spec)
            .expect("graft the adaptation");
        let adapted = face.to_bytes().expect("repack the GLB");
        let cov = vizij_bundle::coverage(&vizij_bundle::Face::parse(&adapted).unwrap());
        assert!(cov.level >= 2, "adapted Quori below L2 (L{})", cov.level);

        let dir = std::env::temp_dir().join("vizij-ros4hri-quori");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("Quori_Current_Extended_adapted.glb");
        std::fs::write(&path, &adapted).expect("write the adapted GLB");

        // Compose like the binary's defaults: standard graphs + the ROS4HRI
        // profile, no autoplay (the idle program would contend on gaze paths).
        let config = FaceConfig {
            wanted: ["rig", "pose-driver", "pose", "standard-adaptation"]
                .map(String::from)
                .to_vec(),
            program: ProgramSelect::None,
            stage_neutral: true,
            ros4hri: true,
        };
        let (_, meta, spec) = load_face(&path, &config).expect("load the adapted face");
        let store = BlackboardStore::new();
        stage_neutral_pose(&store, &meta);
        let mut arora = builder_for(&spec, RigHal::new(), store, &[])
            .expect("build the device")
            .build()
            .expect("build arora");

        // A ROS4HRI expression command reaches Quori's emotion-pose plane.
        stage(&arora, ros4hri::EXPRESSION_NAME_KEY, text("happy"));
        settle(&mut arora);
        let waist = read_f32(&arora, "rig/quori_latest/standard/vizij/expression/happy");
        assert!(waist > 0.95, "profile output missing (happy = {waist})");
        let pose = read_f32(&arora, "rig/quori_latest/poses/pose_d_happy_d.weight");
        assert!(pose > 0.95, "adaptation output missing (pose = {pose})");

        // A viseme command reaches the letter-pose plane.
        stage(&arora, &ros4hri::viseme_key("aa"), float(1.0));
        settle(&mut arora);
        let mouth = read_f32(&arora, "rig/quori_latest/poses/pose_a.weight");
        assert!(mouth > 0.95, "viseme mapping missing (pose_a = {mouth})");
    }
}
