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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlayerCommand {
    Play {
        player: PlayerId,
    },
    Pause {
        player: PlayerId,
    },
    Stop {
        player: PlayerId,
    },
    SetSpeed {
        player: PlayerId,
        speed: f32,
    },
    Seek {
        player: PlayerId,
        time: f32,
    },
    SetLoopMode {
        player: PlayerId,
        mode: LoopMode,
    },
    SetWindow {
        player: PlayerId,
        start_time: f32,
        end_time: Option<f32>,
    },
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LoopMode {
    Once,
    Loop,
    PingPong,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceUpdate {
    pub player: PlayerId,
    pub inst: InstId,
    #[serde(default)]
    pub weight: Option<f32>,
    #[serde(default)]
    pub time_scale: Option<f32>,
    #[serde(default)]
    pub start_offset: Option<f32>,
    #[serde(default)]
    pub enabled: Option<bool>,
}
