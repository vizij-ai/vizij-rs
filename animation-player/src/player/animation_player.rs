use crate::animation::{AnimationInstance, AnimationInstanceSettings};
use crate::player::playback_metrics::PlaybackMetrics;
use crate::{AnimationData, AnimationError, AnimationTime, InterpolationRegistry, Value};
use std::collections::HashMap;

/// Individual animation player instance
#[derive(Debug)]
pub struct AnimationPlayer {
    /// Current animation time
    pub current_time: AnimationTime, // Made public for direct access
    /// Performance metrics
    pub metrics: PlaybackMetrics, // Made public for direct access
    /// Cache for last calculated values
    last_calculated_values: Option<HashMap<String, Value>>,
    /// Time when last_calculated_values was computed
    last_calculated_time: AnimationTime,
    /// Active animation instances managed by this player
    pub instances: HashMap<String, AnimationInstance>, // Made public
}

impl AnimationPlayer {
    /// Create a new animation player
    #[inline]
    pub fn new() -> Self {
        Self {
            current_time: AnimationTime::zero(),
            metrics: PlaybackMetrics::new(),
            last_calculated_values: None,
            last_calculated_time: AnimationTime::zero(),
            instances: HashMap::new(),
        }
    }

    /// Add an animation instance to the player.
    /// The instance's animation_id must correspond to an AnimationData loaded in the engine.
    pub fn add_instance(&mut self, instance: AnimationInstance) -> String {
        let mut id = uuid::Uuid::new_v4().to_string();
        while self.instances.contains_key(&id) {
            id = uuid::Uuid::new_v4().to_string(); // Ensure unique ID
        }
        self.instances.insert(id.clone(), instance);
        id
    }

    /// Remove an animation instance from the player.
    pub fn remove_instance(
        &mut self,
        instance_id: &str,
    ) -> Result<AnimationInstance, AnimationError> {
        self.instances
            .remove(instance_id)
            .ok_or_else(|| AnimationError::Generic {
                message: format!("Animation instance with ID '{}' not found.", instance_id),
            })
    }

    /// Calculates the effective time for a given track across all active instances.
    /// This is a placeholder and will be expanded upon when layering and blending are implemented.
    #[inline]
    pub fn get_effective_time_for_track(
        &self,
        _track: &crate::animation::track::AnimationTrack,
    ) -> Result<AnimationTime, AnimationError> {
        // For now, simply return the player's current time.
        // In a full implementation, this would involve iterating through instances,
        // applying their offsets/timescales, and blending results.
        Ok(self.current_time)
    }

    /// Get an immutable reference to an animation instance.
    #[inline]
    pub fn get_animation_instance(&self, instance_id: &str) -> Option<&AnimationInstance> {
        self.instances.get(instance_id)
    }

    /// Set the weight of an animation instance.
    #[inline]
    pub fn set_instance_weight(
        &mut self,
        instance_id: &str,
        weight: f32,
    ) -> Result<(), AnimationError> {
        let instance =
            self.instances
                .get_mut(instance_id)
                .ok_or_else(|| AnimationError::Generic {
                    message: format!("Animation instance with ID '{}' not found.", instance_id),
                })?;

        if weight < 0.0 {
            return Err(AnimationError::Generic {
                message: format!("Weight must be non-negative, got: {}", weight),
            });
        }

        instance.settings.weight = weight;
        Ok(())
    }

    /// Get the weight of an animation instance.
    #[inline]
    pub fn get_instance_weight(&self, instance_id: &str) -> Result<f32, AnimationError> {
        let instance = self
            .instances
            .get(instance_id)
            .ok_or_else(|| AnimationError::Generic {
                message: format!("Animation instance with ID '{}' not found.", instance_id),
            })?;
        Ok(instance.settings.weight)
    }

