#![allow(dead_code)]
//! Input contracts for the core engine.
//!
//! Inputs bundle per-player commands and per-instance updates. Adapters (Bevy/WASM)
//! build these and pass them into `Engine::update_values` each tick.

use serde::{Deserialize, Serialize};

use crate::ids::{InstId, PlayerId};

/// Input bundle applied before stepping the engine.
///
/// # Examples
/// ```rust
/// use vizij_animation_core::{Inputs, PlayerCommand, PlayerId};
///
/// let mut inputs = Inputs::default();
/// inputs.player_cmds.push(PlayerCommand::Play { player: PlayerId(1) });
/// assert_eq!(inputs.player_cmds.len(), 1);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Inputs {
    /// Player-level commands applied before stepping.
    #[serde(default)]
    pub player_cmds: Vec<PlayerCommand>,
    /// Instance-level updates applied before stepping.
    #[serde(default)]
    pub instance_updates: Vec<InstanceUpdate>,
}

/// Player-scoped commands for playback control.
///
/// Commands are applied in order before stepping the engine.
///
/// # Examples
/// ```rust
/// use vizij_animation_core::{Inputs, LoopMode, PlayerCommand, PlayerId};
///
/// let mut inputs = Inputs::default();
/// inputs.player_cmds.push(PlayerCommand::SetLoopMode {
///     player: PlayerId(7),
///     mode: LoopMode::PingPong,
/// });
/// assert_eq!(inputs.player_cmds.len(), 1);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlayerCommand {
    /// Resume playback (sets speed to 1.0 if currently paused).
    Play { player: PlayerId },
    /// Pause playback (sets speed to 0.0).
    Pause { player: PlayerId },
    /// Stop and rewind to the window start.
    Stop { player: PlayerId },
    /// Set absolute playback speed multiplier (negative reverses playback).
    SetSpeed { player: PlayerId, speed: f32 },
    /// Seek the player's time to a specific value (seconds, in player time).
    Seek { player: PlayerId, time: f32 },
    /// Update the loop mode used for sampling.
    SetLoopMode { player: PlayerId, mode: LoopMode },
    /// Apply a playback window in seconds.
    SetWindow {
        player: PlayerId,
        start_time: f32,
        end_time: Option<f32>,
    },
}

/// Looping behavior applied when mapping player time to clip time.
///
/// This affects both display time in diagnostics and how instances sample tracks.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LoopMode {
    /// Clamp playback to the window `[start_time, end_time?]`.
    Once,
    /// Wrap player time across the full clip duration.
    Loop,
    /// Reflect player time across the full clip duration.
    PingPong,
}

/// Per-instance updates applied before stepping.
///
/// Any field set to `None` leaves the current instance state unchanged.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceUpdate {
    /// Owning player id (used to recompute duration).
    pub player: PlayerId,
    /// Instance id to update.
    pub inst: InstId,
    #[serde(default)]
    /// Optional weight override.
    pub weight: Option<f32>,
    #[serde(default)]
    /// Optional time scale override (negative reverses playback).
    pub time_scale: Option<f32>,
    #[serde(default)]
    /// Optional start offset override (seconds in player time).
    pub start_offset: Option<f32>,
    #[serde(default)]
    /// Optional enabled flag override.
    pub enabled: Option<bool>,
}
