//! Time handling for animations

#[cfg(target_arch = "wasm32")]
use js_sys::Date;
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use instant::Instant;

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Copy)]
struct Instant {
    timestamp: f64,
}

#[cfg(target_arch = "wasm32")]
impl Instant {
    fn now() -> Self {
        Self {
            timestamp: Date::now(),
        }
    }

    fn duration_since(&self, earlier: Instant) -> std::time::Duration {
        let millis = self.timestamp - earlier.timestamp;
        std::time::Duration::from_millis(millis.max(0.0) as u64)
    }

    fn elapsed(&self) -> std::time::Duration {
        let now = Date::now();
        let millis = now - self.timestamp;
        std::time::Duration::from_millis(millis.max(0.0) as u64)
    }
}
use crate::error::AnimationError;

/// Represents a moment in animation time
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
pub struct AnimationTime(u64); // Changed to u64 nanoseconds for Ord compliance

impl AnimationTime {
    /// Create a new animation time
    #[inline]
    pub fn new(seconds: f64) -> Result<Self, AnimationError> {
        if seconds < 0.0 || !seconds.is_finite() {
            return Err(AnimationError::InvalidTime { time: seconds });
        }
        let nanos = (seconds * 1_000_000_000.0) as u64;
        Ok(Self(nanos))
    }

    /// Create animation time from milliseconds
    #[inline]
    pub fn from_millis(milliseconds: f64) -> Result<Self, AnimationError> {
        Self::new(milliseconds / 1000.0)
    }

    /// Create animation time from nanoseconds
    #[inline]
    pub fn from_nanos(nanoseconds: u64) -> Result<Self, AnimationError> {
        Ok(Self(nanoseconds))
    }

    /// Zero time
    #[inline]
    pub fn zero() -> Self {
        Self(0)
    }

    /// Get time in seconds
    #[inline]
    pub fn as_seconds(&self) -> f64 {
        self.0 as f64 / 1_000_000_000.0
    }

    /// Get time in milliseconds
    #[inline]
    pub fn as_millis(&self) -> f64 {
        self.0 as f64 / 1_000_000.0
    }

    /// Get time in nanoseconds
    #[inline]
    pub fn as_nanos(&self) -> u64 {
        self.0
    }

    /// Add duration to this time
    #[inline]
    pub fn add(&self, duration: AnimationTime) -> Self {
        Self(self.0.saturating_add(duration.0))
    }

    /// Subtract duration from this time
    #[inline]
    pub fn sub(&self, duration: AnimationTime) -> Self {
        Self(self.0.saturating_sub(duration.0))
    }

    /// Get the difference between two times
    #[inline]
    pub fn duration_since(&self, earlier: AnimationTime) -> Result<AnimationTime, AnimationError> {
        if self.0 < earlier.0 {
            return Err(AnimationError::InvalidTime {
                time: (self.0 as f64 - earlier.0 as f64) / 1_000_000_000.0,
            });
        }
        Ok(AnimationTime(self.0 - earlier.0))
    }

    /// Clamp time to a range
    #[inline]
    pub fn clamp(&self, min: AnimationTime, max: AnimationTime) -> Self {
        if self.0 < min.0 {
            min
        } else if self.0 > max.0 {
            max
        } else {
            *self
        }
    }

    /// Linearly interpolates between this time and another time.
    /// `t` is the interpolation factor, clamped between 0.0 and 1.0.
    ///
    /// If `t` is 0.0, `self` is returned. If `t` is 1.0, `other` is returned.
    /// For values of `t` between 0.0 and 1.0, a time proportionally between `self` and `other` is returned.
    #[inline]
    pub fn lerp(&self, other: AnimationTime, t: f64) -> Self {
        let t = t.clamp(0.0, 1.0);
        let interpolated_nanos = self.0 as f64 + (other.0 as f64 - self.0 as f64) * t;
        // Ensure the result is within u64 bounds and convert back to AnimationTime
        AnimationTime(interpolated_nanos.round() as u64)
    }
}

impl std::ops::Add for AnimationTime {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }
}

impl std::ops::AddAssign for AnimationTime {
    fn add_assign(&mut self, other: Self) {
        self.0 = self.0.saturating_add(other.0);
    }
}

impl std::ops::Sub for AnimationTime {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

impl std::ops::SubAssign for AnimationTime {
    fn sub_assign(&mut self, other: Self) {
        self.0 = self.0.saturating_sub(other.0);
    }
}

impl From<f64> for AnimationTime {
    fn from(seconds: f64) -> Self {
        Self::new(seconds.max(0.0)).unwrap_or(Self::zero())
    }
}

impl From<AnimationTime> for f64 {
    fn from(time: AnimationTime) -> Self {
        time.as_seconds()
    }
}

/// Represents a time range in animation
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: AnimationTime,
    pub end: AnimationTime,
}

