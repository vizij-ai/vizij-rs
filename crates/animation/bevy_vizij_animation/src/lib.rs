use bevy::prelude::*;
use vizij_animation_core::{AnimationConfig, AnimationCore, AnimationInputs};

#[derive(Resource)]
pub struct AnimationResource(pub AnimationCore);

pub struct VizijAnimationPlugin;

impl Plugin for VizijAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AnimationResource(AnimationCore::new(AnimationConfig {
            frequency_hz: 1.0,
            amplitude: 1.0,
        })))
        .add_systems(Update, tick_animation);
    }
}

fn tick_animation(mut anim: ResMut<AnimationResource>, time: Res<Time>) {
    let dt = time.delta_seconds();
    let _outputs = anim.0.update(dt, AnimationInputs::default());
    // TODO: write outputs into components/resources as needed
}
