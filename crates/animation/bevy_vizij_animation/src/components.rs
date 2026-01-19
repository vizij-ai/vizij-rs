//! ECS marker components for the Vizij animation Bevy plugin.

use bevy::prelude::*;

/// Marker component designating the root of a subtree to bind animation targets under.
/// The binding system will walk descendants of any entity with this marker.
#[derive(Component)]
pub struct VizijTargetRoot;

/// Optional per-entity hint for canonical path override.
///
/// When present, this path prefix is used instead of the entity `Name` when
/// building the `BindingIndex`.
#[derive(Component, Debug, Clone)]
pub struct VizijBindingHint {
    /// Canonical path prefix to use for this entity.
    pub path: String,
}
