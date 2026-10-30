//! The native driver: the device on a worker thread, fronted by arora's
//! operator flow (the terminal UI on desktop, headless elsewhere; the
//! always-on local WS bridge; the bridges the build adds), rebuilt
//! generation by generation when a front end loads another face.
//!
//! Front ends are interchangeable: the terminal UI, a panel or a file dialog
//! all speak to the running device through a [`RuntimeHandle`], and the view
//! learns of the outcome through [`ViewEvent`]s. Who answers the device's
//! questions — a client's access request, and in a build with Studio the
//! device's enrollment — is the front end's operator, or the app's own
//! ([`Mode::OperatedBy`]).

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::stream::BoxStream;
use futures::StreamExt;
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;

use crate::face::{
    builder_for, declare_keys, description, load_animations, load_face, rig_ranges, spawn_program,
    stage_neutral_pose, FaceConfig, LoadedFace, ProgramSelect,
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
    /// A write through it skips the device's agreement of Studio's target
    /// forms (`studio::TargetForms`), which only the device's own store keeps.
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
    #[cfg_attr(not(feature = "studio"), allow(dead_code))]
    studio: Arc<Mutex<StudioSide>>,
}

impl RuntimeHandle {
    /// The handle of the running generation's Studio bridge: where the
    /// device stands with Semio Studio ([`StudioHandle::status`]) and its
    /// Studio record ([`StudioHandle::record`]). `None` in a run with no
    /// Studio bridge ([`BridgeConfig::studio`]). Each generation has a bridge
    /// of its own, so a front end asks for it again rather than keeping it.
    ///
    /// [`StudioHandle::status`]: arora::studio::StudioHandle::status
    /// [`StudioHandle::record`]: arora::studio::StudioHandle::record
    #[cfg(feature = "studio")]
    pub fn studio(&self) -> Option<arora::studio::StudioHandle> {
        self.studio_side().handle.clone()
    }

    /// Why Studio refused the device's last enrollment in this generation,
    /// if it did. The app's operator ([`Mode::OperatedBy`]) is offered
    /// enrollment again at once, its offer waiting for a person; with the
    /// front end's, the device stays offline for the rest of the run.
    #[cfg(feature = "studio")]
    pub fn studio_refusal(&self) -> Option<String> {
        self.studio_side().refused.clone()
    }

    /// Why the run has no Studio bridge although it asked for one: the
    /// device's Studio side could not be read ([`BridgeConfig::studio`]).
    #[cfg(feature = "studio")]
    pub fn studio_unavailable(&self) -> Option<String> {
        self.studio_side().unavailable.clone()
    }