impl TimeRange {
    /// Create a new time range
    #[inline]
    pub fn new(start: AnimationTime, end: AnimationTime) -> Result<Self, AnimationError> {
        if start > end {
            return Err(AnimationError::TimeOutOfRange {
                time: start.as_seconds(),
                start: 0.0,
                end: end.as_seconds(),
            });
        }
        Ok(Self { start, end })
    }

    /// Create a range from zero to the given duration
    #[inline]
    pub fn from_duration(duration: AnimationTime) -> Self {
        Self {
            start: AnimationTime::zero(),
            end: duration,
        }
    }

    /// Get the duration of this range
    #[inline]
    pub fn duration(&self) -> AnimationTime {
        AnimationTime(self.end.0 - self.start.0)
    }

    /// Check if a time is within this range (inclusive)
    #[inline]
    pub fn contains(&self, time: AnimationTime) -> bool {
        time >= self.start && time <= self.end
    }

    /// Check if this range overlaps with another range
    #[inline]
    pub fn overlaps(&self, other: &TimeRange) -> bool {
        self.start <= other.end && self.end >= other.start
    }

    /// Get the intersection of two ranges
    #[inline]
    pub fn intersection(&self, other: &TimeRange) -> Option<TimeRange> {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);

        if start <= end {
            Some(TimeRange { start, end })
        } else {
            None
        }
    }

    /// Get the union of two ranges (if they overlap or are adjacent)
    #[inline]
    pub fn union(&self, other: &TimeRange) -> Option<TimeRange> {
        if self.overlaps(other) || self.end == other.start || other.end == self.start {
            Some(TimeRange {
                start: self.start.min(other.start),
                end: self.end.max(other.end),
            })
        } else {
            None
        }
    }

    /// Normalize a time within this range to [0, 1]
    #[inline]
    pub fn normalize_time(&self, time: AnimationTime) -> f64 {
        if self.duration().as_seconds() == 0.0 {
            return 0.0;
        }
        ((time.as_seconds() - self.start.as_seconds()) / self.duration().as_seconds())
            .clamp(0.0, 1.0)
    }

    /// Denormalize a value from [0, 1] to this range
    #[inline]
    pub fn denormalize_time(&self, normalized: f64) -> AnimationTime {
        let clamped = normalized.clamp(0.0, 1.0);
        AnimationTime::from(self.start.as_seconds() + clamped * self.duration().as_seconds())
    }

    /// Extend the range to include the given time
    #[inline]
    pub fn extend_to_include(&mut self, time: AnimationTime) {
        if time < self.start {
            self.start = time;
        }
        if time > self.end {
            self.end = time;
        }
    }
}

/// High-precision timer for performance measurements
#[derive(Debug, Clone)]
pub struct Timer {
    start: Instant,
}

impl Timer {
    /// Create a new timer and start it
    #[inline]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Restart the timer
    #[inline]
    pub fn restart(&mut self) {
        self.start = Instant::now();
    }

    /// Get elapsed time since start or last restart
    #[inline]
    pub fn elapsed(&self) -> AnimationTime {
        let duration = self.start.elapsed();
        AnimationTime::from_nanos(duration.as_nanos() as u64).unwrap_or(AnimationTime::zero())
    }

    /// Get elapsed time in milliseconds
    #[inline]
    pub fn elapsed_millis(&self) -> f64 {
        self.elapsed().as_millis()
    }

