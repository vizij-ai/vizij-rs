#![allow(dead_code)]
//! Engine: data ownership and public API with time math + sampling/accumulate/blend (v1).
//!
//! Methods:
//! - new, load_animation, create_player, add_instance, prebind (resolver), update (accumulate → blend)

use crate::accumulate::AccumulatorWithDerivatives;
use crate::baking::{
    bake_animation_data, bake_animation_data_with_derivatives, BakeError, BakedAnimationData,
    BakedDerivativeAnimationData, BakingConfig,
};
use crate::binding::{BindingSet, BindingTable, ChannelKey, TargetResolver};
use crate::config::Config;
use crate::data::AnimationData;
use crate::ids::{AnimId, IdAllocator, InstId, PlayerId};
use crate::inputs::{Inputs, LoopMode};
use crate::interp::InterpRegistry;
use crate::outputs::{Change, ChangeWithDerivative, Outputs, OutputsWithDerivatives};
use crate::sampling::{sample_track, sample_track_with_derivative};
use crate::scratch::Scratch;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use vizij_api_core::{Value, WriteBatch};

#[derive(Clone, Debug, Default)]
pub struct PrebindReport {
    /// Total number of track channels examined during prebinding.
    pub total: usize,
    /// Number of channels that resolved to a host handle.
    pub resolved: usize,
    /// Canonical track paths that the resolver could not bind.
    pub unresolved: Vec<String>,
}

/// Per-player controller and instance list.
#[derive(Debug)]
#[non_exhaustive]
pub struct Player {
    /// Stable player identifier.
    pub id: PlayerId,
    /// Human-readable display name.
    pub name: String,
    /// Whether the player's time advances: only a [`PlaybackState::Playing`]
    /// player's does, and not while a [`Self::starts_in`] wait runs. Set by
    /// `Play`, `Pause` and `Stop`, and to playing when a `PlayAfter` wait ends;
    /// independent of [`Self::speed`].
    pub state: PlaybackState,
    /// Multiplier on the time a playing player advances by per update. Set by
    /// `SetSpeed` alone: pausing, stopping and playing keep it, so a player
    /// resumes at the speed it was given.
    pub speed: f32,
    /// Internal accumulated player time in seconds: the sum of the time its
    /// updates advanced it by since it was last set, kept in `f64` and summed
    /// with compensation (see [`Self::advance`]), so a player stepped at a
    /// steady rate stays on the timeline its updates' `dt`s add up to.
    pub time: f64,
    /// Looping mode used when mapping player time into clip-local time.
    pub mode: LoopMode,
    /// Play window start in seconds of player time.
    pub start_time: f64,
    /// Play window end in seconds of player time; `None` ends the window at `total_duration`.
    pub end_time: Option<f64>,
    /// Seconds until a [`PlayerCommand::PlayAfter`](crate::PlayerCommand::PlayAfter) start,
    /// counted down by each update whatever the state; the playhead holds while it is set, and
    /// the player plays when it runs out. `Play`, `Pause` and `Stop` clear it.
    pub starts_in: Option<f64>,
    /// Attached instance ids in evaluation order.
    pub instances: Vec<InstId>,
    /// Player length in seconds: the latest end over its instances,
    /// `start_offset + anim_duration * |time_scale|`.
    pub total_duration: f64,
    /// The low-order part of [`Self::time`]'s running sum that `f64` could
    /// not hold, fed back into the next advance.
    time_carry: f64,
}

impl Player {
    fn new(id: PlayerId, name: String) -> Self {
        Self {
            id,
            name,
            state: PlaybackState::Playing,
            speed: 1.0,
            time: 0.0,
            mode: LoopMode::Loop,
            start_time: 0.0,
            end_time: None,
            starts_in: None,
            instances: Vec::new(),
            total_duration: 0.0,
            time_carry: 0.0,
        }
    }

    /// Set the player's time, starting a new running sum.
    fn set_time(&mut self, time: f64) {
        self.time = time;
        self.time_carry = 0.0;
    }

    /// Advance the player's time by `by` seconds, with compensated (Kahan)
    /// summation: each addition's rounding error is carried into the next,
    /// so the time stays within an `f64` rounding of the exact sum however
    /// many updates it took: stepped by 10 ms for an hour, it stays within
    /// one rounding (4.5e-13 s) of `n × 10 ms`, where a plain sum drifts
    /// 32 ns, and a `Once` player reaches the end of its window on the step
    /// that sums to it.
    fn advance(&mut self, by: f64) {
        let by = by - self.time_carry;
        let time = self.time + by;
        self.time_carry = (time - self.time) - by;
        self.time = time;
    }

    /// The play window `[start, end]` in player time.
    fn window(&self) -> (f64, f64) {
        let start = self.start_time.max(0.0);
        let end = self.end_time.unwrap_or(self.total_duration).max(start);
        (start, end)
    }

    /// The playhead: player time mapped into the play window by the loop mode.
    fn playhead(&self) -> f64 {
        let (start, end) = self.window();
        let span = end - start;
        if span <= 0.0 {
            return start;
        }
        match self.mode {
            LoopMode::Once => self.time.clamp(start, end),
            LoopMode::Loop => start + fmod(self.time - start, span),
            LoopMode::PingPong => start + ping_pong(self.time - start, span),
        }
    }

    /// Whether updates advance the playhead: a playing player not waiting for its start, at
    /// a non-zero speed.
    fn advancing(&self) -> bool {
        self.state == PlaybackState::Playing && self.starts_in.is_none() && self.speed != 0.0
    }

    /// Whether a `Once` player has played to the window bound its speed heads for.
    fn ended(&self) -> bool {
        if self.mode != LoopMode::Once || !self.advancing() {
            return false;
        }
        let (start, end) = self.window();
        if self.speed > 0.0 {
            self.time >= end
        } else {
            self.time <= start
        }
    }
}

