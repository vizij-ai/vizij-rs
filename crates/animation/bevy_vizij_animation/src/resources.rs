use bevy::prelude::*;
use std::collections::HashMap;
use vizij_animation_core::outputs::Change;

/// What property on a Bevy component a handle maps to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TargetProp {
    Translation,
    Rotation,
    Scale,
}

/// Index from canonical string handle (e.g., "Head/Transform.translation")
/// to (Entity, TargetProp). Populated by the binding system by walking
/// under VizijTargetRoot.
#[derive(Resource, Default)]
pub struct BindingIndex {
    pub map: HashMap<String, (Entity, TargetProp)>,
}

/// Outputs staged from Engine::update to be applied in a separate system
/// (keeps ordering explicit: Compute -> Apply).
#[derive(Resource, Default)]
pub struct PendingOutputs {
    pub changes: Vec<Change>,
}

/// Fixed timestep configuration (seconds per tick).
#[derive(Resource)]
pub struct FixedDt(pub f32);

impl Default for FixedDt {
    fn default() -> Self {
        Self(1.0 / 60.0)
    }
}