    #[cfg(feature = "studio")]
    fn studio_side(&self) -> std::sync::MutexGuard<'_, StudioSide> {
        self.studio
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

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

/// The running generation's Studio side, as the driver shares it with the
/// front ends; nothing in a build without Studio.
#[derive(Default)]
struct StudioSide {
    /// The handle of the generation's Studio bridge.
    #[cfg(feature = "studio")]
    handle: Option<arora::studio::StudioHandle>,
    /// Why Studio refused the generation's last enrollment.
    #[cfg(feature = "studio")]
    refused: Option<String>,
    /// Why the run has no Studio bridge although it asked for one.
    #[cfg(feature = "studio")]
    unavailable: Option<String>,
}

enum Command {
    Reload(Vec<u8>),
    Background([u8; 3]),
}

/// How the device fronts the operator.
#[derive(Clone)]
pub enum Mode {
    /// The standard arora operator flow (`AroraBuilder::run`): the terminal UI
    /// on an interactive terminal (headless front end otherwise) with the
    /// vizij commands installed, the open local bridge served where
    /// [`BridgeConfig::local`] says, logging owned by the front end's sink.
    Operator,
    /// [`Mode::Operator`], the app's operator answering the device's
    /// questions in place of the front end's: the settings pane, while the
    /// terminal UI keeps its log pane and its commands. Its enroller offers
    /// the device's enrollment only when a person asks for it, so a refused
    /// enrollment is offered again at once (see [`RuntimeHandle::studio_refusal`]).
    OperatedBy(Arc<dyn arora::operator::Operator>),
    /// Build and step quietly: no bridge, no front end, no commands served —
    /// the snapshot harness, where the process' own logger stays in charge
    /// and a port conflict with a running window instance cannot occur.
    Quiet,
}

/// The bridges the device serves beyond the always-on open local bridge — a
/// build/CLI choice, constant for the process. Empty by default; the fields
/// exist only for the bridge features that are compiled in.
#[derive(Clone)]
pub struct BridgeConfig {
    /// The open local bridge: where it listens, whether it serves the panel.
    pub local: arora::bridge_ws::ServerConfig,
    /// `--ros2 [namespace][:domain]`: expose the device's keys over ROS 2 topics.
    #[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
    pub ros2: Option<(String, u16)>,
    /// The device's Studio side, whose bridge each generation carries from
    /// its start: offline, contacting nobody, while the device holds no
    /// credential, and connecting by itself once it holds one. A front end
    /// enrolls the device through the operator's enroller
    /// (`Operator::enroller`). `None`: no Studio bridge. `Some(Err(why))`:
    /// the run asked for one and the device's Studio side could not be read;
    /// the run goes on without it, logging `why` and showing it to the front
    /// ends ([`RuntimeHandle::studio_unavailable`]).
    #[cfg(feature = "studio")]
    pub studio: Option<Result<arora::studio::StudioDevice, String>>,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            // The control panel on: an app whose window is the face serves
            // the page that drives it.
            local: arora::bridge_ws::ServerConfig::default().serve_control_panel(true),
            #[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
            ros2: None,
            #[cfg(feature = "studio")]
            studio: None,
        }
    }
}

/// A generation's builder with its bridges attached, and what the driver
/// keeps of them.
struct Attached {
    builder: arora::AroraBuilder,
    /// The access requests the Studio bridge yields, for the operator to rule
    /// on.
    access_requests: arora_bridge::AccessRequestStream,
    /// The handle of the generation's Studio bridge, when it has one.
    #[cfg(feature = "studio")]
    studio: Option<arora::studio::StudioHandle>,
}

/// Attach the device's bridges to `builder`: always the open local bridge
/// (the one local editors and apps connect to), plus any the build/CLI adds.
/// Each relays the device as its store and modules describe it — keys, what
/// they are, which a remote may write, and functions — so nothing is declared
/// to a bridge. A bridge that fails to build is logged and skipped, not fatal.
/// `description` is the device's HAL's: the Studio bridge reports it as the
/// device's model (`arora::studio::reported_model`). Called within a Tokio
/// runtime, on which the Studio bridge's connection runs.
#[cfg_attr(
    not(all(any(feature = "ros2-dds", feature = "ros2-zenoh"), feature = "studio")),
    allow(unused_variables)
)]
async fn attach_bridges(
    mut builder: arora::AroraBuilder,
    bridges: &BridgeConfig,
    ros4hri: bool,
    description: &arora_hal::HalDescription,
) -> Attached {
    #[cfg_attr(not(feature = "studio"), allow(unused_mut))]
    let mut access_requests: arora_bridge::AccessRequestStream =
        Box::pin(futures::stream::pending());
    // Studio first: the bridge whose identity the device carries.
    #[cfg(feature = "studio")]
    let studio = match &bridges.studio {
        Some(Ok(device)) => {
            let (bridge, handle) = device.bridge(arora::studio::reported_model(description));
            access_requests = arora_bridge::Bridge::access_requests(&bridge).await;
            builder = builder.with_bridge(Box::new(bridge));
            Some(handle)
        }
        Some(Err(_)) | None => None,
    };
    match arora::local_ws_bridge_with(bridges.local.clone()).await {
        Ok(bridge) => builder = builder.with_bridge(bridge),
        Err(e) => log::error!("local bridge: {e:?}"),
    }
    #[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
    if let Some((namespace, domain)) = &bridges.ros2 {
        let mut config = arora_bridge_ros2::Ros2BridgeConfig::new(namespace.clone(), *domain);
        // The ROS4HRI exposure profile is the whole ROS4HRI surface, so
        // `--no-ros4hri` leaves it out.
        if ros4hri {
            config = config.with_profile(ros4hri_profile());
        }
        builder = builder.with_bridge(Box::new(arora_bridge_ros2::Ros2Bridge::new(config).await));
        log::info!(
            "serving the ROS 2 bridge (namespace {namespace:?}, domain {domain}): the face's \
             inputs subscribed under /{namespace}/keys/<path>{}",
            if ros4hri {
                ", plus the ROS4HRI profile (the typed face topics, the face image, the \
                 /skill/look_at and /skill/say actions)"
            } else {
                ""
            }
        );
    }
    Attached {
        builder,
        access_requests,
        #[cfg(feature = "studio")]
        studio,
    }
}

