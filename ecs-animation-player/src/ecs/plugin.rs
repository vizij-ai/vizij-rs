use crate::{
    animation::{AnimationData, BakedAnimationData},
    ecs::{
        components::{
            AnimatedColor, AnimationBinding, AnimationInstance, AnimationPlayer, Intensity,
        },
        resources::{AnimationOutput, IdMapping},
        systems::*,
    },
    event::AnimationEvent,
    interpolation::InterpolationRegistry,
};
use bevy::{ecs::schedule::IntoScheduleConfigs, prelude::*};

#[derive(SystemSet, Debug, Clone, Eq, PartialEq, Hash)]
pub enum AnimationSystemSet {
    BindInstances,
    UpdatePlayers,
    Accumulate,
    BlendApply,
    Output,
}

pub struct AnimationPlayerPlugin;

impl Plugin for AnimationPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnimationOutput>()
            .init_resource::<IdMapping>()
            .init_resource::<InterpolationRegistry>()
            .add_event::<AnimationEvent>()
            // Register assets and their reflection data
            .register_asset_reflect::<AnimationData>()
            .register_asset_reflect::<BakedAnimationData>()
            // Register components for reflection
            .register_type::<AnimationPlayer>()
            .register_type::<AnimationInstance>()
            .register_type::<AnimationBinding>()
            .register_type::<AnimatedColor>()
            .register_type::<Intensity>()
            // Configure system sets to run in order
            .configure_sets(
                Update,
                (
                    AnimationSystemSet::BindInstances,
                    AnimationSystemSet::UpdatePlayers,
                    AnimationSystemSet::Accumulate,
                    AnimationSystemSet::BlendApply,
                    AnimationSystemSet::Output,
                )
                    .chain(),
            )
            // Add systems to their respective sets
            .add_systems(
                Update,
                bind_new_animation_instances_system.in_set(AnimationSystemSet::BindInstances),
            )
            .add_systems(
                Update,
                update_player_durations_system
                    .in_set(AnimationSystemSet::UpdatePlayers)
                    .before(update_animation_players_system),
            )
            .add_systems(
                Update,
                update_animation_players_system.in_set(AnimationSystemSet::UpdatePlayers),
            )
            .add_systems(
                Update,
                accumulate_animation_values_system.in_set(AnimationSystemSet::Accumulate),
            )
            .add_systems(
                Update,
                blend_and_apply_animation_values_system.in_set(AnimationSystemSet::BlendApply),
            )
            .add_systems(
                Update,
                collect_animation_output_system.in_set(AnimationSystemSet::Output),
            )
            .add_systems(Update, cleanup_id_mapping_on_despawned_system);
    }
}
