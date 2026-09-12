//! The joint ring's material, ported from Studio's scene library.
//!
//! Studio draws a joint's control as three concentric tori sharing one basis:
//! a fat invisible one that catches the pointer, a thin translucent one at
//! rest, and a thick shaded one carrying `materials/torus-material.tsx` while
//! hovered or dragged. The hover affordance is that swap — thin to thick —
//! rather than a colour change.
//!
//! The shader itself ported across unchanged in substance; see
//! `joint_ring.wgsl` for the one place the two engines differ.

use bevy::asset::embedded_asset;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

/// What the ring needs to know to shade itself.
#[derive(ShaderType, Debug, Clone, Default)]
pub struct JointRingUniform {
    /// The reachable range and where the joint stands in it, all normalised to
    /// `[0, 2pi)` because the shader compares them against an angle read off
    /// the mesh's own winding.
    pub min: f32,
    pub max: f32,
    pub current: f32,
    pub _padding: f32,
    pub color: Vec4,
    pub background: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct JointRingMaterial {
    #[uniform(0)]
    pub ring: JointRingUniform,
}

impl Material for JointRingMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://scene/joint_ring.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    /// Drawn in front of the robot. A control buried inside the geometry it
    /// controls cannot be grabbed, and a joint ring sits inside its own link
    /// by construction. Studio's material disables depth testing outright; a
    /// bias is the same intent and keeps the ring sorting sanely against
    /// itself.
    fn depth_bias(&self) -> f32 {
        f32::MAX
    }
}

pub struct JointRingPlugin;

impl Plugin for JointRingPlugin {
    fn build(&self, app: &mut App) {
        // Carried in the binary rather than loaded from the asset root, which
        // here is wherever the robot happens to live.
        embedded_asset!(app, "joint_ring.wgsl");
        app.add_plugins(MaterialPlugin::<JointRingMaterial>::default());
    }
}

/// A joint's range as the ring draws it: measured from one end rather than
/// from the joint's own zero.
///
/// The shader fills from `min` to the current value, so the range has to be
/// **shifted** to start at zero, not wrapped into `[0, 2pi)`. Wrapping a range
/// like `[-pi/2, pi/2]` puts its start at 4.71 and its end at 1.57, which the
/// shader then reads as a range crossing zero — and fills the whole half the
/// joint has already passed. Studio shifts for the same reason, in
/// `getVisualLimits` and `getVisualValueNormalizer`.
///
/// `origin` is where the ring's own zero has to point for the drawing to line
/// up with the directions the joint can actually reach.
#[derive(Debug, Clone, Copy)]
pub struct VisualRange {
    pub origin: f32,
    pub min: f32,
    pub max: f32,
}

impl VisualRange {
    pub fn of(min: f32, max: f32) -> Self {
        // An unbounded joint turns forever, so its ring is a whole turn read
        // from the joint's own zero.
        if !min.is_finite() || !max.is_finite() || max - min >= std::f32::consts::TAU {
            return Self {
                origin: 0.0,
                min: 0.0,
                max: std::f32::consts::TAU,
            };
        }
        Self {
            origin: min,
            min: 0.0,
            max: max - min,
        }
    }

    /// Where a joint value sits in this range.
    pub fn value(&self, value: f32) -> f32 {
        (value - self.origin).rem_euclid(std::f32::consts::TAU)
    }
}