/// Load the face from its GLB bytes (its graph kinds filtered by `config`)
/// and run its device on a worker thread.
///
/// The `Arora` is constructed **inside** the worker thread — it is not `Send`
/// (single-owner by design); only the spec JSON and the sibling rig/store
/// handles cross the thread boundary. The face is loaded here first so
/// composition errors surface to the caller, not in a log.
pub fn start(glb: &[u8], config: FaceConfig, bridges: BridgeConfig, mode: Mode) -> Result<Runtime> {
    let LoadedFace {
        meta,
        spec,
        contended,
    } = load_face(glb, &config)?;
    let rig = face_rig(glb, &meta);
    let store = BlackboardStore::new();
    let (events_tx, events_rx) = std::sync::mpsc::channel();
    let (commands_tx, commands_rx) = futures::channel::mpsc::unbounded();
    let studio = Arc::new(Mutex::new(StudioSide::default()));
    let _ = events_tx.send(ViewEvent::LoadFace {
        face_id: FACE.to_string(),
        meta: Box::new(meta.clone()),
        glb: glb.to_vec(),
        rig: rig.clone(),
    });
    let _ = events_tx.send(ViewEvent::Composed {
        face_id: FACE.to_string(),
        contended,
    });

    let thread = {
        let rig = rig.clone();
        let store = store.clone();
        // The supervisor stages and announces each generation from the meta; the
        // view keeps its own copy for the initial face.
        let meta = meta.clone();
        let glb = glb.to_vec();
        let studio = studio.clone();
        thread::Builder::new().name("arora".into()).spawn(move || {
            let operator = match mode {
                Mode::Operator => None,
                Mode::OperatedBy(operator) => Some(operator),
                Mode::Quiet => return run_quiet(&spec, &meta, &config, rig, store),
            };
            let face = Generation {
                spec,
                meta,
                glb,
                contended: Vec::new(),
            };
            supervise(
                face,
                config,
                bridges,
                operator,
                (rig, store),
                events_tx,
                commands_rx,
                studio,
            )
        })?
    };

    Ok(Runtime {
        rig,
        store,
        meta,
        events: events_rx,
        handle: RuntimeHandle {
            commands: commands_tx,
            studio,
        },
        _thread: thread,
    })
}

/// The rig of a device showing `meta`'s face from `glb`: it describes the
/// device as the face's model ([`description`]) and holds the GLB, the model
/// the device states and serves (Semio Studio's `retrieveModels` /
/// `retrieveModelGlb`). Each generation's is its face's.
fn face_rig(glb: &[u8], meta: &FaceMeta) -> RigHal {
    let rig = RigHal::with_description(description(&meta.bundle));
    rig.set_model_glb(glb.to_vec());
    rig
}

