/// Performance metrics for interpolation operations
#[derive(Debug, Clone)]
pub struct InterpolationMetrics {
    /// Total number of interpolations performed
    pub total_interpolations: u64,
    /// Number of cache hits
    pub cache_hits: u64,
    /// Number of cache misses
    pub cache_misses: u64,
    /// Total time spent interpolating (in microseconds)
    pub total_time_micros: u64,
    /// Average interpolation time (in microseconds)
    pub average_time_micros: f64,
}

impl InterpolationMetrics {
    /// Create new metrics
    #[inline]
    pub fn new() -> Self {
        Self {
            total_interpolations: 0,
            cache_hits: 0,
            cache_misses: 0,
            total_time_micros: 0,
            average_time_micros: 0.0,
        }
    }

    /// Record an interpolation operation
    #[inline]
    pub fn record_interpolation(&mut self, time_micros: u64, cache_hit: bool) {
        self.total_interpolations += 1;
        self.total_time_micros += time_micros;

        if cache_hit {
            self.cache_hits += 1;
        } else {
            self.cache_misses += 1;
        }

        self.average_time_micros = self.total_time_micros as f64 / self.total_interpolations as f64;
    }

    /// Get cache hit rate
    #[inline]
    pub fn cache_hit_rate(&self) -> f64 {
        if self.total_interpolations == 0 {
            0.0
        } else {
            self.cache_hits as f64 / self.total_interpolations as f64
        }
    }

    /// Reset metrics
    #[inline]
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for InterpolationMetrics {
    fn default() -> Self {
        Self::new()
    }
}
