use crate::{
    animation::{AnimationData, BakedAnimationData},
    ecs::{
        components::{
            AnimatedColor, AnimationBinding, AnimationInstance, AnimationPlayer, Intensity,
        },
        resources::{AnimationOutput, IdMapping},
        systems::*,
    },
    interpolation::InterpolationRegistry,
};
use bevy::prelude::*;

pub struct AnimationPlayerPlugin;

impl Plugin for AnimationPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnimationOutput>()
            .init_resource::<IdMapping>()
            .init_resource::<InterpolationRegistry>()
            // Register assets and their reflection data
            .register_asset_reflect::<AnimationData>()
            .register_asset_reflect::<BakedAnimationData>()
            // Register components for reflection
            .register_type::<AnimationPlayer>()
            .register_type::<AnimationInstance>()
            .register_type::<AnimationBinding>()
            .register_type::<AnimatedColor>()
            .register_type::<Intensity>()
            // Add systems in a defined order
            .add_systems(
                Update,
                (
                    bind_new_animation_instances_system,
                    update_animation_players_system,
                    accumulate_animation_values_system,
                    blend_and_apply_animation_values_system,
                    collect_animation_output_system,
                )
                    .chain(),
            );
    }
}