/// The quiet mode's device: built once and stepped, with no bridge and no
/// front end ([`Mode::Quiet`]).
fn run_quiet(
    spec: &str,
    meta: &FaceMeta,
    config: &FaceConfig,
    rig: RigHal,
    store: BlackboardStore,
) {
    if config.stage_neutral {
        stage_neutral_pose(&store, meta);
    }
    let speech = config.speech.as_ref().map(|build| build());
    let Some(builder) = builder_for(spec, rig, store, &meta.bundle.skills, speech) else {
        return;
    };
    match builder.build() {
        Ok(mut arora) => {
            load_face_animations(&mut arora, meta);
            spawn_face_program(&mut arora, meta, &config.program);
            step_forever(&mut arora)
        }
        Err(e) => log::error!("building the arora device: {e:?}"),
    }
}

/// The face a generation runs: its composed spec and meta, its GLB, and what
/// its composition contends.
struct Generation {
    spec: String,
    meta: FaceMeta,
    glb: Vec<u8>,
    contended: Vec<vizij_arora_host::Contention>,
}

/// The operator flow, generation by generation: each reload stops the
/// running device and rebuilds it — graphs, rig, store, bridges — on the new
/// face, its Studio bridge a new one reporting the new face's model, and the
/// view follows through [`ViewEvent::LoadFace`]. `operator`, when given,
/// answers in place of the front end's, and presents the device's enrollment
/// with Studio ([`enroll`]). Runs until a generation ends on its own (a device
/// error; the terminal UI's quit exits the process).
#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(feature = "studio"), allow(unused_variables))]
fn supervise(
    mut face: Generation,
    config: FaceConfig,
    bridges: BridgeConfig,
    operator: Option<Arc<dyn arora::operator::Operator>>,
    handles: (RigHal, BlackboardStore),
    events: Sender<ViewEvent>,
    mut commands: UnboundedReceiver<Command>,
    studio: Arc<Mutex<StudioSide>>,
) {
    let tokio_rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return log::error!("tokio runtime: {e}"),
    };
    // The first generation runs on the handles the view already holds; a
    // later one's face is announced only after its front end is up, so the
    // view's swap (and everything it logs) lands in the live pane.
    let mut fresh = Some(handles);
    loop {
        let (mut frontend, tui) = operator_frontend();
        if let (Some(frontend), Some(operator)) = (frontend.as_mut(), &operator) {
            frontend.operator = operator.clone();
        }
        let (rig, store) = match fresh.take() {
            Some(handles) => handles,
            None => {
                let (rig, store) = (face_rig(&face.glb, &face.meta), BlackboardStore::new());
                let _ = events.send(ViewEvent::LoadFace {
                    face_id: FACE.to_string(),
                    meta: Box::new(face.meta.clone()),
                    glb: face.glb.clone(),
                    rig: rig.clone(),
                });
                let _ = events.send(ViewEvent::Composed {
                    face_id: FACE.to_string(),
                    contended: face.contended.clone(),
                });
                (rig, store)
            }
        };
        if config.stage_neutral {
            stage_neutral_pose(&store, &face.meta);
        }
        // What the face's keys are — the inputs a remote may drive, where
        // each rests — said to the store, which every bridge relays.
        declare_keys(
            &store,
            &face.spec,
            &face.meta.bundle.programs,
            &face.meta.bundle.neutral_stage_writes(),
            &rig_ranges(&face.meta.bundle),
        );
        // What the device says it is, the generation's face: the model its
        // Studio bridge reports, as arora's own run passes its HAL's.
        let description = tokio_rt.block_on(arora_hal::Hal::describe(&rig));
        let speech = config.speech.as_ref().map(|build| build());
        let Some(builder) = builder_for(
            &face.spec,
            rig,
            store.clone(),
            &face.meta.bundle.skills,
            speech,
        ) else {
            return;
        };
        let next = tokio_rt.block_on(async {
            let attached = attach_bridges(builder, &bridges, config.ros4hri, &description).await;
            #[cfg(feature = "studio")]
            let enrollment = {
                let enroller = match &frontend {
                    Some(frontend) => frontend.operator.clone().enroller(),
                    None => Arc::new(arora::studio::UnattendedEnroller),
                };
                let unavailable = match &bridges.studio {
                    Some(Err(why)) => Some(why.clone()),
                    _ => None,
                };
                let offers_on_request = operator.is_some();
                enroll(
                    attached.studio.clone(),
                    unavailable,
                    enroller,
                    offers_on_request,
                    &studio,
                )
            };
            #[cfg(not(feature = "studio"))]
            let enrollment = futures::future::pending::<std::convert::Infallible>();
            // A rebuild drops the run future — arora's stop story: the
            // teardown is complete and synchronous (front end released, local
            // bridge's port freed) before the next generation starts.
            tokio::select! {
                result = run_generation(attached.builder, frontend, attached.access_requests, &face.meta, &config.program) => {
                    if let Err(e) = result {
                        log::error!("arora device stopped: {e:?}");
                    }
                    None
                }
                next = pump(tui, &mut commands, &config, &events) => Some(next),
                never = enrollment => match never {},
            }
        });
        match next {
            None => break,
            Some(next) => face = *next,
        }
    }
}

