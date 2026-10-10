//! `vizij-animation-core` packaged as an Arora module, wasm guest or host-linked.
//!
//! The module's interface is declared in Rust: [`animation`] carries the
//! module and function ids, the header a `module.yaml` is written from, the
//! store record and the guest entry points; the boundary types derive
//! [`AroraType`] with the ids and versions of their records.
//!
//! The module's state — the animation [`Engine`], the key-to-track index and
//! the transport commands buffered for the next step — is an
//! [`AnimationModule`]. The module has no notion of which engine loaded it:
//! the state is scoped by the load. The wasm guest owns one in a **guest
//! global**, in the linear memory of the `Store` the executor creates per
//! `load_module` — so each engine that loads the module gets its own, and it
//! persists across `dispatch` calls; the declared functions act on that
//! global, with no lock, so the call after a trap still reaches the state.
//! A host that links this crate has no executor to scope it, so it
//! builds one [`AnimationModule`] per registration and dispatches to it under
//! the declared ids — two devices in one process never share an engine.
//!
//! A keyframe's `value` is a **dynamic `Value`** (the key-value type), so
//! Vizij composites ride through as `Value::Structure` carrying vizij-arora's
//! Vizij-namespaced UUIDs — no per-composite type has to be declared here. A
//! keypoint's `transitions_in`/`transitions_out` carry its cubic-bezier timing
//! handles (zero or one each; empty = the engine's default ease).
//!
//! A number keeps the width of its keypoints: `F32` keypoints output `F32`,
//! `F64` keypoints `F64` (sampled and blended in `f64`), from every step and
//! bake. A track with any `F64` keypoint outputs `F64` throughout; a key an
//! `F32` and an `F64` track blend into outputs `F64` on the steps the `F64`
//! track weighs on it.
//!
//! Exports:
//! - loading — `load_animation` / `reload_animation` / `create_player` /
//!   `add_instance` / `add_instance_with_weight`, and unloading —
//!   `remove_instance` / `remove_player` / `unload_animation`: structural
//!   edits, applied immediately;
//! - per tick — `step(dt_ns, time_ns?)`, returning **per-track outputs keyed
//!   by track identity**, each carrying the track's **default authored key**
//!   plus its sampled value; the consumer (a runner, or a graph node) decides
//!   the final store key — default = the authored key, overridable. Or
//!   `step_values(dt_ns, time_ns?)`, the same step returning the values
//!   alone, by position in the table `output_keys()` returns: the keys
//!   travel when a structural edit changes them, not every step;
//! - transport — `play` / `play_at(time_ns)` / `pause` / `stop` /
//!   `seek(time_ns)` / `set_speed` (negative plays backwards) / `set_loop` /
//!   `set_window`, and per instance `set_weight` / `set_start_offset` /
//!   `set_time_scale`, buffered into the engine's **next** `step` (issue order
//!   preserved). `play`, `play_at`, `pause` and `stop` set whether a player's
//!   time advances and `set_speed` only the rate it advances at, so a player
//!   resumes at the speed it was given;
//! - feedback — `player_states()`, one `PlayerState` per player: its name,
//!   its playback (including whether a `once` player has ended), its loop
//!   mode, its play window and its instances, so any client finds a player by
//!   the name it was created under and reads where it stands. The behavior
//!   that steps the module conveys it as a state value each step (Vizij's
//!   animation source writes it to `vizij/animations/players`); this call is a
//!   **patch**: the vision is state changes as first-class, combinable values
//!   the behavior conveys, not a second feedback channel;
//! - baking — `bake` / `bake_with_derivatives`, the animation sampled at a
//!   fixed rate as a [`BakedAnimation`] record of `Value`s.

use std::cell::UnsafeCell;
use std::collections::HashMap;

use arora_types::value::Value;
use arora_types::AroraType;

use vizij_animation_core::{
    AnimId, AnimationData, BakedAnimationData, BakedDerivativeAnimationData, BakingConfig, Config,
    Engine, Inputs, InstId, InstanceCfg, InstanceUpdate, Keypoint as CoreKeypoint, LoopMode,
    PlayerCommand, PlayerId, Track as CoreTrack, Transitions, Vec2,
};

// Boundary types --------------------------------------------------------------

/// A clip: its tracks and its duration in milliseconds.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000100", version = "1.1.0")]
pub struct AnimationClip {
    #[arora(id = "76697a69-6a00-0000-0100-000000000001")]
    pub name: String,
    #[arora(id = "76697a69-6a00-0000-0100-000000000002")]
    pub duration: u32,
    #[arora(id = "76697a69-6a00-0000-0100-000000000003")]
    pub tracks: Vec<AnimTrack>,
}

/// One animated key: the track's authored id and name, the key it targets
/// (`animatable_id`), and its keypoints.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000101", version = "1.1.0")]
pub struct AnimTrack {
    #[arora(id = "76697a69-6a00-0000-0101-000000000001")]
    pub id: String,
    #[arora(id = "76697a69-6a00-0000-0101-000000000002")]
    pub name: String,
    #[arora(id = "76697a69-6a00-0000-0101-000000000003")]
    pub animatable_id: String,
    #[arora(id = "76697a69-6a00-0000-0101-000000000004")]
    pub points: Vec<Keypoint>,
}

/// A keyframe: a normalized stamp, a dynamic value, and the cubic-bezier
/// timing handles of the segments it bounds (zero or one per side).
/// A number `value` sets the width its track outputs in: `Value::F32` or
/// `Value::F64`.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000102", version = "1.1.0")]
pub struct Keypoint {
    #[arora(id = "76697a69-6a00-0000-0102-000000000001")]
    pub id: String,
    #[arora(id = "76697a69-6a00-0000-0102-000000000002")]
    pub stamp: f32,
    #[arora(id = "76697a69-6a00-0000-0102-000000000003", keyvalue)]
    pub value: Value,
    #[arora(id = "76697a69-6a00-0000-0102-000000000004")]
    pub transitions_in: Vec<TransitionHandle>,
    #[arora(id = "76697a69-6a00-0000-0102-000000000005")]
    pub transitions_out: Vec<TransitionHandle>,
}

/// A cubic-bezier timing handle in normalized segment space: x = time,
/// y = value; linear timing puts the handles on the segment thirds.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000103")]
pub struct TransitionHandle {
    #[arora(id = "76697a69-6a00-0000-0103-000000000001")]
    pub x: f32,
    #[arora(id = "76697a69-6a00-0000-0103-000000000002")]
    pub y: f32,
}

/// One track's sampled value at a step, with the track's identity and its
/// authored key.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000110")]
pub struct TrackOutput {
    #[arora(id = "76697a69-6a00-0000-0110-000000000001")]
    pub track_id: String,
    #[arora(id = "76697a69-6a00-0000-0110-000000000002")]
    pub default_key: String,
    #[arora(id = "76697a69-6a00-0000-0110-000000000003", keyvalue)]
    pub value: Value,
}

/// What `step_values` returns: every output's value by its position in the
/// table `output_keys` returns, and the revision of that table.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000113")]
pub struct StepValues {
    /// The revision of the output table these values are laid out by: when
    /// it differs from the one a consumer read `output_keys` at, a
    /// structural edit changed the table, and the consumer reads it again.
    #[arora(id = "76697a69-6a00-0000-0113-000000000001")]
    pub revision: u32,
    /// A `Value::ArrayValue` as long as the output table: at each position,
    /// the value written to that output's key this step (encoded as
    /// [`TrackOutput::value`] is), or `Value::Unit` when no instance weighs on
    /// it this step. A record field cannot declare an array of dynamic
    /// values, so the array travels as one dynamic value.
    #[arora(id = "76697a69-6a00-0000-0113-000000000002", keyvalue)]
    pub values: Value,
}

/// What `output_keys` returns: the key each position of `step_values`'
/// values is written to, and the revision of that table.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000114")]
pub struct OutputKeys {
    /// The table's revision, as `step_values` reports it.
    #[arora(id = "76697a69-6a00-0000-0114-000000000001")]
    pub revision: u32,
    /// The key of each output (a track's `animatable_id`), in position
    /// order: per player, in creation order, each key its instances write,
    /// in the order they first write it. A key two players write has a
    /// position per player.
    #[arora(id = "76697a69-6a00-0000-0114-000000000002")]
    pub keys: Vec<String>,
}

/// One player's state: its name (as `create_player` gave it, empty when it
/// gave none), its playback — `"playing"`, `"paused"` or `"stopped"`, the
/// playhead and full length in nanoseconds (the `dt_ns` time base), the speed
/// multiplier as `set_speed` set it (negative backwards), the loop mode
/// (`"once"`, `"loop"` or `"ping_pong"`, as `set_loop` takes it), whether it
/// has ended, its play window — and its instances, in evaluation order.
///
/// A player waiting for its `play_at` start holds its playhead and reports the
/// state it had, `"playing"` from its start on. The engine keeps times in single-precision
/// seconds, so a time read back may differ from the one set by that rounding.
///
/// Record `1.1.0` adds `name`, `instances` and `loop_mode` to `1.0.0`'s five
/// fields; `1.2.0` adds `ended`, `window_start_ns` and `window_end_ns`.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000111", version = "1.2.0")]
pub struct PlayerState {
    #[arora(id = "76697a69-6a00-0000-0111-000000000001")]
    pub player: u32,
    #[arora(id = "76697a69-6a00-0000-0111-000000000002")]
    pub state: String,
    #[arora(id = "76697a69-6a00-0000-0111-000000000003")]
    pub time_ns: u64,
    #[arora(id = "76697a69-6a00-0000-0111-000000000004")]
    pub duration_ns: u64,
    #[arora(id = "76697a69-6a00-0000-0111-000000000005")]
    pub speed: f32,
    #[arora(id = "76697a69-6a00-0000-0111-000000000006")]
    pub name: String,
    #[arora(id = "76697a69-6a00-0000-0111-000000000007")]
    pub instances: Vec<InstanceState>,
    #[arora(id = "76697a69-6a00-0000-0111-000000000008")]
    pub loop_mode: String,
    /// A `"once"` player playing at a non-zero speed has reached the window
    /// bound it heads for — the end, or the start when playing backwards —
    /// and holds there until it is played again, sought or reversed.
    #[arora(id = "76697a69-6a00-0000-0111-000000000009")]
    pub ended: bool,
    /// The play window's start, in nanoseconds.
    #[arora(id = "76697a69-6a00-0000-0111-00000000000a")]
    pub window_start_ns: u64,
    /// The play window's end, in nanoseconds; none ends it at the player's
    /// length (`duration_ns`).
    #[arora(id = "76697a69-6a00-0000-0111-00000000000b")]
    pub window_end_ns: Option<u64>,
}

