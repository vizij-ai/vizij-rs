#![allow(dead_code)]
//! Input contracts for the core engine.
//!
//! v1 keeps this minimal: per-player commands and per-instance updates. Adapters
//! (web and native hosts) build and pass these into Engine::update() each fixed tick.

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
    /// Start or resume playback for `player`, at its speed, from its time; one waiting for a
    /// [`PlayerCommand::PlayAfter`] start plays at once.
    Play { player: PlayerId },
    /// Hold the player's time where it is. The speed is kept for the next `Play`.
    Pause { player: PlayerId },
    /// Stop playback and reset time to the player's window start. The speed is kept for the
    /// next `Play`.
    Stop { player: PlayerId },
    /// Set the multiplier a playing player's time advances at; a negative speed plays backwards
    /// through the window. It does not start, pause or stop the player: a paused player stays
    /// paused, and a playing one at speed 0 holds its time while it reads as playing.
    SetSpeed { player: PlayerId, speed: f32 },
    /// Set the player's internal time in seconds. A stopped player is left paused there.
    Seek { player: PlayerId, time: f64 },
    /// Change how player time maps into clip-local time.
    SetLoopMode { player: PlayerId, mode: LoopMode },
    /// Set the play window, in seconds of player time: [`LoopMode::Once`] clamps the playhead
    /// into it, [`LoopMode::Loop`] wraps within it and [`LoopMode::PingPong`] reflects within it.
    ///
    /// `end_time: None` ends the window at the player's length. A playhead outside the new
    /// window moves to its nearest bound.
    SetWindow {
        player: PlayerId,
        start_time: f64,
        end_time: Option<f64>,
    },
    /// Start playback `delay` seconds after the start of the update that applies the command,
    /// holding the playhead until then: that update, or a later one, advances the player only by
    /// the time after the start. A negative `delay` starts it that long before the update, which
    /// then advances it by the extra time. The player keeps its state until the start, then
    /// plays at its speed, as [`PlayerCommand::Play`] leaves it; a later `Play`, `Pause` or
    /// `Stop` cancels the wait.
    PlayAfter { player: PlayerId, delay: f64 },
}

/// Loop policy used when mapping player time into clip-local time.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LoopMode {
    /// Clamp to the play window, holding at the bound playback reaches.
    Once,
    /// Wrap around the play window.
    Loop,
    /// Reflect back and forth across the play window.
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
    pub start_offset: Option<f64>,
    /// Replacement enabled state.
    #[serde(default)]
    pub enabled: Option<bool>,
}
