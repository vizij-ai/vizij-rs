use crate::AnimationEngineConfig;
use crate::AnimationTime;

/// Performance metrics for animation playback
#[derive(Debug, Clone)]
pub struct PlaybackMetrics {
    /// Frame time in milliseconds
    pub frame_time_ms: f64,
    /// Total playback time
    pub total_time: AnimationTime,
    /// Number of frames rendered
    pub frames_rendered: u64,
    /// Number of interpolations performed
    pub interpolations_performed: u64,
    /// Memory usage in bytes
    pub memory_usage_bytes: usize,
    /// Number of active tracks
    pub active_tracks: usize,
    /// Last update timestamp
    pub last_update: AnimationTime,
}

impl PlaybackMetrics {
    /// Create new metrics
    #[inline]
    pub fn new() -> Self {
        Self {
            frame_time_ms: 0.0,
            total_time: AnimationTime::zero(),
            frames_rendered: 0,
            interpolations_performed: 0,
            memory_usage_bytes: 0,
            active_tracks: 0,
            last_update: AnimationTime::zero(),
        }
    }

    /// Reset metrics
    #[inline]
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Check if performance is within acceptable thresholds
    pub fn is_performance_acceptable(&self, config: &AnimationEngineConfig) -> bool {
        config
            .performance_thresholds
            .is_frame_time_acceptable(self.frame_time_ms)
            && config
                .performance_thresholds
                .is_memory_usage_acceptable(self.memory_usage_bytes, config.max_memory_bytes)
    }
}

impl Default for PlaybackMetrics {
    fn default() -> Self {
        Self::new()
    }
}
