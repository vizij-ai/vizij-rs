use crate::value::vector3::Vector3;
use crate::value::vector4::Vector4;
use nalgebra::UnitQuaternion;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// Performs spherical linear interpolation (SLERP) between two quaternions.
/// Quaternions are expected as [x, y, z, w] components.
/// The result is also [x, y, z, w].
pub fn slerp_quaternion(q1: &[f64; 4], q2: &[f64; 4], t: f64) -> [f64; 4] {
    let q1_nal =
        UnitQuaternion::new_normalize(nalgebra::Quaternion::new(q1[3], q1[0], q1[1], q1[2]));
    let q2_nal =
        UnitQuaternion::new_normalize(nalgebra::Quaternion::new(q2[3], q2[0], q2[1], q2[2]));

    let slerped = q1_nal.slerp(&q2_nal, t);
    [slerped.i, slerped.j, slerped.k, slerped.w]
}

/// Transform representation for position, rotation, and scale
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub position: Vector3,
    pub rotation: Vector4, // Quaternion (x, y, z, w)
    pub scale: Vector3,
}

impl Hash for Transform {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.position.hash(state);
        self.rotation.hash(state);
        self.scale.hash(state);
    }
}

impl Transform {
    pub fn new(position: Vector3, rotation: Vector4, scale: Vector3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }

    pub fn identity() -> Self {
        Self {
            position: Vector3::zero(),
            rotation: Vector4::new(0.0, 0.0, 0.0, 1.0), // Identity quaternion
            scale: Vector3::one(),
        }
    }

    pub fn from_position(position: Vector3) -> Self {
        Self {
            position,
            rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
            scale: Vector3::one(),
        }
    }

    pub fn from_rotation(rotation: Vector4) -> Self {
        Self {
            position: Vector3::zero(),
            rotation,
            scale: Vector3::one(),
        }
    }

    pub fn from_scale(scale: Vector3) -> Self {
        Self {
            position: Vector3::zero(),
            rotation: Vector4::new(0.0, 0.0, 0.0, 1.0),
            scale,
        }
    }

    /// Get rotation as nalgebra UnitQuaternion
    pub fn rotation_quaternion(&self) -> UnitQuaternion<f64> {
        UnitQuaternion::from_quaternion(nalgebra::Quaternion::new(
            self.rotation.w,
            self.rotation.x,
            self.rotation.y,
            self.rotation.z,
        ))
    }
}
