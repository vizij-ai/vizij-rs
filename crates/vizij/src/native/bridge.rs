//! The device's open local bridge: the WebSocket server of `arora-bridge-ws`
//! over a [`ServerConfig`] of this app's, its registry advertising the face's
//! free inputs, its control panel served on the same port — what a local
//! editor, a phone on the LAN or a script connects to.
//!
//! The bridge carries the device itself: writes and reads reach the store, the
//! face's skills are listed and called by name on the signatures the device
//! describes (`say`, `look_at`, `play_viseme`), a skill that runs answers with
//! its run and stops on `halt`, and a client is pushed the keys it subscribes
//! to. None of that is declared here.
//!
//! What this module holds is what only the app knows: the registry's key list —
//! the face's free inputs with their types, ranges and authored defaults, which
//! is also what a client may write, since the server checks writes against it,
//! until a key declares itself in the store
//! ([ARORA-113](https://linear.app/semio-ai/issue/ARORA-113)) — and `reset`, a
//! method the server owns because the neutral pose is the bundle's, not any
//! module's.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use arora::bridge_ws::bridge::WsBridge;
use arora::bridge_ws::{
    AroraWSServer, CancellationToken, InvokeResult, KeyInfo, MethodInfo, ServerConfig,
};
use arora_bridge::{AccessRequestStream, Bridge, BridgeResult, DeviceInfo, InboundStream};
use arora_types::data::{DataStore, Key, StateChange};
use arora_types::value::{Type, Value};
use vizij_api_core::value::float;
use vizij_arora_store::BlackboardStore;

use crate::view::meta::FaceMeta;

/// Where the local bridge listens, with the control panel on: an app whose
/// window is the face serves the page that drives it.
pub fn default_config() -> ServerConfig {
    ServerConfig::default().serve_control_panel(true)
}

/// The open local bridge with its server's lifetime attached: dropping the
/// bridge — the device generation that owned it ending its run — cancels the
/// serving task, so the port is free for the next generation.
pub struct LocalBridge {
    inner: WsBridge,
    cancel: CancellationToken,
}

impl LocalBridge {
    /// Bind and serve `config`, the registry advertising `inputs` (the face's
    /// free inputs, each as a writable key of its type) and holding `reset`,
    /// which writes `defaults` through `store`.
    pub async fn serve(
        config: ServerConfig,
        inputs: &[(String, Type)],
        defaults: &HashMap<String, Value>,
        store: BlackboardStore,
    ) -> Result<Self> {
        let bind = config.bind_address.clone();
        let port = config.port;
        let control_panel = config.serve_control_panel;
        let server = Arc::new(AroraWSServer::new(config));
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
        register_reset(&server, store, defaults.clone()).await;
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
            "serving the local bridge on ws://{bind}:{port}{}",
            if control_panel {
                " (control panel on GET /)"
            } else {
                ""
            }
        );
        Ok(Self { inner, cancel })
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

    fn try_send(&mut self, change: &StateChange) {
        self.inner.try_send(change);
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

/// `reset`: the neutral pose, written through a sibling store handle, so the
/// change fans out to every bridge as any store write does.
async fn register_reset(
    server: &AroraWSServer,
    store: BlackboardStore,
    defaults: HashMap<String, Value>,
) {
    server
        .registry()
        .register_method_fn(
            MethodInfo {
                path: "reset".to_string(),
                description: Some("Return every input to the face's neutral pose".to_string()),
                ..Default::default()
            },
            move |_args| {
                let mut change = StateChange::new();
                for (path, value) in &defaults {
                    change
                        .set
                        .insert(Key::from(path.as_str()), Some(value.clone()));
                }
                match store.write(change) {
                    Ok(()) => InvokeResult::ok(),
                    Err(e) => InvokeResult::err(format!("{e:?}")),
                }
            },
        )
        .await;
}
