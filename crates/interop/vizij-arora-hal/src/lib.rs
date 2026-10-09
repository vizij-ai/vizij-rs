//! [`RigHal`]: a Vizij rig presented as an Arora
//! [`Hal`](arora_hal::Hal) (+ [`HalAssets`](arora_hal::HalAssets)).
//!
//! In Vizij-on-Arora terms the "device" is a GLB rig: the runtime's behavior
//! writes actuator targets (bone transforms, morph weights), the HAL applies
//! them, and a renderer reads what to draw. This HAL is that boundary on the
//! Rust side — it holds the GLB, accumulates the latest actuation `State`,
//! and feeds [`pose_updates`](RigHal::pose_updates) so a renderer knows what
//! changed. Vizij and Arora share one runtime value type
//! ([`vizij_api_core::Value`] is `arora_types::value::Value`), so
//! [`RigHal::pose`] exposes that state to a Vizij renderer directly — the
//! only translation is string [`Key`]s to [`TypedPath`]s; the actual
//! bone/morph application stays in the renderer.
//!
//! The rig itself measures nothing — it applies targets instantly — so the
//! runtime's *reading* feed [`Hal::updates`] carries only what the host
//! explicitly [`push_reading`](RigHal::push_reading)s: a genuine sensor such as
//! the rendered frame (`display/face` raw, `display/face/compressed` encoded),
//! which a renderer publishes each frame.
//! Applied actuations are deliberately **not** echoed here — that would make
//! the runtime re-apply them as readings a frame later, overwriting anything
//! fresher written to the store in between — so their echo lives on
//! [`pose_updates`](RigHal::pose_updates), the renderer-side feed, instead.
//!
//! Modelled on `arora-hal`'s `FakeHal`: cheaply cloneable, clones share state,
//! writes apply synchronously (so [`try_send`](arora_hal::Hal::try_send) needs
//! no internal task), and the pose feed is an owned stream per subscriber.

use std::sync::{Arc, Mutex};

use arora_hal::{
    content_hash, ComponentModel, Hal, HalAssets, HalDescription, HalResult, UpdatesStream, DEVICE,
};
use arora_types::data::{Key, State, StateChange};
use async_trait::async_trait;
use futures_channel::mpsc::UnboundedSender;
use vizij_api_core::{TypedPath, Value};

#[derive(Default)]
struct Inner {
    description: HalDescription,
    /// The face's GLB and its content hash, hashed once when attached.
    model: Option<(Vec<u8>, String)>,
    /// Latest actuation targets the rig has been driven to.
    state: State,
    /// The actuation echo — [`RigHal::pose_updates`], renderer-side.
    subscribers: Vec<UnboundedSender<StateChange>>,
    /// The runtime's sensor-reading feed — [`Hal::updates`], fed by
    /// [`RigHal::push_reading`] (e.g. the rendered frame).
    reading_subscribers: Vec<UnboundedSender<StateChange>>,
}

