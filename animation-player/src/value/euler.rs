//! Defines the Euler angle structure.

use crate::value::utils::hash_f64;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// Represents Euler angles for rotation.
/// In robotics, rpy equates to xyz. In Three.js, this will need to be remapped to zyx.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Euler {
    /// Roll (rotation around the x-axis)
    pub r: f64,
    /// Pitch (rotation around the y-axis)
    pub p: f64,
    /// Yaw (rotation around the z-axis)
    pub y: f64,
}

impl Euler {
    pub fn new(r: f64, p: f64, y: f64) -> Self {
        Self { r, p, y }
    }
}

impl Hash for Euler {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_f64(self.r, state);
        hash_f64(self.p, state);
        hash_f64(self.y, state);
    }
}
