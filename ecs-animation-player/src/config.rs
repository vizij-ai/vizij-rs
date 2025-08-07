//! Configuration for the animation player system

use crate::AnimationError;
use serde::{Deserialize, Serialize};

/// Configuration for the animation engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationEngineConfig {
    /// Target frame rate for animations
    pub target_fps: f64,
    /// Maximum memory usage in bytes
    pub max_memory_bytes: usize,
    /// Maximum cache size for interpolation results
    pub max_cache_size: usize,
    /// Whether to enable performance monitoring
    pub enable_performance_monitoring: bool,
    /// Whether to enable event dispatching
    pub enable_events: bool,
    /// Maximum number of concurrent players
    pub max_players: usize,
    /// Default interpolation function name
    pub default_interpolation: String,
    /// Performance warning thresholds
    pub performance_thresholds: PerformanceThresholds,
}

const MB: usize = 1024 * 1024;

impl Default for AnimationEngineConfig {
    /// Create a new configuration with default values
    fn default() -> Self {
        Self {
            target_fps: 60.0,
            max_memory_bytes: 64 * MB, // 64MB
            max_cache_size: 1000,
            enable_performance_monitoring: true,
            enable_events: true,
            max_players: 100,
            default_interpolation: "linear".to_string(),
            performance_thresholds: PerformanceThresholds::default(),
        }
    }
}

impl AnimationEngineConfig {
    /// Create a configuration optimized for high performance
    pub fn high_performance() -> Self {
        Self {
            target_fps: 120.0,
            max_memory_bytes: 128 * MB, // 128MB
            max_cache_size: 2000,
            enable_performance_monitoring: true,
            enable_events: false, // Disable events for maximum performance
            max_players: 50,
            default_interpolation: "linear".to_string(),
            performance_thresholds: PerformanceThresholds {
                min_fps: 100.0,
                max_frame_time_ms: 8.33, // ~120 FPS
                max_memory_usage_ratio: 0.8,
                max_interpolation_time_us: 500.0,
            },
        }
    }

    /// Create a configuration optimized for low memory usage
    pub fn low_memory() -> Self {
        Self {
            target_fps: 30.0,
            max_memory_bytes: 16 * MB, // 16MB
            max_cache_size: 200,
            enable_performance_monitoring: true,
            enable_events: true,
            max_players: 20,
            default_interpolation: "linear".to_string(),
            performance_thresholds: PerformanceThresholds {
                min_fps: 25.0,
                max_frame_time_ms: 40.0, // ~25 FPS
                max_memory_usage_ratio: 0.9,
                max_interpolation_time_us: 2000.0,
            },
        }
    }

    /// Create a configuration for web deployment
    pub fn web_optimized() -> Self {
        Self {
            target_fps: 60.0,
            max_memory_bytes: 32 * MB, // 32MB
            max_cache_size: 500,
            enable_performance_monitoring: true,
            enable_events: true,
            max_players: 10,
            default_interpolation: "linear".to_string(),
            performance_thresholds: PerformanceThresholds {
                min_fps: 50.0,
                max_frame_time_ms: 20.0, // ~50 FPS
                max_memory_usage_ratio: 0.7,
                max_interpolation_time_us: 1000.0,
            },
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), AnimationError> {
        if self.target_fps <= 0.0 || !self.target_fps.is_finite() {
            return Err(AnimationError::InvalidValue {
                reason: "Target FPS must be positive and finite".to_string(),
            });
        }

        if self.max_memory_bytes == 0 || self.max_cache_size == 0 || self.max_players == 0 {
            return Err(AnimationError::InvalidValue {
                reason: "Maximum memory, cache size, and players must be greater than 0"
                    .to_string(),
            });
        }

        if self.default_interpolation.is_empty() {
            return Err(AnimationError::InvalidValue {
                reason: "Default interpolation must not be empty".to_string(),
            });
        }

        self.performance_thresholds.validate()?;

        Ok(())
    }

    /// Set target frame rate
    #[inline]
    pub fn with_target_fps(mut self, fps: f64) -> Self {
        self.target_fps = fps;
        self
    }

    /// Set maximum memory usage
    #[inline]
    pub fn with_max_memory_mb(mut self, mb: usize) -> Self {
        self.max_memory_bytes = mb * MB;
        self
    }

    /// Set maximum cache size
    #[inline]
    pub fn with_max_cache_size(mut self, size: usize) -> Self {
        self.max_cache_size = size;
        self
    }

    /// Enable or disable performance monitoring
    #[inline]
    pub fn with_performance_monitoring(mut self, enabled: bool) -> Self {
        self.enable_performance_monitoring = enabled;
        self
    }

    /// Enable or disable events
    #[inline]
    pub fn with_events(mut self, enabled: bool) -> Self {
        self.enable_events = enabled;
        self
    }

    /// Set maximum number of players
    #[inline]
    pub fn with_max_players(mut self, max: usize) -> Self {
        self.max_players = max;
        self
    }

    /// Set default interpolation function
    #[inline]
    pub fn with_default_interpolation(mut self, interpolation: impl Into<String>) -> Self {
        self.default_interpolation = interpolation.into();
        self
    }

    /// Set performance thresholds
    #[inline]
    pub fn with_performance_thresholds(mut self, thresholds: PerformanceThresholds) -> Self {
        self.performance_thresholds = thresholds;
        self
    }
}

/// Performance warning thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// Minimum acceptable frame rate (frames per second)
    pub min_fps: f64,
    /// Maximum acceptable frame time in milliseconds
    pub max_frame_time_ms: f64,
    /// Maximum memory usage as a ratio of `max_memory_bytes` (0.0 to 1.0)
    pub max_memory_usage_ratio: f64,
    /// Maximum interpolation time in microseconds
    pub max_interpolation_time_us: f64,
}

