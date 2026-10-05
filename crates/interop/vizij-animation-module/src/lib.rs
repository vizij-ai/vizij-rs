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
//! global. A host that links this crate has no executor to scope it, so it
//! builds one [`AnimationModule`] per registration and dispatches to it under
//! the declared ids — two devices in one process never share an engine.
//!
//! A keyframe's `value` is a **dynamic `Value`** (the key-value type), so
//! Vizij composites ride through as `Value::Structure` carrying vizij-arora's
//! Vizij-namespaced UUIDs — no per-composite type has to be declared here. A
//! keypoint's `transitions_in`/`transitions_out` carry its cubic-bezier timing
//! handles (zero or one each; empty = the engine's default ease).
//!
//! Exports:
//! - loading — `load_animation` / `create_player` / `add_instance`, and
//!   unloading — `remove_instance` / `remove_player` / `unload_animation`:
//!   structural edits, applied immediately;
//! - per tick — `step(dt_ns)`, returning **per-track outputs keyed by track
//!   identity**, each carrying the track's **default authored key** plus its
//!   sampled value; the consumer (a runner, or a graph node) decides the final
//!   store key — default = the authored key, overridable;
//! - transport — `play` / `pause` / `stop` / `seek(time_ns)` / `set_speed` /
//!   `set_loop` / `set_weight`, buffered into the engine's **next** `step`
//!   (issue order preserved);
//! - feedback — `player_states()`, one `PlayerState` per player: its name,
//!   its playback, its loop mode and its instances, so any client finds a
//!   player by the name it was created under and reads where it stands. This call is a **patch**: the vision is state
//!   changes as first-class, combinable values the behavior conveys, not a
//!   second feedback channel;
//! - baking — `bake` / `bake_with_derivatives`, the sampled clip as JSON.

use std::collections::HashMap;
use std::sync::Mutex;

use arora_types::value::Value;
use arora_types::AroraType;

use vizij_animation_core::{
    export_baked_json, export_baked_with_derivatives_json, AnimId, AnimationData, BakingConfig,
    Config, Engine, Inputs, InstId, InstanceCfg, InstanceUpdate, Keypoint as CoreKeypoint,
    LoopMode, PlayerCommand, PlayerId, Track as CoreTrack, Transitions, Vec2,
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

/// One player's state: its name (as `create_player` gave it, empty when it
/// gave none), its playback — `"playing"`, `"paused"` or `"stopped"`, the
/// playhead and full length in nanoseconds (the `dt_ns` time base), the speed
/// multiplier, the loop mode (`"once"`, `"loop"` or `"ping_pong"`, as
/// `set_loop` takes it) — and its instances, in evaluation order.
///
/// Record `1.1.0` adds `name`, `instances` and `loop_mode` to `1.0.0`'s five
/// fields.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000111", version = "1.1.0")]
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
}

/// One instance on a player: its id, the animation it plays (what
/// `unload_animation` takes) and its blend weight.
#[derive(Debug, Clone, PartialEq, AroraType)]
#[arora(id = "76697a69-6a00-0000-0000-000000000112")]
pub struct InstanceState {
    #[arora(id = "76697a69-6a00-0000-0112-000000000001")]
    pub instance: u32,
    #[arora(id = "76697a69-6a00-0000-0112-000000000002")]
    pub anim: u32,
    #[arora(id = "76697a69-6a00-0000-0112-000000000003")]
    pub weight: f32,
}

