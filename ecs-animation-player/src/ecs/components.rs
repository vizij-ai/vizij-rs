use super::path::BevyPath;
use crate::{value::Color, AnimationData, AnimationTime, PlaybackMode, TrackId};
use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy_reflect::ParsedPath;
use std::any::TypeId;
use std::collections::HashMap;

/// Represents an animation player, acting as a timeline and container for animation instances.
#[derive(Component, Reflect)]
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

#[derive(Clone)]
pub struct ResolvedBinding {
    pub entity: Entity,
    pub path: BevyPath,
    pub component_type_id: TypeId,
    pub property_path: Option<ParsedPath>,
}

/// Stores resolved bindings for both raw and baked animation tracks.
///
/// * `raw_track_bindings` map raw `TrackId` values to their target entity and
///   [`BevyPath`].
/// * `baked_track_bindings` map baked track target strings (e.g.
///   "Transform.translation") to the same tuple.  This allows the systems to
///   operate with baked animation metadata side-by-side with raw track data.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct AnimationBinding {
    /// Mapping from raw animation track ID to entity/property path.
    #[reflect(ignore)]
    pub raw_track_bindings: HashMap<TrackId, ResolvedBinding>,
    /// Mapping from baked track target strings to entity/property path.
    #[reflect(ignore)]
    pub baked_track_bindings: HashMap<String, ResolvedBinding>,
}

/// A custom component to hold an animatable `Color` value.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct AnimatedColor(pub Color);

/// A custom component to hold an animatable float value, for example, a light's intensity.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Intensity(pub f32);

impl Default for AnimationPlayer {
    fn default() -> Self {
        Self {
            name: String::new(),
            speed: 1.0, // default playback speed should advance time
            mode: PlaybackMode::Loop,
            current_time: AnimationTime::zero(),
            duration: AnimationTime::zero(),
            playback_state: crate::player::playback_state::PlaybackState::Stopped,
            target_root: None,
        }
    }
}

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
