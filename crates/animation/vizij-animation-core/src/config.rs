#![allow(dead_code)]
//! Core configuration for `vizij-animation-core`.

use serde::{Deserialize, Serialize};

/// Configuration for engine sizing and feature flags.
/// Keep this minimal; extend compatibly when new tuning knobs land.
///
/// # Examples
/// ```rust
/// use vizij_animation_core::Config;
///
/// let mut cfg = Config::default();
/// cfg.max_events_per_tick = 256;
/// assert_eq!(cfg.max_events_per_tick, 256);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// Initial capacity hints for scratch/sample buffers.
    pub scratch_samples: usize,
    /// Initial capacity hint for scalar-valued scratch buffers.
    pub scratch_values_scalar: usize,
    /// Initial capacity hint for vector/array-valued scratch buffers.
    pub scratch_values_vec: usize,
    /// Initial capacity hint for quaternion-valued scratch buffers.
    pub scratch_values_quat: usize,

    /// Maximum events to retain per tick before backpressure policy applies.
    pub max_events_per_tick: usize,

    /// Feature flags (placeholder; future: simd, parallel).
    pub features: Features,
}

/// Feature toggles for experimental runtime behavior.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Features {
    /// Reserved for future toggles (SIMD, parallel passes, etc.).
    pub reserved0: bool,
}

impl Default for Config {
    /// Default configuration tuned for typical real-time playback.
    fn default() -> Self {
        Self {
            scratch_samples: 1024,
            scratch_values_scalar: 512,
            scratch_values_vec: 512,
            scratch_values_quat: 256,
            max_events_per_tick: 1024,
            features: Features::default(),
        }
    }
}
