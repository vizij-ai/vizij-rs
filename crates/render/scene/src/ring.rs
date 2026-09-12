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

/// Normalises a joint's limits the way Studio's `getVisualLimits` does: into
/// `[0, 2pi)`, where a range may wrap through zero, and where anything wider
/// than a full turn is just a full turn.
pub fn visual_limits(min: f32, max: f32) -> (f32, f32) {
    if !min.is_finite() || !max.is_finite() || max - min >= std::f32::consts::TAU {
        return (0.0, std::f32::consts::TAU);
    }
    (wrap(min), wrap(max))
}

/// An angle in `[0, 2pi)`.
pub fn wrap(angle: f32) -> f32 {
    angle.rem_euclid(std::f32::consts::TAU)
}