/// One instance on a player: its id, the animation it plays (what
/// `unload_animation` takes), its blend weight, and its timing on the player
/// timeline (as `set_start_offset` and `set_time_scale` take them).
///
/// Record `1.1.0` adds `start_offset_ns` and `time_scale` to `1.0.0`'s three
/// fields.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000112", version = "1.1.0")]
pub struct InstanceState {
    #[arora(id = "76697a69-6a00-0000-0112-000000000001")]
    pub instance: u32,
    #[arora(id = "76697a69-6a00-0000-0112-000000000002")]
    pub anim: u32,
    #[arora(id = "76697a69-6a00-0000-0112-000000000003")]
    pub weight: f32,
    #[arora(id = "76697a69-6a00-0000-0112-000000000004")]
    pub start_offset_ns: i64,
    #[arora(id = "76697a69-6a00-0000-0112-000000000005")]
    pub time_scale: f32,
}

/// An animation sampled at a fixed rate over a window of its clip, as
/// `bake` and `bake_with_derivatives` return it. Sample `i` of each track is
/// the track at `start_ns / 1e9 + i / frame_rate` seconds into the clip, from
/// the window's start up to the first sample at or past its end; a sample
/// past the clip's end holds the clip's end value.
///
/// The records travel on the self-describing value wire (a module call's,
/// a bridge's); the samples, dynamic values, have no form on a typed wire
/// such as CDR.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000120")]
pub struct BakedAnimation {
    /// The samples per second.
    #[arora(id = "76697a69-6a00-0000-0120-000000000001")]
    pub frame_rate: f32,
    /// Where the window starts in the clip, in nanoseconds: the first
    /// sample's instant.
    #[arora(id = "76697a69-6a00-0000-0120-000000000002")]
    pub start_ns: u64,
    /// Where the window ends in the clip, in nanoseconds.
    #[arora(id = "76697a69-6a00-0000-0120-000000000003")]
    pub end_ns: u64,
    /// One entry per track of the animation, in the clip's track order.
    #[arora(id = "76697a69-6a00-0000-0120-000000000004")]
    pub tracks: Vec<BakedTrack>,
}

/// One track's samples in a [`BakedAnimation`], one per frame.
///
/// The samples travel as one dynamic `Value`, a `Value::ArrayValue`, since a
/// record field cannot declare an array of dynamic values; each element is a
/// dynamic value as [`Keypoint::value`] is.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000121")]
pub struct BakedTrack {
    /// The key the track animates: its [`AnimTrack::animatable_id`].
    #[arora(id = "76697a69-6a00-0000-0121-000000000001")]
    pub animatable_id: String,
    /// The track's value at each frame: a `Value::ArrayValue`.
    #[arora(id = "76697a69-6a00-0000-0121-000000000002", keyvalue)]
    pub values: Value,
    /// The track's rate of change per second at each frame, from
    /// `bake_with_derivatives`: a `Value::ArrayValue` of `Value::Option`s,
    /// `Value::Option(None)` where the sample has no derivative (a boolean
    /// or text track).
    /// Empty when the bake took no derivatives (`bake`).
    #[arora(id = "76697a69-6a00-0000-0121-000000000003", keyvalue)]
    pub derivatives: Value,
}

/// One load's state: the engine, the key-to-track index, the transport
/// commands waiting for the next step and the clock `play_at` reads.
pub struct AnimationModule {
    engine: Engine,
    /// Each loaded animation's tracks as (canonical output key, authored track
    /// id), in load order: what `key_to_track` is built from.
    tracks: Vec<(u32, Vec<(String, String)>)>,
    /// Canonical output key (a track's `animatable_id`) -> the authored track id,
    /// so `step` can report per-track identity alongside the default key. A
    /// key two animations write maps to the track of the later-loaded one.
    key_to_track: HashMap<String, String>,
    /// Each engine output's key and track id, in output order, as of
    /// `outputs_revision`: what `step` labels its values with.
    outputs: Vec<(String, String)>,
    /// The engine's output revision `outputs` was read at; none once
    /// `key_to_track` changed since.
    outputs_revision: Option<u32>,
    /// Player commands and instance updates issued since the previous step,
    /// drained (in issue order) into the next `step`'s engine update.
    pending: Inputs,
    /// Each `play_at` in `pending`: its command's index and its start time,
    /// which the next step turns into the command's delay.
    anchors: Vec<(usize, u64)>,
    /// The time the previous step ended at, in nanoseconds.
    clock_ns: u64,
}

impl Default for AnimationModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationModule {
    /// An empty engine with the core's default configuration.
    pub fn new() -> Self {
        Self {
            engine: Engine::new(Config::default()),
            tracks: Vec::new(),
            key_to_track: HashMap::new(),
            outputs: Vec::new(),
            outputs_revision: None,
            pending: Inputs::default(),
            anchors: Vec::new(),
            clock_ns: 0,
        }
    }

    /// Load a typed animation clip into the engine and return its `AnimId`.
    ///
    /// The keyframe `value`s arrive as raw Arora `Value`s (vizij-arora encoding)
    /// and are converted back to Vizij values for the core model.
    pub fn load_animation(&mut self, clip: AnimationClip) -> u32 {
        let (data, tracks) = to_core_animation(clip);
        let anim = self.engine.load_animation(data).0;
        self.tracks.push((anim, tracks));
        self.index_tracks();
        anim
    }

    /// Replace the data of loaded animation `anim` with `clip`, immediately
    /// (a structural edit, like `load_animation`), keeping its id. Every
    /// instance of it stays on its player with its weight, and samples
    /// `clip`'s tracks from the next step on; the players keep their
    /// playback — state, playhead, speed and loop mode — and their length
    /// follows `clip`'s duration. Returns whether `anim` was loaded; when it
    /// was not, nothing changes.
    pub fn reload_animation(&mut self, anim: u32, clip: AnimationClip) -> bool {
        let (data, tracks) = to_core_animation(clip);
        if !self.engine.replace_animation(AnimId(anim), data) {
            return false;
        }
        if let Some((_, loaded)) = self.tracks.iter_mut().find(|(a, _)| *a == anim) {
            *loaded = tracks;
        }
        self.index_tracks();
        true
    }

    /// Rebuild `key_to_track` from the loaded animations' tracks, and have
    /// the next `step` label its outputs from it.
    fn index_tracks(&mut self) {
        self.outputs_revision = None;
        self.key_to_track = self
            .tracks
            .iter()
            .flat_map(|(_, tracks)| tracks.iter().cloned())
            .collect();
    }

    /// Create a player and return its `PlayerId`.
    pub fn create_player(&mut self, name: Option<String>) -> u32 {
        self.engine.create_player(&name.unwrap_or_default()).0
    }

    /// Attach an instance of animation `anim` to a player, immediately, and
    /// return its `InstId`. It blends at weight 1 from the next step on.
    pub fn add_instance(&mut self, player: u32, anim: u32) -> u32 {
        self.engine
            .add_instance(PlayerId(player), AnimId(anim), InstanceCfg::default())
            .0
    }

    /// [`add_instance`](Self::add_instance) at `weight`: the instance blends
    /// at it from the next step on, so one added at 0 writes nothing until a
    /// `set_weight` gives it a weight. A client learns an instance's id only
    /// from the call's reply, so a `set_weight` it sends after lands a step
    /// later than the instance. A weight that is not finite and non-negative
    /// is rejected with `u32::MAX`, adding nothing.
    pub fn add_instance_with_weight(&mut self, player: u32, anim: u32, weight: f32) -> u32 {
        if !valid_weight(weight) {
            return u32::MAX;
        }
        let cfg = InstanceCfg {
            weight,
            ..InstanceCfg::default()
        };
        self.engine
            .add_instance(PlayerId(player), AnimId(anim), cfg)
            .0
    }

    /// Buffer a player command for the next `step`. Returns the echoed player id.
    fn buffer_command(&mut self, player: u32, command: PlayerCommand) -> u32 {
        self.pending.player_cmds.push(command);
        player
    }

    /// Start or resume advancing the player's time, at the speed `set_speed`
    /// gave it (1 unless set). Cancels a pending `play_at`. Applied at the
    /// next `step`.
    pub fn play(&mut self, player: u32) -> u32 {
        self.buffer_command(
            player,
            PlayerCommand::Play {
                player: PlayerId(player),
            },
        )
    }

    /// Hold the playhead where it is; the player keeps its speed for the next
    /// `play`. Cancels a pending `play_at`. Applied at the next `step`.
    pub fn pause(&mut self, player: u32) -> u32 {
        self.buffer_command(
            player,
            PlayerCommand::Pause {
                player: PlayerId(player),
            },
        )
    }

    /// Stop playback and reset to the window start; the player keeps its
    /// speed for the next `play`. Cancels a pending `play_at`. Applied at the
    /// next `step`.
    pub fn stop(&mut self, player: u32) -> u32 {
        self.buffer_command(
            player,
            PlayerCommand::Stop {
                player: PlayerId(player),
            },
        )
    }

    /// Move the playhead to `time_ns` (nanoseconds, the `dt_ns` time base); a
    /// stopped player is left paused there. Applied at the next `step`.
    pub fn seek(&mut self, player: u32, time_ns: u64) -> u32 {
        self.buffer_command(
            player,
            PlayerCommand::Seek {
                player: PlayerId(player),
                time: ns_to_seconds(time_ns),
            },
        )
    }

    /// Set the multiplier the player's time advances at while it plays:
    /// negative plays backwards — `once` then holds at the window start,
    /// `loop` wraps from the window start to its end, `ping_pong` reflects —
    /// and 0 holds the playhead. It does not play, pause or stop the player:
    /// a paused player stays paused at its new speed. A speed that is not
    /// finite is rejected with `u32::MAX`, as it would make the playhead NaN
    /// or infinite. Applied at the next `step`.
    pub fn set_speed(&mut self, player: u32, speed: f32) -> u32 {
        if !speed.is_finite() {
            return u32::MAX;
        }
        self.buffer_command(
            player,
            PlayerCommand::SetSpeed {
                player: PlayerId(player),
                speed,
            },
        )
    }

    /// Set how player time maps into clip time: `"once"`, `"loop"`, or
    /// `"ping_pong"`. Applied at the next `step`.
    pub fn set_loop(&mut self, player: u32, mode: String) -> u32 {
        let mode = match mode.as_str() {
            "once" => LoopMode::Once,
            "loop" => LoopMode::Loop,
            "ping_pong" => LoopMode::PingPong,
            _ => return u32::MAX,
        };
        self.buffer_command(
            player,
            PlayerCommand::SetLoopMode {
                player: PlayerId(player),
                mode,
            },
        )
    }