/// One load's state: the engine, the key-to-track index and the
/// transport commands waiting for the next step.
pub struct AnimationModule {
    engine: Engine,
    /// Canonical output key (a track's `animatable_id`) -> the authored track id,
    /// so `step` can report per-track identity alongside the default key.
    key_to_track: HashMap<String, String>,
    /// Player commands and instance updates issued since the previous step,
    /// drained (in issue order) into the next `step`'s engine update.
    pending: Inputs,
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
            key_to_track: HashMap::new(),
            pending: Inputs::default(),
        }
    }

    /// Load a typed animation clip into the engine and return its `AnimId`.
    ///
    /// The keyframe `value`s arrive as raw Arora `Value`s (vizij-arora encoding)
    /// and are converted back to Vizij values for the core model.
    pub fn load_animation(&mut self, clip: AnimationClip) -> u32 {
        let tracks = clip
            .tracks
            .into_iter()
            .map(|t| {
                self.key_to_track
                    .insert(t.animatable_id.clone(), t.id.clone());
                CoreTrack {
                    id: t.id,
                    name: t.name,
                    animatable_id: t.animatable_id,
                    points: t.points.into_iter().map(to_core_keypoint).collect(),
                    settings: None,
                }
            })
            .collect();

        let data = AnimationData {
            id: None,
            name: clip.name,
            tracks,
            groups: Default::default(),
            duration_ms: clip.duration,
        };

        self.engine.load_animation(data).0
    }

    /// Create a player and return its `PlayerId`.
    pub fn create_player(&mut self, name: Option<String>) -> u32 {
        self.engine.create_player(&name.unwrap_or_default()).0
    }

    /// Attach an instance of animation `anim` to a player, immediately, and
    /// return its `InstId`. It blends at `weight` (1 when `None`) from the
    /// next step on: an instance added at weight 0 writes nothing until a
    /// `set_weight` gives it one.
    pub fn add_instance(&mut self, player: u32, anim: u32, weight: Option<f32>) -> u32 {
        let cfg = InstanceCfg {
            weight: weight.unwrap_or(1.0),
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

    /// Resume or start playback. Applied at the next `step`.
    pub fn play(&mut self, player: u32) -> u32 {
        self.buffer_command(
            player,
            PlayerCommand::Play {
                player: PlayerId(player),
            },
        )
    }

    /// Hold the playhead where it is. Applied at the next `step`.
    pub fn pause(&mut self, player: u32) -> u32 {
        self.buffer_command(
            player,
            PlayerCommand::Pause {
                player: PlayerId(player),
            },
        )
    }

    /// Stop playback and reset to the window start. Applied at the next `step`.
    pub fn stop(&mut self, player: u32) -> u32 {
        self.buffer_command(
            player,
            PlayerCommand::Stop {
                player: PlayerId(player),
            },
        )
    }

    /// Move the playhead to `time_ns` (nanoseconds, the `dt_ns` time base).
    /// Applied at the next `step`.
    pub fn seek(&mut self, player: u32, time_ns: u64) -> u32 {
        self.buffer_command(
            player,
            PlayerCommand::Seek {
                player: PlayerId(player),
                time: (time_ns as f64 / 1e9) as f32,
            },
        )
    }

    /// Set the playback speed multiplier. Applied at the next `step`.
    pub fn set_speed(&mut self, player: u32, speed: f32) -> u32 {
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

    /// Set an instance's blend weight (weights normalize across a player's
    /// instances). Applied at the next `step`. Returns the echoed instance id.
    pub fn set_weight(&mut self, player: u32, instance: u32, weight: f32) -> u32 {
        self.pending.instance_updates.push(InstanceUpdate {
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
        self.engine.unload_animation(AnimId(anim))
    }

    /// One `PlayerState` per player, in creation order: its name, the
    /// engine's derived playback state, the playhead and full length in
    /// nanoseconds (the `dt_ns` time base), the speed multiplier, the loop
    /// mode, and its instances.
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
                    })
                    .collect(),
                name: info.name,
            })
            .collect()
    }

    /// Bake animation `anim` to sampled per-track values over a fixed window
    /// and return the result as a JSON string (`vizij-animation-core`'s
    /// `export_baked_json` shape). `frame_rate` (Hz) defaults to 60,
    /// `start_time` (seconds) to 0, and `end_time` (seconds) to the clip
    /// duration. Returns an empty string if `anim` is not loaded.
    pub fn bake(
        &self,
        anim: u32,
        frame_rate: Option<f32>,
        start_time: Option<f32>,
        end_time: Option<f32>,
    ) -> String {
        let cfg = baking_config(frame_rate, start_time, end_time);
        match self.engine.bake_animation(AnimId(anim), &cfg) {
            Some(baked) => export_baked_json(&baked).to_string(),
            None => String::new(),
        }
    }

    /// Like [`AnimationModule::bake`], but also samples per-frame derivatives;
    /// returns the combined values-and-derivatives JSON
    /// (`export_baked_with_derivatives_json` shape). Returns an empty string
    /// if `anim` is not loaded.
    pub fn bake_with_derivatives(
        &self,
        anim: u32,
        frame_rate: Option<f32>,
        start_time: Option<f32>,
        end_time: Option<f32>,
    ) -> String {
        let cfg = baking_config(frame_rate, start_time, end_time);
        match self
            .engine
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
    /// `dt_ns` is the runtime's `arora/dt` built-in key. The transport commands
    /// buffered since the previous step apply first, in issue order. Each
    /// output carries the track's authored key as `default_key` and its stable
    /// id as `track_id`; the value uses the vizij-arora `Value` encoding.
    pub fn step(&mut self, dt_ns: u64) -> Vec<TrackOutput> {
        let dt = dt_ns as f64 / 1e9;
        let inputs = std::mem::take(&mut self.pending);
        let outputs = self.engine.update(dt as f32, inputs);
        outputs
            .changes
            .iter()
            .map(|change| TrackOutput {
                track_id: self
                    .key_to_track
                    .get(&change.key)
                    .cloned()
                    .unwrap_or_else(|| change.key.clone()),
                default_key: change.key.clone(),
                value: vizij_arora::to_arora(&change.value),
            })
            .collect()
    }
}