/// Fan a change out to a subscriber list, dropping any closed receiver.
fn notify(subscribers: &mut Vec<UnboundedSender<StateChange>>, change: &StateChange) {
    if change.is_empty() {
        return;
    }
    subscribers.retain(|tx| tx.unbounded_send(change.clone()).is_ok());
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

    /// Attach the face's GLB: the rig's model, which the device states and
    /// serves ([`HalAssets::models`]). Attach it before the device is built:
    /// a device's models are fixed for its life.
    pub fn set_model_glb(&self, glb: Vec<u8>) {
        let hash = content_hash(&glb);
        self.inner.lock().unwrap().model = Some((glb, hash));
    }

    /// A feed of applied actuation changes, for a Vizij renderer that wants
    /// deltas instead of polling [`pose`](RigHal::pose). Each call yields an
    /// independent, owned stream. This is deliberately **not**
    /// [`Hal::updates`]: that seam is the runtime's sensor-reading feed (what
    /// [`push_reading`](RigHal::push_reading) fans out), a separate stream from
    /// this actuation echo.
    pub fn pose_updates(&self) -> UpdatesStream {
        let (tx, rx) = futures_channel::mpsc::unbounded();
        self.inner.lock().unwrap().subscribers.push(tx);
        Box::pin(rx)
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
        notify(&mut inner.subscribers, changes);
    }

    /// Push a sensor reading into the runtime's [`Hal::updates`] feed — e.g. a
    /// renderer publishing a rendered frame under `display/face/compressed`. The runtime
    /// lands it in the store and fans it to every bridge. This is the device
    /// *reporting back*, the counterpart to the actuation the runtime writes.
    pub fn push_reading(&self, reading: StateChange) {
        let mut inner = self.inner.lock().unwrap();
        notify(&mut inner.reading_subscribers, &reading);
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

    /// The runtime's sensor-reading feed: whatever the host
    /// [`push_reading`](RigHal::push_reading)s (e.g. the rendered frame). Each
    /// call yields an independent, owned stream; a finished stream would mean the
    /// hardware is gone, so this one stays open, yielding nothing until the first
    /// reading. Actuation is echoed on [`pose_updates`](RigHal::pose_updates),
    /// not here.
    fn updates(&self) -> UpdatesStream {
        let (tx, rx) = futures_channel::mpsc::unbounded();
        self.inner.lock().unwrap().reading_subscribers.push(tx);
        Box::pin(rx)
    }

    /// The rig's model: what [`set_model_glb`](RigHal::set_model_glb)
    /// attached, which the device serves through its HAL module.
    fn assets(&self) -> Option<&dyn HalAssets> {
        Some(self)
    }
}

/// The rig is not composed, so it is the one component [`DEVICE`]: a HAL that
/// composes it with a robot names it there. Its model is the face's GLB, which
/// is servable — a client needs it to reproduce the face in 3D — and has no
/// published release of its own.
impl HalAssets for RigHal {
    fn models(&self) -> Vec<ComponentModel> {
        let inner = self.inner.lock().unwrap();
        inner
            .model
            .as_ref()
            .map(|(_, hash)| ComponentModel {
                component: DEVICE.to_string(),
                description: Some(inner.description.clone()),
                reference: None,
                content_hash: Some(hash.clone()),
                servable: true,
                mount: None,
            })
            .into_iter()
            .collect()
    }

    fn servable_glb(&self, component: &str) -> HalResult<Option<Vec<u8>>> {
        Ok(match component {
            DEVICE => self
                .inner
                .lock()
                .unwrap()
                .model
                .as_ref()
                .map(|(glb, _)| glb.clone()),
            _ => None,
        })
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
    async fn push_reading_reaches_the_updates_feed_but_actuation_does_not() {
        let hal = RigHal::new();
        let mut readings = hal.updates();

        // A pushed reading (a frame, here a stand-in scalar) fans out to the feed.
        hal.push_reading(StateChange::set("display/face", float(1.0)));
        let got = readings.next().await.unwrap();
        assert_eq!(
            got.set.get(&Key::from("display/face")),
            Some(&Some(float(1.0)))
        );

        // Actuation the runtime writes is echoed on pose_updates, not readings —
        // so the reading feed stays empty after a write.
        hal.write(StateChange::set("mouth/jaw.open", float(0.5)))
            .await
            .unwrap();
        assert!(readings.next().now_or_never().is_none());
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
    async fn pose_feed_sees_writes_and_reading_feed_stays_silent() {
        let hal = RigHal::new();
        let mut pose_feed = hal.pose_updates();
        let mut reading_feed = hal.updates();
        hal.write(StateChange::set("k", bool_(true))).await.unwrap();
        // The renderer's pose feed carries the applied actuation…
        assert!(drain(&mut pose_feed)
            .iter()
            .any(|c| c.contains(&Key::from("k"))));
        // …but the runtime's reading feed never echoes it back: the rig has no
        // sensors, and an echo would clobber fresher store writes a frame later.
        assert!(drain(&mut reading_feed).is_empty());
    }

    #[tokio::test]
    async fn describe_and_model_glb() {
        let hal = RigHal::with_description(HalDescription {
            model_family: Some("vizij".into()),
            ..Default::default()
        });
        assert!(hal.models().is_empty());
        hal.set_model_glb(b"glTF".to_vec());
        assert_eq!(hal.describe().await.model_family.as_deref(), Some("vizij"));
        let assets = hal.assets().expect("the rig hands its model over");
        let models = assets.models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].component, DEVICE);
        assert!(models[0].servable, "a face's GLB is servable");
        assert_eq!(models[0].content_hash, Some(content_hash(b"glTF")));
        assert_eq!(
            models[0]
                .description
                .as_ref()
                .and_then(|d| d.model_family.as_deref()),
            Some("vizij")
        );
        assert_eq!(assets.servable_glb(DEVICE).unwrap(), Some(b"glTF".to_vec()));
        assert_eq!(assets.servable_glb("robot").unwrap(), None);
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
