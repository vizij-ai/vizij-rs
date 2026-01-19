//! ECS resources used by the Vizij animation Bevy plugin.

use bevy::prelude::*;
use std::collections::HashMap;
use vizij_animation_core::outputs::Change;

/// Which `Transform` property a handle maps to for a canonical binding.
///
/// Used by output-application systems to route `Value` payloads into the correct
/// `Transform` field (translation/rotation/scale).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TargetProp {
    /// Apply to `Transform::translation`.
    Translation,
    /// Apply to `Transform::rotation`.
    Rotation,
    /// Apply to `Transform::scale`.
    Scale,
}

/// Index from canonical string handle (e.g., "Head/Transform.translation")
/// to `(Entity, TargetProp)`.
///
/// This map is rebuilt each frame by `build_binding_index_system`; store handles,
/// not indices, if you need stable references across rebuilds.
///
/// Populated by the binding system by walking under `VizijTargetRoot`.
#[derive(Resource, Default)]
pub struct BindingIndex {
    /// Map of canonical handle -> (entity, target property).
    pub map: HashMap<String, (Entity, TargetProp)>,
}

/// Outputs staged from `Engine::update_values` to be applied in a separate system.
///
/// Keeping this resource separate makes the compute/apply ordering explicit.
/// The `apply_outputs_system` consumes and clears the stored changes each tick.
#[derive(Resource, Default)]
pub struct PendingOutputs {
    /// Changes captured in the last fixed update tick.
    pub changes: Vec<Change>,
}

/// Fixed timestep configuration (seconds per tick).
///
/// The plugin uses this in `FixedUpdate` to advance the core engine.
#[derive(Resource)]
pub struct FixedDt(pub f32);

impl Default for FixedDt {
    fn default() -> Self {
        Self(1.0 / 60.0)
    }
}
