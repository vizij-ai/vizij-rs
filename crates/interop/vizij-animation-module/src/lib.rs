//! `vizij-animation-core` packaged as an Arora wasm module.
//!
//! The animation [`Engine`] lives in a **guest global** (like `polly`): a wasm
//! module's `Store`/`Memory` persist across `dispatch`, so the engine's state
//! survives between calls — no engine state round-trips through the store.
//!
//! Boundary types are declared in `module.yaml` and code-generated into
//! [`arora_generated`] as typed `Value::Structure`s (ARORA-55): an
//! `AnimationClip { name, duration, tracks: [AnimTrack{ id, name, animatable_id,
//! points: [Keypoint{ id, stamp, value }] }] }`. A keyframe's `value` is a
//! **dynamic `Value`** (the `KEY_VALUE_ID` escape hatch), so Vizij composites
//! ride through as `Value::Structure` carrying vizij-arora's Vizij-namespaced
//! UUIDs — no per-composite type has to be declared here.
//!
//! Exports: `load_animation` / `create_player` / `add_instance` (setup) and
//! `step(dt_ns)` (per tick). `step` returns **per-track outputs keyed by track
//! identity**, each carrying the track's **default authored key** plus its
//! sampled value; the consumer (a runner, or a graph node) decides the final
//! store key — default = the authored key, overridable.

#[allow(clippy::all, dead_code, unused)]
mod arora_generated;

use std::collections::HashMap;
use std::sync::Mutex;

use arora_generated::vizij::{
    animation_clip::AnimationClip, keypoint::Keypoint as GenKeypoint, track_output::TrackOutput,
};
use arora_types::value::Value as AValue;

use vizij_animation_core::{
    AnimId, AnimationData, Config, Engine, Inputs, InstanceCfg, Keypoint as CoreKeypoint, PlayerId,
    Track as CoreTrack,
};

