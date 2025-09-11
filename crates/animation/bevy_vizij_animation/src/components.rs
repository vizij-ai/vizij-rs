use bevy::prelude::*;

/// Marker component designating the root of a subtree to bind animation targets under.
/// The binding system will walk descendants of any entity with this marker.
#[derive(Component)]
pub struct VizijTargetRoot;

/// Optional per-entity hint for canonical path override.
/// When present, this path prefix will be used instead of the entity's Name.
#[derive(Component, Debug, Clone)]
pub struct VizijBindingHint {
    pub path: String,
}
