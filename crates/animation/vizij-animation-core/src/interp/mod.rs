#![allow(dead_code)]
//! Interpolation registry and helpers.
//!
//! v1 registers linear/cubic/step interpolators and a quaternion NLERP
//! with shortest-arc sign correction.

/// Standalone interpolation helpers used by the registry.
pub mod functions;

/// Registry marker for interpolation functions.
///
/// This currently has no state, but provides a stable type for APIs that will
/// eventually accept configurable interpolation registries.
#[derive(Debug, Default)]
pub struct InterpRegistry;

impl InterpRegistry {
    /// Construct a new interpolation registry.
    pub fn new() -> Self {
        Self
    }
}
