#![allow(dead_code)]
//! Engine: data ownership and public API with time math + sampling/accumulate/blend (v1).
//!
//! Methods:
//! - new, load_animation, create_player, add_instance, prebind (resolver), update (accumulate → blend)

use crate::accumulate::Accumulator;
use crate::baking::{
    bake_animation_data, bake_animation_data_with_derivatives, BakedAnimationData,
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
use std::collections::HashSet;
use vizij_api_core::{TypedPath, WriteBatch, WriteOp};

/// Per-player controller and instance list.
#[derive(Debug)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub speed: f32,
    pub time: f32,
    pub mode: LoopMode,
    pub start_time: f32,
    pub end_time: Option<f32>,
    pub instances: Vec<InstId>,
    /// Effective total duration in player time, computed from instances (offsets/scales) and window.
    pub total_duration: f32,
}

impl Player {
    fn new(id: PlayerId, name: String) -> Self {
        Self {
            id,
            name,
            speed: 1.0,
            time: 0.0,
            mode: LoopMode::Loop,
            start_time: 0.0,
            end_time: None,
            instances: Vec::new(),
            total_duration: 0.0,
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
    pub start_offset: f32,
    pub enabled: bool,
    pub binding_set: BindingSet,
}

/// Configuration for adding an instance.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InstanceCfg {
    pub weight: f32,
    pub time_scale: f32,
    pub start_offset: f32,
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

    // Per-tick outputs
    outputs: Outputs,
    outputs_with_derivatives: OutputsWithDerivatives,
}

fn fmod(a: f32, b: f32) -> f32 {
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
fn ping_pong(t: f32, span: f32) -> f32 {
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnimationInfo {
    pub id: u32,
    pub name: Option<String>,
    pub duration_ms: u32,
    pub track_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerInfo {
    pub id: u32,
    pub name: String,
    pub state: PlaybackState,
    pub time: f32,
    pub speed: f32,
    pub loop_mode: LoopMode,
    pub start_time: f32,
    pub end_time: Option<f32>,
    /// Full player length (seconds): max over instances of start_offset + (anim_duration * |time_scale|)
    pub length: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InstanceInfo {
    pub id: u32,
    pub animation: u32,
    pub cfg: InstanceCfg,
}

impl Engine {
    /// Map player's internal time to a display/playhead time according to loop mode.
    /// Semantics:
    /// - Once: apply window clamp [start_time, end_time?], otherwise [start_time, start_time+total_duration]
    /// - Loop: ignore window, wrap over full clip [0, total_duration)
    /// - PingPong: ignore window, reflect over full clip [0, total_duration]
    fn map_player_time_for_display(&self, p: &Player) -> f32 {
        match p.mode {
            LoopMode::Once => {
                let start = p.start_time.max(0.0);
                let span = if let Some(end) = p.end_time {
                    (end - start).max(0.0)
                } else {
                    (p.total_duration - start).max(0.0)
                };
                if span <= 0.0 {
                    start
                } else {
                    p.time.clamp(start, start + span)
                }
            }
            LoopMode::Loop | LoopMode::PingPong => {
                // Compute full (unwindowed) span across instances
                let mut full_span = 0.0f32;
                for iid in &p.instances {
                    if let Some(inst) = self.instances.iter().find(|ii| ii.id == *iid) {
                        if let Some(anim) = self.anims.get(inst.anim) {
                            let anim_duration = anim.duration_ms as f32 / 1000.0;
                            let ts_abs = inst.time_scale.abs().max(1e-6);
                            let end_time = inst.start_offset + (anim_duration * ts_abs);
                            if end_time > full_span {
                                full_span = end_time;
                            }
                        }
                    }
                }
                if full_span <= 0.0 {
                    0.0
                } else if matches!(p.mode, LoopMode::Loop) {
                    let m = fmod(p.time, full_span);
                    if m < 0.0 {
                        m + full_span
                    } else {
                        m
                    }
                } else {
                    // PingPong over full span
                    ping_pong(p.time, full_span)
                }
            }
        }
    }
    /// Public accessor for a player's computed total duration (in player time).
    pub fn player_total_duration(&self, player: PlayerId) -> Option<f32> {
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
            outputs: Outputs::default(),
            outputs_with_derivatives: OutputsWithDerivatives::default(),
        }
    }

    /// Load animation data into the engine, returning an AnimId.
    pub fn load_animation(&mut self, mut data: AnimationData) -> AnimId {
        let id = self.ids.alloc_anim();
        data.id = Some(id);
        self.anims.insert(id, data);
        id
    }

    /// Bake a stored animation into per-frame samples using the provided config.
    pub fn bake_animation(&self, anim: AnimId, cfg: &BakingConfig) -> Option<BakedAnimationData> {
        self.anims
            .get(anim)
            .map(|data| bake_animation_data(anim, data, cfg))
    }

    /// Bake animation values and derivatives in one pass.
    pub fn bake_animation_with_derivatives(
        &self,
        anim: AnimId,
        cfg: &BakingConfig,
    ) -> Option<(BakedAnimationData, BakedDerivativeAnimationData)> {
        self.anims
            .get(anim)
            .map(|data| bake_animation_data_with_derivatives(anim, data, cfg))
    }

    /// Create a new player with a display name.
    pub fn create_player(&mut self, name: &str) -> PlayerId {
        let pid = self.ids.alloc_player();
        self.players.push(Player::new(pid, name.to_string()));
        pid
    }

    /// Add an animation instance to a player.
    pub fn add_instance(&mut self, player: PlayerId, anim: AnimId, cfg: InstanceCfg) -> InstId {
        let iid = self.ids.alloc_inst();

        // Build binding set for this instance (all channels of the animation).
        let mut binding_set = BindingSet::default();
        if let Some(anim_data) = self.anims.get(anim) {
            for (idx, _) in anim_data.tracks.iter().enumerate() {
                binding_set.channels.push(ChannelKey {
                    anim,
                    track_idx: idx as u32,
                });
            }
        }

        let instance = Instance {
            id: iid,
            anim,
            weight: cfg.weight,
            time_scale: cfg.time_scale,
            start_offset: cfg.start_offset,
            enabled: cfg.enabled,
            binding_set,
        };
        self.instances.push(instance);

        // Attach to player
        if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
            p.instances.push(iid);
        }
        // Recompute player's effective total duration
        self.recalc_player_duration(player);
        iid
    }

    /// One-time binding against a provided resolver.
    /// Iterates all animations/tracks and resolves canonical target paths into handles.
    pub fn prebind(&mut self, resolver: &mut dyn TargetResolver) {
        for (anim_id, data) in self.anims.iter() {
            for (idx, track) in data.tracks.iter().enumerate() {
                if let Some(handle) = resolver.resolve(&track.animatable_id) {
                    self.binds.upsert(
                        ChannelKey {
                            anim: *anim_id,
                            track_idx: idx as u32,
                        },
                        handle,
                    );
                }
            }
        }
    }

    /// Recalculate a player's full length (seconds) from its instances.
    /// length = max over instances of: start_offset + (anim_duration * |time_scale|)
    fn recalc_player_duration(&mut self, player: PlayerId) {
        if let Some(p) = self.players.iter_mut().find(|pp| pp.id == player) {
            let mut max_end = 0.0f32;
            for iid in &p.instances {
                if let Some(inst) = self.instances.iter().find(|ii| ii.id == *iid) {
                    if let Some(anim) = self.anims.get(inst.anim) {
                        let anim_duration = anim.duration_ms as f32 / 1000.0;
                        let ts_abs = inst.time_scale.abs().max(1e-6);
                        let end_time = inst.start_offset + (anim_duration * ts_abs);
                        if end_time > max_end {
                            max_end = end_time;
                        }
                    }
                }
            }
            // Apply window clamp if configured
            if let Some(end) = p.end_time {
                let window_len = (end - p.start_time).max(0.0);
                p.total_duration = max_end.min(window_len);
            } else {
                p.total_duration = max_end;
            }
        }
    }

    /// Apply player/instance commands (minimal semantics for v1 skeleton).
    fn apply_inputs(&mut self, inputs: Inputs) {
        // Player commands
        for cmd in inputs.player_cmds {
            match cmd {
                crate::inputs::PlayerCommand::Play { player } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        if p.speed == 0.0 {
                            p.speed = 1.0;
                        }
                    }
                }
                crate::inputs::PlayerCommand::Pause { player } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        p.speed = 0.0;
                    }
                }
                crate::inputs::PlayerCommand::Stop { player } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        p.speed = 0.0;
                        p.time = p.start_time;
                    }
                }
                crate::inputs::PlayerCommand::SetSpeed { player, speed } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        p.speed = speed;
                    }
                }
                crate::inputs::PlayerCommand::Seek { player, time } => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
                        p.time = time;
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
                        p.start_time = start_time.max(0.0);
                        p.end_time = end_time.map(|e| e.max(p.start_time));
                        // Optionally clamp current time into window
                        if p.time < p.start_time {
                            p.time = p.start_time;
                        }
                        if let Some(e) = p.end_time {
                            if p.time > e {
                                p.time = e;
                            }
                        }
                    }
                    // Recompute player's duration when window changes
                    self.recalc_player_duration(player);
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

    /// Advance logical time. Loop/windowing is applied when mapping to instance local time.
    fn advance_player_times(&mut self, dt: f32) {
        for p in &mut self.players {
            p.time += dt * p.speed;
            // Clamp into window for Once mode convenience (optional; local mapping will also enforce)
            if let Some(end) = p.end_time {
                if p.time > end && matches!(p.mode, crate::inputs::LoopMode::Once) {
                    p.time = end;
                }
            }
            if p.time < p.start_time && matches!(p.mode, crate::inputs::LoopMode::Once) {
                p.time = p.start_time;
            }
        }
    }

    /// Compute instance-local time given a player and animation duration under the player's loop mode.
    fn local_time_for_instance(&self, player: &Player, inst: &Instance, anim_duration: f32) -> f32 {
        // Interpret start_offset as a player-time shift (when the instance starts).
        // Interpret time_scale as a duration multiplier (|ts| > 1 => longer, |ts| < 1 => shorter).
        // Mapping from player time to clip local time:
        //   base = (player.time - inst.start_offset) / inst.time_scale
        // Before the instance starts (player.time < start_offset), we must NOT wrap:
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
        let ts = inst.time_scale;
        // Compute display-mapped player time (already windowed/looped)
        let t_display = self.map_player_time_for_display(player);
        let rel_cycle = t_display;
        if rel_cycle <= 0.0 {
            // At the very start of a cycle, hold initial value before any instance starts.
            return 0.0;
        }
        let rel = rel_cycle - inst.start_offset;
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

    fn step(&mut self, dt: f32, inputs: Inputs, with_derivatives: bool) {
        self.scratch.begin_frame();
        self.outputs.clear();
        if with_derivatives {
            self.outputs_with_derivatives.clear();
        }

        self.apply_inputs(inputs);
        self.advance_player_times(dt);

        for p in &self.players {
            let mut accum = Accumulator::new();

            for iid in &p.instances {
                if let Some(inst) = self.instances.iter().find(|i| i.id == *iid) {
                    if !inst.enabled {
                        continue;
                    }
                    let anim_data = if let Some(a) = self.anims.get(inst.anim) {
                        a
                    } else {
                        continue;
                    };
                    let anim_duration_s = anim_data.duration_ms as f32 / 1000.0;
                    let local_t = self.local_time_for_instance(p, inst, anim_duration_s);

                    for ch in &inst.binding_set.channels {
                        if ch.anim != inst.anim {
                            continue;
                        }
                        let idx = ch.track_idx as usize;
                        if let Some(track) = anim_data.tracks.get(idx) {
                            if track.points.is_empty() {
                                continue;
                            }
                            let u = if anim_duration_s > 0.0 {
                                (local_t / anim_duration_s).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };
                            let (value, derivative) = if with_derivatives {
                                sample_track_with_derivative(track, u, anim_duration_s)
                            } else {
                                (sample_track(track, u), None)
                            };
                            let handle = if let Some(row) = self.binds.get(*ch) {
                                row.handle.as_str()
                            } else {
                                track.animatable_id.as_str()
                            };
                            accum.add_with_derivative(
                                handle,
                                &value,
                                derivative.as_ref(),
                                inst.weight,
                            );
                        }
                    }
                }
            }

            if with_derivatives {
                let blended = accum.finalize_with_derivatives();
                for (key, (value, derivative)) in blended.into_iter() {
                    self.outputs.push_change(Change {
                        player: p.id,
                        key: key.clone(),
                        value: value.clone(),
                    });
                    self.outputs_with_derivatives
                        .push_change(ChangeWithDerivative {
                            player: p.id,
                            key,
                            value,
                            derivative,
                        });
                }
            } else {
                let blended = accum.finalize();
                for (key, value) in blended.into_iter() {
                    self.outputs.push_change(Change {
                        player: p.id,
                        key,
                        value,
                    });
                }
            }
        }

        if with_derivatives {
            self.outputs_with_derivatives.events = self.outputs.events.clone();
        }
    }

    /// Step the simulation by dt with given inputs, returning only values.
    pub fn update_values(&mut self, dt: f32, inputs: Inputs) -> &Outputs {
        self.step(dt, inputs, false);
        &self.outputs
    }

    /// Step the simulation by dt returning values and derivatives.
    pub fn update_values_and_derivatives(
        &mut self,
        dt: f32,
        inputs: Inputs,
    ) -> &OutputsWithDerivatives {
        self.step(dt, inputs, true);
        &self.outputs_with_derivatives
    }

    /// Backwards-compatible alias for `update_values`.
    pub fn update(&mut self, dt: f32, inputs: Inputs) -> &Outputs {
        self.update_values(dt, inputs)
    }

    /// Update and also return a typed WriteBatch (collection of WriteOp) where each
    /// WriteOp.path is parsed as a `TypedPath`. If a change's key does not parse as a
    /// TypedPath it will be skipped in the returned batch. The engine still maintains
    /// its normal Outputs in `self.outputs`.
    pub fn update_writebatch(&mut self, dt: f32, inputs: Inputs) -> WriteBatch {
        // Populate self.outputs as usual.
        let _ = self.update_values(dt, inputs);

        let mut batch = WriteBatch::new();
        for ch in self.outputs.changes.iter() {
            // Parse the canonical path into a TypedPath; skip on parse error.
            if let Ok(tp) = TypedPath::parse(&ch.key) {
                batch.push(WriteOp::new(tp, ch.value.clone()));
            }
        }
        batch
    }

    /// Remove an instance from a player. Returns true if removed.
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
                return true;
            }
        }
        false
    }

    /// Remove a player and all its instances. Returns true if removed.
    pub fn remove_player(&mut self, player: PlayerId) -> bool {
        if let Some(idx) = self.players.iter().position(|p| p.id == player) {
            let inst_ids: Vec<InstId> = self.players[idx].instances.clone();
            // Remove all instances owned by this player
            if !inst_ids.is_empty() {
                self.instances.retain(|ii| !inst_ids.contains(&ii.id));
            }
            // Remove the player
            self.players.remove(idx);
            true
        } else {
            false
        }
    }

    /// Unload an animation and remove all instances referencing it across all players. Returns true if animation existed.
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
        self.anims.remove(anim)
    }

    /// List all animations in the engine.
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

    fn derive_playback_state(p: &Player) -> PlaybackState {
        if p.speed == 0.0 {
            if (p.time - p.start_time).abs() < 1e-6 {
                PlaybackState::Stopped
            } else {
                PlaybackState::Paused
            }
        } else {
            PlaybackState::Playing
        }
    }

    /// List all players with playback info and computed length.
    pub fn list_players(&self) -> Vec<PlayerInfo> {
        self.players
            .iter()
            .map(|p| PlayerInfo {
                id: p.id.0,
                name: p.name.clone(),
                state: Self::derive_playback_state(p),
                time: self.map_player_time_for_display(p),
                speed: p.speed,
                loop_mode: p.mode,
                start_time: p.start_time,
                end_time: p.end_time,
                length: p.total_duration,
            })
            .collect()
    }

    /// List all instances for a given player.
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
    /// Public helper to inspect an instance's bound channel keys (useful for tests and tooling)
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
