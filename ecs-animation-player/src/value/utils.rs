use std::hash::{Hash, Hasher};

/// Helper function to hash f64 values safely (treats NaN as equal)
pub fn hash_f64(value: f64, state: &mut impl Hasher) {
    if value.is_nan() {
        // Hash all NaN values the same way
        0u64.hash(state);
    } else {
        value.to_bits().hash(state);
    }
}
