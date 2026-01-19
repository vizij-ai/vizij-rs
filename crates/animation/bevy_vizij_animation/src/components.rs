//! ECS marker components for the Vizij animation Bevy plugin.

use bevy::prelude::*;

/// Marker component designating the root of a subtree to bind animation targets under.
///
/// The binding system walks descendants of any entity with this marker.
#[derive(Component)]
pub struct VizijTargetRoot;

/// Optional per-entity hint for canonical path override.
///
/// When present, this path prefix replaces the entity `Name` when building
/// canonical handles (for example, `{path}/Transform.translation`).
/// Use a stable, slash-separated path without a trailing slash.
///
/// # Examples
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_vizij_animation::VizijBindingHint;
///
/// fn spawn_hint(mut commands: Commands) {
///     commands.spawn(VizijBindingHint {
///         path: "rig/hips".to_string(),
///     });
/// }
/// ```
#[derive(Component, Debug, Clone)]
pub struct VizijBindingHint {
    /// Canonical path prefix to use for this entity.
    pub path: String,
}
