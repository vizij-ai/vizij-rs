//! The native driver: the device on a worker thread, fronted by arora's
//! operator flow (the terminal UI on desktop, headless elsewhere; the
//! always-on local WS bridge; the bridges the build adds), rebuilt
//! generation by generation when a front end loads another face.
//!
//! Front ends are interchangeable: the terminal UI, a panel or a file dialog
//! all speak to the running device through a [`RuntimeHandle`], and the view
//! learns of the outcome through [`ViewEvent`]s. Who answers the device's
//! questions — the registration Studio needs, a client's access request — is
//! the front end's operator, or the app's own ([`Mode::OperatedBy`]).

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
    builder_for, declare_keys, load_animations, load_face, rig_ranges, spawn_program,
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
    studio: Arc<Mutex<StudioStatus>>,
}

impl RuntimeHandle {
    /// Where the device stands with Semio Studio.
    pub fn studio(&self) -> StudioStatus {
        self.studio
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default()
    }

    /// Connect to Studio for the rest of the run: the device restarts on its
    /// face with the Studio bridge, registered under the identity it keeps,
    /// else as the operator answers. Nothing happens in a run with no Studio
    /// ([`StudioStatus::Off`]).
    pub fn associate(&self) {
        let _ = self.commands.unbounded_send(Command::Associate(None));
    }

    /// [`associate`](Self::associate) a device that keeps no identity yet
    /// under `name` and `owners` (Studio user ids), the identity it then
    /// keeps: the first association, from the settings pane's form. A device
    /// that keeps one already connects under it, the name and owners being
    /// Studio's once it is associated.
    #[cfg_attr(not(feature = "settings"), allow(dead_code))]
    pub(crate) fn associate_as(&self, name: String, owners: Vec<String>) {
        let identity = Identity {
            name,
            owners,
            model_family: None,
        };
        let _ = self
            .commands
            .unbounded_send(Command::Associate(Some(identity)));
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

enum Command {
    Reload(Vec<u8>),
    Background([u8; 3]),
    /// Connect to Studio, keeping this identity first when one is given and
    /// none is kept.
    Associate(Option<Identity>),
}

/// Where the device stands with Semio Studio, as a front end shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum StudioStatus {
    /// The run has no Studio: the build leaves it out, or the app configures
    /// none.
    #[default]
    Off,
    /// Not connected, as the run's [`Connect`] says: the run did not ask for
    /// Studio, or (Android) the device keeps no identity. Carries what the
    /// device is associated as, if it is: the identity it keeps, else on
    /// desktop the registration arora saved for it
    /// ([`StudioConfig::saved_registration`]); [`RuntimeHandle::associate`]
    /// connects it under that. A device associated as nothing associates
    /// under the identity a front end's form gives.
    Idle(Option<arora_bridge::DeviceInfo>),
    /// Connecting; the operator may be asked for the registration.
    Connecting,
    /// Registered with Studio as this.
    Connected(arora_bridge::DeviceInfo),
    /// The operator declined Studio, leaving the owner empty.
    Declined,
    /// The connection failed, for this reason.
    Failed(String),
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
    /// terminal UI keeps its log pane and its commands.
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
    /// `--studio`: attach the Semio Studio bridge.
    #[cfg(feature = "studio")]
    pub studio: Option<StudioConfig>,
}

/// Where a device's Studio connection keeps what it registers as and what it
/// signs in with.
#[cfg(feature = "studio")]
#[derive(Clone)]
pub struct StudioConfig {
    /// The identity the device is associated as (name, owners, model
    /// family), kept across launches: the association form's, then Studio's
    /// record. Without it, the device registers from the operator's answers,
    /// Studio's record persisted here afterwards, else from the environment
    /// alone.
    pub identity: std::path::PathBuf,
    /// The device directory Studio's credentials live in (its `studio/`
    /// subdirectory), for a platform that gives the app its data directory,
    /// as Android does; `None` for the one arora finds from the environment.
    /// Inside a given directory nobody is asked: without a kept identity the
    /// device does not connect, since arora 15 has no operator flow there.
    pub device_dir: Option<std::path::PathBuf>,
    /// The registration arora saves when its operator answers
    /// (`studio/device.json` in arora's device directory, the fields of the
    /// kept identity), for a device directory arora finds from the
    /// environment: a device that keeps no identity but has it is associated
    /// already, and connects under it, nobody asked. `None` where arora's
    /// operator never answers, as in a given device directory.
    pub saved_registration: Option<std::path::PathBuf>,
    /// When the device connects; [`RuntimeHandle::associate`] connects it for
    /// the rest of the run whatever this says.
    pub connect: Connect,
}

/// When a device configured for Studio connects to it.
#[cfg(feature = "studio")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connect {
    /// At launch: the run asks for Studio (`--studio`).
    AtLaunch,
    /// At launch when the device keeps an identity: a device associated once
    /// comes back associated, and one never associated contacts nobody.
    IfKept,
    /// Only once a front end asks ([`RuntimeHandle::associate`]).
    WhenAsked,
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

/// What a device is associated with Studio as — its name, owners and model
/// family — kept across launches (`StudioConfig::identity`): what the
/// settings pane's form gave on its first association, or what Studio held
/// once the operator's answers registered it, then what Studio holds each
/// time it connects. The device registers it only while Studio holds no
/// record of it: once associated, the name and owners are Studio's.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(not(feature = "studio"), allow(dead_code))]
struct Identity {
    name: String,
    /// The owners' Semio Studio user ids (Firebase UIDs).
    owners: Vec<String>,
    #[serde(default)]
    model_family: Option<String>,
}

#[cfg_attr(not(feature = "studio"), allow(dead_code))]
impl Identity {
    /// What the device registers as under this identity.
    fn device_info(&self) -> arora_bridge::DeviceInfo {
        arora_bridge::DeviceInfo {
            name: Some(self.name.clone()),
            model_family: self.model_family.clone(),
            owners: self.owners.clone(),
            ..arora_bridge::DeviceInfo::default()
        }
    }

