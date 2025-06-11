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
pub struct AnimationSettings {
    /// The time offset to start playback from within the animation data.
    pub start_offset: AnimationTime,
    /// The time at which this instance begins relative to the player's timeline.
    pub instance_start_time: AnimationTime,
    /// The duration this instance should play for. If None, plays for the full animation duration.
    pub duration: Option<AnimationTime>,
    /// The playback speed multiplier for this instance.
    pub timescale: f64,
    /// How the animation should loop.
    pub playback_mode: PlaybackMode,
    /// The number of times the animation should loop. None for infinite.
    pub loop_count: Option<u32>,
    /// Whether the instance is enabled for playback.
    pub enabled: bool,
    /// Optional metadata for the instance.
    pub metadata: HashMap<String, String>,
}

impl AnimationSettings {
    /// Creates new default instance settings for a given animation ID.
    pub fn new() -> Self {
        Self {
            start_offset: AnimationTime::zero(),
            instance_start_time: AnimationTime::zero(),
            duration: None,
            timescale: 1.0,
            playback_mode: PlaybackMode::PingPong,
            loop_count: None,
            enabled: true,
            metadata: HashMap::new(),
        }
    }
}

impl Default for AnimationSettings {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents an active animation instance being played by the AnimationPlayer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    /// The unique ID of the animation data this instance refers to.
    pub animation_id: String,
    /// The settings defining this instance's behavior.
    pub settings: AnimationSettings,
    /// The current number of loops completed for this instance.
    pub current_loop_count: u32,
    /// The current direction of playback for PingPong loop mode (true for forward, false for backward).
    pub is_playing_forward: bool,
    /// The actual duration of the animation data this instance refers to.
    /// This is cached from AnimationData for quick access.
    pub animation_data_duration: AnimationTime,
}

impl Animation {
    /// Creates a new animation instance.
    #[inline]
    pub fn new(
        animation_id: impl Into<String>,
        settings: AnimationSettings,
        animation_data_duration: impl Into<AnimationTime>,
    ) -> Self {
        Self {
            animation_id: animation_id.into(),
            settings,
            current_loop_count: 0,
            is_playing_forward: true,
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

        let effective_duration = self
            .settings
            .duration
            .unwrap_or(self.animation_data_duration);

        if effective_duration.as_seconds() <= 0.0 {
            return AnimationTime::zero();
        }

        // Apply looping to the scaled time (without start_offset)
        let looped_time = match self.settings.playback_mode {
            PlaybackMode::Once => scaled_time.clamp(AnimationTime::zero(), effective_duration),
            PlaybackMode::Loop => {
                let total_animation_seconds = effective_duration.as_seconds();
                if total_animation_seconds > 0.0 {
                    let looped_seconds = scaled_time.as_seconds() % total_animation_seconds;
                    AnimationTime::from_seconds(looped_seconds)
                        .unwrap_or_else(|_| AnimationTime::zero())
                } else {
                    AnimationTime::zero()
                }
            }
            PlaybackMode::PingPong => {
                let total_animation_seconds = effective_duration.as_seconds();
                if total_animation_seconds > 0.0 {
                    let cycle_duration = total_animation_seconds * 2.0; // One full ping-pong cycle
                    let cycle_time = scaled_time.as_seconds() % cycle_duration;

                    let time_in_half_cycle = if cycle_time < total_animation_seconds {
                        cycle_time // Forward
                    } else {
                        cycle_duration - cycle_time // Backward
                    };
                    AnimationTime::from_seconds(time_in_half_cycle)
                        .unwrap_or_else(|_| AnimationTime::zero())
                } else {
                    AnimationTime::zero()
                }
            }
        };

        // Add start_offset to the final looped time
        looped_time + self.settings.start_offset
    }

    /// Updates the instance's internal state (e.g., loop count, direction for ping-pong).
    /// Returns true if the instance has completed its loops and should be considered finished.
    pub fn update_loop_state(&mut self, player_time: AnimationTime) -> bool {
        if !self.settings.enabled {
            return false;
        }

        let instance_relative_time = player_time
            .duration_since(self.settings.instance_start_time)
            .unwrap_or_else(|_| AnimationTime::zero());

        let scaled_time_seconds = instance_relative_time.as_seconds() * self.settings.timescale;
        let effective_duration = self
            .settings
            .duration
            .unwrap_or(self.animation_data_duration);

        if effective_duration.as_seconds() <= 0.0 {
            return false;
        }

        let total_animation_seconds = effective_duration.as_seconds();
        // Don't add start_offset here - it only affects where we read data, not loop timing
        let playback_time_seconds = scaled_time_seconds;

        match self.settings.playback_mode {
            PlaybackMode::Once => {
                // Check if we've played through the full effective duration
                if playback_time_seconds >= total_animation_seconds {
                    self.current_loop_count = 1;
                    true // Finished
                } else {
                    false
                }
            }
            PlaybackMode::Loop => {
                let new_loop_count =
                    (playback_time_seconds / total_animation_seconds).floor() as u32;
                if self
                    .settings
                    .loop_count
                    .map_or(false, |lc| new_loop_count >= lc)
                {
                    self.current_loop_count = self.settings.loop_count.unwrap();
                    true // Finished
                } else {
                    self.current_loop_count = new_loop_count;
                    false
                }
            }
            PlaybackMode::PingPong => {
                let cycle_duration = total_animation_seconds * 2.0;
                let new_loop_count = (playback_time_seconds / cycle_duration).floor() as u32;
                let time_in_cycle = playback_time_seconds % cycle_duration;

                self.is_playing_forward = time_in_cycle < total_animation_seconds;

                if self
                    .settings
                    .loop_count
                    .map_or(false, |lc| new_loop_count >= lc)
                {
                    self.current_loop_count = self.settings.loop_count.unwrap();
                    true // Finished
                } else {
                    self.current_loop_count = new_loop_count;
                    false
                }
            }
        }
    }
}
