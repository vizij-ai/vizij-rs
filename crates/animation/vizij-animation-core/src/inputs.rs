#![allow(dead_code)]
//! Input contracts for the core engine.
//!
//! v1 keeps this minimal: per-player commands and per-instance updates. Adapters
//! (web/Bevy) build and pass these into Engine::update() each fixed tick.

use serde::{Deserialize, Serialize};

use crate::ids::{InstId, PlayerId};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Inputs {
    /// Player-level commands applied before stepping.
    #[serde(default)]
    pub player_cmds: Vec<PlayerCommand>,
    /// Instance-level updates applied before stepping.
    #[serde(default)]
    pub instance_updates: Vec<InstanceUpdate>,
}

/// Player-level command applied before instance updates and sampling.
///
/// Commands in one [`Inputs::player_cmds`] batch are processed in order, so later commands for the
/// same player observe the effects of earlier ones in the same tick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlayerCommand {
    /// Resume or start playback for `player`.
    Play { player: PlayerId },
    /// Pause playback by setting the player's speed to zero.
    Pause { player: PlayerId },
    /// Stop playback and reset time to the player's window start.
    Stop { player: PlayerId },
    /// Set the player's playback speed multiplier.
    SetSpeed { player: PlayerId, speed: f32 },
    /// Set the player's internal time in seconds.
    Seek { player: PlayerId, time: f32 },
    /// Change how player time maps into clip-local time.
    SetLoopMode { player: PlayerId, mode: LoopMode },
    /// Update the one-shot playback window in seconds.
    ///
    /// `end_time: None` clears the explicit end bound.
    SetWindow {
        player: PlayerId,
        start_time: f32,
        end_time: Option<f32>,
    },
}

/// Loop policy used when mapping player time into clip-local time.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LoopMode {
    /// Clamp to the configured window or clip end.
    Once,
    /// Wrap around the clip duration.
    Loop,
    /// Reflect back and forth across the clip duration.
    PingPong,
}

/// Partial update for one instance.
///
/// Fields left as `None` keep their existing values.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceUpdate {
    /// Player that owns the instance. Used for duration recomputation.
    pub player: PlayerId,
    /// Target instance id.
    pub inst: InstId,
    /// Replacement blend weight.
    #[serde(default)]
    pub weight: Option<f32>,
    /// Replacement playback scaling factor.
    #[serde(default)]
    pub time_scale: Option<f32>,
    /// Replacement start offset in seconds.
    #[serde(default)]
    pub start_offset: Option<f32>,
    /// Replacement enabled state.
    #[serde(default)]
    pub enabled: Option<bool>,
}