impl Default for PerformanceThresholds {
    /// Create new default thresholds
    fn default() -> Self {
        Self {
            min_fps: 50.0,
            max_frame_time_ms: 20.0, // ~50 FPS
            max_memory_usage_ratio: 0.8,
            max_interpolation_time_us: 1000.0,
        }
    }
}

impl PerformanceThresholds {
    /// Create strict performance thresholds
    pub fn strict() -> Self {
        Self {
            min_fps: 58.0,
            max_frame_time_ms: 17.0, // ~58 FPS
            max_memory_usage_ratio: 0.7,
            max_interpolation_time_us: 500.0,
        }
    }

    /// Create relaxed performance thresholds
    pub fn relaxed() -> Self {
        Self {
            min_fps: 30.0,
            max_frame_time_ms: 33.0, // ~30 FPS
            max_memory_usage_ratio: 0.9,
            max_interpolation_time_us: 2000.0,
        }
    }

    /// Validate the thresholds
    pub fn validate(&self) -> Result<(), AnimationError> {
        if self.min_fps <= 0.0 || !self.min_fps.is_finite() {
            return Err(AnimationError::InvalidValue {
                reason: "Minimum FPS must be positive and finite".to_string(),
            });
        }

        if self.max_frame_time_ms <= 0.0 || !self.max_frame_time_ms.is_finite() {
            return Err(AnimationError::InvalidValue {
                reason: "Maximum frame time must be positive and finite".to_string(),
            });
        }

        if self.max_memory_usage_ratio <= 0.0 || self.max_memory_usage_ratio > 1.0 {
            return Err(AnimationError::InvalidValue {
                reason: "Memory usage ratio must be between 0 and 1".to_string(),
            });
        }

        if self.max_interpolation_time_us <= 0.0 || !self.max_interpolation_time_us.is_finite() {
            return Err(AnimationError::InvalidValue {
                reason: "Maximum interpolation time must be positive and finite".to_string(),
            });
        }

        Ok(())
    }

    /// Checks if the given frame rate is acceptable based on `min_fps`.
    pub fn is_fps_acceptable(&self, fps: f64) -> bool {
        fps >= self.min_fps
    }

    /// Checks if the given frame time is acceptable based on `max_frame_time_ms`.
    pub fn is_frame_time_acceptable(&self, frame_time_ms: f64) -> bool {
        frame_time_ms <= self.max_frame_time_ms
    }

    /// Checks if the given memory usage is acceptable based on `max_memory_usage_ratio`.
    /// Returns true if `max_bytes` is zero (no limit).
    pub fn is_memory_usage_acceptable(&self, used_bytes: usize, max_bytes: usize) -> bool {
        if max_bytes == 0 {
            return true;
        }
        let ratio = used_bytes as f64 / max_bytes as f64;
        ratio <= self.max_memory_usage_ratio
    }

