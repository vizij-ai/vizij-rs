#![allow(dead_code)]
//! Core configuration for vizij-animation-core.

use serde::{Deserialize, Serialize};

/// Configuration for engine sizing and feature flags.
/// Keep this minimal in v1; expand as needed without breaking API.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// Initial capacity hints for scratch/sample buffers.
    pub scratch_samples: usize,
    pub scratch_values_scalar: usize,
    pub scratch_values_vec: usize,
    pub scratch_values_quat: usize,

    /// Maximum events to retain per tick before backpressure policy applies.
    pub max_events_per_tick: usize,

    /// Feature flags (placeholder; future: simd, parallel).
    pub features: Features,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Features {
    /// Reserved for future toggles (SIMD, parallel passes, etc.).
    pub reserved0: bool,
}

impl Default for Config {
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
