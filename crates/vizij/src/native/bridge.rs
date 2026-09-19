//! The device's open local bridge: the WebSocket server of `arora-bridge-ws`
//! over a [`ServerConfig`] of this app's, its registry populated with the
//! face's free inputs and the skills, its control panel served on the same
//! port — what a local editor, a phone on the LAN or a script connects to.
//!
//! The bridge forwards writes and reads into the run; `invoke` never leaves
//! the server, so every method here is a registry handler: `reset` writes the
//! neutral pose through a sibling store handle, and the skills — `say`,
//! `look_at`, `play_viseme` — dispatch a spawn into the run through the
//! device's [`LocalCaller`] (the handler is synchronous, so it answers
//! "accepted" and the run reports on its status key); `stop` halts the last
//! run of a skill. Built-in keys (`arora/*`, the clock at step rate) are not
//! pushed to clients.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use arora::{Caller, LocalCaller};
use arora_behavior::{interpreter_module, RunPolicy, TaskHandle};
use arora_bridge::{AccessRequestStream, Bridge, BridgeResult, DeviceInfo, InboundStream};
use arora_bridge_ws::bridge::WsBridge;
use arora_bridge_ws::{
    AroraWSServer, CancellationToken, InvokeResult, KeyInfo, MethodInfo, MethodParam, ServerConfig,
};
use arora_types::call::Call;
use arora_types::data::{DataStore, Key, StateChange};
use arora_types::value::{StructureField, Type, Value};
use uuid::Uuid;
use vizij_api_core::value::float;
use vizij_arora_behavior::{gaze, viseme};
use vizij_arora_store::BlackboardStore;

use crate::view::meta::FaceMeta;

/// Where the local bridge listens.
#[derive(Clone, Debug)]
pub struct LocalBridgeConfig {
    /// The address to bind (`127.0.0.1` keeps it to this machine).
    pub bind: String,
    pub port: u16,
    /// Serve the control panel page on `GET /` of the same port.
    pub control_panel: bool,
}

impl Default for LocalBridgeConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1".to_string(),
            port: 9000,
            control_panel: true,
        }
    }
}

/// The open local bridge with its server's lifetime attached: dropping the
/// bridge — the device generation that owned it ending its run — cancels the
/// serving task, so the port is free for the next generation.
pub struct LocalBridge {
    inner: WsBridge,
    server: Arc<AroraWSServer>,
    cancel: CancellationToken,
}

impl LocalBridge {
    /// Bind and serve `config`, the registry advertising `inputs` (the
    /// face's free inputs, each as a writable key of its type).
    pub async fn serve(
        config: &LocalBridgeConfig,
        inputs: &[(String, Type)],
        defaults: &HashMap<String, Value>,
    ) -> Result<Self> {
        let server = Arc::new(AroraWSServer::new(
            ServerConfig::with_port(config.port)
                .bind_address(config.bind.clone())
                .serve_control_panel(config.control_panel),
        ));
        server
            .registry()
            .set_keys(
                inputs
                    .iter()
                    .map(|(path, ty)| KeyInfo {
                        path: path.clone(),
                        kind: Some("input".to_string()),
                        value_type: Some(ty.clone()),
                        min: matches!(ty, Type::F64).then_some(0.0),
                        max: matches!(ty, Type::F64).then_some(1.0),
                        default_value: defaults.get(path).cloned(),
                        description: None,
                    })
                    .collect(),
            )
            .await;
        let inner = WsBridge::new(server.clone()).await;
        // Bind before spawning: an unusable address (the port already taken)
        // fails here instead of leaving a device serving a bridge nobody can
        // reach.
        let listener = server
            .bind()
            .await
            .map_err(|e| anyhow!("local bridge: {e}"))?;
        let cancel = CancellationToken::new();
        tokio::spawn({
            let server = server.clone();
            let cancel = cancel.clone();
            async move {
                if let Err(e) = server.run_on(listener, cancel).await {
                    log::error!("local bridge server stopped: {e:?}");
                }
            }
        });
        log::info!(
            "serving the local bridge on ws://{}:{}{}",
            config.bind,
            config.port,
            if config.control_panel {
                " (control panel on GET /)"
            } else {
                ""
            }
        );
        Ok(Self {
            inner,
            server,
            cancel,
        })
    }

    /// The server, for the methods registered once the device exists.
    pub fn server(&self) -> Arc<AroraWSServer> {
        self.server.clone()
    }
}

