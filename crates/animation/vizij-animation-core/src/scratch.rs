#![allow(dead_code)]
//! Scratch buffers and frame lifecycle.
//!
//! v1 skeleton keeps capacity hints and a begin_frame() no-op. Concrete
//! sampling/blending buffers will be added in later steps.

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
    pub fn begin_frame(&mut self) {
        // Later: clear transient vectors, reset cursors; currently a no-op.
    }
}
