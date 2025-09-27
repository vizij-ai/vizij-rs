#![allow(dead_code)]
//! Output contracts from the core engine.
//!
//! Outputs carry only the numeric/value changes for this tick, keyed by
//! stable string TargetHandle, and a separate list of semantic events.
//! Adapters (Bevy/WASM) apply changes to the host and transport events.

use serde::{Deserialize, Serialize};

use crate::ids::PlayerId;
use vizij_api_core::Value;

fn default_zero_derivative() -> Value {
    Value::Float(0.0)
}

/// One changed target value for a given player this tick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub player: PlayerId,
    pub key: String, // TargetHandle (small string key)
    pub value: Value,
}

/// Change paired with a derivative value.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChangeWithDerivative {
    pub player: PlayerId,
    pub key: String,
    pub value: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derivative: Option<Value>,
}

/// Discrete semantic signals emitted during stepping.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CoreEvent {
    PlaybackStarted {
        player: PlayerId,
        animation: Option<String>,
    },
    PlaybackPaused {
        player: PlayerId,
    },
    PlaybackStopped {
        player: PlayerId,
    },
    PlaybackResumed {
        player: PlayerId,
    },
    PlaybackEnded {
        player: PlayerId,
        animation_time: f32,
    },
    TimeChanged {
        player: PlayerId,
        old_time: f32,
        new_time: f32,
    },
    KeypointReached {
        player: PlayerId,
        track_path: String,
        key_index: usize,
        value: Value,
        animation_time: f32,
    },
    PerformanceWarning {
        metric: String,
        value: f32,
        threshold: f32,
    },
    Error {
        message: String,
    },
    /// Catch-all for forward-compatible payloads.
    Custom {
        kind: String,
        data: serde_json::Value,
    },
}

/// Outputs returned by Engine::update().
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Outputs {
    #[serde(default)]
    pub changes: Vec<Change>,
    #[serde(default)]
    pub events: Vec<CoreEvent>,
}

/// Outputs returned when derivatives are requested.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OutputsWithDerivatives {
    #[serde(default)]
    pub changes: Vec<ChangeWithDerivative>,
    #[serde(default)]
    pub events: Vec<CoreEvent>,
}

impl Outputs {
    #[inline]
    pub fn clear(&mut self) {
        self.changes.clear();
        self.events.clear();
    }

    #[inline]
    pub fn push_change(&mut self, change: Change) {
        self.changes.push(change);
    }

    #[inline]
    pub fn push_event(&mut self, event: CoreEvent) {
        self.events.push(event);
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty() && self.events.is_empty()
    }
}

impl OutputsWithDerivatives {
    #[inline]
    pub fn clear(&mut self) {
        self.changes.clear();
        self.events.clear();
    }

    #[inline]
    pub fn push_change(&mut self, change: ChangeWithDerivative) {
        self.changes.push(change);
    }

    #[inline]
    pub fn push_event(&mut self, event: CoreEvent) {
        self.events.push(event);
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty() && self.events.is_empty()
    }
}