    /// The identity a record gives: a name and an owner at least, as arora
    /// requires of its saved registration.
    fn of(info: &arora_bridge::DeviceInfo) -> Option<Self> {
        let name = info.name.clone().filter(|name| !name.trim().is_empty())?;
        (!info.owners.is_empty()).then(|| Self {
            name,
            owners: info.owners.clone(),
            model_family: info.model_family.clone(),
        })
    }
}

/// The identity kept at `path`, if one reads.
#[cfg(feature = "studio")]
fn kept_identity(path: &std::path::Path) -> Option<Identity> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
}

/// Keep `identity` at `path`, for this launch and the next ones.
#[cfg(feature = "studio")]
fn keep_identity(path: &std::path::Path, identity: &Identity) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(identity)
        .map_err(|e| e.to_string())
        .and_then(|json| std::fs::write(path, json).map_err(|e| e.to_string()))
    {
        Ok(()) => log::info!("Studio identity kept in {}", path.display()),
        Err(e) => log::warn!("cannot keep the Studio identity: {e}"),
    }
}

/// The registration arora saved at `path`, when it gives a name and an
/// owner: arora ignores one that does not.
#[cfg(feature = "studio")]
fn saved_registration(path: &std::path::Path) -> Option<Identity> {
    kept_identity(path).filter(|saved| !saved.name.trim().is_empty() && !saved.owners.is_empty())
}

