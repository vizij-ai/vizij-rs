use crate::interpolation::types::InterpolationType;
use crate::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Cache key for interpolation results.
///
/// This struct is used to uniquely identify cached interpolation results
/// based on the interpolation type, the start and end values, and the
/// quantized interpolation parameter `t`.
///
/// # Fields
/// - `interpolation_type`: Specifies the type of interpolation being performed
///   (e.g., linear, cubic, etc.).
/// - `start_hash`: A hash value derived from the `start` value. This ensures
///   that the cache key is tied to the specific starting value of the interpolation.
/// - `end_hash`: A hash value derived from the `end` value. This ensures
///   that the cache key is tied to the specific ending value of the interpolation.
///   The `start` value is re-hashed before combining with the `end` value to
///   ensure that the hash values are mixed properly and avoid collisions.
/// - `t_quantized`: A quantized version of the interpolation parameter `t`.
///   The `t` parameter represents the position along the interpolation curve,
///   typically ranging from 0.0 (start) to 1.0 (end). Quantizing `t` involves
///   scaling it by a factor (in this case, 10,000) and converting it to an
///   integer. This reduces the number of distinct cache entries while maintaining
///   sufficient precision for most interpolation use cases. Quantization is safe
///   because small variations in `t` that fall within the same quantized bucket
///   are unlikely to produce perceptible differences in the interpolation result,
///   making it both efficient and effective for caching.#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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