/// Enroll the device with Semio Studio beside its generation, presented by
/// `enroller` — the settings pane's, the terminal UI's, or nobody's — for as
/// long as the generation runs, as the device client's
/// `StudioHandle::enroll_when_needed` does: whenever the device's Studio
/// bridge finds it not enrolled, once `enroller` accepts. The generation's
/// handle is the front ends' meanwhile ([`RuntimeHandle::studio`]), or why
/// the run has none (`unavailable`, logged).
///
/// The run outlives its enrollment. Studio refusing an enrollment is logged
/// and shown ([`RuntimeHandle::studio_refusal`]); an enroller that
/// `offers_on_request` — the app's operator, whose offer waits for a person
/// — is offered enrollment again at once, and any other stops enrolling for
/// the rest of the run, the device staying offline. A bridge that halts
/// stays offline, its status saying why. Never resolves.
#[cfg(feature = "studio")]
async fn enroll(
    handle: Option<arora::studio::StudioHandle>,
    unavailable: Option<String>,
    enroller: Arc<dyn arora::studio::Enroller>,
    offers_on_request: bool,
    studio: &Mutex<StudioSide>,
) -> std::convert::Infallible {
    let share = |refused: Option<String>| {
        *studio
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = StudioSide {
            handle: handle.clone(),
            refused,
            unavailable: unavailable.clone(),
        };
    };
    share(None);
    if let Some(why) = &unavailable {
        log::error!("no Semio Studio bridge: {why}");
    }
    let Some(studio_handle) = handle.clone() else {
        return futures::future::pending().await;
    };
    loop {
        match studio_handle.enroll_when_needed(&*enroller).await {
            arora::studio::EnrollmentEnded::Refused(e) => {
                log::error!("Studio refused the device's enrollment: {e}");
                share(Some(e.to_string()));
                if !offers_on_request {
                    log::error!("the device stays offline with Studio for the rest of this run");
                    return futures::future::pending().await;
                }
            }
            ended => {
                log::error!("{ended}");
                return futures::future::pending().await;
            }
        }
    }
}

/// Run one device generation: build it — the face's animations loaded, its
/// `program` running — hand the front end its live view, and step until the
/// run ends, the front end's operator ruling on the `access_requests` the
/// while. The steps of arora's own operator flow (`AroraBuilder::run`),
/// taken here because this app owns the generation loop.
async fn run_generation(
    builder: arora::AroraBuilder,
    frontend: Option<arora::operator::Frontend>,
    access_requests: arora_bridge::AccessRequestStream,
    meta: &FaceMeta,
    program: &ProgramSelect,
) -> Result<()> {
    let mut arora = builder.build().context("failed to build the device")?;
    load_face_animations(&mut arora, meta);
    spawn_face_program(&mut arora, meta, program);
    let operator = frontend.map(|frontend| {
        (frontend.on_ready)(arora.store().subscribe(), None, None);
        frontend.operator
    });
    let run = arora.run(arora::Arora::DEFAULT_STEP_PERIOD);
    tokio::pin!(run);
    // Served in this future, as arora serves them: dropping the generation
    // drops a ruling in flight with it. The stream ending ends nothing.
    if let Some(operator) = operator {
        tokio::select! {
            result = &mut run => return result.map_err(|e| anyhow::anyhow!("runtime error: {e}")),
            () = arora::operator::serve_access_requests(access_requests, operator) => {}
        }
    }
    run.await.map_err(|e| anyhow::anyhow!("runtime error: {e}"))
}