    /// Start playback at `time_ns`, in the time base `step` is given — the
    /// device's `arora/time`: the player holds its playhead until then, and
    /// from then on it stands where it would had it started at that instant,
    /// whichever steps it falls between, so players anchored at one instant on
    /// devices sharing a clock reference play in lockstep. A `time_ns` already
    /// past starts it there too: its next step advances it by the time since.
    /// The player starts at its speed (1 unless `set_speed` set another); a
    /// `set_speed` while it waits sets the speed it starts at. A later `play`,
    /// `pause` or `stop` cancels the wait. Applied at the next `step`.
    pub fn play_at(&mut self, player: u32, time_ns: u64) -> u32 {
        self.anchors.push((self.pending.player_cmds.len(), time_ns));
        self.buffer_command(
            player,
            PlayerCommand::PlayAfter {
                player: PlayerId(player),
                delay: 0.0,
            },
        )
    }

    /// Set the play window, in nanoseconds of player time: `once` clamps the
    /// playhead into it, `loop` wraps within it, `ping_pong` reflects within
    /// it, and `stop` returns to its start. `end_ns` left out ends it at the
    /// player's length; an end before the start is the start. A playhead
    /// outside the new window moves to its nearest bound. Applied at the next
    /// `step`.
    pub fn set_window(&mut self, player: u32, start_ns: u64, end_ns: Option<u64>) -> u32 {
        self.buffer_command(
            player,
            PlayerCommand::SetWindow {
                player: PlayerId(player),
                start_time: ns_to_seconds(start_ns),
                end_time: end_ns.map(ns_to_seconds),
            },
        )
    }

    /// Set an instance's blend weight (weights normalize across a player's
    /// instances; at 0 the instance writes nothing). A weight that is not
    /// finite and non-negative is rejected with `u32::MAX`. Applied at the
    /// next `step`. Returns the echoed instance id.
    pub fn set_weight(&mut self, player: u32, instance: u32, weight: f32) -> u32 {
        if !valid_weight(weight) {
            return u32::MAX;
        }
        self.update_instance(InstanceUpdate {
            weight: Some(weight),
            ..instance_update(player, instance)
        })
    }

    /// Set when an instance starts on its player's timeline, in nanoseconds:
    /// before it, the instance holds its clip's start; a negative offset
    /// starts it partway into its clip. Applied at the next `step`. Returns
    /// the echoed instance id.
    pub fn set_start_offset(&mut self, player: u32, instance: u32, offset_ns: i64) -> u32 {
        self.update_instance(InstanceUpdate {
            start_offset: Some((offset_ns as f64 / 1e9) as f32),
            ..instance_update(player, instance)
        })
    }

    /// Set how an instance's clip stretches on its player's timeline: the
    /// clip's local time is (playhead − start offset) / `time_scale`, so 2
    /// plays it over twice its duration and 0.5 over half. It must be finite
    /// and positive (playing backwards is the player's negative speed);
    /// another value is rejected with `u32::MAX`. Applied at the next `step`.
    /// Returns the echoed instance id.
    pub fn set_time_scale(&mut self, player: u32, instance: u32, time_scale: f32) -> u32 {
        if !(time_scale.is_finite() && time_scale > 0.0) {
            return u32::MAX;
        }
        self.update_instance(InstanceUpdate {
            time_scale: Some(time_scale),
            ..instance_update(player, instance)
        })
    }

    /// Buffer an instance update for the next `step`. Returns the echoed
    /// instance id.
    fn update_instance(&mut self, update: InstanceUpdate) -> u32 {
        let instance = update.inst.0;
        self.pending.instance_updates.push(update);
        instance
    }

    /// Detach an instance from its player, immediately (a structural edit, like
    /// `add_instance`). Returns 1 when the instance existed, 0 otherwise.
    pub fn remove_instance(&mut self, player: u32, instance: u32) -> u32 {
        self.engine
            .remove_instance(PlayerId(player), InstId(instance)) as u32
    }

    /// Remove a player and its instances, immediately (a structural edit,
    /// like `add_instance`); commands still buffered for it are dropped at
    /// the next step. The animations it played stay loaded. Returns whether
    /// it existed.
    pub fn remove_player(&mut self, player: u32) -> bool {
        self.engine.remove_player(PlayerId(player))
    }

    /// Unload an animation and every instance of it, on every player,
    /// immediately. Returns whether it was loaded.
    pub fn unload_animation(&mut self, anim: u32) -> bool {
        if !self.engine.unload_animation(AnimId(anim)) {
            return false;
        }
        self.tracks.retain(|(a, _)| *a != anim);
        self.index_tracks();
        true
    }

    /// One `PlayerState` per player, in creation order: its name, its
    /// playback state as the last `play`, `pause` or `stop` left it (a new
    /// player plays; a `play_at` plays it once its start comes), the playhead
    /// and full length in nanoseconds (the `dt_ns` time base), the speed
    /// multiplier as `set_speed` set it, the loop mode, whether it has ended,
    /// its play window, and its instances.
    pub fn player_states(&self) -> Vec<PlayerState> {
        self.engine
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
                loop_mode: match info.loop_mode {
                    LoopMode::Once => "once",
                    LoopMode::Loop => "loop",
                    LoopMode::PingPong => "ping_pong",
                }
                .to_string(),
                instances: self
                    .engine
                    .list_instances(PlayerId(info.id))
                    .into_iter()
                    .map(|instance| InstanceState {
                        instance: instance.id,
                        anim: instance.animation,
                        weight: instance.cfg.weight,
                        start_offset_ns: (f64::from(instance.cfg.start_offset) * 1e9).round()
                            as i64,
                        time_scale: instance.cfg.time_scale,
                    })
                    .collect(),
                ended: info.ended,
                window_start_ns: seconds_to_ns(info.start_time),
                window_end_ns: info.end_time.map(seconds_to_ns),
                name: info.name,
            })
            .collect()
    }

    /// Bake animation `anim`: each of its tracks sampled at `frame_rate` Hz
    /// (60 when left out) over the window from `start_ns` (0 when left out)
    /// to `end_ns` (the clip's end when left out), in nanoseconds of the
    /// clip's time; the window clamps into the clip. `None` when `anim` is
    /// not loaded, or when the bake would take more than the core's
    /// `MAX_BAKE_SAMPLES` samples over all tracks.
    pub fn bake(
        &self,
        anim: u32,
        frame_rate: Option<f32>,
        start_ns: Option<u64>,
        end_ns: Option<u64>,
    ) -> Option<BakedAnimation> {
        let cfg = baking_config(frame_rate, start_ns, end_ns);
        let baked = self.engine.bake_animation(AnimId(anim), &cfg).ok()?;
        let window = self.bake_window(anim, start_ns, end_ns)?;
        Some(baked_animation(baked, None, window))
    }

    /// [`AnimationModule::bake`], each sample with the track's derivative
    /// there. `None` when `anim` is not loaded, or when the bake would take
    /// more than `MAX_BAKE_SAMPLES` samples, values and derivatives together.
    pub fn bake_with_derivatives(
        &self,
        anim: u32,
        frame_rate: Option<f32>,
        start_ns: Option<u64>,
        end_ns: Option<u64>,
    ) -> Option<BakedAnimation> {
        let cfg = baking_config(frame_rate, start_ns, end_ns);
        let (baked, derivatives) = self
            .engine
            .bake_animation_with_derivatives(AnimId(anim), &cfg)
            .ok()?;
        let window = self.bake_window(anim, start_ns, end_ns)?;
        Some(baked_animation(baked, Some(derivatives), window))
    }

    /// The window a bake of `anim` takes, in nanoseconds: the requested one
    /// clamped into the clip, `[start_ns, end_ns]` as the core clamps it,
    /// computed in integers so it reads back as exactly what was asked.
    fn bake_window(
        &self,
        anim: u32,
        start_ns: Option<u64>,
        end_ns: Option<u64>,
    ) -> Option<(u64, u64)> {
        let duration_ns = self
            .engine
            .list_animations()
            .into_iter()
            .find(|info| info.id == anim)
            .map(|info| u64::from(info.duration_ms) * 1_000_000)?;
        let start = start_ns.unwrap_or(0).min(duration_ns);
        let end = end_ns.unwrap_or(duration_ns).clamp(start, duration_ns);
        Some((start, end))
    }

    /// Advance the engine by `dt_ns` nanoseconds and return per-track outputs.
    ///
    /// `dt_ns` is the runtime's `arora/dt` built-in key and `time_ns` its
    /// `arora/time`, the time this step ends at: the time base of `play_at`.
    /// Left out, that time is the previous step's plus `dt_ns` (from 0, so
    /// the time since the module's first step). The transport commands
    /// buffered since the previous step apply first, in issue order. Each
    /// output carries the track's authored key as `default_key` and its stable
    /// id as `track_id`; the value uses the vizij-arora `Value` encoding. The
    /// outputs come in the order of [`output_keys`](Self::output_keys), those
    /// no instance weighs on this step left out.
    pub fn step(&mut self, dt_ns: u64, time_ns: Option<u64>) -> Vec<TrackOutput> {
        let revision = self.engine.output_revision();
        if self.outputs_revision != Some(revision) {
            let key_to_track = &self.key_to_track;
            self.outputs = self
                .engine
                .output_targets()
                .iter()
                .map(|target| {
                    let track = key_to_track.get(&target.key).unwrap_or(&target.key);
                    (target.key.clone(), track.clone())
                })
                .collect();
            self.outputs_revision = Some(revision);
        }
        let (dt, inputs) = self.step_inputs(dt_ns, time_ns);
        let values = self.engine.update_by_target(dt, inputs);
        values
            .iter()
            .zip(&self.outputs)
            .filter_map(|(value, (key, track))| {
                Some(TrackOutput {
                    track_id: track.clone(),
                    default_key: key.clone(),
                    value: value.clone()?,
                })
            })
            .collect()
    }

    /// [`step`](Self::step), returning the values alone, by their position
    /// in the table [`output_keys`](Self::output_keys) returns: no key or
    /// track id is built, copied or sent. A position no instance weighs on
    /// this step holds `Value::Unit`.
    pub fn step_values(&mut self, dt_ns: u64, time_ns: Option<u64>) -> StepValues {
        let revision = self.engine.output_revision();
        let (dt, inputs) = self.step_inputs(dt_ns, time_ns);
        let values = self.engine.update_by_target(dt, inputs);
        StepValues {
            revision,
            values: Value::ArrayValue(
                values
                    .iter()
                    .map(|value| value.clone().unwrap_or(Value::Unit))
                    .collect(),
            ),
        }
    }

    /// The key each position of [`step_values`](Self::step_values)' values
    /// is written to, and the table's revision. Only a structural edit
    /// changes the table: `add_instance`, `add_instance_with_weight`,
    /// `remove_instance`, `remove_player`, `unload_animation`,
    /// `reload_animation`; each that does gives a new revision.
    pub fn output_keys(&self) -> OutputKeys {
        OutputKeys {
            revision: self.engine.output_revision(),
            keys: self
                .engine
                .output_targets()
                .iter()
                .map(|target| target.key.clone())
                .collect(),
        }
    }

    /// The engine update a step of `dt_ns` ending at `time_ns` makes: its
    /// `dt` in seconds and the buffered transport, each `play_at` delay
    /// counted from the step's start.
    fn step_inputs(&mut self, dt_ns: u64, time_ns: Option<u64>) -> (f32, Inputs) {
        let now = time_ns.unwrap_or(self.clock_ns.saturating_add(dt_ns));
        let start = i128::from(now) - i128::from(dt_ns);
        let mut inputs = std::mem::take(&mut self.pending);
        for (index, at) in self.anchors.drain(..) {
            if let PlayerCommand::PlayAfter { delay, .. } = &mut inputs.player_cmds[index] {
                *delay = ((i128::from(at) - start) as f64 / 1e9) as f32;
            }
        }
        self.clock_ns = now;
        (ns_to_seconds(dt_ns), inputs)
    }
}

