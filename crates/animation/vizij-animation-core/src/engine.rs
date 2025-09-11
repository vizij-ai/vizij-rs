#![allow(dead_code)]
//! Engine: data ownership and public API with time math + sampling/accumulate/blend (v1).
//!
//! Methods:
//! - new, load_animation, create_player, add_instance, prebind (resolver), update (accumulate → blend)

use crate::accumulate::Accumulator;
use crate::binding::{BindingSet, BindingTable, ChannelKey, TargetResolver};
use crate::config::Config;
use crate::data::AnimationData;
use crate::ids::{AnimId, IdAllocator, InstId, PlayerId};
use crate::inputs::{Inputs, LoopMode};
use crate::interp::InterpRegistry;
use crate::outputs::{Change, Outputs};
use crate::sampling::sample_track;
use crate::scratch::Scratch;
use serde::{Deserialize, Serialize};

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
#[derive(Clone, Debug, Serialize, Deserialize)]
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

impl Engine {
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
        }
    }

    /// Load animation data into the engine, returning an AnimId.
    pub fn load_animation(&mut self, mut data: AnimationData) -> AnimId {
        let id = self.ids.alloc_anim();
        data.id = Some(id);
        self.anims.insert(id, data);
        id
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

    /// Recalculate a player's effective total duration from its instances and window.
    fn recalc_player_duration(&mut self, player: PlayerId) {
        if let Some(p) = self.players.iter_mut().find(|pp| pp.id == player) {
            // Compute the required span of player time to traverse each instance's remaining local range.
            // local_time = player_time * time_scale + start_offset
            // Remaining local range (forward):
            //   if time_scale >= 0: max(0, anim_duration - start_offset)
            //   if time_scale <  0: max(0, start_offset - 0)
            // Player span for instance = remaining_local / abs(time_scale)
            let mut max_span = 0.0f32;
            for iid in &p.instances {
                if let Some(inst) = self.instances.iter().find(|ii| ii.id == *iid) {
                    if let Some(anim) = self.anims.get(inst.anim) {
                        let anim_duration = anim.duration_ms as f32 / 1000.0;
                        let ts = inst.time_scale;
                        let ts_abs = ts.abs().max(1e-6);
                        let remaining_local = if ts >= 0.0 {
                            (anim_duration - inst.start_offset).max(0.0)
                        } else {
                            (inst.start_offset - 0.0).max(0.0)
                        };
                        let span = remaining_local / ts_abs;
                        if span > max_span {
                            max_span = span;
                        }
                    }
                }
            }
            // Apply player window if specified; window defines the allowed player time domain.
            if let Some(end) = p.end_time {
                let window_len = (end - p.start_time).max(0.0);
                p.total_duration = window_len.min(max_span);
            } else {
                p.total_duration = max_span;
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
        // Map player time into clip time using instance scale and offset:
        // local_t = player.time * time_scale + start_offset
        let base = player.time * inst.time_scale + inst.start_offset;
        if anim_duration <= 0.0 {
            return 0.0;
        }
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

    /// Step the simulation by dt with given inputs, producing outputs.
    /// v1: Apply inputs and advance times; sample tracks → accumulate → blend; emit per-player changes.
    pub fn update(&mut self, dt: f32, inputs: Inputs) -> &Outputs {
        // Begin frame for scratch buffers.
        self.scratch.begin_frame();
        self.outputs.clear();

        // 1) Apply player/instance commands
        self.apply_inputs(inputs);

        // 2) Advance player times (speed)
        self.advance_player_times(dt);

        // 3) For each player, accumulate contributions across enabled instances
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

                    // Iterate all channels referenced by this instance
                    for ch in &inst.binding_set.channels {
                        if ch.anim != inst.anim {
                            continue;
                        }
                        let idx = ch.track_idx as usize;
                        if let Some(track) = anim_data.tracks.get(idx) {
                            // Skip tracks with no keys to avoid emitting meaningless changes
                            if track.points.is_empty() {
                                continue;
                            }
                            let u = if anim_duration_s > 0.0 {
                                (local_t / anim_duration_s).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };
                            let value = sample_track(track, u);
                            // Resolve handle if bound, else fallback to canonical path
                            let handle = if let Some(row) = self.binds.get(*ch) {
                                row.handle.as_str()
                            } else {
                                track.animatable_id.as_str()
                            };
                            accum.add(handle, &value, inst.weight);
                        }
                    }
                }
            }

            // 4) Finalize accumulator → write Outputs as changes
            let blended = accum.finalize();
            for (key, value) in blended.into_iter() {
                self.outputs.push_change(Change {
                    player: p.id,
                    key,
                    value,
                });
            }
        }

        &self.outputs
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
