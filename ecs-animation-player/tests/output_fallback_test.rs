use bevy::prelude::*;
use ecs_animation_player::{
    animation::{AnimationData, AnimationKeypoint, AnimationTrack, BakedAnimationData},
    ecs::{
        components::{AnimationInstance, AnimationPlayer},
        plugin::AnimationPlayerPlugin,
        resources::{AnimationOutput, EngineTime, IdMapping},
    },
    value::{Value, Vector3},
    AnimationTime,
};

fn setup_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), AnimationPlayerPlugin));
    app.init_resource::<Assets<AnimationData>>();
    app.init_resource::<Assets<BakedAnimationData>>();
    app.init_resource::<IdMapping>();
    app
}

fn make_simple_position_anim(id: &str, name: &str) -> AnimationData {
    let mut animation = AnimationData::new(id, name);
    let mut track = AnimationTrack::new("translation", "Transform.translation");
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
    animation
}

#[test]
fn fallback_sampling_produces_output_without_bindings() {
    let mut app = setup_app();

    // Load animation asset
    let anim_handle = {
        let mut assets = app.world_mut().resource_mut::<Assets<AnimationData>>();
        assets.add(make_simple_position_anim("anim_fallback", "Fallback Test"))
    };

    // Create player WITHOUT target_root (no setPlayerRoot/bindings)
    let player_entity = app
        .world_mut()
        .spawn(AnimationPlayer {
            name: "FallbackPlayer".into(),
            playback_state: ecs_animation_player::PlaybackState::Playing,
            mode: ecs_animation_player::PlaybackMode::Once,
            speed: 1.0,
            target_root: None,
            ..default()
        })
        .id();

    // Create instance as child of player
    let instance_entity = app
        .world_mut()
        .spawn(AnimationInstance {
            animation: anim_handle.clone(),
            weight: 1.0,
            time_scale: 1.0,
            ..default()
        })
        .id();
    app.world_mut()
        .entity_mut(player_entity)
        .add_child(instance_entity);

    // Map player id for AnimationOutput
    let player_id = "player_fallback".to_string();
    app.world_mut()
        .resource_mut::<IdMapping>()
        .players
        .insert(player_id.clone(), player_entity);

    // Initial update and advance time by 0.5s
    app.update();
    {
        let mut et = app.world_mut().resource_mut::<EngineTime>();
        et.delta_seconds = 0.5;
        et.elapsed_seconds += 0.5;
    }
    app.update();

    // Verify AnimationOutput contains target path values via fallback (no bindings)
    let out = app.world().resource::<AnimationOutput>();
    let player_map = out
        .values
        .get(&player_id)
        .expect("AnimationOutput must contain entry for player");
    assert!(
        player_map.contains_key("Transform.translation"),
        "Fallback should produce target path output without bindings"
    );
    let v = player_map.get("Transform.translation").unwrap();
    match v {
        Value::Vector3(vec) => {
            assert!(
                vec.x > 0.0,
                "Expected x > 0.0 at t=0.5s, got {}",
                vec.x
            );
        }
        _ => panic!("Expected Vector3 value for Transform.translation"),
    }
}

#[test]
fn disabled_instance_is_skipped_in_fallback_output() {
    let mut app = setup_app();

    // Load animation asset
    let anim_handle = {
        let mut assets = app.world_mut().resource_mut::<Assets<AnimationData>>();
        assets.add(make_simple_position_anim("anim_disabled", "Disabled Test"))
    };

    // Create player WITHOUT target_root (no bindings)
    let player_entity = app
        .world_mut()
        .spawn(AnimationPlayer {
            name: "DisabledFallbackPlayer".into(),
            playback_state: ecs_animation_player::PlaybackState::Playing,
            mode: ecs_animation_player::PlaybackMode::Once,
            speed: 1.0,
            target_root: None,
            ..default()
        })
        .id();

    // Create DISABLED instance as child of player
    let instance_entity = app
        .world_mut()
        .spawn(AnimationInstance {
            animation: anim_handle.clone(),
            weight: 1.0,
            time_scale: 1.0,
            enabled: false,
            ..default()
        })
        .id();
    app.world_mut()
        .entity_mut(player_entity)
        .add_child(instance_entity);

    // Map player id for AnimationOutput
    let player_id = "player_disabled".to_string();
    app.world_mut()
        .resource_mut::<IdMapping>()
        .players
        .insert(player_id.clone(), player_entity);

    // Advance time
    app.update();
    {
        let mut et = app.world_mut().resource_mut::<EngineTime>();
        et.delta_seconds = 0.5;
        et.elapsed_seconds += 0.5;
    }
    app.update();

    // Verify no output was produced for this player (disabled instance skipped)
    let out = app.world().resource::<AnimationOutput>();
    let maybe_player_map = out.values.get(&player_id);
    assert!(
        maybe_player_map.map(|m| m.is_empty()).unwrap_or(true),
        "Expected no output for disabled instance; got: {:?}",
        maybe_player_map
    );
}
