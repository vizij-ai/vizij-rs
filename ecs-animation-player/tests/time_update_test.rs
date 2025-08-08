use bevy::prelude::*;
use ecs_animation_player::animation::transition::{AnimationTransition, TransitionVariant};
use ecs_animation_player::{
    animation::{AnimationData, AnimationKeypoint, AnimationTrack},
    ecs::{
        components::{AnimationInstance, AnimationPlayer},
        plugin::AnimationPlayerPlugin,
        resources::{EngineTime, IdMapping},
    },
    value::{Value, Vector3},
    AnimationTime, BakedAnimationData,
};

fn init_tracing() {
    let default_filter = "ecs_animation_player=debug,ecs_animation_player::ecs=debug";
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_test_writer()
        .try_init();
}

#[test]
fn test_animation_progresses_with_time_updates() {
    init_tracing();
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        AnimationPlayerPlugin,
    ));
    app.init_resource::<Assets<AnimationData>>();
    app.init_resource::<Assets<BakedAnimationData>>();
    // Use EngineTime to drive updates explicitly in tests

    // 1. Setup Scene
    let target_entity = app
        .world_mut()
        .spawn((
            Name::new("TestEntity"),
            Transform::from_translation(Vec3::ZERO),
        ))
        .id();

    // 2. Load Animation Data
    let mut animation = AnimationData::new("test_anim", "TimeUpdateTest");
    let mut track = AnimationTrack::new("translation", "Transform.translation");
    let kp0 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(0.0).unwrap(),
            Value::Vector3(Vector3::new(0.0, 0.0, 0.0)),
        ))
        .unwrap();
    let kp1 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Vector3(Vector3::new(20.0, 0.0, 0.0)),
        ))
        .unwrap();
    animation.add_track(track);

    // Ensure deterministic linear interpolation between the two keypoints for this test
    animation.add_transition(AnimationTransition::new(
        kp0.id,
        kp1.id,
        TransitionVariant::Linear,
    ));

    let mut assets = app.world_mut().resource_mut::<Assets<AnimationData>>();
    let animation_handle = assets.add(animation);

    // 3. Create Player and Instance
    let player_entity = app
        .world_mut()
        .spawn(AnimationPlayer {
            target_root: Some(target_entity),
            playback_state: ecs_animation_player::PlaybackState::Playing,
            mode: ecs_animation_player::PlaybackMode::Once,
            name: "TimeUpdateTestPlayer".into(),
            ..default()
        })
        .id();

    let instance_component = AnimationInstance {
        animation: animation_handle,
        weight: 1.0,
        time_scale: 1.0,
        ..default()
    };
    let instance_entity = app.world_mut().spawn(instance_component).id();
    app.world_mut()
        .entity_mut(player_entity)
        .add_child(instance_entity);

    let player_id = "test_player".to_string();
    app.world_mut()
        .resource_mut::<IdMapping>()
        .players
        .insert(player_id, player_entity);

    // 4. Run App Updates and Assertions
    // Initial state
    app.update();
    let transform = app.world().get::<Transform>(target_entity).unwrap();
    assert_eq!(transform.translation, Vec3::ZERO);

    // Advance time to 0.5s using EngineTime
    {
        let mut et = app.world_mut().resource_mut::<EngineTime>();
        et.delta_seconds = 0.5;
        et.elapsed_seconds += 0.5;
    }
    app.update();
    let transform = app.world().get::<Transform>(target_entity).unwrap();
    assert!(
        (transform.translation.x - 5.0).abs() < 1e-4,
        "Value at 0.5s is {}",
        transform.translation.x
    );

    // Advance time to 1.0s using EngineTime
    {
        let mut et = app.world_mut().resource_mut::<EngineTime>();
        et.delta_seconds = 0.5;
        et.elapsed_seconds += 0.5;
    }
    app.update();
    let transform = app.world().get::<Transform>(target_entity).unwrap();
    assert!(
        (transform.translation.x - 10.0).abs() < 1e-4,
        "Value at 1.0s is {}",
        transform.translation.x
    );

    // Advance time to 2.0s (end of animation) using EngineTime
    {
        let mut et = app.world_mut().resource_mut::<EngineTime>();
        et.delta_seconds = 1.0;
        et.elapsed_seconds += 1.0;
    }

    // Inspect engine time before update (2.0s)
    {
        let et = app.world().resource::<EngineTime>();
        eprintln!(
            "TEST: pre-update engine time (2.0) delta={:.6} elapsed={:.6}",
            et.delta_seconds, et.elapsed_seconds
        );
    }

    app.update();
    let transform = app.world().get::<Transform>(target_entity).unwrap();
    assert!(
        (transform.translation.x - 20.0).abs() < 1e-4,
        "Value at 2.0s is {}",
        transform.translation.x
    );
}