impl Drop for LocalBridge {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[async_trait::async_trait]
impl Bridge for LocalBridge {
    fn take_inbound(&mut self) -> InboundStream {
        self.inner.take_inbound()
    }

    /// Everything but the built-ins: the clock (`arora/time`, `arora/dt`)
    /// changes every step and says nothing a client wants.
    fn try_send(&mut self, change: &StateChange) {
        let built_in = |key: &Key| key.path.starts_with(arora_behavior::built_in::PREFIX);
        if change.set.keys().all(built_in) {
            return;
        }
        if change.set.keys().any(built_in) {
            let mut filtered = StateChange::new();
            for (key, value) in &change.set {
                if !built_in(key) {
                    filtered.set.insert(key.clone(), value.clone());
                }
            }
            self.inner.try_send(&filtered);
        } else {
            self.inner.try_send(change);
        }
    }

    async fn get_device_info(&self) -> BridgeResult<Option<DeviceInfo>> {
        self.inner.get_device_info().await
    }

    async fn update_device_info(
        &self,
        info: Option<DeviceInfo>,
    ) -> BridgeResult<Option<DeviceInfo>> {
        self.inner.update_device_info(info).await
    }

    async fn device_id(&self) -> Option<String> {
        self.inner.device_id().await
    }

    async fn access_requests(&self) -> AccessRequestStream {
        self.inner.access_requests().await
    }
}

/// The face at rest as store values, keyed by input path — what `reset`
/// writes and what the registry advertises as each input's default: every
/// free input's authored default, the rig's under the bundle's neutral pose
/// where it names one. Both planes are covered on purpose: the standard
/// inputs feed the rig through the adaptation, so a rig-only reset is undone
/// on the next tick by whatever the standard inputs still hold.
pub fn neutral_defaults(meta: &FaceMeta, spec: &str) -> HashMap<String, Value> {
    let mut defaults = crate::face::input_rest_values(spec);
    for (path, value) in meta.bundle.neutral_stage_writes() {
        defaults.insert(path, float(value));
    }
    defaults
}

/// A skill a client may invoke by name: its module and function, its
/// parameters by name with the wire type of each.
struct Skill {
    name: &'static str,
    description: &'static str,
    module_id: Uuid,
    function_id: Uuid,
    parameters: Vec<(String, Uuid, Type)>,
}

fn skills(speech_module: Option<Uuid>) -> Vec<Skill> {
    let named = |ids: HashMap<Uuid, String>, types: &[(&str, Type)]| -> Vec<(String, Uuid, Type)> {
        types
            .iter()
            .filter_map(|(name, ty)| {
                ids.iter()
                    .find(|(_, n)| n == name)
                    .map(|(id, _)| (name.to_string(), *id, ty.clone()))
            })
            .collect()
    };
    let mut skills = vec![
        Skill {
            name: "look_at",
            description: "Look at a target (meters, the face's frame) under a gaze policy: \
                          `track` until stopped, `glance`/`reset` then done.",
            module_id: gaze::module_id(),
            function_id: gaze::look_at_id(),
            parameters: named(
                gaze::look_at_parameters(),
                &[
                    ("policy", Type::String),
                    ("target", Type::ArrayF32),
                    ("frame", Type::String),
                ],
            ),
        },
        Skill {
            name: "play_viseme",
            description: "Play one viseme shape of the face standard at a weight through the \
                          lipsync envelope.",
            module_id: viseme::MODULE_ID,
            function_id: viseme::PLAY_VISEME_ID,
            parameters: named(
                viseme::play_viseme_parameters(),
                &[("shape", Type::String), ("weight", Type::F32)],
            ),
        },
    ];
    if let Some(module_id) = speech_module {
        skills.push(Skill {
            name: "say",
            description: "Speak a text; the lips follow the speech.",
            module_id,
            function_id: vizij_arora_behavior::speech::SAY_ID,
            parameters: vec![
                (
                    "text".to_string(),
                    vizij_arora_behavior::speech::SAY_TEXT_PARAM_ID,
                    Type::String,
                ),
                (
                    "voice".to_string(),
                    vizij_arora_behavior::speech::SAY_VOICE_PARAM_ID,
                    Type::String,
                ),
            ],
        });
    }
    skills
}

/// Register the methods on `server`'s registry for one device generation:
/// `reset`, the skills as spawns through `caller`, and `stop`. Called once
/// the device is built — the caller exists from then on — and before it runs.
pub async fn register_methods(
    server: &AroraWSServer,
    caller: LocalCaller,
    store: BlackboardStore,
    defaults: HashMap<String, Value>,
    speech_module: Option<Uuid>,
) {
    let registry = server.registry();
    // `reset`: the neutral pose, through the sibling store handle; the write
    // fans out to every bridge as any store write does.
    let reset_store = store.clone();
    registry
        .register_method_fn(
            MethodInfo {
                path: "reset".to_string(),
                params: Vec::new(),
                return_type: None,
                description: Some("Return every input to the face's neutral pose".to_string()),
            },
            move |_args| {
                let mut change = StateChange::new();
                for (path, value) in &defaults {
                    change
                        .set
                        .insert(Key::from(path.as_str()), Some(value.clone()));
                }
                match reset_store.write(change) {
                    Ok(()) => InvokeResult::ok(),
                    Err(e) => InvokeResult::err(format!("{e:?}")),
                }
            },
        )
        .await;

    // The skills: each a spawn dispatched through the caller on a task (the
    // handler is synchronous; `CallFuture` borrows the caller). The last run
    // of each skill is kept so `stop` can halt it.
    let runs: Arc<Mutex<HashMap<&'static str, TaskHandle>>> = Arc::new(Mutex::new(HashMap::new()));
    let handle = tokio::runtime::Handle::current();
    for skill in skills(speech_module) {
        let info = MethodInfo {
            path: skill.name.to_string(),
            params: skill
                .parameters
                .iter()
                .map(|(name, _, ty)| MethodParam {
                    name: name.clone(),
                    param_type: ty.clone(),
                    required: false,
                    default_value: None,
                    description: None,
                })
                .collect(),
            return_type: None,
            description: Some(skill.description.to_string()),
        };
        let caller = caller.clone();
        let runs = runs.clone();
        let handle = handle.clone();
        registry
            .register_method_fn(info, move |args| {
                let fields: Vec<StructureField> = skill
                    .parameters
                    .iter()
                    .filter_map(|(name, id, _)| {
                        args.get(name).map(|value| StructureField {
                            id: *id,
                            value: Box::new(value.clone()),
                        })
                    })
                    .collect();
                let call = Call {
                    module_id: Some(skill.module_id),
                    id: skill.function_id,
                    args: fields,
                };
                let spawn = interpreter_module::encode_spawn(&call, RunPolicy::Concurrent);
                let caller = caller.clone();
                let runs = runs.clone();
                let name = skill.name;
                handle.spawn(async move {
                    match caller.call(spawn).await {
                        Ok(result) => match interpreter_module::decode_spawn_result(&result.ret) {
                            Ok(run) => {
                                if let Ok(mut runs) = runs.lock() {
                                    runs.insert(name, run);
                                }
                            }
                            Err(e) => log::error!("{name}: the spawn returned no run: {e}"),
                        },
                        Err(e) => log::error!("{name}: spawn failed: {e}"),
                    }
                });
                InvokeResult::ok_with_value(Value::String("accepted".to_string()))
            })
            .await;
    }

    // `stop`: halt the last run of a skill.
    let caller_for_stop = caller.clone();
    let handle_for_stop = handle.clone();
    registry
        .register_method_fn(
            MethodInfo {
                path: "stop".to_string(),
                params: vec![MethodParam {
                    name: "method".to_string(),
                    param_type: Type::String,
                    required: true,
                    default_value: None,
                    description: Some("The skill whose last run to halt".to_string()),
                }],
                return_type: None,
                description: Some("Halt the last run of a skill".to_string()),
            },
            move |args| {
                let Some(Value::String(method)) = args.get("method") else {
                    return InvokeResult::err("stop needs a method name");
                };
                let run = runs
                    .lock()
                    .ok()
                    .and_then(|mut runs| runs.remove_entry(method.as_str()).map(|(_, run)| run));
                let Some(run) = run else {
                    return InvokeResult::err(format!("no run of {method} to stop"));
                };
                let caller = caller_for_stop.clone();
                handle_for_stop.spawn(async move {
                    if let Err(e) = caller.call(run.stop).await {
                        log::error!("stop: the halt failed: {e}");
                    }
                });
                InvokeResult::ok()
            },
        )
        .await;
}
