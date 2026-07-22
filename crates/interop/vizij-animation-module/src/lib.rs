//! `vizij-animation-core` packaged as an Arora wasm module.
//!
//! The animation [`Engine`] lives in a **guest global** (like `polly`): a wasm
//! module's `Store`/`Memory` persist across `dispatch`, so the engine's state
//! survives between calls — no engine state round-trips through the store.
//!
//! Boundary types are declared in `module.yaml` and code-generated into
//! [`arora_generated`] as typed `Value::Structure`s (ARORA-55): an
//! `AnimationClip { name, duration, tracks: [AnimTrack{ id, name, animatable_id,
//! points: [Keypoint{ id, stamp, value, transitions_in, transitions_out }] }] }`.
//! A keyframe's `value` is a **dynamic `Value`** (the `KEY_VALUE_ID` escape
//! hatch), so Vizij composites ride through as `Value::Structure` carrying
//! vizij-arora's Vizij-namespaced UUIDs — no per-composite type has to be
//! declared here. A keypoint's `transitions_in`/`transitions_out` carry its
//! cubic-bezier timing handles (zero or one each; empty = the engine's
//! default ease).
//!
//! Exports:
//! - setup — `load_animation` / `create_player` / `add_instance`;
//! - per tick — `step(dt_ns)`, returning **per-track outputs keyed by track
//!   identity**, each carrying the track's **default authored key** plus its
//!   sampled value; the consumer (a runner, or a graph node) decides the final
//!   store key — default = the authored key, overridable;
//! - transport — `play` / `pause` / `stop` / `seek(time_ns)` / `set_speed` /
//!   `set_loop` / `set_weight`, buffered into the engine's **next** `step`
//!   (issue order preserved), and `remove_instance`, applied immediately like
//!   `add_instance`;
//! - feedback — `player_states()`, one `PlayerState` per player. This call is
//!   a **patch**: the vision is state changes as first-class, combinable
//!   values the behavior conveys, not a second feedback channel.

#[allow(clippy::all, dead_code, unused)]
mod arora_generated;

use std::collections::HashMap;
use std::sync::Mutex;

use arora_generated::vizij::{
    animation_clip::AnimationClip, keypoint::Keypoint as GenKeypoint, player_state::PlayerState,
    track_output::TrackOutput,
};

use vizij_animation_core::{
    export_baked_json, export_baked_with_derivatives_json, AnimId, AnimationData, BakingConfig,
    Config, Engine, Inputs, InstId, InstanceCfg, InstanceUpdate, Keypoint as CoreKeypoint,
    LoopMode, PlayerCommand, PlayerId, Track as CoreTrack, Transitions, Vec2,
};

