use crate::{AnimationError, AnimationTime};
use std::collections::HashMap;

/// Context for interpolation operations
#[derive(Debug, Clone)]
pub struct InterpolationContext {
    /// Start time of the interpolation
    pub start_time: AnimationTime,
    /// End time of the interpolation
    pub end_time: AnimationTime,
    /// Current time
    pub current_time: AnimationTime,
    /// Normalized interpolation parameter (0.0 to 1.0)
    pub t: f64,
    /// Additional properties for the interpolation
    pub properties: HashMap<String, f64>,
}

impl InterpolationContext {
    /// Create a new interpolation context
    #[inline]
    pub fn new(
        start_time: AnimationTime,
        end_time: AnimationTime,
        current_time: AnimationTime,
    ) -> Result<Self, AnimationError> {
        let duration = end_time.duration_since(start_time)?;
        let elapsed = current_time.duration_since(start_time)?;

        let t = if duration.as_seconds() > 0.0 {
            (elapsed.as_seconds() / duration.as_seconds()).clamp(0.0, 1.0)
        } else {
            0.0
        };

        Ok(Self {
            start_time,
            end_time,
            current_time,
            t,
            properties: HashMap::new(),
        })
    }

    /// Set a property for the interpolation
    #[inline]
    pub fn set_property(&mut self, key: impl Into<String>, value: f64) {
        self.properties.insert(key.into(), value);
    }

    /// Get a property for the interpolation
    #[inline]
    pub fn get_property(&self, key: &str) -> Option<f64> {
        self.properties.get(key).copied()
    }

    /// Get a property with a default value
    #[inline]
    pub fn get_property_or(&self, key: &str, default: f64) -> f64 {
        self.properties.get(key).copied().unwrap_or(default)
    }
}
