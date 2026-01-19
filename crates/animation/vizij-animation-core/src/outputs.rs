#![allow(dead_code)]
//! Output contracts from the core engine.
//!
//! Outputs carry per-tick value changes keyed by stable target handles, plus
//! semantic events emitted during playback. Adapters (Bevy/WASM) apply changes
//! to their hosts and forward events.

use serde::{Deserialize, Serialize};

use crate::ids::PlayerId;
use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};

/// One changed target value for a given player this tick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    /// Player that produced the change.
    pub player: PlayerId,
    /// Target handle (resolved binding key or canonical path).
    pub key: String,
    /// Sampled value for this tick.
    pub value: Value,
}

/// Change paired with an optional derivative value.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChangeWithDerivative {
    /// Player that produced the change.
    pub player: PlayerId,
    /// Target handle (resolved binding key or canonical path).
    pub key: String,
    /// Sampled value for this tick.
    pub value: Value,
    /// Optional derivative value (only for numeric kinds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derivative: Option<Value>,
}

/// Discrete semantic signals emitted during stepping.
///
/// Events are best-effort diagnostics intended for UI or tooling; core playback
/// does not require them for correctness.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CoreEvent {
    /// Player entered a playing state.
    PlaybackStarted {
        /// Player that started playback.
        player: PlayerId,
        /// Optional animation display name if known.
        animation: Option<String>,
    },
    /// Player entered a paused state.
    PlaybackPaused {
        /// Player that paused.
        player: PlayerId,
    },
    /// Player entered a stopped state and rewound.
    PlaybackStopped {
        /// Player that stopped.
        player: PlayerId,
    },
    /// Player resumed after being paused.
    PlaybackResumed {
        /// Player that resumed.
        player: PlayerId,
    },
    /// Player reached the end of its window or clip.
    PlaybackEnded {
        /// Player that ended.
        player: PlayerId,
        /// Playback time (seconds) when the end was reached.
        animation_time: f32,
    },
    /// Player time changed via explicit seek.
    TimeChanged {
        /// Player whose time changed.
        player: PlayerId,
        /// Previous time in seconds.
        old_time: f32,
        /// New time in seconds.
        new_time: f32,
    },
    /// A keypoint was crossed while sampling.
    KeypointReached {
        /// Player that crossed the keypoint.
        player: PlayerId,
        /// Canonical track path for the keypoint.
        track_path: String,
        /// Index of the keypoint in the track.
        key_index: usize,
        /// Sampled value at the keypoint.
        value: Value,
        /// Playback time (seconds) when the keypoint was reached.
        animation_time: f32,
    },
    /// Runtime instrumentation emitted a warning (e.g., sample budget).
    PerformanceWarning {
        /// Warning metric identifier.
        metric: String,
        /// Observed value for the metric.
        value: f32,
        /// Threshold that was exceeded.
        threshold: f32,
    },
    /// Non-fatal runtime error or warning surfaced as an event.
    Error {
        /// Human-readable error message.
        message: String,
    },
    /// Catch-all for forward-compatible payloads.
    Custom {
        /// Event kind identifier.
        kind: String,
        /// Opaque payload for downstream consumers.
        data: serde_json::Value,
    },
}

/// Outputs returned by [`Engine::update_values`](crate::engine::Engine::update_values).
///
/// # Examples
/// ```rust
/// use vizij_animation_core::outputs::{Change, Outputs};
/// use vizij_animation_core::PlayerId;
/// use vizij_api_core::Value;
///
/// let mut outputs = Outputs::default();
/// outputs.push_change(Change {
///     player: PlayerId(1),
///     key: "Root/Transform.translation".into(),
///     value: Value::Vec3([0.0, 1.0, 2.0]),
/// });
/// assert_eq!(outputs.changes.len(), 1);
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Outputs {
    /// Sampled changes for this tick, keyed by resolved handle or canonical path.
    #[serde(default)]
    pub changes: Vec<Change>,
    /// Semantic events emitted during this tick.
    #[serde(default)]
    pub events: Vec<CoreEvent>,
}

