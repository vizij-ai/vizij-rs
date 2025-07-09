use crate::animation::data::AnimationData;
use crate::animation::ids::{KeypointId, TrackId};
use crate::animation::keypoint::AnimationKeypoint;
use crate::animation::transition::AnimationTransition;
use crate::interpolation::InterpolationContext;
use crate::interpolation::InterpolationRegistry;
use crate::{AnimationError, AnimationTime, TimeRange, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An animation track containing a sequence of keypoints
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationTrack {
    /// Unique identifier for this track
    pub id: TrackId,
    /// Human-readable name for this track
    pub name: String,
    /// Target property path (e.g., "transform.position.x")
    pub target: String,
    /// Keypoints in chronological order
    pub keypoints: Vec<AnimationKeypoint>,
    /// Whether this track is enabled
    pub enabled: bool,
    /// Track weight for blending (0.0 to 1.0)
    pub weight: f64,
    /// Optional settings, e.g., for UI representation
    #[serde(default)]
    pub settings: Option<HashMap<String, String>>,
    /// Optional metadata
    pub metadata: HashMap<String, String>,
}

impl AnimationTrack {
    /// Create a new empty track
    #[inline]
    pub fn new(name: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            id: TrackId::new(),
            name: name.into(),
            target: target.into(),
            keypoints: Vec::new(),
            enabled: true,
            weight: 1.0,
            settings: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a new empty track with custom ID
    #[inline]
    pub fn new_with_id(
        id: impl AsRef<str>,
        name: impl Into<String>,
        target: impl Into<String>,
    ) -> Result<Self, AnimationError> {
        Ok(Self {
            id: TrackId::from_string(id)?,
            name: name.into(),
            target: target.into(),
            keypoints: Vec::new(),
            enabled: true,
            weight: 1.0,
            settings: None, // Initialize the new field
            metadata: HashMap::new(),
        })
    }

    /// Add a keypoint to this track
    pub fn add_keypoint(
        &mut self,
        keypoint: AnimationKeypoint,
    ) -> Result<AnimationKeypoint, AnimationError> {
        // Validate that the value type is consistent
        if let Some(first_keypoint) = self.keypoints.first() {
            if !first_keypoint.value.can_interpolate_with(&keypoint.value) {
                return Err(AnimationError::ValueTypeMismatch {
                    expected: first_keypoint.value.value_type(),
                    actual: keypoint.value.value_type(),
                });
            }
        }

        // Insert keypoint in chronological order
        let insert_pos = self
            .keypoints
            .binary_search_by(|k| k.time.partial_cmp(&keypoint.time).unwrap())
            .unwrap_or_else(|pos| pos);

        self.keypoints.insert(insert_pos, keypoint.clone());
        Ok(keypoint)
    }

    /// Remove a keypoint by ID
    pub fn remove_keypoint(&mut self, id: KeypointId) -> Result<AnimationKeypoint, AnimationError> {
        let pos = self
            .keypoints
            .iter()
            .position(|k| k.id == id)
            .ok_or_else(|| AnimationError::KeypointNotFound {
                track_id: self.id.to_string(),
                keypoint_id: id.to_string(),
            })?;

        Ok(self.keypoints.remove(pos))
    }

    /// Get a keypoint by ID
    #[inline]
    pub fn get_keypoint(&self, id: KeypointId) -> Option<&AnimationKeypoint> {
        self.keypoints.iter().find(|k| k.id == id)
    }

    /// Get a mutable reference to a keypoint by ID
    #[inline]
    pub fn get_keypoint_mut(&mut self, id: KeypointId) -> Option<&mut AnimationKeypoint> {
        self.keypoints.iter_mut().find(|k| k.id == id)
    }

    /// Find the indices of the keypoints that surround the given time
    pub fn keypoint_indices_at_time(&self, time: AnimationTime) -> (Option<usize>, Option<usize>) {
        if self.keypoints.is_empty() {
            return (None, None);
        }

        match self
            .keypoints
            .binary_search_by(|k| k.time.partial_cmp(&time).unwrap())
        {
            Ok(idx) => (Some(idx), Some(idx)), // Exact match
            Err(idx) => {
                let prev = if idx > 0 { Some(idx - 1) } else { None };
                let next = if idx < self.keypoints.len() {
                    Some(idx)
                } else {
                    None
                };
                (prev, next)
            }
        }
    }

    /// Get the time range covered by this track
    #[inline]
    pub fn time_range(&self) -> Option<TimeRange> {
        if self.keypoints.is_empty() {
            return None;
        }

        let start = self.keypoints.first().unwrap().time;
        let end = self.keypoints.last().unwrap().time;
        TimeRange::new(start, end).ok()
    }

    /// Get keypoints within a time range
    #[inline]
    pub fn keypoints_in_range(&self, range: &TimeRange) -> Vec<&AnimationKeypoint> {
        self.keypoints
            .iter()
            .filter(|k| range.contains(k.time))
            .collect()
    }

    /// Find the keypoint pair that surrounds the given time
    pub fn surrounding_keypoints(
        &self,
        time: AnimationTime,
    ) -> Option<(Option<&AnimationKeypoint>, Option<&AnimationKeypoint>)> {
        if self.keypoints.is_empty() {
            return None;
        }

        // Find the first keypoint at or after the given time
        let next_idx = self
            .keypoints
            .binary_search_by(|k| k.time.partial_cmp(&time).unwrap())
            .unwrap_or_else(|pos| pos);

        let prev = if next_idx > 0 {
            Some(&self.keypoints[next_idx - 1])
        } else {
            None
        };

        let next = if next_idx < self.keypoints.len() {
            Some(&self.keypoints[next_idx])
        } else {
            None
        };

        Some((prev, next))
    }

    /// Set track weight
    #[inline]
    pub fn set_weight(&mut self, weight: f64) {
        self.weight = weight.clamp(0.0, 1.0);
    }

    /// Enable or disable the track
    #[inline]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Add metadata
    #[inline]
    pub fn add_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Get metadata
    #[inline]
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }

    /// Calculate derivative (rate of change) at a specific time using numerical differentiation
    pub fn derivative_at_time(
        &self,
        time: AnimationTime,
        interpolation_registry: &mut InterpolationRegistry,
        transition: Option<&AnimationTransition>,
        derivative_width: Option<AnimationTime>,
        animation_data: &AnimationData,
    ) -> Option<Value> {
        let width = derivative_width
            .unwrap_or_else(|| AnimationTime::from_millis(1.0).unwrap_or(AnimationTime::zero()));
        let half_width = AnimationTime::from_seconds(width.as_seconds() / 2.0).ok()?;

        // Calculate time points for centered difference
        let time_before = time - half_width;
        let time_after = time + half_width;

        // Handle boundary cases
        let (t1, t2, delta_time) = if time_before < AnimationTime::zero() {
            // Use forward difference at the start boundary
            (time, time + width, width.as_seconds())
        } else if let Some(range) = self.time_range() {
            if time_after > range.end {
                // Use backward difference at the end boundary
                (time - width, time, width.as_seconds())
            } else {
                // Use centered difference
                (time_before, time_after, width.as_seconds())
            }
        } else {
            // No time range available, use centered difference anyway
            (time_before, time_after, width.as_seconds())
        };

        // Get values at the two time points
        let value_before =
            self.value_at_time(t1, interpolation_registry, transition, animation_data)?;
        let value_after =
            self.value_at_time(t2, interpolation_registry, transition, animation_data)?;

        // Calculate numerical derivative
        Value::calculate_derivative(&value_before, &value_after, delta_time)
    }

    /// Get the value at a specific time using the interpolation registry
    pub fn value_at_time(
        &self,
        time: AnimationTime,
        interpolation_registry: &mut InterpolationRegistry,
        transition: Option<&AnimationTransition>,
        animation_data: &AnimationData,
    ) -> Option<Value> {
        if self.keypoints.is_empty() {
            return None;
        }

        // Handle time before first keypoint
        if time <= self.keypoints.first().unwrap().time {
            return Some(self.keypoints.first().unwrap().value.clone());
        }

        // Handle time after last keypoint
        if time >= self.keypoints.last().unwrap().time {
            return Some(self.keypoints.last().unwrap().value.clone());
        }

        // Find surrounding keypoints
        let (prev_idx_opt, next_idx_opt) = self.keypoint_indices_at_time(time);

        match (prev_idx_opt, next_idx_opt) {
            (Some(prev_idx), Some(next_idx)) => {
                if prev_idx == next_idx {
                    return Some(self.keypoints[prev_idx].value.clone());
                }
                let prev_kp = &self.keypoints[prev_idx];
                let next_kp = &self.keypoints[next_idx];

                // Create interpolation context
                let context = InterpolationContext::new(
                    prev_kp.time,
                    next_kp.time,
                    time,
                    &self.keypoints,
                    prev_idx,
                )
                .ok()?;

                let result = if let Some(specific_transition) = transition {
                    // Use the transition's interpolation method
                    interpolation_registry
                        .interpolate_with_transition(
                            &specific_transition,
                            &prev_kp.value,
                            &next_kp.value,
                            &context,
                            animation_data,
                        )
                        .ok()
                } else {
                    // Fallback to default cubic interpolation
                    interpolation_registry
                        .interpolate(
                            "cubic",
                            &prev_kp.value,
                            &next_kp.value,
                            &context,
                            animation_data,
                        )
                        .ok()
                };
                result
            }
            (Some(idx), None) => Some(self.keypoints[idx].value.clone()), // At or after last keypoint
            (None, Some(idx)) => Some(self.keypoints[idx].value.clone()), // Before first keypoint
            (None, None) => None,                                         // No keypoints
        }
    }
}
