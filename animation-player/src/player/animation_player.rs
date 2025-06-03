use crate::animation::{AnimationInstance, InstanceSettings};
use crate::player::playback_metrics::PlaybackMetrics;
use crate::{AnimationData, AnimationError, AnimationTime, InterpolationRegistry, Value};
use std::collections::HashMap;

/// Individual animation player instance
#[derive(Debug)]
pub struct AnimationPlayer {
    /// Unique identifier for this player
    pub id: String,
    /// Current animation time
    pub current_time: AnimationTime, // Made public for direct access
    /// Performance metrics
    pub metrics: PlaybackMetrics, // Made public for direct access
    /// Cache for last calculated values
    last_calculated_values: Option<HashMap<String, Value>>,
    /// Time when last_calculated_values was computed
    last_calculated_time: AnimationTime,
    /// ID of the animation data used for last_calculated_values
    last_animation_id: Option<String>,
    /// Active animation instances managed by this player
    pub instances: HashMap<String, AnimationInstance>, // Made public
}

impl AnimationPlayer {
    /// Create a new animation player
    #[inline]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            current_time: AnimationTime::zero(),
            metrics: PlaybackMetrics::new(),
            last_calculated_values: None,
            last_calculated_time: AnimationTime::zero(),
            last_animation_id: None,
            instances: HashMap::new(),
        }
    }

    /// Add an animation instance to the player.
    /// The instance's animation_id must correspond to an AnimationData loaded in the engine.
    #[inline]
    pub fn add_instance(&mut self, instance: AnimationInstance) -> Result<(), AnimationError> {
        if self.instances.contains_key(&instance.id) {
            return Err(AnimationError::Generic {
                message: format!(
                    "Animation instance with ID '{}' already exists.",
                    instance.id
                ),
            });
        }
        self.instances.insert(instance.id.clone(), instance);
        Ok(())
    }

    /// Remove an animation instance from the player.
    #[inline]
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

    /// Get the settings for a specific animation instance.
    #[inline]
    pub fn get_instance_settings(
        &self,
        instance_id: &str,
    ) -> Result<&InstanceSettings, AnimationError> {
        self.instances
            .get(instance_id)
            .map(|instance| &instance.settings)
            .ok_or_else(|| AnimationError::Generic {
                message: format!("Instance settings for ID '{}' not found.", instance_id),
            })
    }

    /// Set the player's current time to a specific `AnimationTime`.
    /// This will clear any previously reached keypoints.
    #[inline]
    pub fn go_to(
        &mut self,
        time: AnimationTime,
        animations: &HashMap<String, AnimationData>,
        interpolation_registry: &mut InterpolationRegistry,
    ) -> Result<HashMap<String, Value>, AnimationError> {
        self.current_time = time;
        self.calculate_values(animations, interpolation_registry)
    }

    /// Increment the player's current time by a `delta_time`.
    /// This is the primary way to advance the animation.
    #[inline]
    pub fn increment(
        &mut self,
        delta_time: AnimationTime,
        animations: &HashMap<String, AnimationData>,
        interpolation_registry: &mut InterpolationRegistry,
    ) -> Result<HashMap<String, Value>, AnimationError> {
        self.current_time += delta_time;
        self.calculate_values(animations, interpolation_registry)
    }

    #[inline]
    pub fn decrement(
        &mut self,
        delta_time: AnimationTime,
        animations: &HashMap<String, AnimationData>,
        interpolation_registry: &mut InterpolationRegistry,
    ) -> Result<HashMap<String, Value>, AnimationError> {
        self.current_time -= delta_time;
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
        if self.current_time == self.last_calculated_time &&
           self.last_animation_id.as_ref().map_or(true, |id| animations.contains_key(id)) && // Check if animation data still exists
           self.last_calculated_values.is_some()
        {
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

            // Update instance loop state
            instance.update_loop_state(self.current_time);

            // Get the effective time for this instance
            let effective_instance_time = instance.get_effective_time(self.current_time);

            // Get the animation data for this instance
            let animation_data =
                animations
                    .get(&instance.settings.animation_id)
                    .ok_or_else(|| AnimationError::AnimationNotFound {
                        id: instance.settings.animation_id.clone(),
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
            estimated_memory_usage += std::mem::size_of::<InstanceSettings>();
        }

        // Update metrics
        self.update_metrics_with_data(active_tracks_count, estimated_memory_usage);

        // Update cache
        self.last_calculated_values = Some(combined_values.clone());
        self.last_calculated_time = self.current_time;
        self.last_animation_id = Some(
            self.instances
                .values()
                .next()
                .map(|i| i.settings.animation_id.clone())
                .unwrap_or_default(),
        ); // Store ID of first instance's animation data

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
        let value = track.value_at_time(time, interpolation_registry, transition);
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

    /// Get the total duration of the animation player, based on the longest instance.
    #[inline]
    pub fn duration(&self) -> AnimationTime {
        self.instances
            .values()
            .filter(|instance| instance.settings.enabled)
            .map(|instance| {
                instance
                    .settings
                    .duration
                    .unwrap_or(instance.animation_data_duration)
            })
            .max()
            .unwrap_or(AnimationTime::zero())
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
            .map(|instance| instance.settings.animation_id.clone())
            .collect()
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
            let animation_data =
                animations
                    .get(&instance.settings.animation_id)
                    .ok_or_else(|| AnimationError::AnimationNotFound {
                        id: instance.settings.animation_id.clone(),
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
    ) -> Result<Option<Value>, AnimationError> {
        if track.keypoints.is_empty() {
            return Ok(None);
        }

        let transition = animation_data.get_track_transition_for_time(time, &track.id);
        let value =
            track.derivative_at_time(time, interpolation_registry, transition, derivative_width);
        return Ok(value);
    }
}
