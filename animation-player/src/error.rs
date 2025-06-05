//! Error types for the animation player

use serde::{Deserialize, Serialize};

/// Comprehensive error type for animation operations
#[derive(thiserror::Error, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AnimationError {
    /// Animation data not found
    #[error("Animation not found: {id}")]
    AnimationNotFound { id: String },

    /// Animation track not found
    #[error("Track not found: {track_id} in animation {animation_id}")]
    TrackNotFound {
        animation_id: String,
        track_id: String,
    },

    /// Animation keypoint not found
    #[error("Keypoint not found: {keypoint_id} in track {track_id}")]
    KeypointNotFound {
        track_id: String,
        keypoint_id: String,
    },

    /// Invalid time value
    #[error("Invalid time value: {time}")]
    InvalidTime { time: f64 },

    /// Time out of range
    #[error("Time {time} is out of range [{start}, {end}]")]
    TimeOutOfRange { time: f64, start: f64, end: f64 },

    /// Value type mismatch
    #[error("Value type mismatch: expected {expected:?}, got {actual:?}")]
    ValueTypeMismatch {
        expected: crate::value::ValueType,
        actual: crate::value::ValueType,
    },

    /// Invalid value
    #[error("Invalid value: {reason}")]
    InvalidValue { reason: String },

    /// Interpolation function not found
    #[error("Interpolation function not found: {name}")]
    InterpolationNotFound { name: String },

    /// Interpolation error
    #[error("Interpolation error: {reason}")]
    InterpolationError { reason: String },

    /// Player not found
    #[error("Player not found: {player_id}")]
    PlayerNotFound { player_id: String },

    /// Invalid player state
    #[error("Invalid player state: {current_state} -> {requested_state}")]
    InvalidPlayerState {
        current_state: String,
        requested_state: String,
    },

    /// Serialization error
    #[error("Serialization error: {reason}")]
    SerializationError { reason: String },

    /// IO error
    #[error("IO error: {reason}")]
    IoError { reason: String },

    /// Memory limit exceeded
    #[error("Memory limit exceeded: {current_bytes} bytes (limit: {limit_bytes} bytes)")]
    MemoryLimitExceeded {
        current_bytes: usize,
        limit_bytes: usize,
    },

    /// Performance threshold exceeded
    #[error("Performance threshold exceeded: {metric} = {value} (threshold: {threshold})")]
    PerformanceThresholdExceeded {
        metric: String,
        value: f64,
        threshold: f64,
    },

    /// Network error
    #[error("Network error: {reason}")]
    NetworkError { reason: String },

    /// Generic animation error
    #[error("Animation error: {message}")]
    Generic { message: String },
}

impl AnimationError {
    /// Create a new generic error
    pub fn new(message: impl Into<String>) -> Self {
        Self::Generic {
            message: message.into(),
        }
    }

    /// Check if this is a recoverable error
    #[inline]
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::TimeOutOfRange { .. }
                | Self::InterpolationError { .. }
                | Self::InvalidPlayerState { .. }
                | Self::IoError { .. }
                | Self::MemoryLimitExceeded { .. }
                | Self::PerformanceThresholdExceeded { .. }
                | Self::NetworkError { .. }
        )
    }

    /// Get error category for logging/metrics
    #[inline]
    pub fn category(&self) -> &'static str {
        match self {
            Self::AnimationNotFound { .. }
            | Self::TrackNotFound { .. }
            | Self::KeypointNotFound { .. } => "data",
            Self::InvalidTime { .. }
            | Self::TimeOutOfRange { .. }
            | Self::ValueTypeMismatch { .. }
            | Self::InvalidValue { .. } => "validation",
            Self::InterpolationNotFound { .. } | Self::InterpolationError { .. } => "interpolation",
            Self::PlayerNotFound { .. } | Self::InvalidPlayerState { .. } => "player",
            Self::SerializationError { .. } => "serialization",
            Self::IoError { .. } => "io",
            Self::MemoryLimitExceeded { .. } | Self::PerformanceThresholdExceeded { .. } => {
                "performance"
            }
            Self::NetworkError { .. } => "network",
            Self::Generic { .. } => "generic",
        }
    }
}

impl From<std::io::Error> for AnimationError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError {
            reason: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for AnimationError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerializationError {
            reason: err.to_string(),
        }
    }
}

impl From<bincode::Error> for AnimationError {
    fn from(err: bincode::Error) -> Self {
        Self::SerializationError {
            reason: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = AnimationError::new("test error");
        assert!(matches!(error, AnimationError::Generic { .. }));
    }

    #[test]
    fn test_error_recoverability() {
        let recoverable = AnimationError::TimeOutOfRange {
            time: 5.0,
            start: 0.0,
            end: 10.0,
        };
        assert!(recoverable.is_recoverable());

        let non_recoverable = AnimationError::AnimationNotFound {
            id: "test".to_string(),
        };
        assert!(!non_recoverable.is_recoverable());
    }

    #[test]
    fn test_error_categories() {
        let data_error = AnimationError::AnimationNotFound {
            id: "test".to_string(),
        };
        assert_eq!(data_error.category(), "data");

        let validation_error = AnimationError::InvalidTime { time: -1.0 };
        assert_eq!(validation_error.category(), "validation");
    }

    #[test]
    fn test_serialization() {
        let error = AnimationError::new("test");
        let serialized = serde_json::to_string(&error).unwrap();
        let deserialized: AnimationError = serde_json::from_str(&serialized).unwrap();
        assert_eq!(error, deserialized);
    }
}
