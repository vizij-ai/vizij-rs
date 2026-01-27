//! Bevy plugin for the Vizij animation runtime.
//!
//! Adds the core engine as a resource, builds canonical bindings from the scene
//! hierarchy, advances the engine on a fixed timestep, and applies sampled
//! outputs to Bevy `Transform` components.

use bevy::prelude::*;
use vizij_animation_core::{Config, Engine};

/// ECS components for driving animation playback.
pub mod components;
/// Resources storing shared animation state.
pub mod resources;
/// Bevy systems that advance animation playback.
pub mod systems;

pub use components::{VizijBindingHint, VizijTargetRoot};
pub use resources::{BindingIndex, FixedDt, PendingOutputs};

/// Bevy resource wrapper around the core animation engine.
///
/// Access this resource to load animations, create players, and drive updates
/// from custom systems.
#[derive(Resource)]
pub struct VizijEngine(pub Engine);

/// Plugin wiring Vizij animation systems into Bevy schedules.
///
/// # Examples
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_vizij_animation::VizijAnimationPlugin;
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(VizijAnimationPlugin)
///     .run();
/// ```
pub struct VizijAnimationPlugin;

impl Plugin for VizijAnimationPlugin {
    /// Builds internal state.
    fn build(&self, app: &mut App) {
        app
            // Core engine resource
            .insert_resource(VizijEngine(Engine::new(Config::default())))
            // Binding/outputs resources
            .init_resource::<BindingIndex>()
            .insert_resource(PendingOutputs::default())
            .insert_resource(FixedDt::default())
            // Writer registry for bevy setters (used by apply_write_batch)
            .insert_resource(bevy_vizij_api::WriterRegistry::new())
            // Build binding index when roots/entities change (for simplicity run in Update every frame; can add change detection)
            .add_systems(Update, systems::build_binding_index_system)
            // Prebind core after binding index is available (order after build_binding_index_system)
            .add_systems(
                Update,
                systems::prebind_core_system.after(systems::build_binding_index_system),
            )
            // Fixed compute and apply stages
            .add_systems(FixedUpdate, systems::fixed_update_core_system)
            .add_systems(
                FixedUpdate,
                systems::apply_outputs_system.after(systems::fixed_update_core_system),
            );
    }
}
