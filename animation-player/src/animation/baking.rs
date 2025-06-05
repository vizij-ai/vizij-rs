use crate::animation::metadata::AnimationMetadata;
use crate::{AnimationError, AnimationTime, TimeRange, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for derivative calculations
#[derive(Debug, Clone)]
pub struct DerivativeConfig {
    /// Width of the numerical differentiation window (default: 1ms)
    pub derivative_width: AnimationTime,
    /// Whether to cache derivative calculations
    pub enable_caching: bool,
    /// Maximum age of cached derivatives before recalculation
    pub cache_max_age: AnimationTime,
}

impl Default for DerivativeConfig {
    fn default() -> Self {
        Self {
            derivative_width: AnimationTime::from_millis(1.0).unwrap(),
            enable_caching: true,
            cache_max_age: AnimationTime::from_millis(10.0).unwrap(),
        }
    }
}

/// Represents baked animation data with pre-calculated values at specific time intervals
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BakedAnimationData {
    /// Original animation ID
    pub animation_id: String,
    /// Frame rate used for baking
    pub frame_rate: f64,
    /// Total duration of the baked animation
    pub duration: AnimationTime,
    /// Time step between frames
    pub frame_duration: AnimationTime,
    /// Number of frames in the baked data
    pub frame_count: usize,
    /// Baked data for each track: track_target -> Vec<(time, value)>
    pub tracks: HashMap<String, Vec<(AnimationTime, Value)>>,
    /// Metadata from the original animation
    pub metadata: AnimationMetadata,
}

impl BakedAnimationData {
    /// Create a new baked animation data structure
    pub fn new(
        animation_id: impl Into<String>,
        frame_rate: f64,
        duration: AnimationTime,
        metadata: AnimationMetadata,
    ) -> Result<Self, AnimationError> {
        if frame_rate <= 0.0 || !frame_rate.is_finite() {
            return Err(AnimationError::InvalidValue {
                reason: "Frame rate must be positive and finite".to_string(),
            });
        }

        let frame_duration = AnimationTime::from_seconds(1.0 / frame_rate)?;
        let frame_count = if duration.as_seconds() > 0.0 {
            ((duration.as_seconds() * frame_rate).ceil() as usize).max(1)
        } else {
            1
        };

        Ok(Self {
            animation_id: animation_id.into(),
            frame_rate,
            duration,
            frame_duration,
            frame_count,
            tracks: HashMap::new(),
            metadata,
        })
    }

    /// Add baked data for a track
    pub fn add_track_data(
        &mut self,
        track_target: impl Into<String>,
        data: Vec<(AnimationTime, Value)>,
    ) {
        self.tracks.insert(track_target.into(), data);
    }

    /// Get baked data for a specific track
    pub fn get_track_data(&self, track_target: &str) -> Option<&Vec<(AnimationTime, Value)>> {
        self.tracks.get(track_target)
    }

    /// Get value at a specific frame index for a track
    pub fn get_value_at_frame(&self, track_target: &str, frame_index: usize) -> Option<&Value> {
        self.tracks
            .get(track_target)
            .and_then(|data| data.get(frame_index))
            .map(|(_, value)| value)
    }

    /// Get value at a specific time for a track (finds nearest frame)
    pub fn get_value_at_time(&self, track_target: &str, time: AnimationTime) -> Option<&Value> {
        let frame_index = (time.as_seconds() * self.frame_rate).round() as usize;
        self.get_value_at_frame(track_target, frame_index)
    }

    /// Get all track targets
    pub fn track_targets(&self) -> Vec<&str> {
        self.tracks.keys().map(|s| s.as_str()).collect()
    }

    /// Export to JSON string
    pub fn to_json(&self) -> Result<String, AnimationError> {
        serde_json::to_string_pretty(self).map_err(|e| AnimationError::SerializationError {
            reason: e.to_string(),
        })
    }

    /// Create from JSON string
    pub fn from_json(json: &str) -> Result<Self, AnimationError> {
        serde_json::from_str(json).map_err(|e| AnimationError::SerializationError {
            reason: e.to_string(),
        })
    }

    /// Get statistics about the baked data
    pub fn get_statistics(&self) -> BakedDataStatistics {
        let total_values = self.tracks.values().map(|data| data.len()).sum();
        let memory_estimate = total_values * std::mem::size_of::<(AnimationTime, Value)>();

        BakedDataStatistics {
            track_count: self.tracks.len(),
            frame_count: self.frame_count,
            total_values,
            memory_estimate_bytes: memory_estimate,
            duration_seconds: self.duration.as_seconds(),
            frame_rate: self.frame_rate,
        }
    }
}

/// Statistics about baked animation data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BakedDataStatistics {
    pub track_count: usize,
    pub frame_count: usize,
    pub total_values: usize,
    pub memory_estimate_bytes: usize,
    pub duration_seconds: f64,
    pub frame_rate: f64,
}

/// Configuration for animation baking
#[derive(Debug, Clone)]
pub struct BakingConfig {
    /// Frame rate for baking (frames per second)
    pub frame_rate: f64,
    /// Whether to include disabled tracks
    pub include_disabled_tracks: bool,
    /// Whether to apply track weights
    pub apply_track_weights: bool,
    /// Custom time range to bake (None = use full animation duration)
    pub time_range: Option<TimeRange>,
    /// Interpolation method to use for baking
    pub interpolation_method: String,
    /// Whether to include derivative data
    pub include_derivatives: bool,
    /// Width for derivative calculations
    pub derivative_width: Option<AnimationTime>,
}

impl Default for BakingConfig {
    fn default() -> Self {
        Self {
            frame_rate: 60.0,
            include_disabled_tracks: false,
            apply_track_weights: true,
            time_range: None,
            interpolation_method: "cubic".to_string(),
            include_derivatives: false,
            derivative_width: None,
        }
    }
}

impl BakingConfig {
    /// Create a new baking config with specified frame rate
    pub fn new(frame_rate: f64) -> Self {
        Self {
            frame_rate,
            ..Default::default()
        }
    }

    /// Set whether to include disabled tracks
    pub fn with_disabled_tracks(mut self, include: bool) -> Self {
        self.include_disabled_tracks = include;
        self
    }

    /// Set whether to apply track weights
    pub fn with_track_weights(mut self, apply: bool) -> Self {
        self.apply_track_weights = apply;
        self
    }

    /// Set a custom time range for baking
    pub fn with_time_range(mut self, time_range: TimeRange) -> Self {
        self.time_range = Some(time_range);
        self
    }

    /// Set the interpolation method
    pub fn with_interpolation_method(mut self, method: impl Into<String>) -> Self {
        self.interpolation_method = method.into();
        self
    }

    /// Enable derivative calculation
    pub fn with_derivatives(mut self, derivative_width: Option<AnimationTime>) -> Self {
        self.include_derivatives = true;
        self.derivative_width = derivative_width;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), AnimationError> {
        if self.frame_rate <= 0.0 || !self.frame_rate.is_finite() {
            return Err(AnimationError::InvalidValue {
                reason: "Frame rate must be positive and finite".to_string(),
            });
        }

        if let Some(range) = &self.time_range {
            if range.start >= range.end {
                return Err(AnimationError::InvalidValue {
                    reason: "Time range start must be before end".to_string(),
                });
            }
        }

        Ok(())
    }
}