    /// Set the time scale of an animation instance.
    #[inline]
    pub fn set_instance_time_scale(
        &mut self,
        instance_id: &str,
        time_scale: f32,
    ) -> Result<(), AnimationError> {
        let instance =
            self.instances
                .get_mut(instance_id)
                .ok_or_else(|| AnimationError::Generic {
                    message: format!("Animation instance with ID '{}' not found.", instance_id),
                })?;

        if time_scale < -5.0 || time_scale > 5.0 {
            return Err(AnimationError::Generic {
                message: format!(
                    "Time scale must be between -5.0 and 5.0, got: {}",
                    time_scale
                ),
            });
        }

        instance.settings.time_scale = time_scale;
        Ok(())
    }

    /// Get the time scale of an animation instance.
    #[inline]
    pub fn get_instance_time_scale(&self, instance_id: &str) -> Result<f32, AnimationError> {
        let instance = self
            .instances
            .get(instance_id)
            .ok_or_else(|| AnimationError::Generic {
                message: format!("Animation instance with ID '{}' not found.", instance_id),
            })?;
        Ok(instance.settings.time_scale)
    }

    /// Set whether an animation instance is enabled.
    #[inline]
    pub fn set_instance_enabled(
        &mut self,
        instance_id: &str,
        enabled: bool,
    ) -> Result<(), AnimationError> {
        let instance =
            self.instances
                .get_mut(instance_id)
                .ok_or_else(|| AnimationError::Generic {
                    message: format!("Animation instance with ID '{}' not found.", instance_id),
                })?;

        instance.settings.enabled = enabled;
        Ok(())
    }

    /// Get whether an animation instance is enabled.
    #[inline]
    pub fn get_instance_enabled(&self, instance_id: &str) -> Result<bool, AnimationError> {
        let instance = self
            .instances
            .get(instance_id)
            .ok_or_else(|| AnimationError::Generic {
                message: format!("Animation instance with ID '{}' not found.", instance_id),
            })?;
        Ok(instance.settings.enabled)
    }

    /// Set the start time of an animation instance.
    #[inline]
    pub fn set_instance_start_time(
        &mut self,
        instance_id: &str,
        start_time: AnimationTime,
    ) -> Result<(), AnimationError> {
        let instance =
            self.instances
                .get_mut(instance_id)
                .ok_or_else(|| AnimationError::Generic {
                    message: format!("Animation instance with ID '{}' not found.", instance_id),
                })?;

        instance.settings.instance_start_time = start_time;
        Ok(())
    }

    /// Get the start time of an animation instance.
    #[inline]
    pub fn get_instance_start_time(
        &self,
        instance_id: &str,
    ) -> Result<AnimationTime, AnimationError> {
        let instance = self
            .instances
            .get(instance_id)
            .ok_or_else(|| AnimationError::Generic {
                message: format!("Animation instance with ID '{}' not found.", instance_id),
            })?;
        Ok(instance.settings.instance_start_time)
    }

    /// Set the player's current time to a specific `AnimationTime`.
    /// This will clear any previously reached keypoints.
    #[inline]
    pub fn go_to(
        &mut self,
        time: impl Into<AnimationTime>,
        animations: &HashMap<String, AnimationData>,
        interpolation_registry: &mut InterpolationRegistry,
    ) -> Result<HashMap<String, Value>, AnimationError> {
        self.current_time = time.into();
        self.calculate_values(animations, interpolation_registry)
    }

    /// Increment the player's current time by a `delta_time`.
    /// This is the primary way to advance the animation.
    #[inline]
    pub fn increment(
        &mut self,
        delta_time: impl Into<AnimationTime>,
        animations: &HashMap<String, AnimationData>,
        interpolation_registry: &mut InterpolationRegistry,
    ) -> Result<HashMap<String, Value>, AnimationError> {
        self.current_time += delta_time.into();
        self.calculate_values(animations, interpolation_registry)
    }

    #[inline]
    pub fn decrement(
        &mut self,
        delta_time: impl Into<AnimationTime>,
        animations: &HashMap<String, AnimationData>,
        interpolation_registry: &mut InterpolationRegistry,
    ) -> Result<HashMap<String, Value>, AnimationError> {
        self.current_time -= delta_time.into();
        self.calculate_values(animations, interpolation_registry)
    }

