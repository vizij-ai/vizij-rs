#![allow(dead_code)]
//! Interpolation registry and helpers.
//!
//! v1 registers linear/cubic/step interpolators and a quaternion NLERP
//! with shortest-arc sign correction.

/// Standalone interpolation helpers used by the registry.
pub mod functions;

/// Registry marker for interpolation functions.
///
/// v1 exposes a placeholder registry type that can grow into a real lookup
/// table when multiple interpolation families are configurable.
#[derive(Debug, Default)]
pub struct InterpRegistry;

impl InterpRegistry {
    /// Construct a new interpolation registry.
    pub fn new() -> Self {
        Self
    }
}