lazy_static::lazy_static! {
    /// The animation engine — one long-lived instance per module instance.
    static ref ENGINE: Mutex<Engine> = Mutex::new(Engine::new(Config::default()));
    /// Canonical output key (a track's `animatable_id`) -> the authored track id,
    /// so `step` can report per-track identity alongside the default key.
    static ref KEY_TO_TRACK: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    /// Player commands and instance updates issued since the previous step,
    /// drained (in issue order) into the next `step`'s engine update.
    static ref PENDING: Mutex<Inputs> = Mutex::new(Inputs::default());
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

/// Buffer a player command for the next `step`. Returns the echoed player id.
fn buffer_command(player: u32, command: PlayerCommand) -> u32 {
    PENDING
        .lock()
        .expect("pending inputs")
        .player_cmds
        .push(command);
    player
}

/// Resume or start playback. Applied at the next `step`.
fn play(player: Option<u32>) -> u32 {
    let Some(player) = player else {
        return u32::MAX;
    };
    buffer_command(
        player,
        PlayerCommand::Play {
            player: PlayerId(player),
        },
    )
}

/// Hold the playhead where it is. Applied at the next `step`.
fn pause(player: Option<u32>) -> u32 {
    let Some(player) = player else {
        return u32::MAX;
    };
    buffer_command(
        player,
        PlayerCommand::Pause {
            player: PlayerId(player),
        },
    )
}

/// Stop playback and reset to the window start. Applied at the next `step`.
fn stop(player: Option<u32>) -> u32 {
    let Some(player) = player else {
        return u32::MAX;
    };
    buffer_command(
        player,
        PlayerCommand::Stop {
            player: PlayerId(player),
        },
    )
}

/// Move the playhead to `time_ns` (nanoseconds, the `dt_ns` time base).
/// Applied at the next `step`.
fn seek(player: Option<u32>, time_ns: Option<u64>) -> u32 {
    let (Some(player), Some(time_ns)) = (player, time_ns) else {
        return u32::MAX;
    };
    buffer_command(
        player,
        PlayerCommand::Seek {
            player: PlayerId(player),
            time: (time_ns as f64 / 1e9) as f32,
        },
    )
}

/// Set the playback speed multiplier. Applied at the next `step`.
fn set_speed(player: Option<u32>, speed: Option<f32>) -> u32 {
    let (Some(player), Some(speed)) = (player, speed) else {
        return u32::MAX;
    };
    buffer_command(
        player,
        PlayerCommand::SetSpeed {
            player: PlayerId(player),
            speed,
        },
    )
}

/// Set how player time maps into clip time: `"once"`, `"loop"`, or
/// `"ping_pong"`. Applied at the next `step`.
fn set_loop(player: Option<u32>, mode: Option<String>) -> u32 {
    let (Some(player), Some(mode)) = (player, mode) else {
        return u32::MAX;
    };
    let mode = match mode.as_str() {
        "once" => LoopMode::Once,
        "loop" => LoopMode::Loop,
        "ping_pong" => LoopMode::PingPong,
        _ => return u32::MAX,
    };
    buffer_command(
        player,
        PlayerCommand::SetLoopMode {
            player: PlayerId(player),
            mode,
        },
    )
}

/// Set an instance's blend weight (weights normalize across a player's
/// instances). Applied at the next `step`. Returns the echoed instance id.
fn set_weight(player: Option<u32>, instance: Option<u32>, weight: Option<f32>) -> u32 {
    let (Some(player), Some(instance), Some(weight)) = (player, instance, weight) else {
        return u32::MAX;
    };
    PENDING
        .lock()
        .expect("pending inputs")
        .instance_updates
        .push(InstanceUpdate {
            player: PlayerId(player),
            inst: InstId(instance),
            weight: Some(weight),
            time_scale: None,
            start_offset: None,
            enabled: None,
        });
    instance
}

/// Detach an instance from its player, immediately (a structural edit, like
/// `add_instance`). Returns 1 when the instance existed, 0 otherwise.
fn remove_instance(player: Option<u32>, instance: Option<u32>) -> u32 {
    let (Some(player), Some(instance)) = (player, instance) else {
        return u32::MAX;
    };
    ENGINE
        .lock()
        .expect("engine")
        .remove_instance(PlayerId(player), InstId(instance)) as u32
}

/// One `PlayerState` per player: the engine's derived playback state, the
/// playhead and full length in nanoseconds (the `dt_ns` time base), and the
/// speed multiplier.
fn player_states() -> Vec<PlayerState> {
    let engine = ENGINE.lock().expect("engine");
    engine
        .list_players()
        .into_iter()
        .map(|info| PlayerState {
            player: info.id,
            state: match info.state {
                vizij_animation_core::engine::PlaybackState::Playing => "playing".to_string(),
                vizij_animation_core::engine::PlaybackState::Paused => "paused".to_string(),
                vizij_animation_core::engine::PlaybackState::Stopped => "stopped".to_string(),
            },
            time_ns: seconds_to_ns(info.time),
            duration_ns: seconds_to_ns(info.length),
            speed: info.speed,
        })
        .collect()
}

// baking ---------------------------------------------------------------------

/// Build a [`BakingConfig`] from the module's optional scalar arguments,
/// falling back to the core defaults (frame rate 60 Hz, start 0 s, end = clip
/// duration) for any argument left unset.
fn baking_config(
    frame_rate: Option<f32>,
    start_time: Option<f32>,
    end_time: Option<f32>,
) -> BakingConfig {
    let defaults = BakingConfig::default();
    BakingConfig {
        frame_rate: frame_rate.unwrap_or(defaults.frame_rate),
        start_time: start_time.unwrap_or(defaults.start_time),
        end_time: end_time.or(defaults.end_time),
        derivative_epsilon: defaults.derivative_epsilon,
    }
}

/// Bake animation `anim` to sampled per-track values over a fixed window and
/// return the result as a JSON string (`vizij-animation-core`'s
/// `export_baked_json` shape). `frame_rate` (Hz) defaults to 60, `start_time`
/// (seconds) to 0, and `end_time` (seconds) to the clip duration. Returns an
/// empty string if `anim` is not loaded.
fn bake(
    anim: Option<u32>,
    frame_rate: Option<f32>,
    start_time: Option<f32>,
    end_time: Option<f32>,
) -> String {
    let Some(anim) = anim else {
        return String::new();
    };
    let cfg = baking_config(frame_rate, start_time, end_time);
    match ENGINE
        .lock()
        .expect("engine")
        .bake_animation(AnimId(anim), &cfg)
    {
        Some(baked) => export_baked_json(&baked).to_string(),
        None => String::new(),
    }
}

/// Like [`bake`], but also samples per-frame derivatives; returns the combined
/// values-and-derivatives JSON (`export_baked_with_derivatives_json` shape).
/// Returns an empty string if `anim` is not loaded.
fn bake_with_derivatives(
    anim: Option<u32>,
    frame_rate: Option<f32>,
    start_time: Option<f32>,
    end_time: Option<f32>,
) -> String {
    let Some(anim) = anim else {
        return String::new();
    };
    let cfg = baking_config(frame_rate, start_time, end_time);
    match ENGINE
        .lock()
        .expect("engine")
        .bake_animation_with_derivatives(AnimId(anim), &cfg)
    {
        Some((baked, derivatives)) => {
            export_baked_with_derivatives_json(&baked, &derivatives).to_string()
        }
        None => String::new(),
    }
}

/// Advance the engine by `dt_ns` nanoseconds and return per-track outputs.
///
/// `dt_ns` is the runtime's `arora/dt` golden key. The transport commands
/// buffered since the previous step apply first, in issue order. Each output
/// carries the track's authored key as `default_key` and its stable id as
/// `track_id`; the value uses the vizij-arora `Value` encoding.
fn step(dt_ns: Option<u64>) -> Vec<TrackOutput> {
    let dt = dt_ns.unwrap_or(0) as f64 / 1e9;
    let inputs = std::mem::take(&mut *PENDING.lock().expect("pending inputs"));

    let mut engine = ENGINE.lock().expect("engine");
    let outputs = engine.update(dt as f32, inputs);
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
            value: vizij_arora::to_arora(&change.value),
        })
        .collect()
}

