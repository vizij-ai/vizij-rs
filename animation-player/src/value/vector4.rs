use crate::value::utils::hash_f64;
use nalgebra::Vector4 as NVector4;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// 4D vector type
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vector4 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Hash for Vector4 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_f64(self.x, state);
        hash_f64(self.y, state);
        hash_f64(self.z, state);
        hash_f64(self.w, state);
    }
}

impl Vector4 {
    pub fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
        Self { x, y, z, w }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }

    pub fn one() -> Self {
        Self::new(1.0, 1.0, 1.0, 1.0)
    }

    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt()
    }

    pub fn normalize(&self) -> Self {
        let len = self.length();
        if len > 0.0 {
            Self::new(self.x / len, self.y / len, self.z / len, self.w / len)
        } else {
            Self::zero()
        }
    }
}

impl From<NVector4<f64>> for Vector4 {
    fn from(v: NVector4<f64>) -> Self {
        Self::new(v.x, v.y, v.z, v.w)
    }
}

impl From<Vector4> for NVector4<f64> {
    fn from(v: Vector4) -> Self {
        NVector4::new(v.x, v.y, v.z, v.w)
    }
}
