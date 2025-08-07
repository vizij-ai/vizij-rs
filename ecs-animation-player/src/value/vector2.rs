use crate::value::utils::hash_f64;
use bevy::prelude::Reflect;
use bevy::prelude::ReflectDefault;
use nalgebra::Vector2 as NVector2;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
/// 2D vector type
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Reflect, Default)]
#[reflect(Default, PartialEq)]
pub struct Vector2 {
    pub x: f64,
    pub y: f64,
}

impl Hash for Vector2 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_f64(self.x, state);
        hash_f64(self.y, state);
    }
}

impl Vector2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0)
    }

    pub fn one() -> Self {
        Self::new(1.0, 1.0)
    }

    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalize(&self) -> Self {
        let len = self.length();
        if len > 0.0 {
            Self::new(self.x / len, self.y / len)
        } else {
            Self::zero()
        }
    }
}

impl From<NVector2<f64>> for Vector2 {
    fn from(v: NVector2<f64>) -> Self {
        Self::new(v.x, v.y)
    }
}

impl From<Vector2> for NVector2<f64> {
    fn from(v: Vector2) -> Self {
        NVector2::new(v.x, v.y)
    }
}
