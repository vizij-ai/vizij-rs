#![allow(dead_code)]
//! Interpolation registry and helpers.
//!
//! v1 registers linear/cubic/step interpolators and a quaternion NLERP
//! with shortest-arc sign correction.

pub mod functions;

#[derive(Debug, Default)]
pub struct InterpRegistry;

impl InterpRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}