fn seconds_to_ns(seconds: f32) -> u64 {
    (seconds.max(0.0) as f64 * 1e9).round() as u64
}

/// Convert a generated keyframe (dynamic Arora `Value`) into a core keyframe:
/// the kernel decodes the shared `Value` into its POD `TrackValue` once, at
/// ingestion. The transition handle arrays (zero or one element each) become
/// the core's optional cubic-bezier timing handles.
fn to_core_keypoint(kp: GenKeypoint) -> CoreKeypoint {
    let value = vizij_animation_core::TrackValue::from(vizij_arora::from_arora(&kp.value));
    let r#in = kp.transitions_in.first().map(|h| Vec2 { x: h.x, y: h.y });
    let out = kp.transitions_out.first().map(|h| Vec2 { x: h.x, y: h.y });
    let transitions = (r#in.is_some() || out.is_some()).then_some(Transitions { r#in, out });
    CoreKeypoint {
        id: kp.id,
        stamp: kp.stamp,
        value,
        transitions,
    }
}

#[cfg(test)]
mod tests {
    //! Exercises the module's exported functions directly (native), the way a
    //! wasm host would — but bypassing the buffer ABI. This proves the
    //! guest-global engine, the clip mapping, the transport buffering, and the
    //! per-track output contract. The equivalent end-to-end path through a
    //! real wasm engine lives in `tests/host_ramp.rs`; see its docs for the
    //! upstream marshaling blocker.

    use super::*;
    use arora_generated::vizij::{
        anim_track::AnimTrack, keypoint::Keypoint as GenKeypoint,
        transition_handle::TransitionHandle,
    };
    use arora_types::value::Value as AValue;

    lazy_static::lazy_static! {
        /// The engine is a guest global shared by every test in this process;
        /// tests run one at a time so a test's `step`s advance only the time
        /// its own assertions account for. (Keys are per-test, so sequenced
        /// tests cannot see each other's outputs.)
        static ref SERIAL: Mutex<()> = Mutex::new(());
    }

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Cubic-bezier handles on the segment thirds: identity easing, so the
    /// sampled value equals normalized time exactly.
    fn linear_handles() -> (Vec<TransitionHandle>, Vec<TransitionHandle>) {
        (
            vec![TransitionHandle {
                x: 2.0 / 3.0,
                y: 2.0 / 3.0,
            }],
            vec![TransitionHandle {
                x: 1.0 / 3.0,
                y: 1.0 / 3.0,
            }],
        )
    }

    fn keypoint(id: &str, stamp: f32, v: f32) -> GenKeypoint {
        GenKeypoint {
            id: id.into(),
            stamp,
            value: AValue::F32(v),
            transitions_in: vec![],
            transitions_out: vec![],
        }
    }

    fn ramp_clip(name: &str, key: &str, linear: bool) -> AnimationClip {
        let mut k0 = keypoint("k0", 0.0, 0.0);
        let mut k1 = keypoint("k1", 1.0, 1.0);
        if linear {
            let (r#in, out) = linear_handles();
            k0.transitions_out = out;
            k1.transitions_in = r#in;
        }
        AnimationClip {
            name: name.into(),
            duration: 1000,
            tracks: vec![AnimTrack {
                id: format!("{name}-t0"),
                name: name.into(),
                animatable_id: key.into(),
                points: vec![k0, k1],
            }],
        }
    }

    /// A single-keypoint clip: samples to `v` at every playhead.
    fn constant_clip(name: &str, key: &str, v: f32) -> AnimationClip {
        AnimationClip {
            name: name.into(),
            duration: 1000,
            tracks: vec![AnimTrack {
                id: format!("{name}-t0"),
                name: name.into(),
                animatable_id: key.into(),
                points: vec![keypoint("k0", 0.0, v)],
            }],
        }
    }

    fn as_f32(v: &AValue) -> f32 {
        match v {
            AValue::F32(f) => *f,
            other => panic!("expected F32, got {other:?}"),
        }
    }

    fn value_of<'o>(outputs: &'o [TrackOutput], key: &str) -> Option<&'o AValue> {
        outputs
            .iter()
            .find(|o| o.default_key == key)
            .map(|o| &o.value)
    }

    fn state_of(player: u32) -> PlayerState {
        player_states()
            .into_iter()
            .find(|s| s.player == player)
            .expect("player state")
    }

    #[test]
    fn ramp_advances_and_carries_the_authored_key() {
        let _serial = serial();
        let anim = load_animation(Some(ramp_clip("ease-ramp", "ease/x", false)));
        let player = create_player(Some("p-ease".into()));
        let inst = add_instance(Some(player), Some(anim));
        assert_ne!(inst, u32::MAX);

        // The clip eases (default S-curve), so it is antisymmetric about the
        // midpoint: at t = 0.5 s (half of the 1 s clip) the value is ~0.5, and it
        // advances monotonically toward it.
        let first = step(Some(250_000_000)); // t = 0.25 s
        let out = first
            .iter()
            .find(|o| o.default_key == "ease/x")
            .expect("ease/x output");
        assert_eq!(out.track_id, "ease-ramp-t0");
        let v0 = as_f32(&out.value);
        assert!(
            v0 > 0.0 && v0 < 0.5,
            "expected advance into (0, 0.5), got {v0}"
        );

        let second = step(Some(250_000_000)); // t = 0.5 s
        let v1 = as_f32(value_of(&second, "ease/x").expect("ease/x output"));
        assert!(v1 > v0, "expected monotonic advance, {v1} !> {v0}");
        assert!(
            (v1 - 0.5).abs() < 1e-3,
            "expected ~0.5 at t=0.5 s, got {v1}"
        );
    }

    #[test]
    fn transitions_ride_through_to_sampling() {
        let _serial = serial();
        // Linear handles: value == normalized time, exactly.
        let anim = load_animation(Some(ramp_clip("lin-ramp", "lin/x", true)));
        let player = create_player(Some("p-lin".into()));
        add_instance(Some(player), Some(anim));

        let outputs = step(Some(250_000_000));
        let v = as_f32(value_of(&outputs, "lin/x").expect("lin/x output"));
        assert!(
            (v - 0.25).abs() < 1e-3,
            "linear transitions sample the identity, got {v} at u=0.25"
        );

        // A slow-out handle holds the curve low early on: strictly below linear.
        let mut slow = ramp_clip("slow-ramp", "slow/x", false);
        slow.tracks[0].points[0].transitions_out = vec![TransitionHandle { x: 1.0, y: 0.0 }];
        let anim = load_animation(Some(slow));
        let player = create_player(Some("p-slow".into()));
        add_instance(Some(player), Some(anim));

        let outputs = step(Some(250_000_000));
        let v_slow = as_f32(value_of(&outputs, "slow/x").expect("slow/x output"));
        assert!(
            v_slow < 0.25 - 1e-3,
            "slow-out handles must undershoot linear at u=0.25, got {v_slow}"
        );
    }

    #[test]
    fn transport_commands_apply_at_the_next_step() {
        let _serial = serial();
        let anim = load_animation(Some(ramp_clip("tr-ramp", "tr/x", true)));
        let player = create_player(Some("p-transport".into()));
        add_instance(Some(player), Some(anim));

        // Advance to 0.25 s.
        let outputs = step(Some(250_000_000));
        assert!((as_f32(value_of(&outputs, "tr/x").unwrap()) - 0.25).abs() < 1e-3);
        let s = state_of(player);
        assert_eq!(s.state, "playing");
        assert_eq!(s.duration_ns, 1_000_000_000, "1 s clip length");
        assert!((s.time_ns as f64 - 0.25e9).abs() < 2e6, "playhead ~0.25 s");

        // pause: the playhead holds through further steps.
        assert_eq!(pause(Some(player)), player);
        step(Some(250_000_000));
        let s = state_of(player);
        assert_eq!(s.state, "paused");
        assert!(
            (s.time_ns as f64 - 0.25e9).abs() < 2e6,
            "paused playhead holds"
        );

        // play resumes from where it held.
        assert_eq!(play(Some(player)), player);
        let outputs = step(Some(250_000_000));
        assert!((as_f32(value_of(&outputs, "tr/x").unwrap()) - 0.5).abs() < 1e-3);

        // seek lands exactly (u64 nanoseconds in).
        assert_eq!(seek(Some(player), Some(100_000_000)), player);
        let outputs = step(Some(0));
        assert!((as_f32(value_of(&outputs, "tr/x").unwrap()) - 0.1).abs() < 1e-3);

        // set_speed scales dt: 0.2 s of wall clock at 2x advances 0.4 s.
        assert_eq!(set_speed(Some(player), Some(2.0)), player);
        let outputs = step(Some(200_000_000));
        assert!((as_f32(value_of(&outputs, "tr/x").unwrap()) - 0.5).abs() < 1e-3);

        // stop resets to the window start.
        assert_eq!(stop(Some(player)), player);
        step(Some(0));
        let s = state_of(player);
        assert_eq!(s.state, "stopped");
        assert_eq!(s.time_ns, 0);
    }

    #[test]
    fn loop_once_clamps_at_the_clip_end() {
        let _serial = serial();
        let anim = load_animation(Some(ramp_clip("once-ramp", "once/x", true)));
        let player = create_player(Some("p-once".into()));
        add_instance(Some(player), Some(anim));

        assert_eq!(set_loop(Some(player), Some("once".into())), player);
        assert_eq!(seek(Some(player), Some(900_000_000)), player);
        step(Some(0));
        step(Some(300_000_000)); // 0.9 s + 0.3 s, clamped to the 1 s end
        let s = state_of(player);
        assert_eq!(s.time_ns, 1_000_000_000, "Once clamps at the clip end");

        assert_eq!(
            set_loop(Some(player), Some("sideways".into())),
            u32::MAX,
            "unknown loop modes are rejected"
        );
    }

    #[test]
    fn weights_skew_the_blend_and_removal_silences_the_key() {
        let _serial = serial();
        let zero = load_animation(Some(constant_clip("mix-zero", "mix/x", 0.0)));
        let one = load_animation(Some(constant_clip("mix-one", "mix/x", 1.0)));
        let player = create_player(Some("p-mix".into()));
        let inst_zero = add_instance(Some(player), Some(zero));
        let inst_one = add_instance(Some(player), Some(one));

        // Equal weights: the normalized blend of 0 and 1.
        let outputs = step(Some(100_000_000));
        assert!((as_f32(value_of(&outputs, "mix/x").unwrap()) - 0.5).abs() < 1e-3);

        // Silencing the zero-instance leaves only the one-instance.
        assert_eq!(
            set_weight(Some(player), Some(inst_zero), Some(0.0)),
            inst_zero
        );
        let outputs = step(Some(100_000_000));
        assert!((as_f32(value_of(&outputs, "mix/x").unwrap()) - 1.0).abs() < 1e-3);

        // Removing both instances stops the key from being emitted at all.
        assert_eq!(remove_instance(Some(player), Some(inst_zero)), 1);
        assert_eq!(remove_instance(Some(player), Some(inst_one)), 1);
        assert_eq!(
            remove_instance(Some(player), Some(inst_one)),
            0,
            "already gone"
        );
        let outputs = step(Some(100_000_000));
        assert!(
            value_of(&outputs, "mix/x").is_none(),
            "no instances, no output for the key"
        );
    }

    #[test]
    fn bake_exports_sampled_tracks_as_json() {
        let _serial = serial();
        let anim = load_animation(Some(constant_clip("bake-me", "joint/x", 0.5)));

        // A loaded clip bakes to a JSON object echoing the requested frame rate
        // and carrying at least one track of sampled values.
        let json = bake(Some(anim), Some(30.0), None, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("baked JSON parses");
        assert_eq!(parsed["frame_rate"].as_f64(), Some(30.0));
        let tracks = parsed["tracks"].as_array().expect("tracks array");
        assert!(!tracks.is_empty(), "at least one baked track");
        assert!(
            tracks[0].get("target_path").is_some(),
            "track carries a path"
        );
        assert!(
            tracks[0]["values"]
                .as_array()
                .is_some_and(|v| !v.is_empty()),
            "track has sampled values"
        );

        // The derivatives variant wraps values + derivatives.
        let deriv = bake_with_derivatives(Some(anim), Some(30.0), None, None);
        let dparsed: serde_json::Value =
            serde_json::from_str(&deriv).expect("derivative JSON parses");
        assert!(dparsed.get("values").is_some() && dparsed.get("derivatives").is_some());

        // An unloaded animation bakes to an empty string.
        assert!(bake(Some(u32::MAX), None, None, None).is_empty());
    }
}
