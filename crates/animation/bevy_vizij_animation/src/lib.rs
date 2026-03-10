//! Bevy adapter for [`vizij_animation_core`] playback.
//!
//! This crate wires the shared animation engine into Bevy schedules and resources. The
//! public surface is intentionally small: a plugin, binding marker components, and a few
//! resources used to control fixed-step playback and inspect pending outputs.

use bevy::prelude::*;
use vizij_animation_core::{Config, Engine};

pub mod components;
pub mod resources;
pub mod systems;

pub use components::{VizijBindingHint, VizijTargetRoot};
pub use resources::{BindingIndex, FixedDt, PendingOutputs};

#[derive(Resource)]
pub struct VizijEngine(pub Engine);

/// Bevy plugin that inserts the animation engine, binding index, and fixed-step systems.
pub struct VizijAnimationPlugin;

impl Plugin for VizijAnimationPlugin {
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
