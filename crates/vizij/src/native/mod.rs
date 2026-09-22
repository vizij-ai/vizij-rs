//! The native driver: the device on a worker thread, fronted by arora's
//! operator flow (the terminal UI on desktop, headless elsewhere; the
//! always-on local WS bridge; the bridges the build adds), rebuilt
//! generation by generation when a front end loads another face.
//!
//! Front ends are interchangeable: the terminal UI, a panel or a file dialog
//! all speak to the running device through a [`RuntimeHandle`], and the view
//! learns of the outcome through [`ViewEvent`]s.

use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::stream::BoxStream;
use futures::StreamExt;
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;

use crate::face::{
    builder_for, free_inputs, load_face, stage_neutral_pose, FaceConfig, LoadedFace,
};
use crate::view::meta::FaceMeta;
use crate::view::ViewEvent;

/// The one face slot the desktop shows: what its view events name.
pub const FACE: &str = "face";

/// A running face device. Both handles share storage with the device's own
/// (they are sibling clones), so the view reads the rig and the store live.
pub struct Runtime {
    pub rig: RigHal,
    /// Sibling store handle; unused until input staging/UI lands (VIZ-47 UI stage).
    #[allow(dead_code)]
    pub store: BlackboardStore,
    /// The face the device booted on.
    pub meta: FaceMeta,
    /// What the view applies: the face to show (the first event, already
    /// queued), the faces the device moves onto and the background changes a
    /// front end asks for.
    pub events: Receiver<ViewEvent>,
    /// The way into the running device for any front end.
    pub handle: RuntimeHandle,
    _thread: thread::JoinHandle<()>,
}

/// What a front end asks of the running device. Cloneable, so the terminal
/// UI, a panel and a file dialog each hold one.
#[derive(Clone)]
pub struct RuntimeHandle {
    commands: UnboundedSender<Command>,
}

impl RuntimeHandle {
    /// Load another face from its GLB bytes: the device restarts on its
    /// graphs and the view swaps over ([`ViewEvent::LoadFace`]). A GLB
    /// that does not compose is an error in the log, not a dead device.
    pub fn reload(&self, glb: Vec<u8>) {
        let _ = self.commands.unbounded_send(Command::Reload(glb));
    }

    /// Change the view's background ([`ViewEvent::Background`]).
    pub fn set_background(&self, rgb: [u8; 3]) {
        let _ = self.commands.unbounded_send(Command::Background(rgb));
    }
}

enum Command {
    Reload(Vec<u8>),
    Background([u8; 3]),
}

/// How the device fronts the operator.
pub enum Mode {
    /// The standard arora operator flow (`AroraBuilder::run`): the terminal UI
    /// on an interactive terminal (headless front end otherwise) with the
    /// vizij commands installed, the open local bridge auto-attached
    /// (`ws://127.0.0.1:9000`), logging owned by the front end's sink.
    Operator,
    /// Build and step quietly: no bridge, no front end, no commands served —
    /// the snapshot harness, where the process' own logger stays in charge
    /// and a port conflict with a running window instance cannot occur.
    Quiet,
}

