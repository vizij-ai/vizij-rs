#![allow(dead_code)]
//! Scratch buffers and frame lifecycle.
//!
//! v1 skeleton keeps capacity hints and a begin_frame() no-op. Concrete
//! sampling/blending buffers will be added in later steps.

use crate::config::Config;

#[derive(Debug, Default)]
pub struct Scratch {
    pub cap_samples: usize,
    pub cap_values_scalar: usize,
    pub cap_values_vec: usize,
    pub cap_values_quat: usize,
}

impl Scratch {
    pub fn new(cfg: &Config) -> Self {
        Self {
            cap_samples: cfg.scratch_samples,
            cap_values_scalar: cfg.scratch_values_scalar,
            cap_values_vec: cfg.scratch_values_vec,
            cap_values_quat: cfg.scratch_values_quat,
        }
    }

    #[inline]
    pub fn begin_frame(&mut self) {
        // Later: clear transient vectors, reset cursors; currently a no-op.
    }
}