/// An animation instance attached to a player.
#[derive(Debug)]
pub struct Instance {
    pub id: InstId,
    pub anim: AnimId,
    pub weight: f32,
    pub time_scale: f32,
    pub start_offset: f64,
    pub enabled: bool,
    pub binding_set: BindingSet,
    /// The output each channel of `binding_set` writes, an index into
    /// [`Engine::output_targets`]; [`NO_TARGET`] for a channel that writes
    /// none (another animation's, or a track without keypoints).
    targets: Vec<u32>,
}

/// The output index of a channel that writes no output.
const NO_TARGET: u32 = u32::MAX;

/// One output of the engine: a key a player's instances write, blended
/// across those instances. Two players writing one key are two outputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputTarget {
    /// The player whose instances write it.
    pub player: PlayerId,
    /// The key: the channel's bound handle when one is bound, otherwise its
    /// track's canonical path.
    pub key: String,
}

/// What an update reports.
#[derive(Clone, Copy, PartialEq)]
enum Report {
    /// Each output's value by its index: [`Engine::update_by_target`].
    ByTarget,
    /// The outputs written, as keyed [`Change`]s.
    Changes,
    /// Keyed changes with their derivatives.
    ChangesWithDerivatives,
}

/// Configuration for adding an instance.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InstanceCfg {
    /// Blend weight applied to the instance contribution.
    pub weight: f32,
    /// Playback scaling factor used by the local-time mapping.
    pub time_scale: f32,
    /// Start offset in seconds on the player timeline.
    pub start_offset: f64,
    /// Whether the instance participates in evaluation.
    pub enabled: bool,
}

impl Default for InstanceCfg {
    fn default() -> Self {
        Self {
            weight: 1.0,
            time_scale: 1.0,
            start_offset: 0.0,
            enabled: true,
        }
    }
}

/// Minimal animation library storage.
#[derive(Default, Debug)]
struct AnimLib {
    items: Vec<(AnimId, AnimationData)>,
}

impl AnimLib {
    fn insert(&mut self, id: AnimId, data: AnimationData) {
        self.items.push((id, data));
    }
    fn get(&self, id: AnimId) -> Option<&AnimationData> {
        self.items
            .iter()
            .find_map(|(a, d)| if *a == id { Some(d) } else { None })
    }
    fn iter(&self) -> impl Iterator<Item = &(AnimId, AnimationData)> {
        self.items.iter()
    }
    /// Swap the data under `id`; `false` when `id` is not loaded.
    fn replace(&mut self, id: AnimId, data: AnimationData) -> bool {
        match self.items.iter_mut().find(|(a, _)| *a == id) {
            Some((_, slot)) => {
                *slot = data;
                true
            }
            None => false,
        }
    }
    fn remove(&mut self, id: AnimId) -> bool {
        let before = self.items.len();
        self.items.retain(|(a, _)| *a != id);
        before != self.items.len()
    }
    fn contains(&self, id: AnimId) -> bool {
        self.items.iter().any(|(a, _)| *a == id)
    }
}

/// Engine (core) with engine-agnostic handle type fixed to String for v1.
#[derive(Debug)]
pub struct Engine {
    // Owned data
    cfg: Config,
    ids: IdAllocator,
    anims: AnimLib,
    players: Vec<Player>,
    instances: Vec<Instance>,

    // Systems
    binds: BindingTable,
    interp: InterpRegistry,
    scratch: Scratch,

    // Outputs, laid out by structural edits
    targets: Vec<OutputTarget>,
    revision: u32,
    accum: AccumulatorWithDerivatives,

    // Per-tick outputs
    outputs: Outputs,
    outputs_with_derivatives: OutputsWithDerivatives,
    target_values: Vec<Option<Value>>,
}

fn fmod(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        return 0.0;
    }
    let m = a % b;
    if (m < 0.0 && b > 0.0) || (m > 0.0 && b < 0.0) {
        m + b
    } else {
        m
    }
}

/// Reflect t into [0, period] with ping-pong behavior, where period = 2 * span.
fn ping_pong(t: f64, span: f64) -> f64 {
    if span <= 0.0 {
        return 0.0;
    }
    let period = 2.0 * span;
    let m = fmod(t, period);
    if m < 0.0 {
        // Normalize negative
        let mm = m + period;
        if mm <= span {
            mm
        } else {
            period - mm
        }
    } else if m <= span {
        m
    } else {
        period - m
    }
}

/// A clip's duration in seconds, the time domain players and bakes sample it
/// in.
pub(crate) fn clip_seconds(duration_ms: u32) -> f64 {
    f64::from(duration_ms) / 1000.0
}

/// The normalized time `[0, 1]` the tracks of a clip `duration` seconds long
/// are sampled at, `time` seconds into it. Time stays in `f64` up to here;
/// only the normalized time a sample takes is `f32`, so a step and a bake at
/// one clip time sample alike.
pub(crate) fn normalized_time(time: f64, duration: f64) -> f32 {
    if duration > 0.0 {
        (time / duration).clamp(0.0, 1.0) as f32
    } else {
        0.0
    }
}

/// Whether a player's time advances, as `Play`, `Pause` and `Stop` set it.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum PlaybackState {
    /// Player time advances by the update's `dt` times the player's speed. A
    /// new player plays.
    Playing,
    /// Player time is held where it is; `Play` resumes from it.
    Paused,
    /// Player time is held at the start of its window; `Play` starts from
    /// it, and a `Seek` leaves the player paused at the time it seeks to.
    Stopped,
}