// The wasm guest's module state: one per wasm instance. The declared
// functions act on it; a host that links the crate as an rlib builds its own
// [`AnimationModule`] instead.

lazy_static::lazy_static! {
    static ref GUEST: Mutex<AnimationModule> = Mutex::new(AnimationModule::new());
}

/// Run `f` on the guest global.
fn guest<T>(f: impl FnOnce(&mut AnimationModule) -> T) -> T {
    let mut guest = GUEST.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guest)
}

/// The module's interface: every function and parameter pinned by id, each
/// acting on the guest global.
#[arora_module::module(
    id = "76697a69-6a00-0000-0d00-000000000000",
    name = "vizij-animation",
    version = "0.3.0",
    author = "Semio",
    license = "Proprietary",
    description = "vizij-animation-core as an Arora wasm module",
    executable_mime = "application/wasm"
)]
pub mod animation {
    use super::{guest, AnimationClip, PlayerState, TrackOutput};

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
        #[param(id = "76697a69-6a00-0000-0f03-000000000003")] weight: Option<f32>,
    ) -> u32 {
        guest(|a| a.add_instance(player, anim, weight))
    }

    /// [`AnimationModule::step`](super::AnimationModule::step). `dt_ns` is the
    /// runtime's `arora/dt` built-in key.
    #[export(id = "76697a69-6a00-0000-0f00-000000000004")]
    pub fn step(
        #[param(id = "76697a69-6a00-0000-0f04-000000000001")] dt_ns: u64,
    ) -> Vec<TrackOutput> {
        guest(|a| a.step(dt_ns))
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
        #[param(id = "76697a69-6a00-0000-0f0e-000000000003")] start_time: Option<f32>,
        #[param(id = "76697a69-6a00-0000-0f0e-000000000004")] end_time: Option<f32>,
    ) -> String {
        guest(|a| a.bake(anim, frame_rate, start_time, end_time))
    }

    /// [`AnimationModule::bake_with_derivatives`](super::AnimationModule::bake_with_derivatives).
    #[export(id = "76697a69-6a00-0000-0f00-00000000000f")]
    pub fn bake_with_derivatives(
        #[param(id = "76697a69-6a00-0000-0f0f-000000000001")] anim: u32,
        #[param(id = "76697a69-6a00-0000-0f0f-000000000002")] frame_rate: Option<f32>,
        #[param(id = "76697a69-6a00-0000-0f0f-000000000003")] start_time: Option<f32>,
        #[param(id = "76697a69-6a00-0000-0f0f-000000000004")] end_time: Option<f32>,
    ) -> String {
        guest(|a| a.bake_with_derivatives(anim, frame_rate, start_time, end_time))
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

fn seconds_to_ns(seconds: f32) -> u64 {
    (seconds.max(0.0) as f64 * 1e9).round() as u64
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
        let inst = a.add_instance(player, anim, None);
        assert_ne!(inst, u32::MAX);

        // The clip eases (default S-curve), so it is antisymmetric about the
        // midpoint: at t = 0.5 s (half of the 1 s clip) the value is ~0.5, and it
        // advances monotonically toward it.
        let first = a.step(250_000_000); // t = 0.25 s
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

        let second = a.step(250_000_000); // t = 0.5 s
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
        let inst = a.add_instance(player, anim, Some(0.0));
        assert!(a.step(100_000_000).is_empty(), "silent at weight 0");
        a.set_weight(player, inst, 1.0);
        let out = a.step(0);
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
        let inst = a.add_instance(player, anim, Some(0.0));
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
                weight: 0.0
            }]
        );
        assert_eq!(wave.duration_ns, 1_000_000_000);
        assert_eq!((states[1].player, states[1].name.as_str()), (unnamed, ""));
        assert!(states[1].instances.is_empty());

        assert_eq!(wave.loop_mode, "loop", "a player loops from its creation");

        a.set_weight(player, inst, 0.5);
        a.set_loop(player, "ping_pong".into());
        a.step(0);
        let wave = state_of(&a, player);
        assert_eq!(wave.instances[0].weight, 0.5);
        assert_eq!(wave.loop_mode, "ping_pong");
    }

    /// `PlayerState` crosses the value plane as its 1.1.0 record: a structure
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
            }],
            loop_mode: "once".into(),
        };
        assert_eq!(PlayerState::arora_type_version().to_string(), "1.1.0");
        let value = Value::from(state.clone());
        let Value::Structure(structure) = &value else {
            panic!("a structure, got {value:?}");
        };
        assert_eq!(structure.id, PlayerState::arora_type_id());
        assert_eq!(structure.fields.len(), 8);
        assert_eq!(PlayerState::try_from(value).expect("decodes"), state);
    }

    /// Removing a player takes its instances and leaves the animation loaded
    /// for another player; unloading an animation takes every instance of it.
    #[test]
    fn removing_a_player_and_unloading_an_animation() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(constant_clip("shared", "x", 0.75));
        let first = a.create_player(Some("first".into()));
        a.add_instance(first, anim, None);
        let second = a.create_player(Some("second".into()));
        a.add_instance(second, anim, None);
        a.pause(first); // buffered for a player about to go

        assert!(a.remove_player(first));
        assert!(!a.remove_player(first), "already gone");
        let out = a.step(0);
        assert_eq!(
            as_f32(value_of(&out, "x").expect("the other player still writes")),
            0.75
        );
        let names: Vec<String> = a.player_states().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["second"]);

        assert!(a.unload_animation(anim));
        assert!(!a.unload_animation(anim), "already unloaded");
        assert!(a.step(0).is_empty(), "no animation, no output");
        assert!(
            state_of(&a, second).instances.is_empty(),
            "the player stays, without the instance"
        );
        assert!(a.bake(anim, None, None, None).is_empty());
    }

    #[test]
    fn transitions_ride_through_to_sampling() {
        let mut a = AnimationModule::new();
        // Linear handles: value == normalized time, exactly.
        let anim = a.load_animation(ramp_clip("lin-ramp", "lin/x", true));
        let player = a.create_player(Some("p-lin".into()));
        a.add_instance(player, anim, None);

        let outputs = a.step(250_000_000);
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
        a.add_instance(player, anim, None);

        let outputs = a.step(250_000_000);
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
        a.add_instance(player, anim, None);

        // Advance to 0.25 s.
        let outputs = a.step(250_000_000);
        assert!((as_f32(value_of(&outputs, "tr/x").unwrap()) - 0.25).abs() < 1e-3);
        let s = state_of(&a, player);
        assert_eq!(s.state, "playing");
        assert_eq!(s.duration_ns, 1_000_000_000, "1 s clip length");
        assert!((s.time_ns as f64 - 0.25e9).abs() < 2e6, "playhead ~0.25 s");

        // pause: the playhead holds through further steps.
        assert_eq!(a.pause(player), player);
        a.step(250_000_000);
        let s = state_of(&a, player);
        assert_eq!(s.state, "paused");
        assert!(
            (s.time_ns as f64 - 0.25e9).abs() < 2e6,
            "paused playhead holds"
        );

        // play resumes from where it held.
        assert_eq!(a.play(player), player);
        let outputs = a.step(250_000_000);
        assert!((as_f32(value_of(&outputs, "tr/x").unwrap()) - 0.5).abs() < 1e-3);

        // seek lands exactly (u64 nanoseconds in).
        assert_eq!(a.seek(player, 100_000_000), player);
        let outputs = a.step(0);
        assert!((as_f32(value_of(&outputs, "tr/x").unwrap()) - 0.1).abs() < 1e-3);

        // set_speed scales dt: 0.2 s of wall clock at 2x advances 0.4 s.
        assert_eq!(a.set_speed(player, 2.0), player);
        let outputs = a.step(200_000_000);
        assert!((as_f32(value_of(&outputs, "tr/x").unwrap()) - 0.5).abs() < 1e-3);

        // stop resets to the window start.
        assert_eq!(a.stop(player), player);
        a.step(0);
        let s = state_of(&a, player);
        assert_eq!(s.state, "stopped");
        assert_eq!(s.time_ns, 0);
    }

    #[test]
    fn loop_once_clamps_at_the_clip_end() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(ramp_clip("once-ramp", "once/x", true));
        let player = a.create_player(Some("p-once".into()));
        a.add_instance(player, anim, None);

        assert_eq!(a.set_loop(player, "once".into()), player);
        assert_eq!(a.seek(player, 900_000_000), player);
        a.step(0);
        a.step(300_000_000); // 0.9 s + 0.3 s, clamped to the 1 s end
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
        let inst_zero = a.add_instance(player, zero, None);
        let inst_one = a.add_instance(player, one, None);

        // Equal weights: the normalized blend of 0 and 1.
        let outputs = a.step(100_000_000);
        assert!((as_f32(value_of(&outputs, "mix/x").unwrap()) - 0.5).abs() < 1e-3);

        // Silencing the zero-instance leaves only the one-instance.
        assert_eq!(a.set_weight(player, inst_zero, 0.0), inst_zero);
        let outputs = a.step(100_000_000);
        assert!((as_f32(value_of(&outputs, "mix/x").unwrap()) - 1.0).abs() < 1e-3);

        // Removing both instances stops the key from being emitted at all.
        assert_eq!(a.remove_instance(player, inst_zero), 1);
        assert_eq!(a.remove_instance(player, inst_one), 1);
        assert_eq!(a.remove_instance(player, inst_one), 0, "already gone");
        let outputs = a.step(100_000_000);
        assert!(
            value_of(&outputs, "mix/x").is_none(),
            "no instances, no output for the key"
        );
    }

    #[test]
    fn bake_exports_sampled_tracks_as_json() {
        let mut a = AnimationModule::new();
        let anim = a.load_animation(constant_clip("bake-me", "joint/x", 0.5));

        // A loaded clip bakes to a JSON object echoing the requested frame rate
        // and carrying at least one track of sampled values.
        let json = a.bake(anim, Some(30.0), None, None);
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
        let deriv = a.bake_with_derivatives(anim, Some(30.0), None, None);
        let dparsed: serde_json::Value =
            serde_json::from_str(&deriv).expect("derivative JSON parses");
        assert!(dparsed.get("values").is_some() && dparsed.get("derivatives").is_some());

        // An unloaded animation bakes to an empty string.
        assert!(a.bake(u32::MAX, None, None, None).is_empty());
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
        assert!(second.step(250_000_000).is_empty());
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
