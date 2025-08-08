use super::path::BevyPath;
use crate::{value::Color, AnimationData, AnimationTime, PlaybackMode, TrackId};
use bevy::prelude::*;
use bevy::reflect::Reflect;
use std::collections::HashMap;

/// Represents an animation player, acting as a timeline and container for animation instances.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct AnimationPlayer {
    pub name: String,
    pub speed: f64,
    pub mode: PlaybackMode,
    pub current_time: AnimationTime,
    pub duration: AnimationTime,
    pub playback_state: crate::player::playback_state::PlaybackState,
    pub target_root: Option<Entity>,
}

/// Represents a single, active animation being played.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct AnimationInstance {
    pub animation: Handle<AnimationData>,
    pub weight: f32,
    pub time_scale: f32,
    pub start_time: AnimationTime,
}

/// Stores the resolved mapping from an animation track to a target entity and component property.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct AnimationBinding {
    #[reflect(ignore)]
    pub bindings: HashMap<TrackId, (Entity, BevyPath)>,
}

/// A custom component to hold an animatable `Color` value.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct AnimatedColor(pub Color);

/// A custom component to hold an animatable float value, for example, a light's intensity.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Intensity(pub f32);

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        prelude::{App, AppTypeRegistry},
        reflect::TypePath,
    };

    #[test]
    fn animation_binding_reflection_path_registered() {
        let mut app = App::new();
        app.register_type::<AnimationBinding>();

        let registry = app.world().resource::<AppTypeRegistry>().clone();
        let registry = registry.read();

        assert!(registry
            .get_with_type_path(AnimationBinding::type_path())
            .is_some());
    }
}