/// Serve one device generation's front ends: the terminal UI's command
/// events (`g` reads the GLB at the given path, `b` parses a background) and
/// the [`RuntimeHandle`] commands, each validated here — a bad path, a GLB
/// that does not compose or a malformed colour is an error in the log, not
/// a dead device. Returns the face to rebuild the device on, once one is
/// validated for a reload. A generation whose front ends have all gone quiet
/// (a headless run, every handle dropped) never returns, so that the pump
/// merely ending cannot stop a healthy run.
async fn pump(
    tui: BoxStream<'static, Command>,
    commands: &mut UnboundedReceiver<Command>,
    config: &FaceConfig,
    events: &Sender<ViewEvent>,
) -> Box<Generation> {
    let mut front_ends = futures::stream::select(tui, commands.by_ref());
    while let Some(command) = front_ends.next().await {
        match command {
            Command::Reload(glb) => match load_face(&glb, config) {
                Ok(loaded) => {
                    return Box::new(Generation {
                        spec: loaded.spec,
                        meta: loaded.meta,
                        glb,
                        contended: loaded.contended,
                    })
                }
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

/// Without a terminal UI, arora's headless front end: it installs the log
/// sink and never answers a prompt.
#[cfg(not(feature = "desktop"))]
fn operator_frontend() -> (
    Option<arora::operator::Frontend>,
    BoxStream<'static, Command>,
) {
    (
        Some(arora::operator::default_frontend()),
        futures::stream::pending().boxed(),
    )
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

/// Load the face's animations into its freshly built device
/// ([`load_animations`]). A failure is logged and the device runs on without
/// them: every other skill still serves.
fn load_face_animations(arora: &mut arora::Arora, meta: &FaceMeta) {
    if let Err(e) = load_animations(arora, &meta.bundle) {
        log::error!("loading the face's animations: {e:#}");
    }
}

/// Run the face's program on its freshly built device ([`spawn_program`]).
/// A failure is logged and the device runs on without it.
fn spawn_face_program(arora: &mut arora::Arora, meta: &FaceMeta, program: &ProgramSelect) {
    if let Err(e) = spawn_program(arora, &meta.bundle, program) {
        log::error!("running the face's program: {e:#}");
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

/// The ROS4HRI exposure profile as this device serves it: the typed face
/// topics fanning onto the standard keys, the face image on its
/// `image_transport` pair, and the standard skills — `/skill/look_at`,
/// `/skill/say` — bound to the gaze and speech task runs the device
/// describes.
///
/// The preset comes from arora-bridge-ros2, which knows nothing of Vizij and
/// binds skills by name. `/skill/set_expression` lands on the
/// `standard/ros4hri/expression/*` keys the face's graphs read, since the
/// device describes no `set_expression` method. A skill the preset binds to a
/// method this device does not describe (`play_visemes`,
/// `set_facial_action_units`, `set_viseme`) is left out: the bridge would
/// refuse it at startup, and a ROS graph would list an action nothing serves.
#[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
fn ros4hri_profile() -> arora_bridge_ros2::ExposureProfile {
    let declared = |function: &str| vizij_arora_host::skills::skill(function).is_some();
    let mut profile = arora_bridge_ros2::ExposureProfile::ros4hri().with_set_expression_on_keys();
    profile
        .actions
        .retain(|binding| declared(&binding.function));
    profile
        .topic_skills
        .retain(|binding| declared(&binding.function));
    profile
}

/// The profile this device serves binds skills by name: each skill it binds
/// is one this device declares, taking every parameter a goal field is
/// routed to, and the gaze and speech skills are among them.
#[cfg(all(test, any(feature = "ros2-dds", feature = "ros2-zenoh")))]
mod ros4hri_profile_tests {
    use arora_types::record::module::frozen::ExportKind;
    use uuid::Uuid;
    use vizij_arora_host::skills::{look_at, play_viseme, say};

    /// The closed caption the profile takes lands on the keys the `ros4hri`
    /// profile declares for it — the ones the mapping relays to the face's
    /// caption.
    #[test]
    fn the_ros4hri_profile_lands_a_caption_on_the_mapping_s_keys() {
        use vizij_arora_host::ros4hri::{CAPTION_SPEAKER_KEY, CAPTION_TEXT_KEY};
        let profile = arora_bridge_ros2::ExposureProfile::ros4hri();
        let endpoint = profile
            .endpoints
            .iter()
            .find(|e| e.ros_type == "hri_actions_msgs/ClosedCaption")
            .expect("the profile takes captions");
        let routes: Vec<(&str, &str)> = endpoint
            .routes
            .iter()
            .map(|r| (r.field.as_str(), r.key.as_str()))
            .collect();
        assert_eq!(
            routes,
            [
                ("text", CAPTION_TEXT_KEY),
                ("speaker_id", CAPTION_SPEAKER_KEY)
            ]
        );
    }

    #[test]
    fn every_skill_the_ros4hri_profile_binds_is_a_declared_contract() {
        let contracts = [
            look_at::record(Uuid::nil()),
            play_viseme::record(Uuid::nil()),
            say::record(Uuid::nil()),
        ];
        let profile = super::ros4hri_profile();
        for skill in [look_at::NAME, say::NAME] {
            assert!(
                profile.actions.iter().any(|b| b.function == skill),
                "the profile binds `{skill}`"
            );
        }
        assert!(
            profile
                .topic_skills
                .iter()
                .all(|b| b.function != "set_expression"),
            "set_expression lands on the expression keys, not on a method"
        );
        for binding in &profile.actions {
            let signature = contracts
                .iter()
                .flat_map(|contract| contract.exports.values())
                .find(|export| export.name == binding.function)
                .map(|export| {
                    let ExportKind::Function(signature) = &export.kind;
                    signature
                })
                .unwrap_or_else(|| {
                    panic!(
                        "{} binds `{}`, which no skill contract declares",
                        binding.action, binding.function
                    )
                });
            for route in &binding.goal_routes {
                assert!(
                    signature.parameters.values().any(|p| p.name == route.key),
                    "{} routes `{}` to `{}`, which `{}` does not take",
                    binding.action,
                    route.field,
                    route.key,
                    binding.function
                );
            }
        }
    }
}

/// What each device generation says it is: its face's model, the first
/// generation's and a reload's.
#[cfg(test)]
mod model_tests {
    use super::*;
    use arora_hal::{Hal, HalDescription};

    /// A GLB holding only a face's bundle: a rig reading one input, and
    /// `metadata`.
    fn face_glb(metadata: serde_json::Value) -> Vec<u8> {
        let gltf = serde_json::json!({
            "asset": { "version": "2.0" },
            "nodes": [{ "name": "root", "extensions": { "VIZIJ_bundle": {
                "version": 1,
                "metadata": metadata,
                "graphs": [{ "id": "rig", "kind": "rig", "spec": { "nodes": [
                    { "id": "x", "type": "input", "params": { "path": "x", "value": 0.0 } },
                ], "edges": [] } }],
            } } }],
        });
        let mut json = serde_json::to_vec(&gltf).unwrap();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let mut glb = Vec::new();
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&((12 + 8 + json.len()) as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json);
        glb
    }

    fn config() -> FaceConfig {
        FaceConfig {
            wanted: vec!["rig".to_string()],
            program: ProgramSelect::None,
            stage_neutral: false,
            ros4hri: false,
            #[cfg(feature = "studio")]
            studio: false,
            speech: None,
        }
    }

    fn vizij(hardware: &str, software: Option<&str>) -> HalDescription {
        HalDescription {
            model_family: Some("vizij".into()),
            hardware_version: Some(hardware.into()),
            software_version: software.map(str::to_string),
        }
    }

    /// The device describes the face it starts on; reloading another face
    /// rebuilds the device on a rig describing that one, the description the
    /// generation's Studio bridge reports.
    #[test]
    fn each_generation_describes_its_face() {
        let quori = face_glb(serde_json::json!({
            "faceId": "quori",
            "hardwareVersion": "quori-1.3",
            "softwareVersion": "1.2",
        }));
        let device = start(&quori, config(), BridgeConfig::default(), Mode::Quiet)
            .expect("start the device");
        assert_eq!(
            futures::executor::block_on(device.rig.describe()),
            vizij("quori-1.3", Some("1.2"))
        );

        let (commands_tx, mut commands) = futures::channel::mpsc::unbounded();
        let (events, _events) = std::sync::mpsc::channel();
        let emy = face_glb(serde_json::json!({ "faceId": "emy" }));
        commands_tx
            .unbounded_send(Command::Reload(emy))
            .expect("send the reload");
        let next = futures::executor::block_on(pump(
            futures::stream::pending().boxed(),
            &mut commands,
            &config(),
            &events,
        ));
        let rig = face_rig(&next.glb, &next.meta);
        assert_eq!(
            futures::executor::block_on(rig.describe()),
            vizij("emy", None),
            "a face stating no version is described by its id"
        );
    }
}

/// A device's Studio side before it is enrolled.
#[cfg(all(test, feature = "studio"))]
mod studio_tests {
    use super::*;
    use arora::studio::{Enroller, StudioStatus};

    /// An enroller whose person never answers the offer to enroll.
    struct Undecided;

    #[async_trait::async_trait]
    impl Enroller for Undecided {
        async fn offer_enrollment(&self) -> bool {
            futures::future::pending().await
        }
    }

    /// A device holding no credential is not enrolled: its bridge stays
    /// offline, and nothing enrolls it until its enroller accepts, the
    /// generation's handle shared with the front ends meanwhile.
    #[tokio::test]
    async fn a_device_holding_no_credential_waits_offline_for_its_enrollment() {
        let dir = std::env::temp_dir().join(format!("vizij-studio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let device = arora::studio::in_device_dir(
            &dir,
            arora::device_dir::DEFAULT_NAME,
            Some(&dir.join("devices")),
        )
        .expect("the device's Studio side");
        let (bridge, handle) = device.bridge(None);
        let side = Mutex::new(StudioSide::default());
        let enrolling = tokio::time::timeout(
            Duration::from_millis(300),
            enroll(Some(handle.clone()), None, Arc::new(Undecided), true, &side),
        )
        .await;
        assert!(enrolling.is_err(), "the enrollment runs as long as the run");
        assert_eq!(*handle.status().borrow(), StudioStatus::NotEnrolled);
        let shared = side.lock().unwrap();
        assert!(shared.handle.is_some(), "the front ends hold the handle");
        assert_eq!(shared.refused, None);
        drop(bridge);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A run whose Studio side could not be read goes on without its
    /// bridge, and tells the front ends why.
    #[tokio::test]
    async fn a_run_without_its_studio_side_says_why() {
        let side = Mutex::new(StudioSide::default());
        let enrolling = tokio::time::timeout(
            Duration::from_millis(100),
            enroll(
                None,
                Some("no device store".into()),
                Arc::new(arora::studio::UnattendedEnroller),
                false,
                &side,
            ),
        )
        .await;
        assert!(enrolling.is_err(), "the run goes on");
        let shared = side.lock().unwrap();
        assert!(shared.handle.is_none());
        assert_eq!(shared.unavailable.as_deref(), Some("no device store"));
    }
}