/// The Studio bridge for one generation, the device connected under the
/// persisted identity when there is one, else as the operator answers (the
/// terminal UI's prompt or the settings pane; an unattended run declines),
/// and Studio's record then persisted; in a given device directory, only
/// under a persisted identity ([`StudioConfig::device_dir`]). `None` when
/// the run does not connect ([`StudioConfig::connect`]) or Studio is
/// declined or fails; `status` says which.
#[cfg(feature = "studio")]
async fn studio_bridge(
    studio: &StudioConfig,
    operator: Option<&dyn arora::operator::Operator>,
    status: &Mutex<StudioStatus>,
) -> Option<Box<dyn arora_bridge::Bridge>> {
    let report = |now: StudioStatus| {
        if let Ok(mut status) = status.lock() {
            *status = now;
        }
    };
    let identity = studio.identity.as_path();
    let persisted = kept_identity(identity);
    let connects = match studio.connect {
        Connect::AtLaunch => true,
        Connect::IfKept => persisted.is_some(),
        Connect::WhenAsked => false,
    };
    if !connects {
        let associated = persisted.or_else(|| {
            studio
                .saved_registration
                .as_deref()
                .and_then(saved_registration)
        });
        report(StudioStatus::Idle(
            associated.as_ref().map(Identity::device_info),
        ));
        return None;
    }
    report(StudioStatus::Connecting);
    if let Some(known) = persisted {
        let connected = match &studio.device_dir {
            Some(dir) => arora::studio::connect_with_device_dir(dir).await,
            None => arora::studio::connect().await,
        };
        let bridge = match connected {
            Ok(bridge) => bridge,
            Err(e) => {
                log::error!("studio bridge: {e:?}");
                report(StudioStatus::Failed(format!("{e:#}")));
                return None;
            }
        };
        // Studio's record is Studio's: the identity is registered only while
        // Studio holds none, on the device's first association; afterwards
        // the device keeps what Studio holds.
        let held = match bridge.get_device_info().await {
            Ok(Some(held)) if Identity::of(&held).is_some() => Ok(held),
            Ok(_) => {
                let info = known.device_info();
                log::info!(
                    "registering with Studio as {:?} (owners {:?}, from {})",
                    known.name,
                    known.owners,
                    identity.display()
                );
                bridge
                    .update_device_info(Some(info.clone()))
                    .await
                    .map(|held| held.unwrap_or(info))
            }
            Err(e) => Err(e),
        };
        match held {
            Ok(held) => {
                log::info!(
                    "Studio holds the device as {:?} (owners {:?})",
                    held.name,
                    held.owners
                );
                if let Some(record) = Identity::of(&held).filter(|record| *record != known) {
                    keep_identity(identity, &record);
                }
                report(StudioStatus::Connected(held));
            }
            Err(e) => {
                log::error!("studio registration: {e:?}");
                report(StudioStatus::Failed(format!("registration: {e}")));
            }
        }
        return Some(bridge);
    }
    if studio.device_dir.is_some() {
        log::info!(
            "Studio: no identity kept in {}, not connecting",
            identity.display()
        );
        report(StudioStatus::Idle(None));
        return None;
    }
    let connected = match operator {
        Some(operator) => arora::studio::connect_with_operator(operator).await,
        None => arora::studio::connect().await.map(Some),
    };
    match connected {
        Ok(Some(bridge)) => {
            let info = bridge.get_device_info().await.ok().flatten();
            if let Some(known) = info.as_ref().and_then(Identity::of) {
                keep_identity(identity, &known);
            }
            report(StudioStatus::Connected(info.unwrap_or_default()));
            Some(bridge)
        }
        Ok(None) => {
            log::info!("Studio declined (no owner)");
            report(StudioStatus::Declined);
            None
        }
        Err(e) => {
            log::error!("studio bridge: {e:?}");
            report(StudioStatus::Failed(format!("{e:#}")));
            None
        }
    }
}

