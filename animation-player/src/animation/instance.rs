use crate::AnimationTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Defines how an animation instance should loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackMode {
    /// Play once and stop at the end.
    Once,
    /// Loop indefinitely from start to end.
    Loop,
    /// Loop indefinitely, playing forward then backward.
    PingPong,
}

impl Default for PlaybackMode {
    fn default() -> Self {
        PlaybackMode::Once
    }
}

/// Settings for a specific animation instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationInstanceSettings {
    /// The time at which this instance begins relative to the player's timeline.
    pub instance_start_time: AnimationTime,
    /// The playback speed multiplier for this instance.
    pub timescale: f64,
    /// Whether the instance is enabled for playback.
    pub enabled: bool,
}

/// Properties for a specific animation instance tracked at runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceProperties {
    /// Optional metadata for the instance.
    pub metadata: HashMap<String, String>,
}

impl Default for InstanceProperties {
    fn default() -> Self {
        Self {
            metadata: HashMap::new(),
        }
    }
}

impl AnimationInstanceSettings {
    /// Creates new default instance settings for a given animation ID.
    pub fn new() -> Self {
        Self {
            instance_start_time: AnimationTime::zero(),
            timescale: 1.0,
            enabled: true,
        }
    }
}

impl Default for AnimationInstanceSettings {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents an active animation instance being played by the AnimationPlayer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationInstance {
    /// The unique ID of the animation data this instance refers to.
    pub animation_id: String,
    /// The settings defining this instance's behavior.
    pub settings: AnimationInstanceSettings,
    /// Runtime properties for this instance.
    pub properties: InstanceProperties,
    /// The actual duration of the animation data this instance refers to.
    /// This is cached from AnimationData for quick access.
    pub animation_data_duration: AnimationTime,
}

impl AnimationInstance {
    /// Creates a new animation instance.
    #[inline]
    pub fn new(
        animation_id: impl Into<String>,
        settings: AnimationInstanceSettings,
        animation_data_duration: impl Into<AnimationTime>,
    ) -> Self {
        Self {
            animation_id: animation_id.into(),
            settings,
            properties: InstanceProperties::default(),
            animation_data_duration: animation_data_duration.into(),
        }
    }

    /// Translates the given player time into the time relative to this animation,
    /// with respect to the playback settings.
    pub fn get_effective_time(&self, player_time: AnimationTime) -> AnimationTime {
        if !self.settings.enabled {
            return AnimationTime::zero();
        }

        let instance_relative_time = player_time
            .duration_since(self.settings.instance_start_time)
            .unwrap_or_else(|_| AnimationTime::zero());

        let scaled_time = instance_relative_time.as_seconds() * self.settings.timescale;
        let scaled_time =
            AnimationTime::from_seconds(scaled_time).unwrap_or_else(|_| AnimationTime::zero());

        if self.animation_data_duration.as_seconds() <= 0.0 {
            return AnimationTime::zero();
        }

        scaled_time.clamp(AnimationTime::zero(), self.animation_data_duration)
    }

}