    /// Checks if the given interpolation time is acceptable based on `max_interpolation_time_us`.
    pub fn is_interpolation_time_acceptable(&self, time_us: f64) -> bool {
        time_us <= self.max_interpolation_time_us
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AnimationEngineConfig::default();
        assert_eq!(config.target_fps, 60.0);
        assert_eq!(config.max_memory_bytes, 64 * 1024 * 1024);
        assert_eq!(config.max_cache_size, 1000);
        assert!(config.enable_performance_monitoring);
        assert!(config.enable_events);
        assert_eq!(config.max_players, 100);
        assert_eq!(config.default_interpolation, "linear");
    }

    #[test]
    fn test_high_performance_config() {
        let config = AnimationEngineConfig::high_performance();
        assert_eq!(config.target_fps, 120.0);
        assert_eq!(config.max_memory_bytes, 128 * 1024 * 1024);
        assert!(!config.enable_events); // Events disabled for performance
    }

    #[test]
    fn test_low_memory_config() {
        let config = AnimationEngineConfig::low_memory();
        assert_eq!(config.target_fps, 30.0);
        assert_eq!(config.max_memory_bytes, 16 * 1024 * 1024);
        assert_eq!(config.max_cache_size, 200);
        assert_eq!(config.max_players, 20);
    }

    #[test]
    fn test_web_optimized_config() {
        let config = AnimationEngineConfig::web_optimized();
        assert_eq!(config.target_fps, 60.0);
        assert_eq!(config.max_memory_bytes, 32 * 1024 * 1024);
        assert_eq!(config.max_cache_size, 500);
        assert_eq!(config.max_players, 10);
    }

    #[test]
    fn test_config_validation() {
        let mut config = AnimationEngineConfig::default();
        assert!(config.validate().is_ok());

        config.target_fps = 0.0;
        assert!(config.validate().is_err());

        config.target_fps = 60.0;
        config.max_memory_bytes = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_builder() {
        let config = AnimationEngineConfig::default()
            .with_target_fps(90.0)
            .with_max_memory_mb(128)
            .with_max_cache_size(2000)
            .with_performance_monitoring(false)
            .with_events(false)
            .with_max_players(50)
            .with_default_interpolation("cubic");

        assert_eq!(config.target_fps, 90.0);
        assert_eq!(config.max_memory_bytes, 128 * 1024 * 1024);
        assert_eq!(config.max_cache_size, 2000);
        assert!(!config.enable_performance_monitoring);
        assert!(!config.enable_events);
        assert_eq!(config.max_players, 50);
        assert_eq!(config.default_interpolation, "cubic");
    }

    #[test]
    fn test_performance_thresholds() {
        let thresholds = PerformanceThresholds::default();

        assert!(thresholds.is_fps_acceptable(60.0));
        assert!(!thresholds.is_fps_acceptable(30.0));

        assert!(thresholds.is_frame_time_acceptable(16.0));
        assert!(!thresholds.is_frame_time_acceptable(30.0));

        assert!(thresholds.is_memory_usage_acceptable(512 * 1024, 1024 * 1024)); // 50%
        assert!(!thresholds.is_memory_usage_acceptable(900 * 1024, 1024 * 1024)); // 90%

        assert!(thresholds.is_interpolation_time_acceptable(500.0));
        assert!(!thresholds.is_interpolation_time_acceptable(2000.0));
    }

    #[test]
    fn test_threshold_validation() {
        let mut thresholds = PerformanceThresholds::default();
        assert!(thresholds.validate().is_ok());

        thresholds.min_fps = 0.0;
        assert!(thresholds.validate().is_err());

        thresholds.min_fps = 60.0;
        thresholds.max_memory_usage_ratio = 1.5;
        assert!(thresholds.validate().is_err());
    }

    #[test]
    fn test_threshold_presets() {
        let strict = PerformanceThresholds::strict();
        assert_eq!(strict.min_fps, 58.0);
        assert!(strict.validate().is_ok());

        let relaxed = PerformanceThresholds::relaxed();
        assert_eq!(relaxed.min_fps, 30.0);
        assert!(relaxed.validate().is_ok());
    }
}
