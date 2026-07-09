//! [`RigHal`]: a Vizij rig presented as an Arora
//! [`Hal`](arora_hal::Hal) (+ [`HalAssets`](arora_hal::HalAssets)).
//!
//! In Vizij-on-Arora terms the "device" is a GLB rig: the runtime's behavior
//! writes actuator targets (bone transforms, morph weights), the HAL applies
//! them, and a renderer reads what to draw. This HAL is that boundary on the
//! Rust side — it holds the GLB, accumulates the latest actuation `State`,
//! and feeds [`updates`](arora_hal::Hal::updates) so a renderer knows what
//! changed. Vizij and Arora share one runtime value type
//! ([`vizij_api_core::Value`] is `arora_types::value::Value`), so
//! [`RigHal::pose`] exposes that state to a Vizij renderer directly — the
//! only translation is string [`Key`]s to [`TypedPath`]s; the actual
//! bone/morph application stays in the renderer.
//!
//! Modelled on `arora-hal`'s `FakeHal`: cheaply cloneable, clones share state,
//! writes apply synchronously (so [`try_send`](arora_hal::Hal::try_send) needs
//! no internal task), and the update feed is an owned stream per subscriber.

use std::sync::{Arc, Mutex};

use arora_hal::{Hal, HalAssets, HalDescription, HalResult, UpdatesStream};
use arora_types::data::{Key, State, StateChange};
use async_trait::async_trait;
use futures_channel::mpsc::UnboundedSender;
use vizij_api_core::{TypedPath, Value};

#[derive(Default)]
struct Inner {
    description: HalDescription,
    model_glb: Option<Vec<u8>>,
    /// Latest actuation targets the rig has been driven to.
    state: State,
    subscribers: Vec<UnboundedSender<StateChange>>,
}

impl Inner {
    fn notify(&mut self, change: &StateChange) {
        if change.is_empty() {
            return;
        }
        self.subscribers
            .retain(|tx| tx.unbounded_send(change.clone()).is_ok());
    }
}

/// A Vizij rig as an Arora HAL. Clone to share the same rig.
#[derive(Clone, Default)]
pub struct RigHal {
    inner: Arc<Mutex<Inner>>,
}

impl RigHal {
    /// An empty rig HAL with no model and no description.
    pub fn new() -> Self {
        Self::default()
    }

    /// A rig HAL with device metadata (model family / versions).
    pub fn with_description(description: HalDescription) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                description,
                ..Default::default()
            })),
        }
    }

    /// Attach (or replace) the GLB model served by [`HalAssets::model_glb`].
    pub fn set_model_glb(&self, glb: Vec<u8>) {
        self.inner.lock().unwrap().model_glb = Some(glb);
    }

    /// The current rig pose for a Vizij renderer: the accumulated actuation
    /// state keyed by [`TypedPath`]. Entries whose key is not a valid typed
    /// path are skipped.
    pub fn pose(&self) -> Vec<(TypedPath, Value)> {
        let inner = self.inner.lock().unwrap();
        inner
            .state
            .iter()
            .filter_map(|(key, value)| {
                let value = value.as_ref()?;
                let tp = TypedPath::parse(&key.path).ok()?;
                Some((tp, value.clone()))
            })
            .collect()
    }

    /// Apply a write synchronously: store it and echo it to subscribers.
    /// Shared by [`Hal::write`] and [`Hal::try_send`] — the rig is in-memory,
    /// so applying never blocks.
    fn apply_write(&self, changes: &StateChange) {
        if changes.is_empty() {
            return;
        }
        let mut inner = self.inner.lock().unwrap();
        inner.state.apply(changes.clone());
        inner.notify(changes);
    }
}

#[async_trait]
impl Hal for RigHal {
    async fn describe(&self) -> HalDescription {
        self.inner.lock().unwrap().description.clone()
    }

    async fn read(&self, keys: &[Key]) -> HalResult<Vec<Option<Value>>> {
        let inner = self.inner.lock().unwrap();
        Ok(keys
            .iter()
            .map(|k| inner.state.get(k).cloned().flatten())
            .collect())
    }

    async fn read_all(&self) -> HalResult<State> {
        Ok(self.inner.lock().unwrap().state.clone())
    }

    async fn write(&self, changes: StateChange) -> HalResult<()> {
        self.apply_write(&changes);
        Ok(())
    }

    fn try_send(&self, changes: &StateChange) {
        self.apply_write(changes);
    }

    fn updates(&self) -> UpdatesStream {
        let (tx, rx) = futures_channel::mpsc::unbounded();
        self.inner.lock().unwrap().subscribers.push(tx);
        Box::pin(rx)
    }
}

#[async_trait]
impl HalAssets for RigHal {
    async fn model_glb(&self) -> HalResult<Option<Vec<u8>>> {
        Ok(self.inner.lock().unwrap().model_glb.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{FutureExt, StreamExt};
    use vizij_api_core::value::{as_float, as_vec3, bool_, float, vec3};

    /// Drain everything the feed has already buffered, without blocking.
    fn drain(feed: &mut UpdatesStream) -> Vec<StateChange> {
        let mut out = Vec::new();
        while let Some(Some(change)) = feed.next().now_or_never() {
            out.push(change);
        }
        out
    }

    #[tokio::test]
    async fn write_target_then_read_and_pose() {
        let hal = RigHal::new();
        // Drive a scalar morph and a vec3 bone translation.
        hal.write(StateChange::set("mouth/jaw.open", float(0.8)))
            .await
            .unwrap();
        hal.write(StateChange::set(
            "head/root.translation",
            vec3([0.0, 1.0, 0.0]),
        ))
        .await
        .unwrap();

        // Read back through the HAL value view.
        assert_eq!(
            hal.read(&[Key::from("mouth/jaw.open")]).await.unwrap(),
            vec![Some(float(0.8))]
        );

        // The Vizij renderer reads the same values keyed by TypedPath.
        let pose = hal.pose();
        assert!(pose
            .iter()
            .any(|(_, v)| as_vec3(v) == Some([0.0, 1.0, 0.0])));
        assert!(pose.iter().any(|(_, v)| as_float(v) == Some(0.8)));
    }

    #[tokio::test]
    async fn try_send_applies_like_write() {
        let hal = RigHal::new();
        hal.try_send(&StateChange::set("mouth/jaw.open", float(0.4)));
        assert_eq!(
            hal.read(&[Key::from("mouth/jaw.open")]).await.unwrap(),
            vec![Some(float(0.4))]
        );
    }

    #[tokio::test]
    async fn updates_feed_sees_writes() {
        let hal = RigHal::new();
        let mut sub = hal.updates();
        hal.write(StateChange::set("k", bool_(true))).await.unwrap();
        assert!(drain(&mut sub).iter().any(|c| c.contains(&Key::from("k"))));
    }

    #[tokio::test]
    async fn describe_and_model_glb() {
        let hal = RigHal::with_description(HalDescription {
            model_family: Some("vizij".into()),
            ..Default::default()
        });
        hal.set_model_glb(b"glTF".to_vec());
        assert_eq!(hal.describe().await.model_family.as_deref(), Some("vizij"));
        assert_eq!(hal.model_glb().await.unwrap(), Some(b"glTF".to_vec()));
    }

    #[tokio::test]
    async fn clones_share_the_rig() {
        let hal = RigHal::new();
        let other = hal.clone();
        hal.write(StateChange::set("shared", float(1.0)))
            .await
            .unwrap();
        assert_eq!(
            other.read(&[Key::from("shared")]).await.unwrap(),
            vec![Some(float(1.0))]
        );
    }
}