/// Lightweight metadata snapshot for one loaded animation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnimationInfo {
    /// Animation id.
    pub id: u32,
    /// Optional clip name.
    pub name: Option<String>,
    /// Source animation duration in milliseconds.
    pub duration_ms: u32,
    /// Number of tracks in the clip.
    pub track_count: usize,
}

/// Inspection snapshot for one player.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerInfo {
    /// Player id.
    pub id: u32,
    /// Player display name.
    pub name: String,
    /// Playback state, as the last `Play`, `Pause` or `Stop` left it.
    pub state: PlaybackState,
    /// Display/playhead time in seconds after loop/window mapping.
    pub time: f64,
    /// Playback speed multiplier, as `SetSpeed` set it, whatever the state.
    pub speed: f32,
    /// Active loop mode.
    pub loop_mode: LoopMode,
    /// Play window start in seconds.
    pub start_time: f64,
    /// Play window end in seconds; `None` ends the window at `length`.
    pub end_time: Option<f64>,
    /// Full player length (seconds): max over instances of start_offset + (anim_duration * |time_scale|)
    pub length: f64,
    /// A [`LoopMode::Once`] player advancing at a non-zero speed has reached the window bound
    /// it heads for (the end, or the start when playing backwards) and holds there.
    pub ended: bool,
}

/// Inspection snapshot for one instance attached to a player.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InstanceInfo {
    /// Instance id.
    pub id: u32,
    /// Referenced animation id.
    pub animation: u32,
    /// Effective instance configuration.
    pub cfg: InstanceCfg,
}

impl Engine {
    /// Public accessor for a player's computed total duration (in player time).
    pub fn player_total_duration(&self, player: PlayerId) -> Option<f64> {
        self.players
            .iter()
            .find(|p| p.id == player)
            .map(|p| p.total_duration)
    }

    /// Create a new engine with the given config.
    pub fn new(cfg: Config) -> Self {
        Self {
            scratch: Scratch::new(&cfg),
            cfg,
            ids: IdAllocator::new(),
            anims: AnimLib::default(),
            players: Vec::new(),
            instances: Vec::new(),
            binds: BindingTable::new(),
            interp: InterpRegistry::new(),
            targets: Vec::new(),
            revision: 0,
            accum: AccumulatorWithDerivatives::new(),
            outputs: Outputs::default(),
            outputs_with_derivatives: OutputsWithDerivatives::default(),
            target_values: Vec::new(),
        }
    }

    /// Load animation data into the engine and return its assigned [`AnimId`].
    ///
    /// The engine stores the clip internally and stamps the allocated id into `data.id`.
    pub fn load_animation(&mut self, mut data: AnimationData) -> AnimId {
        let id = self.ids.alloc_anim();
        data.id = Some(id);
        self.anims.insert(id, data);
        id
    }

    /// Replace the data of loaded animation `anim` in place, keeping its id.
    ///
    /// Every instance of `anim` stays on its player, with its weight, time scale, offset and
    /// enabled flag, and samples every track of `data` from the next update on; the players
    /// keep their state, time, speed, loop mode and window, and their lengths follow the new
    /// duration. `anim`'s host bindings are dropped, because a binding names a track by its
    /// index and `data`'s tracks need not match the old ones: its tracks write their canonical
    /// paths until the next [`Self::prebind`]. The engine stamps `anim` into `data.id`.
    ///
    /// Returns `false`, changing nothing, when `anim` is not loaded.
    pub fn replace_animation(&mut self, anim: AnimId, mut data: AnimationData) -> bool {
        data.id = Some(anim);
        let channels = Self::channels_of(anim, &data);
        if !self.anims.replace(anim, data) {
            return false;
        }
        self.binds.remove_animation(anim);
        for inst in self.instances.iter_mut().filter(|i| i.anim == anim) {
            inst.binding_set.channels = channels.clone();
        }
        let player_ids: Vec<PlayerId> = self.players.iter().map(|p| p.id).collect();
        for pid in player_ids {
            self.recalc_player_duration(pid);
        }
        self.lay_out_targets();
        true
    }

    /// One channel per track of `data`, loaded as `anim`.
    fn channels_of(anim: AnimId, data: &AnimationData) -> Vec<ChannelKey> {
        (0..data.tracks.len())
            .map(|idx| ChannelKey {
                anim,
                track_idx: idx as u32,
            })
            .collect()
    }

    /// Bake a loaded animation into per-frame samples using the provided config
    /// (see [`bake_animation_data`]).
    ///
    /// Refused with [`BakeError::NotLoaded`] when `anim` is not currently
    /// loaded, and as [`bake_animation_data`] refuses a bake.
    pub fn bake_animation(
        &self,
        anim: AnimId,
        cfg: &BakingConfig,
    ) -> Result<BakedAnimationData, BakeError> {
        let data = self.anims.get(anim).ok_or(BakeError::NotLoaded(anim))?;
        bake_animation_data(anim, data, cfg)
    }

    /// Bake animation values and derivatives in one pass (see
    /// [`bake_animation_data_with_derivatives`]).
    ///
    /// Refused with [`BakeError::NotLoaded`] when `anim` is not currently
    /// loaded, and as [`bake_animation_data_with_derivatives`] refuses a bake.
    pub fn bake_animation_with_derivatives(
        &self,
        anim: AnimId,
        cfg: &BakingConfig,
    ) -> Result<(BakedAnimationData, BakedDerivativeAnimationData), BakeError> {
        let data = self.anims.get(anim).ok_or(BakeError::NotLoaded(anim))?;
        bake_animation_data_with_derivatives(anim, data, cfg)
    }

    /// Create a new player with a display name and default looping behavior.
    pub fn create_player(&mut self, name: &str) -> PlayerId {
        let pid = self.ids.alloc_player();
        self.players.push(Player::new(pid, name.to_string()));
        pid
    }