/// Attach the device's bridges to `builder`: always the open local bridge
/// (the one local editors and apps connect to), plus any the build/CLI adds.
/// Each relays the device as its store and modules describe it — keys, what
/// they are, which a remote may write, and functions — so nothing is declared
/// to a bridge. A bridge that fails to build is logged and skipped, not fatal.
/// Returned beside the builder: the access requests the Studio bridge yields,
/// for the operator to rule on; `studio` says where the device stands with
/// Studio.
#[cfg_attr(
    not(all(any(feature = "ros2-dds", feature = "ros2-zenoh"), feature = "studio")),
    allow(unused_variables)
)]
pub async fn attach_bridges(
    mut builder: arora::AroraBuilder,
    bridges: &BridgeConfig,
    ros4hri: bool,
    operator: Option<&dyn arora::operator::Operator>,
    studio: &Mutex<StudioStatus>,
) -> (arora::AroraBuilder, arora_bridge::AccessRequestStream) {
    #[cfg_attr(not(feature = "studio"), allow(unused_mut))]
    let mut access_requests: arora_bridge::AccessRequestStream =
        Box::pin(futures::stream::pending());
    // Studio first: the bridge whose identity the device carries.
    #[cfg(feature = "studio")]
    if let Some(config) = &bridges.studio {
        if let Some(bridge) = studio_bridge(config, operator, studio).await {
            access_requests = bridge.access_requests().await;
            builder = builder.with_bridge(bridge);
        }
    }
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
    (builder, access_requests)
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
    // The rig holds the face it shows: the model the device states and serves
    // (Semio Studio's `retrieveModels` / `retrieveModelGlb`).
    let rig = RigHal::new();
    rig.set_model_glb(glb.to_vec());
    let store = BlackboardStore::new();
    let (events_tx, events_rx) = std::sync::mpsc::channel();
    let (commands_tx, commands_rx) = futures::channel::mpsc::unbounded();
    let studio = Arc::new(Mutex::new(StudioStatus::Off));
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

/// Why a generation ends: another face to run, or the Studio connection asked
/// for.
enum Rebuild {
    Face(Box<Generation>),
    Associate(Option<Identity>),
}

/// The operator flow, generation by generation: each reload stops the
/// running device and rebuilds it — graphs, rig, store — on the new face,
/// and the view follows through [`ViewEvent::LoadFace`]; an association
/// rebuilds it on the same face with the Studio bridge. `operator`, when
/// given, answers in place of the front end's. Runs until a generation ends
/// on its own (a device error; the terminal UI's quit exits the process).
#[allow(clippy::too_many_arguments)]
fn supervise(
    mut face: Generation,
    config: FaceConfig,
    mut bridges: BridgeConfig,
    operator: Option<Arc<dyn arora::operator::Operator>>,
    handles: (RigHal, BlackboardStore),
    events: Sender<ViewEvent>,
    mut commands: UnboundedReceiver<Command>,
    studio: Arc<Mutex<StudioStatus>>,
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
            frontend.interactive = true;
        }
        let (rig, store) = match fresh.take() {
            Some(handles) => handles,
            None => {
                let (rig, store) = (RigHal::new(), BlackboardStore::new());
                rig.set_model_glb(face.glb.clone());
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
        let associable = studio_configured(&bridges);
        let rebuild = tokio_rt.block_on(async {
            let operator = frontend.as_ref().map(|frontend| frontend.operator.clone());
            let (builder, access_requests) = attach_bridges(
                builder,
                &bridges,
                config.ros4hri,
                operator.as_deref(),
                &studio,
            )
            .await;
            // A rebuild drops the run future — arora's stop story: the
            // teardown is complete and synchronous (front end released, local
            // bridge's port freed) before the next generation starts.
            tokio::select! {
                result = run_generation(builder, frontend, access_requests, &face.meta, &config.program) => {
                    if let Err(e) = result {
                        log::error!("arora device stopped: {e:?}");
                    }
                    None
                }
                rebuild = pump(tui, &mut commands, &config, &events, associable) => Some(rebuild),
            }
        });
        match rebuild {
            None => break,
            Some(Rebuild::Face(next)) => face = *next,
            Some(Rebuild::Associate(identity)) => associate(&mut bridges, identity),
        }
    }
}

/// Whether the run is configured for Studio, so that associating it means
/// something.
fn studio_configured(bridges: &BridgeConfig) -> bool {
    #[cfg(feature = "studio")]
    return bridges.studio.is_some();
    #[cfg(not(feature = "studio"))]
    {
        let _ = bridges;
        false
    }
}

/// Connect the run to Studio from its next generation on, keeping `identity`
/// first when one is given and the device keeps none: a device associated
/// already connects under what it keeps, its name and owners being Studio's.
fn associate(bridges: &mut BridgeConfig, identity: Option<Identity>) {
    #[cfg(feature = "studio")]
    if let Some(studio) = bridges.studio.as_mut() {
        if let Some(identity) = &identity {
            if kept_identity(&studio.identity).is_none() {
                keep_identity(&studio.identity, identity);
            } else {
                log::warn!(
                    "the device keeps an identity in {}: its name and owners are edited in \
                     Semio Studio",
                    studio.identity.display()
                );
            }
        }
        studio.connect = Connect::AtLaunch;
    }
    #[cfg(not(feature = "studio"))]
    let _ = (bridges, identity);
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
/// a dead device. Returns when the device is to be rebuilt: on a face
/// validated for a reload, or on an association in a run `associable` to
/// Studio. A generation whose front ends have all gone quiet (a headless run,
/// every handle dropped) never returns, so that the pump merely ending
/// cannot stop a healthy run.
async fn pump(
    tui: BoxStream<'static, Command>,
    commands: &mut UnboundedReceiver<Command>,
    config: &FaceConfig,
    events: &Sender<ViewEvent>,
    associable: bool,
) -> Rebuild {
    let mut front_ends = futures::stream::select(tui, commands.by_ref());
    while let Some(command) = front_ends.next().await {
        match command {
            Command::Reload(glb) => match load_face(&glb, config) {
                Ok(loaded) => {
                    return Rebuild::Face(Box::new(Generation {
                        spec: loaded.spec,
                        meta: loaded.meta,
                        glb,
                        contended: loaded.contended,
                    }))
                }
                Err(e) => log::error!("cannot load the face: {e:#}"),
            },
            Command::Background(rgb) => {
                let _ = events.send(ViewEvent::Background(rgb));
            }
            Command::Associate(identity) if associable => return Rebuild::Associate(identity),
            Command::Associate(_) => log::warn!("this run has no Studio to associate with"),
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

/// When a device configured for Studio contacts it: never before it is
/// asked to or keeps an identity, and what it says of itself then.
#[cfg(all(test, feature = "studio"))]
mod studio_tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vizij-studio-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    async fn status_of(config: &StudioConfig) -> StudioStatus {
        let status = Mutex::new(StudioStatus::Off);
        assert!(
            studio_bridge(config, None, &status).await.is_none(),
            "nothing is contacted"
        );
        status.into_inner().unwrap()
    }

    #[tokio::test]
    async fn a_device_never_associated_contacts_nobody_until_asked() {
        let dir = scratch("never");
        // Android's: a device directory, connecting when an identity is kept.
        let in_dir = StudioConfig {
            identity: dir.join("studio-identity.json"),
            device_dir: Some(dir.clone()),
            saved_registration: None,
            connect: Connect::IfKept,
        };
        assert_eq!(
            status_of(&in_dir).await,
            StudioStatus::Idle(None),
            "the identity comes from the front end's form"
        );
        // The desktop's without `--studio`, arora having saved nothing.
        let desktop = StudioConfig {
            device_dir: None,
            saved_registration: Some(dir.join("studio").join("device.json")),
            connect: Connect::WhenAsked,
            ..in_dir.clone()
        };
        assert_eq!(status_of(&desktop).await, StudioStatus::Idle(None));
    }

    /// A desktop run that did not ask for Studio keeps its identity to
    /// itself, and says what it is associated as.
    #[tokio::test]
    async fn an_idle_device_says_the_identity_it_keeps() {
        let dir = scratch("idle-kept");
        let config = StudioConfig {
            identity: dir.join("studio-identity.json"),
            device_dir: None,
            saved_registration: None,
            connect: Connect::WhenAsked,
        };
        let identity = Identity {
            name: "vizij-bench".into(),
            owners: vec!["uid-1".into(), "uid-2".into()],
            model_family: Some("vizij".into()),
        };
        keep_identity(&config.identity, &identity);
        let StudioStatus::Idle(Some(info)) = status_of(&config).await else {
            panic!("an idle device with its identity");
        };
        assert_eq!(info.name.as_deref(), Some("vizij-bench"));
        assert_eq!(info.owners, ["uid-1", "uid-2"]);
        assert_eq!(info.model_family.as_deref(), Some("vizij"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A desktop whose operator registered it through arora, keeping no
    /// identity of its own, is associated as arora's saved registration; one
    /// that names no owner is no association.
    #[tokio::test]
    async fn a_desktop_arora_registered_is_associated_as_arora_saved_it() {
        let dir = scratch("arora-saved");
        let saved = dir.join("studio").join("device.json");
        let config = StudioConfig {
            identity: dir.join("studio-identity.json"),
            device_dir: None,
            saved_registration: Some(saved.clone()),
            connect: Connect::WhenAsked,
        };
        std::fs::create_dir_all(saved.parent().unwrap()).unwrap();
        std::fs::write(
            &saved,
            r#"{ "owners": ["uid-1"], "name": "pepper-lab", "model_family": "pepper" }"#,
        )
        .unwrap();
        let StudioStatus::Idle(Some(info)) = status_of(&config).await else {
            panic!("associated as arora's saved registration");
        };
        assert_eq!(info.name.as_deref(), Some("pepper-lab"));
        assert_eq!(info.owners, ["uid-1"]);

        std::fs::write(&saved, r#"{ "owners": [], "name": "pepper-lab" }"#).unwrap();
        assert_eq!(status_of(&config).await, StudioStatus::Idle(None));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn associating_keeps_the_first_identity_given_and_connects_from_then_on() {
        let dir = scratch("associate");
        let identity = dir.join("studio-identity.json");
        let mut bridges = BridgeConfig {
            studio: Some(StudioConfig {
                identity: identity.clone(),
                device_dir: Some(dir.clone()),
                saved_registration: None,
                connect: Connect::IfKept,
            }),
            ..BridgeConfig::default()
        };
        let first = || Identity {
            name: "vizij-test".into(),
            owners: vec!["uid-1".into()],
            model_family: None,
        };
        associate(&mut bridges, Some(first()));
        assert_eq!(kept_identity(&identity), Some(first()));
        assert_eq!(bridges.studio.as_ref().unwrap().connect, Connect::AtLaunch);

        // Associated, the device's name and owners are Studio's: another
        // identity given is not kept.
        associate(
            &mut bridges,
            Some(Identity {
                name: "vizij-renamed".into(),
                owners: vec!["uid-2".into()],
                model_family: None,
            }),
        );
        assert_eq!(kept_identity(&identity), Some(first()));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// What a device registers as under an identity: its name, owners and
    /// model family, and nothing it does not keep.
    #[test]
    fn an_identity_registers_its_fields() {
        let identity = Identity {
            name: "vizij-bench".into(),
            owners: vec!["uid-1".into()],
            model_family: None,
        };
        assert_eq!(
            identity.device_info(),
            arora_bridge::DeviceInfo {
                name: Some("vizij-bench".into()),
                owners: vec!["uid-1".into()],
                ..arora_bridge::DeviceInfo::default()
            }
        );
    }

    /// A record of Studio's gives an identity with a name and an owner: one
    /// without is a device Studio holds no record of.
    #[test]
    fn a_record_with_a_name_and_an_owner_gives_an_identity() {
        let record = arora_bridge::DeviceInfo {
            name: Some("vizij-bench".into()),
            owners: vec!["uid-1".into()],
            software_version: Some("1.0".into()),
            ..arora_bridge::DeviceInfo::default()
        };
        assert_eq!(
            Identity::of(&record),
            Some(Identity {
                name: "vizij-bench".into(),
                owners: vec!["uid-1".into()],
                model_family: None,
            })
        );
        let ownerless = arora_bridge::DeviceInfo {
            owners: Vec::new(),
            ..record.clone()
        };
        assert_eq!(Identity::of(&ownerless), None);
        let unnamed = arora_bridge::DeviceInfo {
            name: Some("  ".into()),
            ..record
        };
        assert_eq!(Identity::of(&unnamed), None);
    }
}
