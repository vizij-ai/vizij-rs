//! [`RigHal`]: a Vizij rig presented as an Arora
//! [`Hal`](arora_hal::Hal) (+ [`HalAssets`](arora_hal::HalAssets)).
//!
//! In Vizij-on-Arora terms the "device" is a GLB rig: the runtime's behavior
//! writes actuator targets (bone transforms, morph weights), the HAL applies
//! them, and a renderer reads what to draw. This HAL is that boundary on the
//! Rust side — it holds the GLB, accumulates the latest actuation `State`
//! (Arora values), and feeds [`updates`](arora_hal::Hal::updates) so a renderer
//! knows what changed. [`RigHal::pose`] exposes that state as native Vizij
//! values (via [`vizij_arora`]) for a Vizij renderer; the actual bone/morph
//! application stays in the renderer.
//!
//! Modelled on `arora-hal`'s `FakeHal`: cheaply cloneable, clones share state,
//! std-channel update feed.

use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

use arora_hal::{Hal, HalAssets, HalDescription, HalResult};
use arora_types::data::{Key, State, StateChange, Subscription};
use arora_types::value::Value as AValue;
use async_trait::async_trait;
use vizij_api_core::{TypedPath, Value as VValue};

#[derive(Default)]
struct Inner {
    description: HalDescription,
    model_glb: Option<Vec<u8>>,
    /// Latest actuation targets the rig has been driven to (Arora values).
    state: State,
    subscribers: Vec<Sender<StateChange>>,
}

impl Inner {
    fn notify(&mut self, change: &StateChange) {
        if change.is_empty() {
            return;
        }
        self.subscribers
            .retain(|tx| tx.send(change.clone()).is_ok());
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

    /// The current rig pose as native Vizij values, for a Vizij renderer.
    /// Entries whose path or value cannot be converted are skipped.
    pub fn pose(&self) -> Vec<(TypedPath, VValue)> {
        let inner = self.inner.lock().unwrap();
        inner
            .state
            .iter()
            .filter_map(|(key, value)| {
                let arora = value.as_ref()?;
                let tp = TypedPath::parse(&key.path).ok()?;
                let vizij = vizij_arora::from_arora(arora).ok()?;
                Some((tp, vizij))
            })
            .collect()
    }
}

#[async_trait]
impl Hal for RigHal {
    async fn describe(&self) -> HalDescription {
        self.inner.lock().unwrap().description.clone()
    }

    async fn read(&self, keys: &[Key]) -> HalResult<Vec<Option<AValue>>> {
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
        if changes.is_empty() {
            return Ok(());
        }
        let mut inner = self.inner.lock().unwrap();
        inner.state.apply(changes.clone());
        inner.notify(&changes);
        Ok(())
    }

    fn updates(&self) -> Subscription {
        let (tx, rx) = channel();
        self.inner.lock().unwrap().subscribers.push(tx);
        Subscription::new(rx)
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

    fn av(v: VValue) -> AValue {
        vizij_arora::to_arora(&v).unwrap()
    }

    #[tokio::test]
    async fn write_target_then_read_and_pose() {
        let hal = RigHal::new();
        // Drive a scalar morph and a Vec3 bone translation.
        hal.write(StateChange::set("mouth/jaw.open", av(VValue::Float(0.8))))
            .await
            .unwrap();
        hal.write(StateChange::set(
            "head/root.translation",
            av(VValue::Vec3([0.0, 1.0, 0.0])),
        ))
        .await
        .unwrap();

        // Read back through the Arora value view.
        assert_eq!(
            hal.read(&[Key::from("mouth/jaw.open")]).await.unwrap(),
            vec![Some(AValue::F32(0.8))]
        );

        // The Vizij renderer sees native Vizij values.
        let pose = hal.pose();
        assert!(pose
            .iter()
            .any(|(_, v)| matches!(v, VValue::Vec3(a) if *a == [0.0, 1.0, 0.0])));
        assert!(pose
            .iter()
            .any(|(_, v)| matches!(v, VValue::Float(f) if *f == 0.8)));
    }

    #[tokio::test]
    async fn updates_feed_sees_writes() {
        let hal = RigHal::new();
        let sub = hal.updates();
        hal.write(StateChange::set("k", av(VValue::Bool(true))))
            .await
            .unwrap();
        assert!(sub.try_iter().any(|c| c.contains(&Key::from("k"))));
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
        hal.write(StateChange::set("shared", av(VValue::Float(1.0))))
            .await
            .unwrap();
        assert_eq!(
            other.read(&[Key::from("shared")]).await.unwrap(),
            vec![Some(AValue::F32(1.0))]
        );
    }
}
