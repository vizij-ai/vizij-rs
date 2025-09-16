use bevy::prelude::*;
use vizij_animation_core::{Config, Engine};

pub mod components;
pub mod resources;
pub mod systems;

pub use components::{VizijBindingHint, VizijTargetRoot};
pub use resources::{BindingIndex, FixedDt, PendingOutputs};

#[derive(Resource)]
pub struct VizijEngine(pub Engine);

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