thread_local! {
    /// The module state the declared functions act on: in the wasm guest,
    /// one per wasm instance (its single thread); in a native build, one per
    /// thread. A host that links the crate as an rlib builds its own
    /// [`AnimationModule`] instead.
    static GUEST: UnsafeCell<AnimationModule> = UnsafeCell::new(AnimationModule::new());
}

/// Run `f` on the guest global.
///
/// The access takes no lock: a trap runs no destructor, so a lock the
/// trapping call held would stay held and fail every later call.
fn guest<T>(f: impl FnOnce(&mut AnimationModule) -> T) -> T {
    GUEST.with(|state| {
        // SAFETY: the state is thread-local, so no other thread reaches it,
        // and `f` is a declared function's body, an `AnimationModule` method
        // that never calls back into `guest`: this is the only reference to
        // the state while `f` runs, and none outlives the call.
        f(unsafe { &mut *state.get() })
    })
}

/// The module's interface: every function and parameter pinned by id, each
/// acting on the guest global.
#[arora_module::module(
    id = "76697a69-6a00-0000-0d00-000000000000",
    name = "vizij-animation",
    version = "2.0.0",
    author = "Semio",
    license = "Proprietary",
    description = "vizij-animation-core as an Arora wasm module",
    executable_mime = "application/wasm"
)]
pub mod animation {
    use super::{
        guest, AnimationClip, BakedAnimation, OutputKeys, PlayerState, StepValues, TrackOutput,
    };

    /// [`AnimationModule::load_animation`](super::AnimationModule::load_animation).
    #[export(id = "76697a69-6a00-0000-0f00-000000000001")]
    pub fn load_animation(
        #[param(id = "76697a69-6a00-0000-0f01-000000000001")] clip: AnimationClip,
    ) -> u32 {
        guest(|a| a.load_animation(clip))
    }

    /// [`AnimationModule::create_player`](super::AnimationModule::create_player).
    #[export(id = "76697a69-6a00-0000-0f00-000000000002")]
    pub fn create_player(
        #[param(id = "76697a69-6a00-0000-0f02-000000000001")] name: Option<String>,
    ) -> u32 {
        guest(|a| a.create_player(name))
    }

    /// [`AnimationModule::add_instance`](super::AnimationModule::add_instance).
    #[export(id = "76697a69-6a00-0000-0f00-000000000003")]
    pub fn add_instance(
        #[param(id = "76697a69-6a00-0000-0f03-000000000001")] player: u32,
        #[param(id = "76697a69-6a00-0000-0f03-000000000002")] anim: u32,
    ) -> u32 {
        guest(|a| a.add_instance(player, anim))
    }

    /// [`AnimationModule::step`](super::AnimationModule::step). `dt_ns` is the
    /// runtime's `arora/dt` built-in key and `time_ns` its `arora/time`.
    #[export(id = "76697a69-6a00-0000-0f00-000000000004")]
    pub fn step(
        #[param(id = "76697a69-6a00-0000-0f04-000000000001")] dt_ns: u64,
        #[param(id = "76697a69-6a00-0000-0f04-000000000002")] time_ns: Option<u64>,
    ) -> Vec<TrackOutput> {
        guest(|a| a.step(dt_ns, time_ns))
    }

    /// [`AnimationModule::play`](super::AnimationModule::play).
    #[export(id = "76697a69-6a00-0000-0f00-000000000005")]
    pub fn play(#[param(id = "76697a69-6a00-0000-0f05-000000000001")] player: u32) -> u32 {
        guest(|a| a.play(player))
    }

    /// [`AnimationModule::pause`](super::AnimationModule::pause).
    #[export(id = "76697a69-6a00-0000-0f00-000000000006")]
    pub fn pause(#[param(id = "76697a69-6a00-0000-0f06-000000000001")] player: u32) -> u32 {
        guest(|a| a.pause(player))
    }

    /// [`AnimationModule::stop`](super::AnimationModule::stop).
    #[export(id = "76697a69-6a00-0000-0f00-000000000007")]
    pub fn stop(#[param(id = "76697a69-6a00-0000-0f07-000000000001")] player: u32) -> u32 {
        guest(|a| a.stop(player))
    }

    /// [`AnimationModule::seek`](super::AnimationModule::seek).
    #[export(id = "76697a69-6a00-0000-0f00-000000000008")]
    pub fn seek(
        #[param(id = "76697a69-6a00-0000-0f08-000000000001")] player: u32,
        #[param(id = "76697a69-6a00-0000-0f08-000000000002")] time_ns: u64,
    ) -> u32 {
        guest(|a| a.seek(player, time_ns))
    }

    /// [`AnimationModule::set_speed`](super::AnimationModule::set_speed).
    #[export(id = "76697a69-6a00-0000-0f00-000000000009")]
    pub fn set_speed(
        #[param(id = "76697a69-6a00-0000-0f09-000000000001")] player: u32,
        #[param(id = "76697a69-6a00-0000-0f09-000000000002")] speed: f32,
    ) -> u32 {
        guest(|a| a.set_speed(player, speed))
    }

    /// [`AnimationModule::set_loop`](super::AnimationModule::set_loop).
    #[export(id = "76697a69-6a00-0000-0f00-00000000000a")]
    pub fn set_loop(
        #[param(id = "76697a69-6a00-0000-0f0a-000000000001")] player: u32,
        #[param(id = "76697a69-6a00-0000-0f0a-000000000002")] mode: String,
    ) -> u32 {
        guest(|a| a.set_loop(player, mode))
    }

    /// [`AnimationModule::set_weight`](super::AnimationModule::set_weight).
    #[export(id = "76697a69-6a00-0000-0f00-00000000000b")]
    pub fn set_weight(
        #[param(id = "76697a69-6a00-0000-0f0b-000000000001")] player: u32,
        #[param(id = "76697a69-6a00-0000-0f0b-000000000002")] instance: u32,
        #[param(id = "76697a69-6a00-0000-0f0b-000000000003")] weight: f32,
    ) -> u32 {
        guest(|a| a.set_weight(player, instance, weight))
    }

    /// [`AnimationModule::remove_instance`](super::AnimationModule::remove_instance).
    #[export(id = "76697a69-6a00-0000-0f00-00000000000c")]
    pub fn remove_instance(
        #[param(id = "76697a69-6a00-0000-0f0c-000000000001")] player: u32,
        #[param(id = "76697a69-6a00-0000-0f0c-000000000002")] instance: u32,
    ) -> u32 {
        guest(|a| a.remove_instance(player, instance))
    }

    /// [`AnimationModule::player_states`](super::AnimationModule::player_states).
    #[export(id = "76697a69-6a00-0000-0f00-00000000000d")]
    pub fn player_states() -> Vec<PlayerState> {
        guest(|a| a.player_states())
    }

    /// [`AnimationModule::bake`](super::AnimationModule::bake).
    #[export(id = "76697a69-6a00-0000-0f00-00000000000e")]
    pub fn bake(
        #[param(id = "76697a69-6a00-0000-0f0e-000000000001")] anim: u32,
        #[param(id = "76697a69-6a00-0000-0f0e-000000000002")] frame_rate: Option<f32>,
        #[param(id = "76697a69-6a00-0000-0f0e-000000000003")] start_ns: Option<u64>,
        #[param(id = "76697a69-6a00-0000-0f0e-000000000004")] end_ns: Option<u64>,
    ) -> Option<BakedAnimation> {
        guest(|a| a.bake(anim, frame_rate, start_ns, end_ns))
    }

    /// [`AnimationModule::bake_with_derivatives`](super::AnimationModule::bake_with_derivatives).
    #[export(id = "76697a69-6a00-0000-0f00-00000000000f")]
    pub fn bake_with_derivatives(
        #[param(id = "76697a69-6a00-0000-0f0f-000000000001")] anim: u32,
        #[param(id = "76697a69-6a00-0000-0f0f-000000000002")] frame_rate: Option<f32>,
        #[param(id = "76697a69-6a00-0000-0f0f-000000000003")] start_ns: Option<u64>,
        #[param(id = "76697a69-6a00-0000-0f0f-000000000004")] end_ns: Option<u64>,
    ) -> Option<BakedAnimation> {
        guest(|a| a.bake_with_derivatives(anim, frame_rate, start_ns, end_ns))
    }

    /// [`AnimationModule::unload_animation`](super::AnimationModule::unload_animation).
    #[export(id = "76697a69-6a00-0000-0f00-000000000010")]
    pub fn unload_animation(
        #[param(id = "76697a69-6a00-0000-0f10-000000000001")] anim: u32,
    ) -> bool {
        guest(|a| a.unload_animation(anim))
    }

    /// [`AnimationModule::remove_player`](super::AnimationModule::remove_player).
    #[export(id = "76697a69-6a00-0000-0f00-000000000011")]
    pub fn remove_player(
        #[param(id = "76697a69-6a00-0000-0f11-000000000001")] player: u32,
    ) -> bool {
        guest(|a| a.remove_player(player))
    }

    /// [`AnimationModule::add_instance_with_weight`](super::AnimationModule::add_instance_with_weight).
    #[export(id = "76697a69-6a00-0000-0f00-000000000012")]
    pub fn add_instance_with_weight(
        #[param(id = "76697a69-6a00-0000-0f12-000000000001")] player: u32,
        #[param(id = "76697a69-6a00-0000-0f12-000000000002")] anim: u32,
        #[param(id = "76697a69-6a00-0000-0f12-000000000003")] weight: f32,
    ) -> u32 {
        guest(|a| a.add_instance_with_weight(player, anim, weight))
    }