    /// Get elapsed time in microseconds
    #[inline]
    pub fn elapsed_micros(&self) -> f64 {
        self.elapsed().as_seconds() * 1_000_000.0
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

/// Frame rate calculator
#[derive(Debug, Clone)]
pub struct FrameRateCalculator {
    frame_times: Vec<AnimationTime>,
    max_samples: usize,
    last_frame_time: Option<Instant>,
}

impl FrameRateCalculator {
    /// Create a new frame rate calculator
    #[inline]
    pub fn new(max_samples: usize) -> Self {
        Self {
            frame_times: Vec::with_capacity(max_samples),
            max_samples,
            last_frame_time: None,
        }
    }

    /// Record a new frame
    #[inline]
    pub fn record_frame(&mut self) {
        let now = Instant::now();

        if let Some(last_time) = self.last_frame_time {
            let frame_duration = now.duration_since(last_time);
            if let Ok(duration) = AnimationTime::from_nanos(frame_duration.as_nanos() as u64) {
                self.frame_times.push(duration);

                // Keep only the most recent samples
                if self.frame_times.len() > self.max_samples {
                    self.frame_times.remove(0);
                }
            }
        }

        self.last_frame_time = Some(now);
    }

    /// Get the current frame rate (frames per second)
    #[inline]
    pub fn fps(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }

        let total_time: f64 = self.frame_times.iter().map(|t| t.as_seconds()).sum();
        let avg_frame_time = total_time / self.frame_times.len() as f64;

        if avg_frame_time > 0.0 {
            1.0 / avg_frame_time
        } else {
            0.0
        }
    }

    /// Get average frame time in milliseconds
    #[inline]
    pub fn avg_frame_time_millis(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }

        let total_time: f64 = self.frame_times.iter().map(|t| t.as_millis()).sum();
        total_time / self.frame_times.len() as f64
    }

    /// Get the minimum frame time in the current sample window
    #[inline]
    pub fn min_frame_time_millis(&self) -> f64 {
        self.frame_times
            .iter()
            .map(|t| t.as_millis())
            .fold(f64::INFINITY, f64::min)
    }

    /// Get the maximum frame time in the current sample window
    #[inline]
    pub fn max_frame_time_millis(&self) -> f64 {
        self.frame_times
            .iter()
            .map(|t| t.as_millis())
            .fold(0.0, f64::max)
    }

    /// Reset the calculator
    #[inline]
    pub fn reset(&mut self) {
        self.frame_times.clear();
        self.last_frame_time = None;
    }
}

impl Default for FrameRateCalculator {
    fn default() -> Self {
        Self::new(60) // Default to 60 samples (1 second at 60 FPS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_time() {
        let time1 = AnimationTime::new(1.5).unwrap();
        let time2 = AnimationTime::new(2.0).unwrap();

        assert_eq!(time1.as_seconds(), 1.5);
        assert_eq!(time1.as_millis(), 1500.0);

        let sum = time1.add(time2);
        assert_eq!(sum.as_seconds(), 3.5);

        let diff = time2.duration_since(time1).unwrap();
        assert_eq!(diff.as_seconds(), 0.5);
    }

    #[test]
    fn test_invalid_time() {
        assert!(AnimationTime::new(-1.0).is_err());
        assert!(AnimationTime::new(f64::NAN).is_err());
        assert!(AnimationTime::new(f64::INFINITY).is_err());
    }

    #[test]
    fn test_time_range() {
        let start = AnimationTime::new(1.0).unwrap();
        let end = AnimationTime::new(3.0).unwrap();
        let range = TimeRange::new(start, end).unwrap();

        assert_eq!(range.duration().as_seconds(), 2.0);
        assert!(range.contains(AnimationTime::new(2.0).unwrap()));
        assert!(!range.contains(AnimationTime::new(4.0).unwrap()));

        assert_eq!(range.normalize_time(AnimationTime::new(2.0).unwrap()), 0.5);
        assert_eq!(range.denormalize_time(0.5).as_seconds(), 2.0);
    }

    #[test]
    fn test_range_operations() {
        let range1 = TimeRange::new(
            AnimationTime::new(1.0).unwrap(),
            AnimationTime::new(3.0).unwrap(),
        )
        .unwrap();

        let range2 = TimeRange::new(
            AnimationTime::new(2.0).unwrap(),
            AnimationTime::new(4.0).unwrap(),
        )
        .unwrap();

        assert!(range1.overlaps(&range2));

        let intersection = range1.intersection(&range2).unwrap();
        assert_eq!(intersection.start.as_seconds(), 2.0);
        assert_eq!(intersection.end.as_seconds(), 3.0);

        let union = range1.union(&range2).unwrap();
        assert_eq!(union.start.as_seconds(), 1.0);
        assert_eq!(union.end.as_seconds(), 4.0);
    }

    #[test]
    fn test_timer() {
        let timer = Timer::new();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = timer.elapsed();
        assert!(elapsed.as_millis() >= 10.0);
    }

    #[test]
    fn test_frame_rate_calculator() {
        let mut calc = FrameRateCalculator::new(10);

        // Simulate 60 FPS (16.67ms per frame)
        for _ in 0..5 {
            calc.record_frame();
            std::thread::sleep(std::time::Duration::from_millis(16));
        }

        let fps = calc.fps();
        assert!(fps > 50.0 && fps < 70.0); // Rough check accounting for timing variance
    }
}
