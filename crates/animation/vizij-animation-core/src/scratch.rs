#![allow(dead_code)]
//! Scratch buffers and frame lifecycle.
//!
//! The v1 skeleton stores capacity hints and exposes a `begin_frame()` no-op so
//! future buffer reuse can be wired in without changing the public API.

use crate::config::Config;

/// Scratch buffer sizing hints used by the engine during sampling.
#[derive(Debug, Default)]
pub struct Scratch {
    /// Sample scratch capacity hint.
    pub cap_samples: usize,
    /// Scalar value scratch capacity hint.
    pub cap_values_scalar: usize,
    /// Vector/array value scratch capacity hint.
    pub cap_values_vec: usize,
    /// Quaternion value scratch capacity hint.
    pub cap_values_quat: usize,
}

impl Scratch {
    /// Create scratch buffers sized from the engine config.
    pub fn new(cfg: &Config) -> Self {
        Self {
            cap_samples: cfg.scratch_samples,
            cap_values_scalar: cfg.scratch_values_scalar,
            cap_values_vec: cfg.scratch_values_vec,
            cap_values_quat: cfg.scratch_values_quat,
        }
    }

    #[inline]
    /// Reset per-frame state (currently a no-op placeholder).
    ///
    /// This exists so call sites can centralize per-tick lifecycle even before
    /// scratch buffers are fully implemented.
    pub fn begin_frame(&mut self) {
        // Later: clear transient vectors, reset cursors; currently a no-op.
    }
}