    /// [`AnimationModule::reload_animation`](super::AnimationModule::reload_animation).
    #[export(id = "76697a69-6a00-0000-0f00-000000000013")]
    pub fn reload_animation(
        #[param(id = "76697a69-6a00-0000-0f13-000000000001")] anim: u32,
        #[param(id = "76697a69-6a00-0000-0f13-000000000002")] clip: AnimationClip,
    ) -> bool {
        guest(|a| a.reload_animation(anim, clip))
    }

    /// [`AnimationModule::set_window`](super::AnimationModule::set_window).
    #[export(id = "76697a69-6a00-0000-0f00-000000000014")]
    pub fn set_window(
        #[param(id = "76697a69-6a00-0000-0f14-000000000001")] player: u32,
        #[param(id = "76697a69-6a00-0000-0f14-000000000002")] start_ns: u64,
        #[param(id = "76697a69-6a00-0000-0f14-000000000003")] end_ns: Option<u64>,
    ) -> u32 {
        guest(|a| a.set_window(player, start_ns, end_ns))
    }

    /// [`AnimationModule::play_at`](super::AnimationModule::play_at).
    #[export(id = "76697a69-6a00-0000-0f00-000000000015")]
    pub fn play_at(
        #[param(id = "76697a69-6a00-0000-0f15-000000000001")] player: u32,
        #[param(id = "76697a69-6a00-0000-0f15-000000000002")] time_ns: u64,
    ) -> u32 {
        guest(|a| a.play_at(player, time_ns))
    }

    /// [`AnimationModule::set_start_offset`](super::AnimationModule::set_start_offset).
    #[export(id = "76697a69-6a00-0000-0f00-000000000016")]
    pub fn set_start_offset(
        #[param(id = "76697a69-6a00-0000-0f16-000000000001")] player: u32,
        #[param(id = "76697a69-6a00-0000-0f16-000000000002")] instance: u32,
        #[param(id = "76697a69-6a00-0000-0f16-000000000003")] offset_ns: i64,
    ) -> u32 {
        guest(|a| a.set_start_offset(player, instance, offset_ns))
    }

    /// [`AnimationModule::set_time_scale`](super::AnimationModule::set_time_scale).
    #[export(id = "76697a69-6a00-0000-0f00-000000000017")]
    pub fn set_time_scale(
        #[param(id = "76697a69-6a00-0000-0f17-000000000001")] player: u32,
        #[param(id = "76697a69-6a00-0000-0f17-000000000002")] instance: u32,
        #[param(id = "76697a69-6a00-0000-0f17-000000000003")] time_scale: f32,
    ) -> u32 {
        guest(|a| a.set_time_scale(player, instance, time_scale))
    }

    /// [`AnimationModule::step_values`](super::AnimationModule::step_values).
    /// `dt_ns` is the runtime's `arora/dt` built-in key and `time_ns` its
    /// `arora/time`.
    #[export(id = "76697a69-6a00-0000-0f00-000000000018")]
    pub fn step_values(
        #[param(id = "76697a69-6a00-0000-0f18-000000000001")] dt_ns: u64,
        #[param(id = "76697a69-6a00-0000-0f18-000000000002")] time_ns: Option<u64>,
    ) -> StepValues {
        guest(|a| a.step_values(dt_ns, time_ns))
    }

    /// [`AnimationModule::output_keys`](super::AnimationModule::output_keys).
    #[export(id = "76697a69-6a00-0000-0f00-000000000019")]
    pub fn output_keys() -> OutputKeys {
        guest(|a| a.output_keys())
    }
}

// baking ---------------------------------------------------------------------

/// Build a [`BakingConfig`] from the module's optional arguments, falling
/// back to the core defaults (frame rate 60 Hz, start 0 s, end = clip
/// duration) for any argument left unset.
fn baking_config(
    frame_rate: Option<f32>,
    start_ns: Option<u64>,
    end_ns: Option<u64>,
) -> BakingConfig {
    let defaults = BakingConfig::default();
    BakingConfig {
        frame_rate: frame_rate.unwrap_or(defaults.frame_rate),
        start_time: start_ns.map_or(defaults.start_time, ns_to_seconds),
        end_time: end_ns.map(ns_to_seconds).or(defaults.end_time),
        derivative_epsilon: defaults.derivative_epsilon,
    }
}

/// An [`AnimationClip`] as the core's animation data, with its tracks as
/// (canonical output key, authored track id). The keyframe `value`s arrive as
/// raw Arora `Value`s (vizij-arora encoding) and are converted back to Vizij
/// values for the core model.
fn to_core_animation(clip: AnimationClip) -> (AnimationData, Vec<(String, String)>) {
    let keys = clip
        .tracks
        .iter()
        .map(|t| (t.animatable_id.clone(), t.id.clone()))
        .collect();
    let tracks = clip
        .tracks
        .into_iter()
        .map(|t| CoreTrack {
            id: t.id,
            name: t.name,
            animatable_id: t.animatable_id,
            points: t.points.into_iter().map(to_core_keypoint).collect(),
            settings: None,
        })
        .collect();
    let data = AnimationData {
        id: None,
        name: clip.name,
        tracks,
        groups: Default::default(),
        duration_ms: clip.duration,
    };
    (data, keys)
}

/// The core's bake as the module returns it, over the window `bake_window`
/// gives in nanoseconds: each track's samples as one array value, with
/// `derivatives`' when the bake took them (one track per track of `baked`, in
/// its order).
fn baked_animation(
    baked: BakedAnimationData,
    derivatives: Option<BakedDerivativeAnimationData>,
    (start_ns, end_ns): (u64, u64),
) -> BakedAnimation {
    let mut derivatives = derivatives.map(|d| d.tracks.into_iter());
    BakedAnimation {
        frame_rate: baked.frame_rate,
        start_ns,
        end_ns,
        tracks: baked
            .tracks
            .into_iter()
            .map(|track| BakedTrack {
                animatable_id: track.target_path,
                values: Value::ArrayValue(track.values),
                derivatives: Value::ArrayValue(
                    derivatives
                        .as_mut()
                        .and_then(Iterator::next)
                        .map(|d| {
                            d.values
                                .into_iter()
                                .map(|sample| Value::Option(sample.map(Box::new)))
                                .collect()
                        })
                        .unwrap_or_default(),
                ),
            })
            .collect(),
    }
}

/// Whether `weight` is a blend weight: finite and not negative.
fn valid_weight(weight: f32) -> bool {
    weight.is_finite() && weight >= 0.0
}

fn seconds_to_ns(seconds: f32) -> u64 {
    (seconds.max(0.0) as f64 * 1e9).round() as u64
}

fn ns_to_seconds(ns: u64) -> f32 {
    (ns as f64 / 1e9) as f32
}

/// An update of `instance` on `player` that changes nothing.
fn instance_update(player: u32, instance: u32) -> InstanceUpdate {
    InstanceUpdate {
        player: PlayerId(player),
        inst: InstId(instance),
        weight: None,
        time_scale: None,
        start_offset: None,
        enabled: None,
    }
}