/// The bridges the device serves beyond the always-on open local bridge — a
/// build/CLI choice, constant for the process. Empty by default; the fields
/// exist only for the bridge features that are compiled in.
#[derive(Clone, Default)]
pub struct BridgeConfig {
    /// `--ros2 [namespace][:domain]`: expose the device's keys over ROS 2 topics.
    #[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
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
    not(any(feature = "ros2-dds", feature = "ros2-zenoh", feature = "studio")),
    allow(unused_variables)
)]
pub async fn attach_bridges(
    mut builder: arora::AroraBuilder,
    bridges: &BridgeConfig,
    ros4hri: bool,
    data_inputs: &[(String, arora_types::value::Type)],
) -> arora::AroraBuilder {
    #[cfg(not(any(feature = "ros2-dds", feature = "ros2-zenoh")))]
    let _ = (data_inputs, ros4hri);
    match arora::local_ws_bridge().await {
        Ok(bridge) => builder = builder.with_bridge(bridge),
        Err(e) => log::error!("local bridge: {e:?}"),
    }
    #[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
    if let Some((namespace, domain)) = &bridges.ros2 {
        let mut config = arora_bridge_ros2::Ros2BridgeConfig::new(namespace.clone(), *domain);
        // The ROS4HRI exposure profile: the typed face topics fanning onto the
        // standard keys, the face image on its `image_transport` pair, and the
        // standard skills — `/skill/look_at`, `/skill/say` — bound to the gaze
        // and speech task runs the device describes. It is the whole ROS4HRI
        // surface, so `--no-ros4hri` leaves it out.
        if ros4hri {
            config = config.with_profile(arora_bridge_ros2::ExposureProfile::ros4hri());
        }
        // The face's free inputs — what nothing in the composed graph writes —
        // are the keys a remote may drive, each subscribed as the std_msgs type
        // of its kind (`ros2 topic info -v` shows it).
        for (path, ty) in data_inputs {
            config = config.with_input(path.clone(), ty.clone());
        }
        builder = builder.with_bridge(Box::new(arora_bridge_ros2::Ros2Bridge::new(config).await));
        log::info!(
            "serving the ROS 2 bridge (namespace {namespace:?}, domain {domain}): {} input keys \
             subscribed under /{namespace}/keys/<path>{}",
            data_inputs.len(),
            if ros4hri {
                ", plus the ROS4HRI profile (the typed face topics, the face image, the \
                 /skill/look_at and /skill/say actions)"
            } else {
                ""
            }
        );
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

/// Load the face from its GLB bytes (its graph kinds filtered by `config`)
/// and run its device on a worker thread.
///
/// The `Arora` is constructed **inside** the worker thread — it is not `Send`
/// (single-owner by design); only the spec JSON and the sibling rig/store
/// handles cross the thread boundary. The face is loaded here first so
/// composition errors surface to the caller, not in a log.
pub fn start(glb: &[u8], config: FaceConfig, bridges: BridgeConfig, mode: Mode) -> Result<Runtime> {
    let LoadedFace { meta, spec } = load_face(glb, &config)?;
    let rig = RigHal::new();
    let store = BlackboardStore::new();
    let (events_tx, events_rx) = std::sync::mpsc::channel();
    let (commands_tx, commands_rx) = futures::channel::mpsc::unbounded();
    let _ = events_tx.send(ViewEvent::LoadFace {
        face_id: FACE.to_string(),
        meta: Box::new(meta.clone()),
        glb: glb.to_vec(),
        rig: rig.clone(),
    });

    let thread = {
        let rig = rig.clone();
        let store = store.clone();
        // The supervisor stages and announces each generation from the meta; the
        // view keeps its own copy for the initial face.
        let meta = meta.clone();
        thread::Builder::new()
            .name("arora".into())
            .spawn(move || match mode {
                Mode::Operator => supervise(
                    spec,
                    meta,
                    config,
                    bridges,
                    rig,
                    store,
                    events_tx,
                    commands_rx,
                ),
                Mode::Quiet => {
                    if config.stage_neutral {
                        stage_neutral_pose(&store, &meta);
                    }
                    let speech = config.speech.as_ref().map(|build| build());
                    let Some(builder) = builder_for(&spec, rig, store, &meta.bundle.skills, speech)
                    else {
                        return;
                    };
                    match builder.build() {
                        Ok(mut arora) => step_forever(&mut arora),
                        Err(e) => log::error!("building the arora device: {e:?}"),
                    }
                }
            })?
    };

    Ok(Runtime {
        rig,
        store,
        meta,
        events: events_rx,
        handle: RuntimeHandle {
            commands: commands_tx,
        },
        _thread: thread,
    })
}

/// The operator flow, generation by generation: each reload stops the
/// running device and rebuilds it — graphs, rig, store — on the new face,
/// and the view follows through [`ViewEvent::LoadFace`]. Runs until a
/// generation ends without a reload (a device error; the terminal UI's quit
/// exits the process).
#[allow(clippy::too_many_arguments)]
fn supervise(
    mut spec: String,
    mut meta: FaceMeta,
    config: FaceConfig,
    bridges: BridgeConfig,
    rig: RigHal,
    store: BlackboardStore,
    events: Sender<ViewEvent>,
    mut commands: UnboundedReceiver<Command>,
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
    let mut pending_glb: Option<Vec<u8>> = None;
    loop {
        let (rig, store) = fresh
            .take()
            .unwrap_or_else(|| (RigHal::new(), BlackboardStore::new()));
        let (frontend, tui) = operator_frontend();
        if let Some(glb) = pending_glb.take() {
            let _ = events.send(ViewEvent::LoadFace {
                face_id: FACE.to_string(),
                meta: Box::new(meta.clone()),
                glb,
                rig: rig.clone(),
            });
        }
        if config.stage_neutral {
            stage_neutral_pose(&store, &meta);
        }
        let speech = config.speech.as_ref().map(|build| build());
        let Some(builder) = builder_for(&spec, rig, store, &meta.bundle.skills, speech) else {
            return;
        };
        let reload = tokio_rt.block_on(async {
            let builder = match frontend {
                Some(frontend) => builder.with_frontend(frontend),
                None => builder,
            };
            let builder =
                attach_bridges(builder, &bridges, config.ros4hri, &free_inputs(&spec)).await;
            // A reload drops the run future — arora's stop story: the
            // teardown is complete and synchronous (front end released, local
            // bridge's port freed) before the next generation starts.
            tokio::select! {
                result = builder.run() => {
                    if let Err(e) = result {
                        log::error!("arora device stopped: {e:?}");
                    }
                    None
                }
                reload = pump(tui, &mut commands, &config, &events) => Some(reload),
            }
        });
        let Some((loaded, glb)) = reload else {
            break;
        };
        spec = loaded.spec;
        meta = loaded.meta;
        pending_glb = Some(glb);
    }
}

/// Serve one device generation's front ends: the terminal UI's command
/// events (`g` reads the GLB at the given path, `b` parses a background) and
/// the [`RuntimeHandle`] commands, each validated here — a bad path, a GLB
/// that does not compose or a malformed colour is an error in the log, not
/// a dead device. Returns when a face has been validated for a reload, with
/// what the supervisor needs to rebuild; a generation whose front ends have
/// all gone quiet (a headless run, every handle dropped) never returns, so
/// that the pump merely ending cannot stop a healthy run.
async fn pump(
    tui: BoxStream<'static, Command>,
    commands: &mut UnboundedReceiver<Command>,
    config: &FaceConfig,
    events: &Sender<ViewEvent>,
) -> (LoadedFace, Vec<u8>) {
    let mut front_ends = futures::stream::select(tui, commands.by_ref());
    while let Some(command) = front_ends.next().await {
        match command {
            Command::Reload(glb) => match load_face(&glb, config) {
                Ok(loaded) => return (loaded, glb),
                Err(e) => log::error!("cannot load the face: {e:#}"),
            },
            Command::Background(rgb) => {
                let _ = events.send(ViewEvent::Background(rgb));
            }
        }
    }
    futures::future::pending().await
}

/// The operator's front end for one generation and the commands it issues:
/// on desktop the terminal UI with the vizij commands installed (`g` loads a
/// GLB by path, `b` sets the background); elsewhere none — arora picks its
/// headless front end and no command ever comes from it.
#[cfg(feature = "desktop")]
fn operator_frontend() -> (
    Option<arora::operator::Frontend>,
    BoxStream<'static, Command>,
) {
    use arora::tui::{commands_frontend, TuiCommand};
    let (frontend, events) = commands_frontend(vec![
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
    let commands = events.filter_map(|event| futures::future::ready(tui_command(event)));
    (Some(frontend), commands.boxed())
}

#[cfg(not(feature = "desktop"))]
fn operator_frontend() -> (
    Option<arora::operator::Frontend>,
    BoxStream<'static, Command>,
) {
    (None, futures::stream::pending().boxed())
}

/// The command a terminal UI event carries, once its input is read or parsed;
/// `None` for an event that is no command (logged when it was a bad one).
#[cfg(feature = "desktop")]
fn tui_command(event: arora::tui::TuiCommandEvent) -> Option<Command> {
    match (event.key, event.input) {
        ('g', Some(path)) => match std::fs::read(&path) {
            Ok(glb) => Some(Command::Reload(glb)),
            Err(e) => {
                log::error!("cannot read {path}: {e}");
                None
            }
        },
        ('b', Some(hex)) => match crate::face::parse_rgb(&hex) {
            Ok(rgb) => Some(Command::Background(rgb)),
            Err(e) => {
                log::error!("background: {e}");
                None
            }
        },
        _ => None,
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

/// The device's terminal UI runs on the worker thread; returning from a Bevy
/// run ends the process without unwinding that thread, which would leave the
/// terminal in raw mode on the alternate screen. Undo its setup (arora's
/// `restore_terminal` recipe) on the way out.
#[cfg(feature = "desktop")]
pub fn restore_terminal() {
    if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        use crossterm::event::DisableMouseCapture;
        use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
    }
}
