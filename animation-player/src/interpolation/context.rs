use crate::animation::keypoint::AnimationKeypoint;
use crate::{AnimationError, AnimationTime, Value};
use std::collections::HashMap;

/// Provides context for an interpolation operation, including the time factor `t`
/// and access to surrounding keyframes for complex spline calculations.
#[derive(Clone, Debug)]
pub struct InterpolationContext<'a> {
    /// The interpolation factor, ranging from 0.0 (at start) to 1.0 (at end).
    pub t: f64,
    /// A slice of all keypoints in the track for context.
    keypoints: &'a [AnimationKeypoint],
    /// The index of the starting keypoint for the current interpolation segment.
    start_index: usize,
    /// A map for additional, ad-hoc properties that can be passed to the interpolator.
    properties: HashMap<String, Value>,
}

impl<'a> InterpolationContext<'a> {
    /// Creates a new `InterpolationContext`.
    pub fn new(
        start_time: AnimationTime,
        end_time: AnimationTime,
        current_time: AnimationTime,
        keypoints: &'a [AnimationKeypoint],
        start_index: usize,
    ) -> Result<Self, AnimationError> {
        let duration = end_time - start_time;
        let t = if duration > AnimationTime::zero() {
            let elapsed = current_time - start_time;
            (elapsed.as_seconds() / duration.as_seconds()).clamp(0.0, 1.0)
        } else {
            0.0
        };

        Ok(Self {
            t,
            keypoints,
            start_index,
            properties: HashMap::new(),
        })
    }

    /// Retrieves a keypoint value relative to the current segment's start keypoint.
    ///
    /// - `offset = 0`: Returns the start keypoint value.
    /// - `offset = 1`: Returns the end keypoint value.
    /// - `offset = -1`: Returns the keypoint before the start keypoint.
    /// - `offset = 2`: Returns the keypoint after the end keypoint.
    pub fn get_point(&self, offset: i32) -> Option<Value> {
        // First, validate that start_index is within bounds
        if self.start_index >= self.keypoints.len() {
            return None;
        }

        // Handle the offset calculation safely
        let target_idx = if offset >= 0 {
            // For positive offsets, check for potential overflow
            self.start_index.checked_add(offset as usize)?
        } else {
            // For negative offsets, check for underflow
            let abs_offset = offset.unsigned_abs() as usize;
            self.start_index.checked_sub(abs_offset)?
        };

        // Now safely get the keypoint at the calculated index
        self.keypoints.get(target_idx).map(|kp| kp.value.clone())
    }

    /// Gets a generic property from the context.
    pub fn get_property<T: TryFrom<Value>>(&self, key: &str) -> Option<T> {
        self.properties
            .get(key)
            .cloned()
            .and_then(|v| v.try_into().ok())
    }

    /// Sets a generic property in the context.
    pub fn set_property(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        self.properties.insert(key.into(), value.into());
    }
}