    /// Calculate the interpolated values at the current time.
    /// This method is called internally by `go_to` and `increment`.
    #[inline]
    pub fn calculate_values(
        &mut self, // Made public
        animations: &HashMap<String, AnimationData>,
        interpolation_registry: &mut InterpolationRegistry,
    ) -> Result<HashMap<String, Value>, AnimationError> {
        // Check cache first
        if self.current_time == self.last_calculated_time && self.last_calculated_values.is_some() {
            return Ok(self.last_calculated_values.clone().unwrap());
        }

        self.metrics.frames_rendered += 1;

        let mut combined_values: HashMap<String, Value> = HashMap::new();
        let mut active_tracks_count = 0;
        let mut estimated_memory_usage = 0;

        // Iterate over active instances
        for instance in self.instances.values_mut() {
            if !instance.settings.enabled {
                continue;
            }

            // Check if the instance should be active at this time
            if self.current_time < instance.settings.instance_start_time {
                continue; // Instance hasn't started yet
            }

            // Get the effective time for this instance
            let effective_instance_time = instance.get_effective_time(self.current_time);

            // Get the animation data for this instance
            let animation_data = animations.get(&instance.animation_id).ok_or_else(|| {
                AnimationError::AnimationNotFound {
                    id: instance.animation_id.clone(),
                }
            })?;

            // Process tracks for this instance
            for track in animation_data.tracks.values() {
                if !track.enabled {
                    continue;
                }

                // Interpolate value for the track at the effective instance time
                if let Some(value) = AnimationPlayer::interpolate_track_value_for_instance(
                    track,
                    effective_instance_time,
                    animation_data,
                    interpolation_registry,
                )? {
                    // For now, simple overwrite. Blending logic would go here.
                    combined_values.insert(track.target.clone(), value);
                    self.metrics.interpolations_performed += 1;
                }

                active_tracks_count += 1;
            }
            estimated_memory_usage += std::mem::size_of::<AnimationInstance>();
            estimated_memory_usage += std::mem::size_of::<AnimationInstanceSettings>();
        }

        // Update metrics
        self.update_metrics_with_data(active_tracks_count, estimated_memory_usage);

        // Update cache
        self.last_calculated_values = Some(combined_values.clone());
        self.last_calculated_time = self.current_time;

        Ok(combined_values)
    }

    /// Interpolate the value for a specific track at a given time for an instance.
    #[inline]
    fn interpolate_track_value_for_instance(
        track: &crate::animation::track::AnimationTrack,
        time: AnimationTime, // Use the effective time for the instance
        animation_data: &AnimationData,
        interpolation_registry: &mut InterpolationRegistry,
    ) -> Result<Option<Value>, AnimationError> {
        if track.keypoints.is_empty() {
            return Ok(None);
        }

        let transition = animation_data.get_track_transition_for_time(time, &track.id);
        let value = track.value_at_time(time, interpolation_registry, transition, animation_data);
        return Ok(value);
    }

    /// Update performance metrics
    #[inline]
    fn update_metrics_with_data(
        &mut self,
        active_tracks_count: usize,
        estimated_memory_usage: usize,
    ) {
        self.metrics.last_update = self.current_time; // Use current_time as last_update

        self.metrics.active_tracks = active_tracks_count;
        self.metrics.memory_usage_bytes = estimated_memory_usage;
    }

    /// Get the total duration of the animation player, taking into account each
    /// instance's start time and time scale. This finds the latest point on the
    /// player's timeline that any instance contributes animation data.
    #[inline]
    pub fn duration(&self) -> AnimationTime {
        let mut max_seconds = 0.0;

        for instance in self.instances.values() {
            if !instance.settings.enabled {
                continue;
            }

            // When time_scale is zero the instance never progresses so it does
            // not extend the player's duration beyond its start time.
            let scale = instance.settings.time_scale.abs() as f64;

            let instance_duration_seconds = if scale > 0.0 {
                instance.animation_data_duration.as_seconds() / scale
            } else {
                0.0
            };

            let end_seconds =
                instance.settings.instance_start_time.as_seconds() + instance_duration_seconds;
            if end_seconds > max_seconds {
                max_seconds = end_seconds;
            }
        }

        AnimationTime::from_seconds(max_seconds).unwrap_or(AnimationTime::zero())
    }