lazy_static::lazy_static! {
    /// The animation engine — one long-lived instance per module instance.
    static ref ENGINE: Mutex<Engine> = Mutex::new(Engine::new(Config::default()));
    /// Canonical output key (a track's `animatable_id`) -> the authored track id,
    /// so `step` can report per-track identity alongside the default key.
    static ref KEY_TO_TRACK: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

/// Load a typed animation clip into the engine and return its `AnimId`.
///
/// The keyframe `value`s arrive as raw Arora `Value`s (vizij-arora encoding) and
/// are converted back to Vizij values for the core model.
fn load_animation(clip: Option<AnimationClip>) -> u32 {
    let Some(clip) = clip else {
        return u32::MAX;
    };

    let mut key_map = KEY_TO_TRACK.lock().expect("key map");
    let tracks = clip
        .tracks
        .into_iter()
        .map(|t| {
            key_map.insert(t.animatable_id.clone(), t.id.clone());
            CoreTrack {
                id: t.id,
                name: t.name,
                animatable_id: t.animatable_id,
                points: t.points.into_iter().map(to_core_keypoint).collect(),
                settings: None,
            }
        })
        .collect();
    drop(key_map);

    let data = AnimationData {
        id: None,
        name: clip.name,
        tracks,
        groups: Default::default(),
        duration_ms: clip.duration,
    };

    ENGINE.lock().expect("engine").load_animation(data).0
}

/// Create a player and return its `PlayerId`.
fn create_player(name: Option<String>) -> u32 {
    ENGINE
        .lock()
        .expect("engine")
        .create_player(&name.unwrap_or_default())
        .0
}

/// Attach an animation instance to a player and return its `InstId`.
fn add_instance(player: Option<u32>, anim: Option<u32>) -> u32 {
    let (Some(player), Some(anim)) = (player, anim) else {
        return u32::MAX;
    };
    ENGINE
        .lock()
        .expect("engine")
        .add_instance(PlayerId(player), AnimId(anim), InstanceCfg::default())
        .0
}

/// Advance the engine by `dt_ns` nanoseconds and return per-track outputs.
///
/// `dt_ns` is the runtime's `arora/dt` golden key. Each output carries the
/// track's authored key as `default_key` and its stable id as `track_id`; the
/// value uses the vizij-arora `Value` encoding.
fn step(dt_ns: Option<u64>) -> Vec<TrackOutput> {
    let dt = dt_ns.unwrap_or(0) as f64 / 1e9;

    let mut engine = ENGINE.lock().expect("engine");
    let outputs = engine.update(dt as f32, Inputs::default());
    let key_map = KEY_TO_TRACK.lock().expect("key map");

    outputs
        .changes
        .iter()
        .map(|change| TrackOutput {
            track_id: key_map
                .get(&change.key)
                .cloned()
                .unwrap_or_else(|| change.key.clone()),
            default_key: change.key.clone(),
            value: vizij_arora::to_arora(&change.value).unwrap_or(AValue::Unit),
        })
        .collect()
}

/// Convert a generated keyframe (dynamic Arora `Value`) into a core keyframe
/// (Vizij `Value`). A value that has no Vizij mapping falls back to `0.0`.
fn to_core_keypoint(kp: GenKeypoint) -> CoreKeypoint {
    let value = vizij_arora::from_arora(&kp.value).unwrap_or(vizij_api_core::Value::Float(0.0));
    CoreKeypoint {
        id: kp.id,
        stamp: kp.stamp,
        value,
        transitions: None,
    }
}

#[cfg(test)]
mod tests {
    //! Exercises the module's exported functions directly (native), the way a
    //! wasm host would — but bypassing the buffer ABI. This proves the
    //! guest-global engine, the clip mapping, and the per-track output contract.
    //! The equivalent end-to-end path through a real wasm engine lives in
    //! `tests/host_ramp.rs`; see its docs for the upstream marshaling blocker.

    use super::*;
    use arora_generated::vizij::{anim_track::AnimTrack, keypoint::Keypoint as GenKeypoint};

    fn ramp_clip() -> AnimationClip {
        let kp = |id: &str, stamp: f32, v: f32| GenKeypoint {
            id: id.into(),
            stamp,
            value: AValue::F32(v),
        };
        AnimationClip {
            name: "ramp".into(),
            duration: 1000,
            tracks: vec![AnimTrack {
                id: "t0".into(),
                name: "ramp".into(),
                animatable_id: "node/x".into(),
                points: vec![kp("k0", 0.0, 0.0), kp("k1", 1.0, 1.0)],
            }],
        }
    }

    #[test]
    fn ramp_advances_and_carries_the_authored_key() {
        let anim = load_animation(Some(ramp_clip()));
        let player = create_player(Some("p".into()));
        let inst = add_instance(Some(player), Some(anim));
        assert_ne!(inst, u32::MAX);

        // The clip eases (default S-curve), so it is antisymmetric about the
        // midpoint: at t = 0.5 s (half of the 1 s clip) the value is ~0.5, and it
        // advances monotonically toward it.
        let first = step(Some(250_000_000)); // t = 0.25 s
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].track_id, "t0");
        assert_eq!(first[0].default_key, "node/x");
        let v0 = as_f32(&first[0].value);
        assert!(
            v0 > 0.0 && v0 < 0.5,
            "expected advance into (0, 0.5), got {v0}"
        );

        let second = step(Some(250_000_000)); // t = 0.5 s
        let v1 = as_f32(&second[0].value);
        assert!(v1 > v0, "expected monotonic advance, {v1} !> {v0}");
        assert!(
            (v1 - 0.5).abs() < 1e-3,
            "expected ~0.5 at t=0.5 s, got {v1}"
        );
    }

    fn as_f32(v: &AValue) -> f32 {
        match v {
            AValue::F32(f) => *f,
            other => panic!("expected F32, got {other:?}"),
        }
    }
}
