use bevy::prelude::*;
use ecs_animation_player::{
    animation::{AnimationData, AnimationKeypoint, AnimationTrack},
    ecs::{components::*, plugin::AnimationPlayerPlugin, resources::*},
    value::{Value, Vector3},
    AnimationTime,
};

fn setup_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        AnimationPlayerPlugin,
    ));
    app
}

#[test]
fn test_animation_player_integration() {
    let mut app = setup_test_app();

    // 1. Setup Scene
    let target_entity = app
        .world
        .spawn((Name::new("Cube"), Transform::default()))
        .id();

    // 2. Load Animation Data
    let mut animation = AnimationData::new("test_anim", "Test Animation");
    let mut track = AnimationTrack::new("position", "Transform.translation");
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(0.0).unwrap(),
            Value::Vector3(Vector3::new(0.0, 0.0, 0.0)),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Vector3(Vector3::new(10.0, 0.0, 0.0)),
        ))
        .unwrap();
    animation.add_track(track);

    let mut assets = app.world.resource_mut::<Assets<AnimationData>>();
    let animation_handle = assets.add(animation);

    // 3. Create Player and Instance
    let player_entity = app
        .world
        .spawn(AnimationPlayer {
            target_root: Some(target_entity),
            playback_state: ecs_animation_player::PlaybackState::Playing,
            ..default()
        })
        .id();

    let instance_component = AnimationInstance {
        animation: animation_handle,
        ..default()
    };
    let instance_entity = app.world.spawn(instance_component).id();
    app.world
        .get_mut::<Children>(player_entity)
        .unwrap()
        .add(instance_entity);

    // 4. Run App Update
    // First update for binding
    app.update();

    // Advance time to halfway through the animation
    let mut time = app.world.resource_mut::<Time>();
    time.advance_by(std::time::Duration::from_secs_f64(0.5));
    app.update();

    // 5. Assertions
    let transform = app.world.get::<Transform>(target_entity).unwrap();
    assert!((transform.translation.x - 5.0).abs() < 1e-6);
}
