use crate::interpolation::types::InterpolationType;
use crate::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Cache key for interpolation results
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct InterpolationCacheKey {
    interpolation_type: InterpolationType,
    start_hash: u64,
    end_hash: u64,
    t_quantized: u32, // Quantized t value for cache efficiency
}

impl InterpolationCacheKey {
    #[inline]
    pub fn new(interpolation_type: InterpolationType, start: &Value, end: &Value, t: f64) -> Self {
        let mut hasher = DefaultHasher::new();
        start.hash(&mut hasher);
        let start_hash = hasher.finish();

        start.hash(&mut hasher); // Re-hash start to mix with end_hash
        end.hash(&mut hasher);
        let end_hash = hasher.finish();

        // Quantize t to reduce cache entries while maintaining precision
        let t_quantized = (t * 10000.0) as u32;

        Self {
            interpolation_type,
            start_hash,
            end_hash,
            t_quantized,
        }
    }
}