    /// Get progress as a value between 0.0 and 1.0
    #[inline]
    pub fn progress(&self) -> f64 {
        let duration = self.duration();
        if duration.as_seconds() == 0.0 {
            return 0.0;
        }
        (self.current_time.as_seconds() / duration.as_seconds()).clamp(0.0, 1.0)
    }

    /// Get the animation IDs of all active instances
    #[inline]
    pub fn get_active_animation_ids(&self) -> Vec<String> {
        self.instances
            .values()
            .filter(|instance| instance.settings.enabled)
            .map(|instance| instance.animation_id.clone())
            .collect()
    }

    /// Get all instance IDs in this player
    #[inline]
    pub fn get_instance_ids(&self) -> Vec<String> {
        self.instances.keys().map(|s| s.to_string()).collect()
    }

    /// Get all instance IDs in this player
    #[inline]
    pub fn instance_ids(&self) -> Vec<&str> {
        self.instances.keys().map(|s| s.as_str()).collect()
    }

    /// Get performance metrics
    #[inline]
    pub fn metrics(&self) -> &PlaybackMetrics {
        &self.metrics
    }

    /// Calculate derivatives (rates of change) for all tracks at the current time
    pub fn calculate_derivatives(
        &mut self,
        animations: &HashMap<String, AnimationData>,
        interpolation_registry: &mut InterpolationRegistry,
        derivative_width: Option<AnimationTime>,
        speed: f64,
    ) -> Result<HashMap<String, Value>, AnimationError> {
        let mut derivatives: HashMap<String, Value> = HashMap::new();

        // Iterate over active instances
        for instance in self.instances.values_mut() {
            if !instance.settings.enabled {
                continue;
            }

            // Check if the instance should be active at this time
            if self.current_time < instance.settings.instance_start_time {
                continue; // Instance hasn't started yet
            }

            // Get the effective time for this instance
            let effective_instance_time = instance.get_effective_time(self.current_time);

            // Get the animation data for this instance
            let animation_data = animations.get(&instance.animation_id).ok_or_else(|| {
                AnimationError::AnimationNotFound {
                    id: instance.animation_id.clone(),
                }
            })?;

            // Process tracks for this instance
            for track in animation_data.tracks.values() {
                if !track.enabled {
                    continue;
                }
                // Calculate derivative for the track at the effective instance time
                if let Some(derivative) =
                    AnimationPlayer::interpolate_derivative_value_for_instance(
                        track,
                        effective_instance_time,
                        animation_data,
                        interpolation_registry,
                        derivative_width,
                        speed,
                    )?
                {
                    derivatives.insert(track.target.clone(), derivative);
                }
            }
        }

        Ok(derivatives)
    }

    /// Interpolate the value for a specific track at a given time for an instance.
    #[inline]
    fn interpolate_derivative_value_for_instance(
        track: &crate::animation::track::AnimationTrack,
        time: AnimationTime, // Use the effective time for the instance
        animation_data: &AnimationData,
        interpolation_registry: &mut InterpolationRegistry,
        derivative_width: Option<AnimationTime>,
        speed: f64,
    ) -> Result<Option<Value>, AnimationError> {
        if track.keypoints.is_empty() {
            return Ok(None);
        }

        let transition = animation_data.get_track_transition_for_time(time, &track.id);
        let value = track.derivative_at_time(
            time,
            interpolation_registry,
            transition,
            derivative_width,
            animation_data,
        );

        if let Some(v) = value {
            Ok(Some(v.multiply_by_scalar(speed)))
        } else {
            Ok(None)
        }
    }
}
