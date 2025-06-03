use crate::animation::instance::PlaybackMode;
use crate::AnimationTime;

/// Playback options for animation player
#[derive(Debug, Clone)]
pub struct PlaybackOptions {
    /// Whether to loop the animation
    pub mode: PlaybackMode,
    /// Playback speed multiplier (1.0 = normal speed)
    pub speed: f64,
    /// Start time for playback
    pub start_time: AnimationTime,
    /// End time for playback (None = use animation duration)
    pub end_time: Option<AnimationTime>,
    /// Whether to auto-start playback when animation is loaded
    pub auto_start: bool,
    /// Whether to emit performance warning events
    pub emit_performance_events: bool,
}

impl PlaybackOptions {
    /// Create new default playback options
    #[inline]
    pub fn new() -> Self {
        Self {
            mode: PlaybackMode::Once,
            speed: 1.0,
            start_time: AnimationTime::zero(),
            end_time: None,
            auto_start: false,
            emit_performance_events: true,
        }
    }

    /// Enable looping
    #[inline]
    pub fn with_loop(mut self) -> Self {
        self.mode = PlaybackMode::Loop;
        self
    }

    /// Set playback speed
    #[inline]
    pub fn with_speed(mut self, speed: f64) -> Self {
        self.speed = speed; // Allow negative speed for reverse playback
        self
    }

    /// Set time range for playback
    #[inline]
    pub fn with_time_range(mut self, start: AnimationTime, end: AnimationTime) -> Self {
        self.start_time = start;
        self.end_time = Some(end);
        self
    }

    /// Enable auto-start
    #[inline]
    pub fn with_auto_start(mut self) -> Self {
        self.auto_start = true;
        self
    }

    /// Disable performance events
    #[inline]
    pub fn without_performance_events(mut self) -> Self {
        self.emit_performance_events = false;
        self
    }
}

impl Default for PlaybackOptions {
    fn default() -> Self {
        Self::new()
    }
}