/// Outputs returned when derivatives are requested.
///
/// Derivative values are provided only for numeric kinds; non-numeric tracks produce `None`.
///
/// # Examples
/// ```rust
/// use vizij_animation_core::outputs::{ChangeWithDerivative, OutputsWithDerivatives};
/// use vizij_animation_core::PlayerId;
/// use vizij_api_core::Value;
///
/// let mut outputs = OutputsWithDerivatives::default();
/// outputs.push_change(ChangeWithDerivative {
///     player: PlayerId(1),
///     key: "Root/Transform.translation".into(),
///     value: Value::Vec3([0.0, 0.0, 0.0]),
///     derivative: Some(Value::Vec3([1.0, 0.0, 0.0])),
/// });
/// assert_eq!(outputs.changes.len(), 1);
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OutputsWithDerivatives {
    /// Sampled changes with derivative metadata for this tick.
    #[serde(default)]
    pub changes: Vec<ChangeWithDerivative>,
    /// Semantic events emitted during this tick.
    #[serde(default)]
    pub events: Vec<CoreEvent>,
}

impl Outputs {
    /// Clear all accumulated changes and events.
    #[inline]
    pub fn clear(&mut self) {
        self.changes.clear();
        self.events.clear();
    }

    /// Append a sampled change.
    #[inline]
    pub fn push_change(&mut self, change: Change) {
        self.changes.push(change);
    }

    /// Append a semantic event.
    #[inline]
    pub fn push_event(&mut self, event: CoreEvent) {
        self.events.push(event);
    }

    /// Return true when both changes and events are empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty() && self.events.is_empty()
    }

    /// Convert the current set of changes into a [`WriteBatch`], parsing each
    /// change key as a [`TypedPath`].
    ///
    /// Entries whose keys do not parse are skipped.
    ///
    /// This is useful when piping animation outputs into systems that expect typed paths.
    /// Order matches the order of `changes`, skipping entries that fail to parse.
    ///
    /// # Examples
    /// ```rust
    /// use vizij_animation_core::outputs::{Change, Outputs};
    /// use vizij_animation_core::PlayerId;
    /// use vizij_api_core::Value;
    ///
    /// let mut outputs = Outputs::default();
    /// outputs.push_change(Change {
    ///     player: PlayerId(1),
    ///     key: "not a typed path".into(),
    ///     value: Value::Bool(true),
    /// });
    /// let batch = outputs.to_writebatch();
    /// assert_eq!(batch.iter().count(), 0);
    /// ```
    pub fn to_writebatch(&self) -> WriteBatch {
        let mut batch = WriteBatch::new();
        for change in &self.changes {
            if let Ok(path) = TypedPath::parse(&change.key) {
                batch.push(WriteOp::new(path, change.value.clone()));
            }
        }
        batch
    }
}

impl OutputsWithDerivatives {
    /// Clear all accumulated changes and events.
    #[inline]
    pub fn clear(&mut self) {
        self.changes.clear();
        self.events.clear();
    }

    /// Append a sampled change with derivative.
    #[inline]
    pub fn push_change(&mut self, change: ChangeWithDerivative) {
        self.changes.push(change);
    }

    /// Append a semantic event.
    #[inline]
    pub fn push_event(&mut self, event: CoreEvent) {
        self.events.push(event);
    }

    /// Return true when both changes and events are empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty() && self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vizij_api_core::Value;

    #[test]
    fn to_writebatch_skips_invalid_paths() {
        let mut outputs = Outputs::default();
        outputs.push_change(Change {
            player: PlayerId(1),
            key: "anim/player/1/cmd/play".into(),
            value: Value::Bool(true),
        });
        outputs.push_change(Change {
            player: PlayerId(2),
            key: "not a typed path".into(),
            value: Value::Float(1.0),
        });

        let batch = outputs.to_writebatch();
        assert_eq!(batch.iter().count(), 1);
        let op = batch.iter().next().unwrap();
        assert_eq!(op.path.to_string(), "anim/player/1/cmd/play");
        assert!(matches!(op.value, Value::Bool(true)));
    }
}