/// Convert a boundary keyframe (dynamic Arora `Value`) into a core keyframe:
/// the kernel decodes the shared `Value` into its POD `TrackValue` once, at
/// ingestion. The transition handle arrays (zero or one element each) become
/// the core's optional cubic-bezier timing handles.
fn to_core_keypoint(kp: Keypoint) -> CoreKeypoint {
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
    //! Exercises the module's functions on an [`AnimationModule`] of the test's own
    //! (native), the way a host does — bypassing the buffer ABI. This proves
    //! the clip mapping, the transport buffering, and the per-track output
    //! contract. The equivalent end-to-end path through a real wasm engine
    //! lives in `tests/host_ramp.rs`.

    use super::*;
    use arora_types::value::Value as AValue;

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

    fn keypoint(id: &str, stamp: f32, v: f32) -> Keypoint {
        Keypoint {
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

    fn state_of(animation: &AnimationModule, player: u32) -> PlayerState {
        animation
            .player_states()
            .into_iter()
            .find(|s| s.player == player)
            .expect("player state")
    }

    #[test]
    fn ramp_advances_and_carries_the_authored_key() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(ramp_clip("ease-ramp", "ease/x", false));
        let player = a.create_player(Some("p-ease".into()));
        let inst = a.add_instance(player, anim);
        assert_ne!(inst, u32::MAX);

        // The clip eases (default S-curve), so it is antisymmetric about the
        // midpoint: at t = 0.5 s (half of the 1 s clip) the value is ~0.5, and it
        // advances monotonically toward it.
        let first = a.step(250_000_000, None); // t = 0.25 s
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

        let second = a.step(250_000_000, None); // t = 0.5 s
        let v1 = as_f32(value_of(&second, "ease/x").expect("ease/x output"));
        assert!(v1 > v0, "expected monotonic advance, {v1} !> {v0}");
        assert!(
            (v1 - 0.5).abs() < 1e-3,
            "expected ~0.5 at t=0.5 s, got {v1}"
        );
    }

    /// An instance added at weight 0 is silent until weighted.
    #[test]
    fn an_instance_added_at_weight_zero_writes_nothing_until_weighted() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(constant_clip("quiet", "x", 0.25));
        let player = a.create_player(Some("quiet".into()));
        let inst = a.add_instance_with_weight(player, anim, 0.0);
        assert!(a.step(100_000_000, None).is_empty(), "silent at weight 0");
        a.set_weight(player, inst, 1.0);
        let out = a.step(0, None);
        assert_eq!(as_f32(value_of(&out, "x").expect("x output")), 0.25);
    }

    /// A player's state names it, says its loop mode and lists its
    /// instances, each with the animation it plays and its weight; an
    /// unnamed player's name is empty.
    #[test]
    fn player_states_name_each_player_and_list_its_instances() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(constant_clip("wave", "x", 0.5));
        let player = a.create_player(Some("wave".into()));
        let inst = a.add_instance_with_weight(player, anim, 0.0);
        let unnamed = a.create_player(None);

        let states = a.player_states();
        assert_eq!(states.len(), 2);
        let wave = &states[0];
        assert_eq!((wave.player, wave.name.as_str()), (player, "wave"));
        assert_eq!(
            wave.instances,
            vec![InstanceState {
                instance: inst,
                anim,
                weight: 0.0,
                start_offset_ns: 0,
                time_scale: 1.0,
            }]
        );
        assert_eq!(wave.duration_ns, 1_000_000_000);
        assert_eq!((states[1].player, states[1].name.as_str()), (unnamed, ""));
        assert!(states[1].instances.is_empty());

        assert_eq!(wave.loop_mode, "loop", "a player loops from its creation");

        a.set_weight(player, inst, 0.5);
        a.set_loop(player, "ping_pong".into());
        a.step(0, None);
        let wave = state_of(&a, player);
        assert_eq!(wave.instances[0].weight, 0.5);
        assert_eq!(wave.loop_mode, "ping_pong");
    }

    /// `PlayerState` crosses the value plane as its 1.2.0 record: a structure
    /// under its id, its instances an array of `InstanceState` structures.
    #[test]
    fn a_player_state_round_trips_through_its_record() {
        let state = PlayerState {
            player: 3,
            state: "paused".into(),
            time_ns: 250_000_000,
            duration_ns: 1_000_000_000,
            speed: 0.0,
            name: "wave".into(),
            instances: vec![InstanceState {
                instance: 7,
                anim: 2,
                weight: 1.0,
                start_offset_ns: -250_000_000,
                time_scale: 2.0,
            }],
            loop_mode: "once".into(),
            ended: true,
            window_start_ns: 100_000_000,
            window_end_ns: Some(900_000_000),
        };
        assert_eq!(PlayerState::arora_type_version().to_string(), "1.2.0");
        assert_eq!(InstanceState::arora_type_version().to_string(), "1.1.0");
        let value = Value::from(state.clone());
        let Value::Structure(structure) = &value else {
            panic!("a structure, got {value:?}");
        };
        assert_eq!(structure.id, PlayerState::arora_type_id());
        assert_eq!(structure.fields.len(), 11);
        assert_eq!(PlayerState::try_from(value).expect("decodes"), state);
    }

    /// Removing a player takes its instances and leaves the animation loaded
    /// for another player; unloading an animation takes every instance of it.
    #[test]
    fn removing_a_player_and_unloading_an_animation() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(constant_clip("shared", "x", 0.75));
        let first = a.create_player(Some("first".into()));
        a.add_instance(first, anim);
        let second = a.create_player(Some("second".into()));
        a.add_instance(second, anim);
        a.pause(first); // buffered for a player about to go

        assert!(a.remove_player(first));
        assert!(!a.remove_player(first), "already gone");
        let out = a.step(0, None);
        assert_eq!(
            as_f32(value_of(&out, "x").expect("the other player still writes")),
            0.75
        );
        let names: Vec<String> = a.player_states().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["second"]);

        assert!(a.unload_animation(anim));
        assert!(!a.unload_animation(anim), "already unloaded");
        assert!(a.step(0, None).is_empty(), "no animation, no output");
        assert!(
            state_of(&a, second).instances.is_empty(),
            "the player stays, without the instance"
        );
        assert!(a.bake(anim, None, None, None).is_none());
    }

    #[test]
    fn transitions_ride_through_to_sampling() {
        let mut a = AnimationModule::new();
        // Linear handles: value == normalized time, exactly.
        let anim = a.load_animation(ramp_clip("lin-ramp", "lin/x", true));
        let player = a.create_player(Some("p-lin".into()));
        a.add_instance(player, anim);

        let outputs = a.step(250_000_000, None);
        let v = as_f32(value_of(&outputs, "lin/x").expect("lin/x output"));
        assert!(
            (v - 0.25).abs() < 1e-3,
            "linear transitions sample the identity, got {v} at u=0.25"
        );

        // A slow-out handle holds the curve low early on: strictly below linear.
        let mut slow = ramp_clip("slow-ramp", "slow/x", false);
        slow.tracks[0].points[0].transitions_out = vec![TransitionHandle { x: 1.0, y: 0.0 }];
        let anim = a.load_animation(slow);
        let player = a.create_player(Some("p-slow".into()));
        a.add_instance(player, anim);

        let outputs = a.step(250_000_000, None);
        let v_slow = as_f32(value_of(&outputs, "slow/x").expect("slow/x output"));
        assert!(
            v_slow < 0.25 - 1e-3,
            "slow-out handles must undershoot linear at u=0.25, got {v_slow}"
        );
    }

    #[test]
    fn transport_commands_apply_at_the_next_step() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(ramp_clip("tr-ramp", "tr/x", true));
        let player = a.create_player(Some("p-transport".into()));
        a.add_instance(player, anim);

        // Advance to 0.25 s.
        let outputs = a.step(250_000_000, None);
        assert!((as_f32(value_of(&outputs, "tr/x").unwrap()) - 0.25).abs() < 1e-3);
        let s = state_of(&a, player);
        assert_eq!(s.state, "playing");
        assert_eq!(s.duration_ns, 1_000_000_000, "1 s clip length");
        assert!((s.time_ns as f64 - 0.25e9).abs() < 2e6, "playhead ~0.25 s");

        // pause: the playhead holds through further steps.
        assert_eq!(a.pause(player), player);
        a.step(250_000_000, None);
        let s = state_of(&a, player);
        assert_eq!(s.state, "paused");
        assert!(
            (s.time_ns as f64 - 0.25e9).abs() < 2e6,
            "paused playhead holds"
        );

        // play resumes from where it held.
        assert_eq!(a.play(player), player);
        let outputs = a.step(250_000_000, None);
        assert!((as_f32(value_of(&outputs, "tr/x").unwrap()) - 0.5).abs() < 1e-3);

        // seek lands exactly (u64 nanoseconds in).
        assert_eq!(a.seek(player, 100_000_000), player);
        let outputs = a.step(0, None);
        assert!((as_f32(value_of(&outputs, "tr/x").unwrap()) - 0.1).abs() < 1e-3);

        // set_speed scales dt: 0.2 s of wall clock at 2x advances 0.4 s.
        assert_eq!(a.set_speed(player, 2.0), player);
        let outputs = a.step(200_000_000, None);
        assert!((as_f32(value_of(&outputs, "tr/x").unwrap()) - 0.5).abs() < 1e-3);

        // stop resets to the window start.
        assert_eq!(a.stop(player), player);
        a.step(0, None);
        let s = state_of(&a, player);
        assert_eq!(s.state, "stopped");
        assert_eq!(s.time_ns, 0);
    }

    /// `play`, `pause` and `stop` say whether time advances, `set_speed` how
    /// fast: the speed a client set survives a pause and a stop, and setting
    /// it on a paused player does not resume it.
    #[test]
    fn the_speed_survives_pause_and_play() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(ramp_clip("speed-ramp", "speed/x", true));
        let player = a.create_player(Some("p-speed".into()));
        a.add_instance(player, anim);

        a.set_speed(player, 0.5);
        a.step(500_000_000, None); // 0.25 s at half speed
        a.pause(player);
        a.step(500_000_000, None);
        let s = state_of(&a, player);
        assert_eq!((s.state.as_str(), s.speed), ("paused", 0.5));
        assert!(
            (s.time_ns as f64 - 0.25e9).abs() < 2e6,
            "paused playhead holds"
        );

        a.set_speed(player, 0.25);
        a.step(500_000_000, None);
        let s = state_of(&a, player);
        assert_eq!(
            (s.state.as_str(), s.speed),
            ("paused", 0.25),
            "still paused"
        );
        assert!((s.time_ns as f64 - 0.25e9).abs() < 2e6);

        a.play(player);
        let outputs = a.step(400_000_000, None); // + 0.1 s at quarter speed
        assert!((as_f32(value_of(&outputs, "speed/x").unwrap()) - 0.35).abs() < 1e-3);
        assert_eq!(state_of(&a, player).speed, 0.25);

        a.stop(player);
        a.step(0, None);
        a.play(player);
        a.step(400_000_000, None);
        let s = state_of(&a, player);
        assert_eq!((s.state.as_str(), s.speed), ("playing", 0.25));
        assert!(
            (s.time_ns as f64 - 0.1e9).abs() < 2e6,
            "replays at its speed"
        );
    }

    /// Reloading an animation swaps its tracks under the same id: the player
    /// keeps its playback and the instance its weight, a track the new
    /// animation adds writes from the next step, and outputs name the new
    /// tracks.
    #[test]
    fn reload_animation_keeps_the_playback() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(ramp_clip("re-ramp", "re/x", true));
        let player = a.create_player(Some("p-reload".into()));
        let inst = a.add_instance_with_weight(player, anim, 0.5);
        a.set_speed(player, 0.5);
        a.step(500_000_000, None); // playhead 0.25 s
        a.pause(player);
        a.step(0, None);

        let mut clip = ramp_clip("re-ramp", "re/x", true);
        clip.duration = 2000;
        clip.tracks[0].id = "reloaded-x".into();
        clip.tracks
            .push(constant_clip("added", "re/y", 0.75).tracks.remove(0));
        assert!(a.reload_animation(anim, clip));
        assert!(
            !a.reload_animation(anim + 1, constant_clip("none", "z", 0.0)),
            "an animation not loaded is not reloaded"
        );

        let s = state_of(&a, player);
        assert_eq!((s.state.as_str(), s.speed), ("paused", 0.5));
        assert!(
            (s.time_ns as f64 - 0.25e9).abs() < 2e6,
            "the playhead holds"
        );
        assert_eq!(s.duration_ns, 2_000_000_000, "the new duration");
        assert_eq!(
            s.instances,
            vec![InstanceState {
                instance: inst,
                anim,
                weight: 0.5,
                start_offset_ns: 0,
                time_scale: 1.0,
            }]
        );

        let outputs = a.step(0, None);
        let y = outputs
            .iter()
            .find(|o| o.default_key == "re/y")
            .expect("the added track writes");
        assert_eq!((y.track_id.as_str(), as_f32(&y.value)), ("added-t0", 0.75));
        let x = outputs.iter().find(|o| o.default_key == "re/x").unwrap();
        assert_eq!(x.track_id, "reloaded-x");
        // 0.25 s into the 2 s linear ramp.
        assert!((as_f32(&x.value) - 0.125).abs() < 1e-3);
    }

    #[test]
    fn loop_once_clamps_at_the_clip_end() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(ramp_clip("once-ramp", "once/x", true));
        let player = a.create_player(Some("p-once".into()));
        a.add_instance(player, anim);

        assert_eq!(a.set_loop(player, "once".into()), player);
        assert_eq!(a.seek(player, 900_000_000), player);
        a.step(0, None);
        a.step(300_000_000, None); // 0.9 s + 0.3 s, clamped to the 1 s end
        let s = state_of(&a, player);
        assert_eq!(s.time_ns, 1_000_000_000, "Once clamps at the clip end");

        assert_eq!(
            a.set_loop(player, "sideways".into()),
            u32::MAX,
            "unknown loop modes are rejected"
        );
    }

    #[test]
    fn weights_skew_the_blend_and_removal_silences_the_key() {
        let mut a = AnimationModule::new();
        let zero = a.load_animation(constant_clip("mix-zero", "mix/x", 0.0));
        let one = a.load_animation(constant_clip("mix-one", "mix/x", 1.0));
        let player = a.create_player(Some("p-mix".into()));
        let inst_zero = a.add_instance(player, zero);
        let inst_one = a.add_instance(player, one);

        // Equal weights: the normalized blend of 0 and 1.
        let outputs = a.step(100_000_000, None);
        assert!((as_f32(value_of(&outputs, "mix/x").unwrap()) - 0.5).abs() < 1e-3);

        // Silencing the zero-instance leaves only the one-instance.
        assert_eq!(a.set_weight(player, inst_zero, 0.0), inst_zero);
        let outputs = a.step(100_000_000, None);
        assert!((as_f32(value_of(&outputs, "mix/x").unwrap()) - 1.0).abs() < 1e-3);

        // Removing both instances stops the key from being emitted at all.
        assert_eq!(a.remove_instance(player, inst_zero), 1);
        assert_eq!(a.remove_instance(player, inst_one), 1);
        assert_eq!(a.remove_instance(player, inst_one), 0, "already gone");
        let outputs = a.step(100_000_000, None);
        assert!(
            value_of(&outputs, "mix/x").is_none(),
            "no instances, no output for the key"
        );
    }

    fn near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-3,
            "expected {expected}, got {actual}"
        );
    }

    fn near_ns(actual: u64, expected: u64) {
        assert!(
            actual.abs_diff(expected) < 1_000,
            "expected {expected} ns, got {actual}"
        );
    }

    /// A window bounds `once`, a negative speed plays backwards through it,
    /// and the state says when the player has ended and where its window is.
    #[test]
    fn a_window_and_a_negative_speed_bound_playback_and_report_its_end() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(ramp_clip("win", "win/x", true));
        let player = a.create_player(Some("win".into()));
        a.add_instance(player, anim);
        a.set_loop(player, "once".into());
        assert_eq!(a.set_window(player, 200_000_000, Some(800_000_000)), player);
        a.seek(player, 700_000_000);
        let out = a.step(200_000_000, None); // 0.7 s + 0.2 s, held at 0.8 s
        near(as_f32(value_of(&out, "win/x").unwrap()), 0.8);
        let s = state_of(&a, player);
        assert!(s.ended, "held at the window end");
        assert_eq!(s.state, "playing");
        near_ns(s.window_start_ns, 200_000_000);
        near_ns(s.window_end_ns.expect("an explicit end"), 800_000_000);
        assert_eq!(s.duration_ns, 1_000_000_000, "a window leaves the length");

        assert_eq!(a.set_speed(player, -1.0), player);
        let out = a.step(300_000_000, None);
        near(as_f32(value_of(&out, "win/x").unwrap()), 0.5);
        let s = state_of(&a, player);
        assert!(!s.ended);
        near(s.speed, -1.0);
        a.step(1_000_000_000, None);
        let s = state_of(&a, player);
        assert!(s.ended, "held at the window start");
        near_ns(s.time_ns, 200_000_000);

        // `loop` wraps backwards from the window start to its end.
        a.set_loop(player, "loop".into());
        let out = a.step(100_000_000, None);
        near(as_f32(value_of(&out, "win/x").unwrap()), 0.7);

        a.set_window(player, 0, None);
        a.step(0, None);
        let s = state_of(&a, player);
        assert_eq!((s.window_start_ns, s.window_end_ns), (0, None));
    }

    /// Loads stepping at different rates, anchored at one `arora/time`, stand
    /// at the playhead the time since the anchor gives, whichever steps the
    /// anchor falls between.
    #[test]
    fn play_at_anchors_the_start_to_the_step_time() {
        for step_ns in [100_000_000u64, 140_000_000, 175_000_000] {
            let mut a = AnimationModule::new();
            let anim = a.load_animation(ramp_clip("sync", "sync/x", true));
            let player = a.create_player(None);
            a.add_instance(player, anim);
            a.stop(player);
            // The device's clock reads 5 s at the module's first step.
            let mut now = 5_000_000_000u64;
            a.step(0, Some(now));
            assert_eq!(a.play_at(player, 5_300_000_000), player);
            let mut out = Vec::new();
            while now < 5_700_000_000 {
                now += step_ns;
                out = a.step(step_ns, Some(now));
            }
            assert_eq!(now, 5_700_000_000);
            near(as_f32(value_of(&out, "sync/x").unwrap()), 0.4);
            near_ns(state_of(&a, player).time_ns, 400_000_000);
        }
    }

    /// An anchor on a step boundary starts the player at that boundary; one
    /// already past starts it there, the next step catching up; without
    /// `time_ns`, the time base is the sum of the steps.
    #[test]
    fn play_at_on_a_boundary_in_the_past_and_without_a_step_time() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(ramp_clip("b", "b/x", true));
        let player = a.create_player(None);
        a.add_instance(player, anim);
        a.stop(player);
        a.play_at(player, 200_000_000);
        a.step(100_000_000, Some(100_000_000));
        assert_eq!(state_of(&a, player).state, "stopped", "holds until 0.2 s");
        a.step(100_000_000, Some(200_000_000));
        let s = state_of(&a, player);
        assert_eq!((s.state.as_str(), s.time_ns), ("playing", 0));
        a.step(100_000_000, Some(300_000_000));
        near_ns(state_of(&a, player).time_ns, 100_000_000);

        a.stop(player);
        a.play_at(player, 100_000_000);
        a.step(100_000_000, Some(400_000_000));
        near_ns(state_of(&a, player).time_ns, 300_000_000);

        let mut b = AnimationModule::new();
        let anim = b.load_animation(ramp_clip("c", "c/x", true));
        let player = b.create_player(None);
        b.add_instance(player, anim);
        b.stop(player);
        b.step(100_000_000, None);
        b.play_at(player, 150_000_000);
        b.step(100_000_000, None);
        near_ns(state_of(&b, player).time_ns, 50_000_000);
    }

    /// An instance's start offset and time scale apply to it alone and read
    /// back from its state; a time scale that is not finite and positive is
    /// rejected.
    #[test]
    fn instance_offset_and_time_scale_apply_per_instance() {
        let mut a = AnimationModule::new();
        let slow = a.load_animation(ramp_clip("slow", "slow/x", true));
        let plain = a.load_animation(ramp_clip("plain", "plain/x", true));
        let player = a.create_player(None);
        let shifted = a.add_instance(player, slow);
        a.add_instance(player, plain);
        assert_eq!(a.set_start_offset(player, shifted, 250_000_000), shifted);
        assert_eq!(a.set_time_scale(player, shifted, 2.0), shifted);
        let out = a.step(750_000_000, None);
        near(as_f32(value_of(&out, "slow/x").unwrap()), 0.25); // (0.75 - 0.25) / 2
        near(as_f32(value_of(&out, "plain/x").unwrap()), 0.75);

        let s = state_of(&a, player);
        assert_eq!(s.duration_ns, 2_250_000_000, "0.25 s + 2 x 1 s");
        let instance = &s.instances[0];
        assert_eq!(
            (instance.start_offset_ns, instance.time_scale),
            (250_000_000, 2.0)
        );
        assert_eq!(
            (s.instances[1].start_offset_ns, s.instances[1].time_scale),
            (0, 1.0)
        );

        for rejected in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(a.set_time_scale(player, shifted, rejected), u32::MAX);
        }
    }

    /// A speed that is not finite and a weight that is not finite and
    /// non-negative are rejected before they reach the engine: the playhead
    /// and the blend stay what they were.
    #[test]
    fn a_speed_or_a_weight_out_of_range_is_rejected() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(ramp_clip("guard", "guard/x", true));
        let player = a.create_player(None);
        let inst = a.add_instance(player, anim);
        for speed in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(a.set_speed(player, speed), u32::MAX, "speed {speed}");
        }
        for weight in [f32::NAN, f32::INFINITY, -0.5] {
            assert_eq!(
                a.set_weight(player, inst, weight),
                u32::MAX,
                "weight {weight}"
            );
            assert_eq!(
                a.add_instance_with_weight(player, anim, weight),
                u32::MAX,
                "weight {weight}"
            );
        }
        let out = a.step(250_000_000, None);
        near(as_f32(value_of(&out, "guard/x").unwrap()), 0.25);
        let s = state_of(&a, player);
        assert_eq!((s.speed, s.instances.len()), (1.0, 1));
        assert_eq!(s.instances[0].weight, 1.0);
        assert_eq!(
            a.set_weight(player, inst, 0.0),
            inst,
            "0 silences, it is a weight"
        );
    }

    /// A bake comes back as a `BakedAnimation`: the effective rate and
    /// window in nanoseconds, and per track its key and its samples, the
    /// derivatives only when asked for.
    #[test]
    fn bake_returns_typed_samples() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(ramp_clip("bake-me", "joint/x", true));

        let baked = a
            .bake(anim, Some(4.0), Some(250_000_000), None)
            .expect("a loaded animation bakes");
        assert_eq!(baked.frame_rate, 4.0);
        assert_eq!((baked.start_ns, baked.end_ns), (250_000_000, 1_000_000_000));
        assert_eq!(baked.tracks.len(), 1);
        let track = &baked.tracks[0];
        assert_eq!(track.animatable_id, "joint/x");
        let Value::ArrayValue(values) = &track.values else {
            panic!("an array value, got {:?}", track.values);
        };
        let samples: Vec<f32> = values.iter().map(as_f32).collect();
        assert_eq!(samples.len(), 4, "0.25 s to 1 s at 4 Hz, both ends");
        for (sample, expected) in samples.iter().zip([0.25, 0.5, 0.75, 1.0]) {
            near(*sample, expected);
        }
        assert_eq!(track.derivatives, Value::ArrayValue(Vec::new()));

        let with = a
            .bake_with_derivatives(anim, Some(4.0), Some(250_000_000), None)
            .expect("a loaded animation bakes");
        assert_eq!(with.tracks[0].values, track.values);
        let Value::ArrayValue(derivatives) = &with.tracks[0].derivatives else {
            panic!("an array value");
        };
        assert_eq!(derivatives.len(), 4);
        let Value::Option(Some(slope)) = &derivatives[1] else {
            panic!("a derivative at 0.5 s, got {:?}", derivatives[1]);
        };
        near(as_f32(slope), 1.0);

        let value = Value::from(baked.clone());
        assert_eq!(BakedAnimation::try_from(value).expect("decodes"), baked);

        // The window reads back as requested, whatever f32 makes of it.
        let window = a
            .bake(anim, Some(10.0), Some(100_000_000), Some(1_234_000_000))
            .expect("a loaded animation bakes");
        assert_eq!(
            (window.start_ns, window.end_ns),
            (100_000_000, 1_000_000_000)
        );
        assert!(a.bake(u32::MAX, None, None, None).is_none());
    }

    /// A clip of four number tracks: `w/f32` ramps 0 to 1 over `F32`
    /// keypoints, `w/f64` the same over `F64` keypoints, `w/held` holds
    /// `F64` 0.1, a double no `f32` holds, and `w/mixed` ramps from an `F32`
    /// keypoint to an `F64` one.
    fn widths_clip() -> AnimationClip {
        let (r#in, out) = linear_handles();
        let ramp = |name: &str, k0: AValue, k1: AValue| AnimTrack {
            id: format!("{name}-t"),
            name: name.into(),
            animatable_id: format!("w/{name}"),
            points: vec![
                Keypoint {
                    value: k0,
                    transitions_out: out.clone(),
                    ..keypoint("k0", 0.0, 0.0)
                },
                Keypoint {
                    value: k1,
                    transitions_in: r#in.clone(),
                    ..keypoint("k1", 1.0, 0.0)
                },
            ],
        };
        let held = AnimTrack {
            id: "held-t".into(),
            name: "held".into(),
            animatable_id: "w/held".into(),
            points: vec![Keypoint {
                value: AValue::F64(0.1),
                ..keypoint("k0", 0.0, 0.0)
            }],
        };
        AnimationClip {
            name: "widths".into(),
            duration: 1000,
            tracks: vec![
                ramp("f32", AValue::F32(0.0), AValue::F32(1.0)),
                ramp("f64", AValue::F64(0.0), AValue::F64(1.0)),
                held,
                ramp("mixed", AValue::F32(0.0), AValue::F64(1.0)),
            ],
        }
    }

    /// Whether `value` is a number of the width `wide` names: `F64` when
    /// set, `F32` when not.
    fn is_width(value: &AValue, wide: bool) -> bool {
        matches!(
            (value, wide),
            (AValue::F64(_), true) | (AValue::F32(_), false)
        )
    }

    /// A number track's values come out in the width of its keypoints —
    /// `F32` from `F32` keypoints, `F64` from `F64` ones, a double at full
    /// precision, and `F64` throughout from a track with any `F64` keypoint
    /// — through `step`, `step_values` and both bakes.
    #[test]
    fn a_number_track_outputs_in_the_width_of_its_keypoints() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(widths_clip());
        let player = a.create_player(None);
        a.add_instance(player, anim);
        assert_eq!(
            a.output_keys().keys,
            ["w/f32", "w/f64", "w/held", "w/mixed"]
        );

        let Value::ArrayValue(values) = a.step_values(0, None).values else {
            panic!("an array value");
        };
        assert!(
            matches!(
                values[..],
                [
                    AValue::F32(_),
                    AValue::F64(_),
                    AValue::F64(_),
                    AValue::F64(_)
                ]
            ),
            "at the F32 keypoint of the mixed track too: {values:?}"
        );
        let Value::ArrayValue(values) = a.step_values(500_000_000, None).values else {
            panic!("an array value");
        };
        near(as_f32(&values[0]), 0.5);
        let AValue::F64(v) = values[1] else {
            panic!("an F64 track gives F64, got {:?}", values[1]);
        };
        assert!((v - 0.5).abs() < 1e-3, "~0.5 at the midpoint, got {v}");
        assert_eq!(values[2], AValue::F64(0.1));

        let records = a.step(0, None);
        for (key, wide) in [
            ("w/f32", false),
            ("w/f64", true),
            ("w/held", true),
            ("w/mixed", true),
        ] {
            let value = value_of(&records, key).expect("written");
            assert!(is_width(value, wide), "{key}: {value:?}");
        }
        assert_eq!(value_of(&records, "w/held"), Some(&AValue::F64(0.1)));

        let baked = a
            .bake_with_derivatives(anim, Some(4.0), None, None)
            .expect("a loaded animation bakes");
        let values_only = a
            .bake(anim, Some(4.0), None, None)
            .expect("a loaded animation bakes");
        for ((track, only), wide) in baked
            .tracks
            .iter()
            .zip(&values_only.tracks)
            .zip([false, true, true, true])
        {
            assert_eq!(only.values, track.values);
            let (Value::ArrayValue(values), Value::ArrayValue(derivatives)) =
                (&track.values, &track.derivatives)
            else {
                panic!("array values");
            };
            assert_eq!(values.len(), 5, "0 to 1 s at 4 Hz, both ends");
            assert!(
                values.iter().all(|v| is_width(v, wide)),
                "{}: {values:?}",
                track.animatable_id
            );
            assert!(
                derivatives.iter().all(|d| match d {
                    Value::Option(Some(d)) => is_width(d, wide),
                    Value::Option(None) => true,
                    _ => false,
                }),
                "{}: {derivatives:?}",
                track.animatable_id
            );
        }
        let Value::ArrayValue(slopes) = &baked.tracks[1].derivatives else {
            panic!("an array value");
        };
        assert!(
            matches!(slopes[2], Value::Option(Some(_))),
            "the ramp has a slope at 0.5 s"
        );
        let Value::ArrayValue(held) = &baked.tracks[2].values else {
            panic!("an array value");
        };
        assert!(held.iter().all(|v| *v == AValue::F64(0.1)), "{held:?}");
    }

    /// `step_values` reports each output's value at its position in
    /// `output_keys`, `Unit` where no instance weighs on it; the table and
    /// its revision change with structural edits alone, and `step` reports
    /// the same values in the same order, keyed.
    #[test]
    fn step_values_are_laid_out_by_the_output_keys() {
        let mut a = AnimationModule::new();
        let first = a.load_animation(constant_clip("one", "k/one", 0.25));
        let second = a.load_animation(constant_clip("two", "k/two", 0.75));
        let player = a.create_player(None);
        let empty = a.output_keys();
        assert!(empty.keys.is_empty());
        a.add_instance(player, first);
        let quiet = a.add_instance_with_weight(player, second, 0.0);
        let keys = a.output_keys();
        assert_ne!(keys.revision, empty.revision);
        assert_eq!(keys.keys, ["k/one", "k/two"]);

        let step = a.step_values(100_000_000, None);
        assert_eq!(step.revision, keys.revision);
        assert_eq!(
            step.values,
            Value::ArrayValue(vec![Value::F32(0.25), Value::Unit])
        );
        a.set_weight(player, quiet, 1.0);
        a.pause(player);
        let step = a.step_values(0, None);
        assert_eq!(
            step.revision, keys.revision,
            "transport is no structural edit"
        );
        assert_eq!(
            step.values,
            Value::ArrayValue(vec![Value::F32(0.25), Value::F32(0.75)])
        );
        let records = a.step(0, None);
        let keyed: Vec<(&str, &str)> = records
            .iter()
            .map(|o| (o.default_key.as_str(), o.track_id.as_str()))
            .collect();
        assert_eq!(keyed, [("k/one", "one-t0"), ("k/two", "two-t0")]);

        a.remove_instance(player, quiet);
        let after = a.output_keys();
        assert_ne!(after.revision, keys.revision);
        assert_eq!(after.keys, ["k/one"]);
        let other = a.create_player(None);
        a.add_instance(other, first);
        assert_eq!(
            a.output_keys().keys,
            ["k/one", "k/one"],
            "a position per player"
        );
        assert_eq!(a.step(0, None).len(), 2);

        // An edit that leaves the table keeps its revision.
        let revision = a.output_keys().revision;
        a.add_instance(u32::MAX, first);
        assert_eq!(a.output_keys().revision, revision, "no player, no output");
    }

    /// A reload that changes an animation's keys lays the table out again,
    /// under a new revision, and `step` names the new tracks; a reload with
    /// the same keys keeps the revision.
    #[test]
    fn a_reload_lays_out_the_keys_it_changes() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(constant_clip("one", "k/x", 0.25));
        let player = a.create_player(None);
        a.add_instance(player, anim);
        let before = a.output_keys();
        assert_eq!(before.keys, ["k/x"]);

        let mut wider = constant_clip("two", "k/x", 0.5);
        wider
            .tracks
            .push(constant_clip("three", "k/y", 0.75).tracks.remove(0));
        assert!(a.reload_animation(anim, wider));
        let after = a.output_keys();
        assert_eq!(after.keys, ["k/x", "k/y"]);
        assert_ne!(after.revision, before.revision);
        let named: Vec<String> = a.step(0, None).into_iter().map(|o| o.track_id).collect();
        assert_eq!(named, ["two-t0", "three-t0"]);

        let mut same = constant_clip("four", "k/x", 0.5);
        same.tracks
            .push(constant_clip("five", "k/y", 0.75).tracks.remove(0));
        assert!(a.reload_animation(anim, same));
        assert_eq!(a.output_keys().revision, after.revision, "the same keys");
        let named: Vec<String> = a.step(0, None).into_iter().map(|o| o.track_id).collect();
        assert_eq!(named, ["four-t0", "five-t0"]);
    }

    /// `step` names each output's track as the latest load of its key
    /// says, even when that load changed no output.
    #[test]
    fn step_names_the_track_the_latest_load_gives_a_key() {
        let mut a = AnimationModule::new();
        let first = a.load_animation(constant_clip("one", "k/x", 0.25));
        let player = a.create_player(None);
        a.add_instance(player, first);
        assert_eq!(a.step(0, None)[0].track_id, "one-t0");
        let revision = a.output_keys().revision;
        a.load_animation(constant_clip("two", "k/x", 0.75));
        assert_eq!(
            a.output_keys().revision,
            revision,
            "a load is no structural edit"
        );
        assert_eq!(a.step(0, None)[0].track_id, "two-t0");
    }

    /// A bake beyond the core's sample bound is refused with `None`, and a
    /// window past the clip's end bakes its end.
    #[test]
    fn a_bake_too_large_is_refused_and_a_late_window_clamps() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(ramp_clip("big", "big/x", true));
        assert!(a.bake(anim, Some(1e9), None, None).is_none());
        assert!(a
            .bake_with_derivatives(anim, Some(1e9), None, None)
            .is_none());
        assert!(a.bake(anim, Some(f32::MAX), None, None).is_none());

        let late = a
            .bake(anim, Some(30.0), Some(5_000_000_000), None)
            .expect("a window past the end bakes");
        assert_eq!((late.start_ns, late.end_ns), (1_000_000_000, 1_000_000_000));
        assert_eq!(
            late.tracks[0].values,
            Value::ArrayValue(vec![Value::F32(1.0)])
        );
    }
}

#[cfg(test)]
mod instances {
    //! Two loads in one process are two engines.
    use super::*;

    #[test]
    fn instances_do_not_share_players() {
        let mut first = AnimationModule::new();
        let mut second = AnimationModule::new();
        let player = first.create_player(Some("only-in-first".into()));
        assert_eq!(first.player_states().len(), 1);
        assert!(second.player_states().is_empty());
        // A command addressed to the other instance's player, and a step of
        // that instance, leave the first one untouched.
        assert_eq!(second.pause(player), player);
        assert!(second.step(250_000_000, None).is_empty());
        assert!(second.player_states().is_empty());
        let state = state_of(&first, player);
        assert_eq!(state.state, "playing");
        assert_eq!(state.time_ns, 0);
    }

    fn state_of(animation: &AnimationModule, player: u32) -> PlayerState {
        animation
            .player_states()
            .into_iter()
            .find(|s| s.player == player)
            .expect("player state")
    }
}
