//! Animation baking functionality for pre-calculating values at specific frame rates

use crate::{
    animation::AnimationMetadata, AnimationData, AnimationError, AnimationTime,
    InterpolationRegistry, TimeRange, Value,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

        let frame_duration = AnimationTime::new(1.0 / frame_rate)?;
        let frame_count = (duration.as_seconds() * frame_rate).ceil() as usize + 1;

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
#[derive(Debug, Clone, serde::Deserialize)]
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

/// Extension trait for AnimationData to add baking functionality
pub trait AnimationBaking {
    /// Bake the animation into a set of pre-calculated values at specified frame rate
    fn bake(
        &self,
        config: &BakingConfig,
        interpolation_registry: &mut InterpolationRegistry,
    ) -> Result<BakedAnimationData, AnimationError>;
}

impl AnimationBaking for AnimationData {
    fn bake(
        &self,
        config: &BakingConfig,
        interpolation_registry: &mut InterpolationRegistry,
    ) -> Result<BakedAnimationData, AnimationError> {
        // Validate configuration
        config.validate()?;

        // Determine time range to bake
        let time_range = config
            .time_range
            .unwrap_or_else(|| TimeRange::from_duration(self.duration()));

        // Calculate frame parameters
        let frame_count = if time_range.duration().as_seconds() > 0.0 {
            (time_range.duration().as_seconds() * config.frame_rate + 1.0).ceil() as usize
        } else {
            1
        };

        // Create baked animation data structure
        let mut baked_data = BakedAnimationData::new(
            &self.id,
            config.frame_rate,
            time_range.duration(),
            self.metadata.clone(),
        )?;

        // Process each track
        for track in self.tracks.values() {
            // Skip disabled tracks unless configured to include them
            if !track.enabled && !config.include_disabled_tracks {
                continue;
            }

            let mut track_data = Vec::with_capacity(frame_count);
            let mut derivative_data = if config.include_derivatives {
                Some(Vec::with_capacity(frame_count))
            } else {
                None
            };

            // Generate values for each frame
            for frame_index in 0..frame_count {
                let frame_time =
                    time_range.start + AnimationTime::new(frame_index as f64 / config.frame_rate)?;

                // Clamp to time range
                let clamped_time = frame_time.clamp(time_range.start, time_range.end);

                // Get transition for this time (following animation player pattern)
                let transition = self.get_track_transition_for_time(clamped_time, &track.id);

                // Get interpolated value at this time using track's built-in method
                if let Some(mut value) =
                    track.value_at_time(clamped_time, interpolation_registry, transition)
                {
                    // Apply track weight if configured
                    if config.apply_track_weights && track.weight != 1.0 {
                        value = apply_track_weight(&value, track.weight);
                    }

                    track_data.push((clamped_time, value));
                } else {
                    // If no value can be interpolated, skip this frame
                    continue;
                }

                // Calculate derivative if requested
                if let Some(ref mut deriv_data) = derivative_data {
                    if let Some(derivative) = track.derivative_at_time(
                        clamped_time,
                        interpolation_registry,
                        transition,
                        config.derivative_width,
                    ) {
                        println!(
                            "{:?} {:?} {:?} {:?}",
                            clamped_time, transition, config.derivative_width, derivative
                        );
                        deriv_data.push((clamped_time, derivative));
                    }
                }
            }

            // Add track data to baked animation
            if !track_data.is_empty() {
                baked_data.add_track_data(&track.target, track_data);
            }

            // Add derivative track data if calculated
            if let Some(deriv_data) = derivative_data {
                if !deriv_data.is_empty() {
                    let derivative_track_name = format!("{} (derivative)", track.target);
                    baked_data.add_track_data(&derivative_track_name, deriv_data);
                }
            }
        }

        Ok(baked_data)
    }
}

/// Helper function to apply track weight to a value
#[inline]
fn apply_track_weight(value: &Value, weight: f64) -> Value {
    match value {
        Value::Float(val) => Value::Float(val * weight),
        Value::Vector3(vec) => Value::Vector3(crate::value::Vector3::new(
            vec.x * weight,
            vec.y * weight,
            vec.z * weight,
        )),
        Value::Transform(transform) => {
            // For transforms, only position and scale components are scaled by weight.
            // Rotation is not scaled as it represents orientation, not magnitude.
            let scaled_position = crate::value::Vector3::new(
                transform.position.x * weight,
                transform.position.y * weight,
                transform.position.z * weight,
            );
            let scaled_scale = crate::value::Vector3::new(
                transform.scale.x * weight,
                transform.scale.y * weight,
                transform.scale.z * weight,
            );
            Value::Transform(crate::value::Transform::new(
                scaled_position,
                transform.rotation, // rotation unchanged
                scaled_scale,
            ))
        }
        Value::Color(color) => {
            // Scale RGB components by weight. Alpha (transparency) is also scaled.
            let (r, g, b, a) = color.to_rgba();
            Value::Color(crate::value::Color::rgba(
                r * weight,
                g * weight,
                b * weight,
                a * weight,
            ))
        }
        _ => value.clone(), // For non-scalable types, return as-is
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{value::Vector3, AnimationKeypoint, AnimationTrack};

    #[test]
    fn test_baking_config() {
        let config = BakingConfig::new(30.0)
            .with_disabled_tracks(true)
            .with_track_weights(false)
            .with_interpolation_method("linear");

        assert_eq!(config.frame_rate, 30.0);
        assert!(config.include_disabled_tracks);
        assert!(!config.apply_track_weights);
        assert_eq!(config.interpolation_method, "linear");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_baking_config_validation() {
        let mut config = BakingConfig::default();
        assert!(config.validate().is_ok());

        config.frame_rate = 0.0;
        assert!(config.validate().is_err());

        config.frame_rate = f64::INFINITY;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_baked_animation_data_creation() {
        let metadata = AnimationMetadata::new();
        let duration = AnimationTime::new(2.0).unwrap();

        let baked_data = BakedAnimationData::new("test_anim", 60.0, duration, metadata);
        assert!(baked_data.is_ok());

        let baked = baked_data.unwrap();
        assert_eq!(baked.animation_id, "test_anim");
        assert_eq!(baked.frame_rate, 60.0);
        assert_eq!(baked.duration, duration);
        assert_eq!(baked.frame_count, 121); // 2 seconds * 60 fps + 1
    }

    #[test]
    fn test_baked_data_operations() {
        let metadata = AnimationMetadata::new();
        let duration = AnimationTime::new(1.0).unwrap();
        let mut baked_data = BakedAnimationData::new("test", 10.0, duration, metadata).unwrap();

        // Add some test data
        let track_data = vec![
            (AnimationTime::zero(), Value::Float(0.0)),
            (AnimationTime::new(0.1).unwrap(), Value::Float(1.0)),
            (AnimationTime::new(0.2).unwrap(), Value::Float(2.0)),
        ];

        baked_data.add_track_data("test_track", track_data);

        // Test retrieval
        assert!(baked_data.get_track_data("test_track").is_some());
        assert_eq!(baked_data.track_targets(), vec!["test_track"]);

        // Test frame access
        let value = baked_data.get_value_at_frame("test_track", 1);
        assert!(value.is_some());
        if let Some(Value::Float(f)) = value {
            assert_eq!(*f, 1.0);
        }

        // Test statistics
        let stats = baked_data.get_statistics();
        assert_eq!(stats.track_count, 1);
        assert_eq!(stats.total_values, 3);
    }

    #[test]
    fn test_apply_track_weight() {
        let value = Value::Float(10.0);
        let weighted = apply_track_weight(&value, 0.5);
        if let Value::Float(f) = weighted {
            assert_eq!(f, 5.0);
        }

        let vec_value = Value::Vector3(Vector3::new(2.0, 4.0, 6.0));
        let weighted_vec = apply_track_weight(&vec_value, 0.5);
        if let Value::Vector3(v) = weighted_vec {
            assert_eq!(v.x, 1.0);
            assert_eq!(v.y, 2.0);
            assert_eq!(v.z, 3.0);
        }
    }

    #[test]
    fn test_simple_animation_baking() {
        use crate::InterpolationRegistry;

        // Create a simple animation with one track
        let mut animation = AnimationData::new("test_bake", "Test Baking");
        let mut track = AnimationTrack::new("position", "transform.position.x");

        track
            .add_keypoint(AnimationKeypoint::new(
                AnimationTime::zero(),
                Value::Float(0.0),
            ))
            .unwrap();

        track
            .add_keypoint(AnimationKeypoint::new(
                AnimationTime::new(1.0).unwrap(),
                Value::Float(10.0),
            ))
            .unwrap();

        animation.add_track(track);

        // Bake the animation
        let config = BakingConfig::new(100.0); // 100 fps for easy testing
        let mut registry = InterpolationRegistry::default();

        let baked = animation.bake(&config, &mut registry);
        assert!(baked.is_ok());

        let baked_data = baked.unwrap();
        assert_eq!(baked_data.frame_count, 101); // 1 second * 100 fps + 1
        assert!(baked_data.get_track_data("transform.position.x").is_some());

        // Check that we have values at each frame
        let track_data = baked_data.get_track_data("transform.position.x").unwrap();
        assert_eq!(track_data.len(), 101);

        // Check first and last values
        if let Value::Float(first) = &track_data[0].1 {
            assert!((first - 0.0).abs() < 0.01);
        }
        if let Value::Float(last) = &track_data[track_data.len() - 1].1 {
            assert!((last - 10.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_animation_baking_with_derivatives() {
        use crate::InterpolationRegistry;

        // Create a simple animation with one track
        let mut animation = AnimationData::new("test_derivative_bake", "Test Derivative Baking");
        let mut track = AnimationTrack::new("position", "transform.position.x");

        track
            .add_keypoint(AnimationKeypoint::new(
                AnimationTime::zero(),
                Value::Float(0.0),
            ))
            .unwrap();

        track
            .add_keypoint(AnimationKeypoint::new(
                AnimationTime::new(1.0).unwrap(),
                Value::Float(10.0),
            ))
            .unwrap();

        animation.add_track(track);

        // Bake the animation with derivatives enabled
        let config =
            BakingConfig::new(10.0).with_derivatives(Some(AnimationTime::new(1.0).unwrap()));
        let mut registry = InterpolationRegistry::default();
        println!("{:?}", config);
        println!("{:?}", animation);

        let baked = animation.bake(&config, &mut registry);
        assert!(baked.is_ok());

        let baked_data = baked.unwrap();

        // Should have both original track and derivative track
        assert!(baked_data.get_track_data("transform.position.x").is_some());
        assert!(baked_data
            .get_track_data("transform.position.x (derivative)")
            .is_some());

        // Track count should be 2 (original + derivative)
        let stats = baked_data.get_statistics();
        assert_eq!(stats.track_count, 2);

        // Check that derivative track has data
        let derivative_data = baked_data
            .get_track_data("transform.position.x (derivative)")
            .unwrap();
        assert!(!derivative_data.is_empty());
        println!("{:?}", derivative_data);

        // For a linear track from 0 to 10 over 1 second, derivative should be approximately 10
        // Since this is numerical differentiation, we allow some tolerance
        if let Some(Value::Float(derivative_value)) = derivative_data.get(5).map(|(_, v)| v) {
            // Verify the derivative is reasonable for a linear track with slope ~10
            assert!(derivative_value.abs() > 5.0); // Should be at least 5 for a steep slope
            assert!(derivative_value.abs() < 25.0); // Should not be unreasonably large
        }
    }

    #[test]
    fn test_baking_config_with_derivatives() {
        let derivative_width = AnimationTime::from_millis(5.0).unwrap();
        let config = BakingConfig::new(60.0).with_derivatives(Some(derivative_width));

        assert!(config.include_derivatives);
        assert_eq!(config.derivative_width, Some(derivative_width));
        assert!(config.validate().is_ok());
    }
}
