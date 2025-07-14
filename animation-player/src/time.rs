/**
 * Time handling and types for animations.
 * We are using standard types mostly,
 * but some are not available in WASM.
 * For them, we need few custom implementations.
 */
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::AnimationError;

/// Represents a moment in animation time
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize, Default)]
pub struct AnimationTime(u64); // Changed to u64 nanoseconds for Ord compliance

impl AnimationTime {
    /// Create animation time from nanoseconds
    #[inline]
    pub fn from_nanos(nanoseconds: u64) -> Self {
        Self(nanoseconds)
    }

    /// Create animation time from milliseconds
    #[inline]
    pub fn from_millis(milliseconds: f64) -> Result<Self, AnimationError> {
        Self::from_seconds(milliseconds / 1000.0)
    }

    /// Create a new animation time
    #[inline]
    pub fn from_seconds(seconds: f64) -> Result<Self, AnimationError> {
        if seconds < 0.0 || !seconds.is_finite() {
            return Err(AnimationError::InvalidTime { time: seconds });
        }
        let nanos = (seconds * 1_000_000_000.0) as u64;
        Ok(Self(nanos))
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

// Easier conversions
impl From<u64> for AnimationTime {
    fn from(nanos: u64) -> Self {
        Self::from_nanos(nanos)
    }
}

impl Into<u64> for AnimationTime {
    fn into(self) -> u64 {
        self.0
    }
}

impl From<f64> for AnimationTime {
    fn from(seconds: f64) -> Self {
        Self::from_seconds(seconds.max(0.0)).unwrap_or(Self::zero())
    }
}

impl Into<f64> for AnimationTime {
    fn into(self) -> f64 {
        self.as_seconds()
    }
}

impl From<Duration> for AnimationTime {
    fn from(duration: Duration) -> Self {
        AnimationTime::from_nanos(duration.as_nanos() as u64)
    }
}

impl Into<Duration> for AnimationTime {
    fn into(self) -> Duration {
        Duration::from_nanos(self.0)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_time() {
        let time1 = AnimationTime::from_seconds(1.5).unwrap();
        let time2 = AnimationTime::from_seconds(2.0).unwrap();

        assert_eq!(time1.as_seconds(), 1.5);
        assert_eq!(time1.as_millis(), 1500.0);

        let sum = time1.add(time2);
        assert_eq!(sum.as_seconds(), 3.5);

        let diff = time2.duration_since(time1).unwrap();
        assert_eq!(diff.as_seconds(), 0.5);
    }

    #[test]
    fn test_invalid_time() {
        assert!(AnimationTime::from_seconds(-1.0).is_err());
        assert!(AnimationTime::from_seconds(f64::NAN).is_err());
        assert!(AnimationTime::from_seconds(f64::INFINITY).is_err());
    }

    #[test]
    fn test_time_range() {
        let start = AnimationTime::from_seconds(1.0).unwrap();
        let end = AnimationTime::from_seconds(3.0).unwrap();
        let range = TimeRange::new(start, end).unwrap();

        assert_eq!(range.duration().as_seconds(), 2.0);
        assert!(range.contains(AnimationTime::from_seconds(2.0).unwrap()));
        assert!(!range.contains(AnimationTime::from_seconds(4.0).unwrap()));

        assert_eq!(
            range.normalize_time(AnimationTime::from_seconds(2.0).unwrap()),
            0.5
        );
        assert_eq!(range.denormalize_time(0.5).as_seconds(), 2.0);
    }

    #[test]
    fn test_range_operations() {
        let range1 = TimeRange::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            AnimationTime::from_seconds(3.0).unwrap(),
        )
        .unwrap();

        let range2 = TimeRange::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            AnimationTime::from_seconds(4.0).unwrap(),
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
}