    /// Add an animation instance to a player.
    ///
    /// The returned instance is appended to the player's evaluation order. If `player` does not
    /// exist, the instance is still stored internally but is not attached to any player.
    pub fn add_instance(&mut self, player: PlayerId, anim: AnimId, cfg: InstanceCfg) -> InstId {
        let iid = self.ids.alloc_inst();

        // Build binding set for this instance (all channels of the animation).
        let binding_set = BindingSet {
            channels: self
                .anims
                .get(anim)
                .map(|data| Self::channels_of(anim, data))
                .unwrap_or_default(),
        };

        let instance = Instance {
            id: iid,
            anim,
            weight: cfg.weight,
            time_scale: cfg.time_scale,
            start_offset: cfg.start_offset,
            enabled: cfg.enabled,
            binding_set,
            targets: Vec::new(),
        };
        self.instances.push(instance);

        // Attach to player
        if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
            p.instances.push(iid);
        }
        // Recompute player's effective total duration
        self.recalc_player_duration(player);
        self.lay_out_targets();
        iid
    }

    /// One-time binding against a provided resolver.
    /// Iterates all animations/tracks and resolves canonical target paths into handles.
    /// Returns a report indicating how many bindings were resolved.
    pub fn prebind_with_report(&mut self, resolver: &mut dyn TargetResolver) -> PrebindReport {
        let mut report = PrebindReport::default();
        for (anim_id, data) in self.anims.iter() {
            for (idx, track) in data.tracks.iter().enumerate() {
                report.total += 1;
                if let Some(handle) = resolver.resolve(&track.animatable_id) {
                    self.binds.upsert(
                        ChannelKey {
                            anim: *anim_id,
                            track_idx: idx as u32,
                        },
                        handle,
                    );
                    report.resolved += 1;
                } else {
                    report.unresolved.push(track.animatable_id.clone());
                }
            }
        }
        self.lay_out_targets();
        report
    }

    /// Assign every channel of every player's instances its output: one per
    /// key per player, in player creation order, then in the order the
    /// player's instances and their channels first write each key. A
    /// layout that differs from the previous one bumps
    /// [`Engine::output_revision`].
    fn lay_out_targets(&mut self) {
        let mut targets = Vec::new();
        let mut assigned: HashMap<InstId, Vec<u32>> = HashMap::new();
        let instances: HashMap<InstId, &Instance> =
            self.instances.iter().map(|inst| (inst.id, inst)).collect();
        for p in &self.players {
            let mut index: HashMap<&str, u32> = HashMap::new();
            for iid in &p.instances {
                let Some(inst) = instances.get(iid) else {
                    continue;
                };
                let anim = self.anims.get(inst.anim);
                let of_instance = inst
                    .binding_set
                    .channels
                    .iter()
                    .map(|ch| {
                        let track = anim
                            .filter(|_| ch.anim == inst.anim)
                            .and_then(|a| a.tracks.get(ch.track_idx as usize))
                            .filter(|track| !track.points.is_empty());
                        let Some(track) = track else {
                            return NO_TARGET;
                        };
                        let key = self
                            .binds
                            .get(*ch)
                            .map_or(track.animatable_id.as_str(), |row| row.handle.as_str());
                        *index.entry(key).or_insert_with(|| {
                            targets.push(OutputTarget {
                                player: p.id,
                                key: key.to_string(),
                            });
                            (targets.len() - 1) as u32
                        })
                    })
                    .collect();
                assigned.insert(inst.id, of_instance);
            }
        }
        for inst in &mut self.instances {
            inst.targets = assigned.remove(&inst.id).unwrap_or_default();
        }
        if targets != self.targets {
            self.targets = targets;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// The engine's outputs, in the order [`Engine::update_by_target`]
    /// reports their values: one per key a player's instances write. Only
    /// structural edits change it — `add_instance`, `remove_instance`,
    /// `remove_player`, `unload_animation`, `replace_animation` and
    /// `prebind` — each that does bumping [`Engine::output_revision`].
    pub fn output_targets(&self) -> &[OutputTarget] {
        &self.targets
    }

    /// The layout [`Engine::output_targets`] is in: it changes whenever a
    /// structural edit changes the targets, so a consumer holding them knows
    /// to read them again when it differs. Wraps around after `u32::MAX`
    /// changes.
    pub fn output_revision(&self) -> u32 {
        self.revision
    }

    /// Backwards compatible wrapper accepting resolvers that ignore the report.
    pub fn prebind(&mut self, resolver: &mut dyn TargetResolver) {
        let _ = self.prebind_with_report(resolver);
    }

    /// Recalculate a player's full length (seconds) from its instances.
    /// length = max over instances of: start_offset + (anim_duration * |time_scale|)
    fn recalc_player_duration(&mut self, player: PlayerId) {
        if let Some(p) = self.players.iter_mut().find(|pp| pp.id == player) {
            let mut max_end = 0.0f64;
            for iid in &p.instances {
                if let Some(inst) = self.instances.iter().find(|ii| ii.id == *iid) {
                    if let Some(anim) = self.anims.get(inst.anim) {
                        let anim_duration = clip_seconds(anim.duration_ms);
                        let ts_abs = f64::from(inst.time_scale.abs().max(1e-6));
                        let end_time = inst.start_offset + (anim_duration * ts_abs);
                        if end_time > max_end {
                            max_end = end_time;
                        }
                    }
                }
            }
            p.total_duration = max_end;
        }
    }

    /// Apply player/instance commands (minimal semantics for v1 skeleton).
    fn apply_inputs(&mut self, inputs: Inputs) {
        // Player commands
        for cmd in inputs.player_cmds {
            match cmd {
                crate::inputs::PlayerCommand::Play { player } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        p.state = PlaybackState::Playing;
                        p.starts_in = None;
                    }
                }
                crate::inputs::PlayerCommand::Pause { player } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        p.state = PlaybackState::Paused;
                        p.starts_in = None;
                    }
                }
                crate::inputs::PlayerCommand::Stop { player } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        p.state = PlaybackState::Stopped;
                        p.starts_in = None;
                        p.set_time(p.start_time);
                    }
                }
                crate::inputs::PlayerCommand::SetSpeed { player, speed } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        p.speed = speed;
                    }
                }
                crate::inputs::PlayerCommand::Seek { player, time } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        p.set_time(time);
                        // A stopped player is at its window start: one moved
                        // off it holds there, paused.
                        if p.state == PlaybackState::Stopped {
                            p.state = PlaybackState::Paused;
                        }
                    }
                }
                crate::inputs::PlayerCommand::SetLoopMode { player, mode } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        p.mode = mode;
                    }
                }
                crate::inputs::PlayerCommand::SetWindow {
                    player,
                    start_time,
                    end_time,
                } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        let playhead = p.playhead();
                        p.start_time = start_time.max(0.0);
                        p.end_time = end_time.map(|e| e.max(p.start_time));
                        let (start, end) = p.window();
                        // A stopped player stays at its window start.
                        p.set_time(if p.state == PlaybackState::Stopped {
                            start
                        } else {
                            playhead.clamp(start, end)
                        });
                    }
                }
                crate::inputs::PlayerCommand::PlayAfter { player, delay } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        p.starts_in = Some(delay);
                    }
                }
            }
        }

        // Instance updates
        for upd in inputs.instance_updates {
            if let Some(inst) = self.instances.iter_mut().find(|i| i.id == upd.inst) {
                if let Some(w) = upd.weight {
                    inst.weight = w;
                }
                if let Some(ts) = upd.time_scale {
                    inst.time_scale = ts;
                }
                if let Some(so) = upd.start_offset {
                    inst.start_offset = so;
                }
                if let Some(en) = upd.enabled {
                    inst.enabled = en;
                }
            }
            // Update the associated player's total duration
            self.recalc_player_duration(upd.player);
        }
        // Note: We don't enforce that upd.player actually owns upd.inst here; adapters should pass
        // consistent player+inst pairs. Validation can be added later.
    }

    /// Advance logical time of the playing players. A player waiting for its start, whatever
    /// its state, holds until the start, then plays and advances only by the time after it.
    /// `Once` holds the time inside the play window, so playback reverses from the bound it
    /// reached; `Loop` and `PingPong` map it into the window when sampling.
    fn advance_player_times(&mut self, dt: f64) {
        for p in &mut self.players {
            let run = match p.starts_in {
                Some(wait) if wait > dt => {
                    p.starts_in = Some(wait - dt);
                    continue;
                }
                Some(wait) => {
                    p.starts_in = None;
                    p.state = PlaybackState::Playing;
                    dt - wait
                }
                None if p.state == PlaybackState::Playing => dt,
                None => continue,
            };
            p.advance(run * f64::from(p.speed));
            if p.mode == LoopMode::Once {
                let (start, end) = p.window();
                if !(start..=end).contains(&p.time) {
                    p.set_time(p.time.clamp(start, end));
                }
            }
        }
    }

    /// Compute instance-local time given a player and animation duration under the player's loop mode.
    fn local_time_for_instance(player: &Player, inst: &Instance, anim_duration: f64) -> f64 {
        // Interpret start_offset as a player-time shift (when the instance starts).
        // Interpret time_scale as a duration multiplier (|ts| > 1 => longer, |ts| < 1 => shorter).
        // Mapping from the playhead to clip local time:
        //   base = (playhead - inst.start_offset) / inst.time_scale
        // Before the instance starts (playhead < start_offset), we must NOT wrap:
        //   return 0.0 so the instance outputs its initial values until start.
        // After start, apply Once/Loop/PingPong in the clip's [0, anim_duration] domain.
        if anim_duration <= 0.0 {
            return 0.0;
        }
        // Special-case zero time scale: hold at start_offset in clip time.
        if inst.time_scale == 0.0 {
            return inst.start_offset.clamp(0.0, anim_duration);
        }
        // Guard against division by zero while preserving sign semantics
        let ts = f64::from(inst.time_scale);
        let rel = player.playhead() - inst.start_offset;
        if rel <= 0.0 {
            // Hold initial value up to the instance start within each cycle.
            return 0.0;
        }
        let base = rel / ts;
        match player.mode {
            crate::inputs::LoopMode::Once => base.clamp(0.0, anim_duration),
            crate::inputs::LoopMode::Loop => {
                let m = fmod(base, anim_duration);
                if m < 0.0 {
                    m + anim_duration
                } else {
                    m
                }
            }
            crate::inputs::LoopMode::PingPong => ping_pong(base, anim_duration),
        }
    }

    fn step(&mut self, dt: f64, inputs: Inputs, report: Report) {
        let with_derivatives = report == Report::ChangesWithDerivatives;
        self.scratch.begin_frame();
        self.outputs.clear();
        if with_derivatives {
            self.outputs_with_derivatives.clear();
        }

        self.apply_inputs(inputs);
        self.advance_player_times(dt);

        self.accum.reset(self.targets.len());
        for p in &self.players {
            for iid in &p.instances {
                let Some(inst) = self.instances.iter().find(|i| i.id == *iid) else {
                    continue;
                };
                if !inst.enabled {
                    continue;
                }
                let Some(anim_data) = self.anims.get(inst.anim) else {
                    continue;
                };
                let anim_duration_s = clip_seconds(anim_data.duration_ms);
                let local_t = Self::local_time_for_instance(p, inst, anim_duration_s);
                let u = normalized_time(local_t, anim_duration_s);
                let anim_duration_s = anim_duration_s as f32;
                for (ch, &target) in inst.binding_set.channels.iter().zip(&inst.targets) {
                    if target == NO_TARGET {
                        continue;
                    }
                    let Some(track) = anim_data.tracks.get(ch.track_idx as usize) else {
                        continue;
                    };
                    let (value, derivative) = if with_derivatives {
                        sample_track_with_derivative(track, u, anim_duration_s)
                    } else {
                        (sample_track(track, u), None)
                    };
                    self.accum
                        .add(target as usize, &value, derivative.as_ref(), inst.weight);
                }
            }
        }

        self.target_values.clear();
        for (index, target) in self.targets.iter().enumerate() {
            let blended = self.accum.take(index);
            match report {
                Report::ByTarget => self.target_values.push(blended.map(|(value, _)| value)),
                Report::Changes => {
                    if let Some((value, _)) = blended {
                        self.outputs.push_change(Change {
                            player: target.player,
                            key: target.key.clone(),
                            value,
                        });
                    }
                }
                Report::ChangesWithDerivatives => {
                    if let Some((value, derivative)) = blended {
                        self.outputs.push_change(Change {
                            player: target.player,
                            key: target.key.clone(),
                            value: value.clone(),
                        });
                        self.outputs_with_derivatives
                            .push_change(ChangeWithDerivative {
                                player: target.player,
                                key: target.key.clone(),
                                value,
                                derivative,
                            });
                    }
                }
            }
        }

        if with_derivatives {
            self.outputs_with_derivatives.events = self.outputs.events.clone();
        }
    }

    /// Step the simulation by `dt` seconds with the provided inputs, returning
    /// each output's blended value by its index in [`Engine::output_targets`]:
    /// `None` for an output no instance weighs on this step. No key is built
    /// or copied, so a host that holds the targets reads the values by
    /// position.
    ///
    /// The returned slice borrows the engine's buffer and is invalidated by
    /// the next call that mutates outputs. It builds no keyed [`Change`].
    pub fn update_by_target(&mut self, dt: f64, inputs: Inputs) -> &[Option<Value>] {
        self.step(dt, inputs, Report::ByTarget);
        &self.target_values
    }

    /// Step the simulation by `dt` seconds with the provided inputs, returning value changes only.
    ///
    /// The returned reference borrows the engine's internal output buffer and is invalidated by
    /// the next call that mutates outputs (`update*`, `step`, or `update_writebatch`).
    pub fn update_values(&mut self, dt: f64, inputs: Inputs) -> &Outputs {
        self.step(dt, inputs, Report::Changes);
        &self.outputs
    }

    /// Step the simulation by `dt` seconds, returning both values and derivatives.
    ///
    /// The returned reference borrows the engine's internal derivative-output buffer and is
    /// invalidated by the next call that mutates outputs.
    pub fn update_values_and_derivatives(
        &mut self,
        dt: f64,
        inputs: Inputs,
    ) -> &OutputsWithDerivatives {
        self.step(dt, inputs, Report::ChangesWithDerivatives);
        &self.outputs_with_derivatives
    }

    /// Backwards-compatible alias for [`Self::update_values`].
    pub fn update(&mut self, dt: f64, inputs: Inputs) -> &Outputs {
        self.update_values(dt, inputs)
    }

    /// Update and also return a typed WriteBatch (collection of WriteOp) where each
    /// WriteOp.path is parsed as a `TypedPath`. If a change's key does not parse as a
    /// TypedPath it will be skipped in the returned batch. The engine still maintains
    /// its normal Outputs in `self.outputs`.
    pub fn update_writebatch(&mut self, dt: f64, inputs: Inputs) -> WriteBatch {
        // Populate self.outputs as usual.
        let _ = self.update_values(dt, inputs);

        self.outputs.to_writebatch()
    }

    /// Remove an instance from a player.
    ///
    /// Returns `true` only when the instance was attached to `player` and was removed.
    pub fn remove_instance(&mut self, player: PlayerId, inst: InstId) -> bool {
        // Detach from player
        if let Some(p) = self.players.iter_mut().find(|pp| pp.id == player) {
            let before = p.instances.len();
            p.instances.retain(|iid| *iid != inst);
            let removed = before != p.instances.len();
            if removed {
                // Remove from engine.instances
                self.instances.retain(|ii| ii.id != inst);
                // Recompute duration
                self.recalc_player_duration(player);
                self.lay_out_targets();
                return true;
            }
        }
        false
    }

    /// Remove a player and all its attached instances.
    ///
    /// Returns `true` when the player existed.
    pub fn remove_player(&mut self, player: PlayerId) -> bool {
        if let Some(idx) = self.players.iter().position(|p| p.id == player) {
            let inst_ids: Vec<InstId> = self.players[idx].instances.clone();
            // Remove all instances owned by this player
            if !inst_ids.is_empty() {
                self.instances.retain(|ii| !inst_ids.contains(&ii.id));
            }
            // Remove the player
            self.players.remove(idx);
            self.lay_out_targets();
            true
        } else {
            false
        }
    }

    /// Unload an animation and remove all instances referencing it across all players.
    ///
    /// Returns `true` when the animation existed.
    pub fn unload_animation(&mut self, anim: AnimId) -> bool {
        if !self.anims.contains(anim) {
            return false;
        }
        // Determine all instances to remove
        let to_remove: Vec<InstId> = self
            .instances
            .iter()
            .filter(|ii| ii.anim == anim)
            .map(|ii| ii.id)
            .collect();

        if !to_remove.is_empty() {
            // Detach from players
            for p in &mut self.players {
                p.instances.retain(|iid| !to_remove.contains(iid));
                // TODO: Evaluate if the drop below is necessary
                // Recompute duration after detaching
                // let pid = p.id;
                // recalc will run in a separate pass below to avoid borrow conflicts
                // let _ = drop(pid);
            }
            // Remove instance structs
            self.instances.retain(|ii| ii.anim != anim);
            // Recompute durations for all players
            let player_ids: Vec<PlayerId> = self.players.iter().map(|p| p.id).collect();
            for pid in player_ids {
                self.recalc_player_duration(pid);
            }
        }
        // Remove animation from library
        self.anims.remove(anim);
        self.lay_out_targets();
        true
    }

    /// List all loaded animations.
    pub fn list_animations(&self) -> Vec<AnimationInfo> {
        self.anims
            .iter()
            .map(|(id, data)| AnimationInfo {
                id: id.0,
                name: if data.name.is_empty() {
                    None
                } else {
                    Some(data.name.clone())
                },
                duration_ms: data.duration_ms,
                track_count: data.tracks.len(),
            })
            .collect()
    }

    /// List all players with playback info and computed length.
    pub fn list_players(&self) -> Vec<PlayerInfo> {
        self.players
            .iter()
            .map(|p| PlayerInfo {
                id: p.id.0,
                name: p.name.clone(),
                state: p.state,
                time: p.playhead(),
                speed: p.speed,
                loop_mode: p.mode,
                start_time: p.start_time,
                end_time: p.end_time,
                length: p.total_duration,
                ended: p.ended(),
            })
            .collect()
    }

    /// List all instances attached to a given player.
    ///
    /// Returns an empty vector when the player does not exist.
    pub fn list_instances(&self, player: PlayerId) -> Vec<InstanceInfo> {
        if let Some(p) = self.players.iter().find(|pp| pp.id == player) {
            p.instances
                .iter()
                .filter_map(|iid| self.instances.iter().find(|ii| ii.id == *iid))
                .map(|ii| InstanceInfo {
                    id: ii.id.0,
                    animation: ii.anim.0,
                    cfg: InstanceCfg {
                        weight: ii.weight,
                        time_scale: ii.time_scale,
                        start_offset: ii.start_offset,
                        enabled: ii.enabled,
                    },
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// List the set of resolved output keys currently associated with the player's instances.
    /// Keys match those produced in Outputs (bound handle if available, else canonical track path).
    pub fn list_player_keys(&self, player: PlayerId) -> Vec<String> {
        let mut set: HashSet<String> = HashSet::new();
        let Some(p) = self.players.iter().find(|pp| pp.id == player) else {
            return Vec::new();
        };
        for iid in &p.instances {
            if let Some(inst) = self.instances.iter().find(|ii| ii.id == *iid) {
                if let Some(anim) = self.anims.get(inst.anim) {
                    for ch in &inst.binding_set.channels {
                        if ch.anim != inst.anim {
                            continue;
                        }
                        let idx = ch.track_idx as usize;
                        if let Some(track) = anim.tracks.get(idx) {
                            // Resolve handle if bound, else fallback to canonical path
                            let handle = if let Some(row) = self.binds.get(*ch) {
                                row.handle.as_str().to_string()
                            } else {
                                track.animatable_id.clone()
                            };
                            set.insert(handle);
                        }
                    }
                }
            }
        }
        set.into_iter().collect()
    }
}

impl Engine {
    /// Inspect an instance's bound channel keys.
    ///
    /// Useful for tests and tooling that need to understand binding coverage.
    pub fn get_instance_channels(&self, inst: InstId) -> Option<Vec<ChannelKey>> {
        self.instances
            .iter()
            .find(|i| i.id == inst)
            .map(|i| i.binding_set.channels.clone())
    }
}

#[cfg(test)]
impl Engine {
    /// it should expose instance channel keys to tests to validate BindingSet construction
    pub fn __test_get_instance_channels(&self, inst: InstId) -> Option<Vec<ChannelKey>> {
        self.instances
            .iter()
            .find(|i| i.id == inst)
            .map(|i| i.binding_set.channels.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding::TargetHandle;
    use crate::data::{Keypoint, Track};
    use crate::inputs::PlayerCommand;
    use crate::value::TrackValue;
    use vizij_api_core::Value;

    /// A track holding `value` at every time.
    fn constant_track(key: &str, value: f32) -> Track {
        Track {
            id: format!("{key}-track"),
            name: key.into(),
            animatable_id: key.into(),
            points: vec![Keypoint {
                id: format!("{key}-k0"),
                stamp: 0.0,
                value: TrackValue::Float(value),
                transitions: None,
            }],
            settings: None,
        }
    }

    fn animation(duration_ms: u32, tracks: Vec<Track>) -> AnimationData {
        AnimationData {
            id: None,
            name: "a".into(),
            tracks,
            groups: Default::default(),
            duration_ms,
        }
    }

    /// An engine with one 10 s animation on one player.
    fn engine() -> (Engine, AnimId, PlayerId, InstId) {
        let mut eng = Engine::new(Config::default());
        let anim = eng.load_animation(animation(10_000, vec![constant_track("x", 0.5)]));
        let player = eng.create_player("p");
        let inst = eng.add_instance(player, anim, InstanceCfg::default());
        (eng, anim, player, inst)
    }

    fn command(eng: &mut Engine, cmd: PlayerCommand) {
        eng.update(
            0.0,
            Inputs {
                player_cmds: vec![cmd],
                instance_updates: Vec::new(),
            },
        );
    }

    fn info(eng: &Engine, player: PlayerId) -> PlayerInfo {
        eng.list_players()
            .into_iter()
            .find(|p| p.id == player.0)
            .expect("player")
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-5, "{a} != {b}");
    }

    #[test]
    fn a_new_player_plays_at_speed_1() {
        let (mut eng, _, player, _) = engine();
        let p = info(&eng, player);
        assert_eq!((p.state, p.speed), (PlaybackState::Playing, 1.0));
        eng.update(1.0, Inputs::default());
        approx(info(&eng, player).time, 1.0);
    }

    #[test]
    fn pause_keeps_the_speed_and_play_resumes_at_it() {
        let (mut eng, _, player, _) = engine();
        command(&mut eng, PlayerCommand::SetSpeed { player, speed: 0.5 });
        eng.update(1.0, Inputs::default());
        approx(info(&eng, player).time, 0.5);

        command(&mut eng, PlayerCommand::Pause { player });
        eng.update(1.0, Inputs::default());
        let p = info(&eng, player);
        assert_eq!((p.state, p.speed), (PlaybackState::Paused, 0.5));
        approx(p.time, 0.5);

        command(&mut eng, PlayerCommand::Play { player });
        eng.update(1.0, Inputs::default());
        let p = info(&eng, player);
        assert_eq!((p.state, p.speed), (PlaybackState::Playing, 0.5));
        approx(p.time, 1.0);
    }

    #[test]
    fn stop_resets_the_time_and_reads_stopped() {
        let (mut eng, _, player, _) = engine();
        command(&mut eng, PlayerCommand::SetSpeed { player, speed: 2.0 });
        eng.update(1.0, Inputs::default());
        command(&mut eng, PlayerCommand::Stop { player });
        eng.update(1.0, Inputs::default());
        let p = info(&eng, player);
        assert_eq!((p.state, p.speed), (PlaybackState::Stopped, 2.0));
        approx(p.time, 0.0);

        command(&mut eng, PlayerCommand::Play { player });
        eng.update(1.0, Inputs::default());
        approx(info(&eng, player).time, 2.0);
    }

    #[test]
    fn set_speed_while_paused_does_not_resume() {
        let (mut eng, _, player, _) = engine();
        eng.update(1.0, Inputs::default());
        command(&mut eng, PlayerCommand::Pause { player });
        command(&mut eng, PlayerCommand::SetSpeed { player, speed: 3.0 });
        eng.update(1.0, Inputs::default());
        let p = info(&eng, player);
        assert_eq!((p.state, p.speed), (PlaybackState::Paused, 3.0));
        approx(p.time, 1.0);

        command(&mut eng, PlayerCommand::Play { player });
        eng.update(1.0, Inputs::default());
        approx(info(&eng, player).time, 4.0);
    }

    #[test]
    fn a_seek_leaves_a_stopped_player_paused_where_it_seeks() {
        let (mut eng, _, player, _) = engine();
        command(&mut eng, PlayerCommand::Stop { player });
        command(&mut eng, PlayerCommand::Seek { player, time: 2.0 });
        eng.update(1.0, Inputs::default());
        let p = info(&eng, player);
        assert_eq!(p.state, PlaybackState::Paused);
        approx(p.time, 2.0);
    }

    struct Prefix;
    impl TargetResolver for Prefix {
        fn resolve(&mut self, path: &str) -> Option<TargetHandle> {
            Some(format!("bound/{path}"))
        }
    }

    #[test]
    fn replace_animation_keeps_the_playback_and_samples_the_new_tracks() {
        let (mut eng, anim, player, inst) = engine();
        eng.prebind(&mut Prefix);
        eng.update(
            0.0,
            Inputs {
                player_cmds: vec![PlayerCommand::SetSpeed { player, speed: 0.5 }],
                instance_updates: vec![crate::inputs::InstanceUpdate {
                    player,
                    inst,
                    weight: Some(0.25),
                    time_scale: None,
                    start_offset: None,
                    enabled: None,
                }],
            },
        );
        eng.update(2.0, Inputs::default());
        command(&mut eng, PlayerCommand::Pause { player });
        let before = info(&eng, player);
        approx(before.time, 1.0);
        approx(before.length, 10.0);

        let replacement = animation(
            20_000,
            vec![constant_track("x", 0.5), constant_track("y", 0.75)],
        );
        assert!(eng.replace_animation(anim, replacement));

        let after = info(&eng, player);
        assert_eq!((after.state, after.speed), (PlaybackState::Paused, 0.5));
        approx(after.time, 1.0);
        approx(after.length, 20.0);
        let instances = eng.list_instances(player);
        assert_eq!(instances.len(), 1);
        assert_eq!((instances[0].id, instances[0].cfg.weight), (inst.0, 0.25));
        assert_eq!(eng.get_instance_channels(inst).map(|c| c.len()), Some(2));
        assert_eq!(eng.list_animations()[0].id, anim.0);
        assert_eq!(eng.list_animations()[0].track_count, 2);

        // The bindings named the old tracks: the new ones write their paths.
        assert!(eng.binds.rows.iter().all(|r| r.channel.anim != anim));
        let out = eng.update(0.0, Inputs::default());
        let value = |key: &str| {
            out.changes
                .iter()
                .find(|c| c.key == key)
                .map(|c| c.value.clone())
        };
        assert_eq!(
            value("y"),
            Some(Value::F32(0.75)),
            "the added track samples"
        );
        assert_eq!(value("x"), Some(Value::F32(0.5)));
        assert_eq!(value("bound/x"), None);
    }

    #[test]
    fn replacing_an_animation_not_loaded_changes_nothing() {
        let (mut eng, anim, player, _) = engine();
        assert!(!eng.replace_animation(AnimId(anim.0 + 1), animation(1_000, Vec::new())));
        approx(info(&eng, player).length, 10.0);
    }
}
