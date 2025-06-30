//! Integration tests for AnimationEngine

use std::time::Duration;

use animation_player::{
    animation::{Animation, AnimationSettings, PlaybackMode},
    player::PlayerState,
    value::Vector3,
    AnimationData, AnimationEngine, AnimationEngineConfig, AnimationKeypoint, AnimationTime,
    AnimationTrack, PlaybackState, Value,
};

/// Helper function to create a simple test animation
fn create_simple_animation() -> AnimationData {
    let mut animation = AnimationData::new("simple_anim", "Simple Animation");

    // Position track
    let mut position_track = AnimationTrack::new("position", "transform.position");
    position_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(0.0),
            Value::Vector3(Vector3::new(0.0, 0.0, 0.0)),
        ))
        .unwrap();
    position_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(2.0),
            Value::Vector3(Vector3::new(10.0, 5.0, 0.0)),
        ))
        .unwrap();

    animation.add_track(position_track);
    animation.recalculate_duration();
    animation
}

/// Helper function to create a multi-track animation
fn create_multi_track_animation() -> AnimationData {
    let mut animation = AnimationData::new("multi_track", "Multi Track Animation");

    // Position track
    let mut position_track = AnimationTrack::new("position", "transform.position");
    position_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(0.0),
            Value::Vector3(Vector3::new(0.0, 0.0, 0.0)),
        ))
        .unwrap();
    position_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(1.0),
            Value::Vector3(Vector3::new(5.0, 0.0, 0.0)),
        ))
        .unwrap();
    position_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(3.0),
            Value::Vector3(Vector3::new(15.0, 10.0, 5.0)),
        ))
        .unwrap();

    // Scale track
    let mut scale_track = AnimationTrack::new("scale", "transform.scale");
    scale_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(0.0),
            Value::Float(1.0),
        ))
        .unwrap();
    scale_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(1.5),
            Value::Float(2.0),
        ))
        .unwrap();
    scale_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(3.0),
            Value::Float(0.5),
        ))
        .unwrap();

    // Rotation track
    let mut rotation_track = AnimationTrack::new("rotation", "transform.rotation");
    rotation_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(0.0),
            Value::Float(0.0),
        ))
        .unwrap();
    rotation_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(3.0),
            Value::Float(360.0),
        ))
        .unwrap();

    animation.add_track(position_track);
    animation.add_track(scale_track);
    animation.add_track(rotation_track);
    animation.recalculate_duration();
    animation
}

#[test]
fn test_engine_creation() {
    let config = AnimationEngineConfig::default();
    let engine = AnimationEngine::new(config);

    assert_eq!(engine.player_count(), 0);
    assert_eq!(engine.playing_player_count(), 0);
    assert!(engine.player_ids().is_empty());
}

#[test]
fn test_engine_load_unload_animation() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();
    let animation_id = engine.load_animation_data(animation).unwrap();

    // Load animation
    assert!(engine.get_animation_data(&animation_id).is_some());

    // Try to load duplicate
    let duplicate_animation = create_simple_animation();
    assert!(engine.load_animation_data(duplicate_animation).is_ok());

    // Unload animation
    let unloaded = engine.unload_animation_data(&animation_id);
    assert!(unloaded.is_ok());
    assert!(engine.get_animation_data(&animation_id).is_none());

    // Try to unload non-existent
    assert!(engine.unload_animation_data("non_existent").is_err());
}

#[test]
fn test_engine_player_management() {
    let mut engine = AnimationEngine::default();

    // Create players
    let player1_id = engine.create_player();
    engine.create_player();
    assert_eq!(engine.player_count(), 2);

    // Get players
    assert!(engine.get_player(&player1_id).is_some());
    assert!(engine.get_player("non_existent").is_none());

    // Get player states
    assert!(engine.get_player_state(&player1_id).is_some());
    assert!(engine.get_player_state("non_existent").is_none());

    // Remove player
    assert!(engine.remove_player(&player1_id).is_some());
    assert_eq!(engine.player_count(), 1);
    assert!(engine.get_player(&player1_id).is_none());

    // Remove non-existent
    assert!(engine.remove_player("non_existent").is_none());
}

/// Helper function to set up a player with a simple animation
fn setup_player_of_simple_animation(engine: &mut AnimationEngine) -> String {
    let animation = create_simple_animation();
    let animation_id = engine.load_animation_data(animation.clone()).unwrap();
    let player_id = engine.create_player();
    engine
        .add_animation_to_player(&player_id, &animation_id, None)
        .unwrap();
    player_id
}

#[test]
fn test_engine_player_playback_control() {
    let mut engine = AnimationEngine::default();
    let player_id = setup_player_of_simple_animation(&mut engine);

    // Test playback controls
    assert!(engine.play_player(&player_id).is_ok());
    let state = engine.get_player_state(&player_id).unwrap();
    assert_eq!(state.playback_state, PlaybackState::Playing);
    assert_eq!(engine.playing_player_count(), 1);

    assert!(engine.pause_player(&player_id).is_ok());
    let state = engine.get_player_state(&player_id).unwrap();
    assert_eq!(state.playback_state, PlaybackState::Paused);
    assert_eq!(engine.playing_player_count(), 0);

    assert!(engine.stop_player(&player_id).is_ok());
    let state = engine.get_player_state(&player_id).unwrap();
    assert_eq!(state.playback_state, PlaybackState::Stopped);

    // Test invalid state transitions
    assert!(engine.pause_player(&player_id).is_err()); // Can't pause stopped player
}

#[test]
fn test_engine_player_seeking() {
    let mut engine = AnimationEngine::default();
    let player_id = setup_player_of_simple_animation(&mut engine);

    // Seek to different time
    assert!(engine
        .seek_player(&player_id, AnimationTime::from(1.5))
        .is_ok());
    let player = engine.get_player(&player_id).unwrap();
    assert_eq!(player.current_time, AnimationTime::from(1.5));

    // Seek non-existent player
    assert!(engine
        .seek_player("non_existent", AnimationTime::from(1.0))
        .is_err());
}

#[test]
fn test_engine_update_with_single_player() {
    let mut engine = AnimationEngine::default();
    let animation = create_multi_track_animation();
    let animation_id = engine.load_animation_data(animation.clone()).unwrap();
    let player_id = engine.create_player();
    engine
        .add_animation_to_player(&player_id, &animation_id, None)
        .unwrap();

    // Start playback
    engine.play_player(&player_id).unwrap();

    // Update with small delta time
    let delta = Duration::from_secs_f64(1.0 / 60.0); // 60 FPS
    let result = engine.update(delta);
    assert!(result.is_ok());

    let values = result.unwrap();
    assert!(values.contains_key(&player_id));

    let player_values = &values[&player_id];
    assert!(player_values.contains_key("transform.position"));
    assert!(player_values.contains_key("transform.scale"));
    assert!(player_values.contains_key("transform.rotation"));

    // Check that player time advanced
    let player = engine.get_player(&player_id).unwrap();
    assert!(player.current_time.as_seconds() > 0.0);
    assert!(player.current_time <= delta.into());
}

#[test]
fn test_engine_update_with_multiple_players() {
    let mut engine = AnimationEngine::default();
    let animation1 = create_simple_animation();
    let animation2 = create_multi_track_animation();

    let animation1_id = engine.load_animation_data(animation1.clone()).unwrap();
    let animation2_id = engine.load_animation_data(animation2.clone()).unwrap();

    // Create two players
    let player1_id = engine.create_player();
    let player2_id = engine.create_player();

    // Add instances
    let player1 = engine.get_player_mut(&player1_id).unwrap();
    let instance1 = Animation::new(
        animation1_id,
        AnimationSettings::new(),
        animation1.metadata.duration,
    );
    player1.add_instance(instance1);

    let player2 = engine.get_player_mut(&player2_id).unwrap();
    let instance2 = Animation::new(
        animation2_id,
        AnimationSettings::new(),
        animation2.metadata.duration,
    );
    player2.add_instance(instance2);

    // Start both players
    engine.play_player(&player1_id).unwrap();
    engine.play_player(&player2_id).unwrap();

    // Update engine
    let delta = Duration::from_secs_f64(1.0 / 60.0); // 60 FPS
    let result = engine.update(delta);
    assert!(result.is_ok());

    let values = result.unwrap();
    assert_eq!(values.len(), 2);
    assert!(values.contains_key(&player1_id));
    assert!(values.contains_key(&player2_id));

    // Player1 should have position values
    let player1_values = &values[&player1_id];
    assert!(player1_values.contains_key("transform.position"));

    // Player2 should have position, scale, and rotation values
    let player2_values = &values[&player2_id];
    assert!(player2_values.contains_key("transform.position"));
    assert!(player2_values.contains_key("transform.scale"));
    assert!(player2_values.contains_key("transform.rotation"));
}

#[test]
fn test_engine_update_paused_players() {
    let mut engine = AnimationEngine::default();
    let player_id = setup_player_of_simple_animation(&mut engine);

    // Start then pause
    engine.play_player(&player_id).unwrap();
    let delta = Duration::from_secs_f64(1.0 / 60.0); // 60 FPS
    let initial_result = engine.update(delta);
    assert!(initial_result.is_ok());

    let initial_values = initial_result.unwrap();
    engine.pause_player(&player_id).unwrap();

    let initial_time = engine.get_player(&player_id).unwrap().current_time;

    // Update should not advance paused player
    let result = engine.update(Duration::from_secs_f64(1.0 / 60.0));
    assert!(result.is_ok());

    let values = result.unwrap();
    assert_eq!(values, initial_values);

    let final_time = engine.get_player(&player_id).unwrap().current_time;
    assert_eq!(initial_time, final_time);
}

#[test]
fn test_engine_update_with_looping() {
    let mut engine = AnimationEngine::default();
    let player_id = setup_player_of_simple_animation(&mut engine);

    // Enable looping for the player state
    let player_state = engine.get_player_state_mut(&player_id).unwrap();
    player_state.mode = PlaybackMode::Loop;

    engine.play_player(&player_id).unwrap();

    // Update past the animation duration
    let result = engine.update(Duration::from_millis(2500)); // Beyond 2 second duration
    assert!(result.is_ok());

    // Player should have looped back
    let player = engine.get_player(&player_id).unwrap();
    assert!(player.current_time.as_seconds() < 2.0); // Should have wrapped

    let state = engine.get_player_state(&player_id).unwrap();
    assert_eq!(state.playback_state, PlaybackState::Playing); // Still playing
}

#[test]
fn test_engine_update_without_looping() {
    let mut engine = AnimationEngine::default();
    let player_id = setup_player_of_simple_animation(&mut engine);

    // Set player to play once
    let player_state = engine.get_player_state_mut(&player_id).unwrap();
    player_state.mode = PlaybackMode::Once;

    engine.play_player(&player_id).unwrap();

    // Update past the animation duration
    let result = engine.update(Duration::from_millis(2500)); // Beyond 2 second duration
    assert!(result.is_ok());

    // Player should have ended
    let state = engine.get_player_state(&player_id).unwrap();
    assert_eq!(state.playback_state, PlaybackState::Ended);
}

#[test]
fn test_engine_update_with_speed_variations() {
    let mut engine = AnimationEngine::default();
    let player_id = setup_player_of_simple_animation(&mut engine);

    // Set speed to 2x
    let player_state = engine.get_player_state_mut(&player_id).unwrap();
    player_state.speed = 2.0;

    engine.play_player(&player_id).unwrap();

    // Update with 1 second delta
    let result = engine.update(Duration::from_millis(500));
    assert!(result.is_ok());

    // Player time should have advanced by 1 seconds (.5 * 2.0 speed)
    let player = engine.get_player(&player_id).unwrap();
    assert!((player.current_time.as_seconds() - 1.0).abs() < 0.001);
}

#[test]
fn test_engine_update_with_reverse_speed() {
    let mut engine = AnimationEngine::default();
    let player_id = setup_player_of_simple_animation(&mut engine);

    // Find the animation back in the engine
    let loaded_animation_ids = engine.animation_ids();
    assert_eq!(1, loaded_animation_ids.len());
    let animation_id = loaded_animation_ids[0];
    let animation = engine.get_animation_data(&animation_id).unwrap();

    // Start at end and play backwards
    engine
        .seek_player(&player_id, animation.metadata.duration)
        .unwrap();

    let player_state = engine.get_player_state_mut(&player_id).unwrap();
    player_state.speed = -1.0;

    engine.play_player(&player_id).unwrap();

    // Update
    let result = engine.update(Duration::from_millis(500));
    if let Err(e) = &result {
        eprintln!("Update error: {:?}", e);
    }
    assert!(result.is_ok());

    // Player time should have moved backwards
    let player = engine.get_player(&player_id).unwrap();
    assert!((player.current_time.as_seconds() - 1.5).abs() < 0.001);
}

#[test]
fn test_engine_stop_all_players() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();

    let animation_id = engine.load_animation_data(animation.clone()).unwrap();
    let player1_id = engine.create_player();
    let player2_id = engine.create_player();

    // Add instances and start playback
    for player_id in [&player1_id, &player2_id] {
        let player = engine.get_player_mut(player_id).unwrap();
        let instance = Animation::new(
            animation_id.clone(),
            AnimationSettings::new(),
            animation.metadata.duration,
        );
        player.add_instance(instance);
        engine.play_player(player_id).unwrap();
    }

    assert_eq!(engine.playing_player_count(), 2);

    // Stop all players
    assert!(engine.stop_all_players().is_ok());
    assert_eq!(engine.playing_player_count(), 0);

    // Check that all players are stopped
    for player_id in [&player1_id, &player2_id] {
        let state = engine.get_player_state(player_id).unwrap();
        assert_eq!(state.playback_state, PlaybackState::Stopped);
    }
}

#[test]
fn test_engine_pause_resume_all_players() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();

    let animation_id = engine.load_animation_data(animation.clone()).unwrap();
    let player1_id = engine.create_player();
    let player2_id = engine.create_player();

    // Add instances and start playback
    for player_id in [&player1_id, &player2_id] {
        let player = engine.get_player_mut(player_id).unwrap();
        let instance = Animation::new(
            animation_id.clone(),
            AnimationSettings::new(),
            animation.metadata.duration,
        );
        player.add_instance(instance);
        engine.play_player(player_id).unwrap();
    }

    assert_eq!(engine.playing_player_count(), 2);

    // Pause all players
    assert!(engine.pause_all_players().is_ok());
    assert_eq!(engine.playing_player_count(), 0);

    for player_id in [&player1_id, &player2_id] {
        let state = engine.get_player_state(player_id).unwrap();
        assert_eq!(state.playback_state, PlaybackState::Paused);
    }

    // Resume all players
    assert!(engine.resume_all_players().is_ok());
    assert_eq!(engine.playing_player_count(), 2);

    for player_id in [&player1_id, &player2_id] {
        let state = engine.get_player_state(player_id).unwrap();
        assert_eq!(state.playback_state, PlaybackState::Playing);
    }
}

#[test]
fn test_engine_metrics() {
    let mut engine = AnimationEngine::default();
    let player_id = setup_player_of_simple_animation(&mut engine);

    engine.play_player(&player_id).unwrap();

    // Update to generate metrics
    engine.update(Duration::from_secs_f64(1.0 / 60.0)).unwrap();

    let metrics = engine.metrics();
    assert!(metrics.contains_key("total_players"));
    assert!(metrics.contains_key("playing_players"));
    assert!(metrics.contains_key("total_memory_mb"));
    assert!(metrics.contains_key("average_fps"));
    assert!(metrics.contains_key("cache_hit_rate"));

    assert_eq!(metrics["total_players"], 1.0);
    assert_eq!(metrics["playing_players"], 1.0);
    assert!(metrics["total_memory_mb"] >= 0.0);
}

#[test]
fn test_engine_config_access() {
    let config = AnimationEngineConfig::default()
        .with_target_fps(120.0)
        .with_max_memory_mb(1) // 1MB
        .with_max_cache_size(512);

    let mut engine = AnimationEngine::new(config.clone());

    assert_eq!(engine.config().target_fps, 120.0);
    assert_eq!(engine.config().max_memory_bytes, 1024 * 1024);
    assert_eq!(engine.config().max_cache_size, 512);

    // Update config
    let new_config = AnimationEngineConfig::default()
        .with_target_fps(30.0)
        .with_max_memory_mb(1) // 512KB -> 1MB for simplicity
        .with_max_cache_size(256);

    engine.set_config(new_config);
    assert_eq!(engine.config().target_fps, 30.0);
    assert_eq!(engine.config().max_memory_bytes, 1024 * 1024);
    assert_eq!(engine.config().max_cache_size, 256);
}

#[test]
fn test_engine_interpolation_registry_access() {
    let mut engine = AnimationEngine::default();

    // Access immutable registry
    let registry = engine.interpolation_registry();
    let metrics = registry.metrics();
    assert_eq!(metrics.total_interpolations, 0);

    // Access mutable registry (would be used to register new interpolation functions)
    let _registry_mut = engine.interpolation_registry_mut();
}

#[test]
fn test_engine_event_dispatcher_access() {
    let mut engine = AnimationEngine::default();

    // Access immutable dispatcher
    let _dispatcher = engine.event_dispatcher();

    // Access mutable dispatcher (would be used to add event handlers)
    let _dispatcher_mut = engine.event_dispatcher_mut();
}

#[test]
fn test_engine_default_construction() {
    let engine = AnimationEngine::default();

    assert_eq!(engine.player_count(), 0);
    assert_eq!(engine.playing_player_count(), 0);
    assert!(engine.player_ids().is_empty());
    assert_eq!(engine.config().target_fps, 60.0); // Default config values
}

#[test]
fn test_player_state_initialization() {
    let state = PlayerState::default();

    assert_eq!(state.playback_state, PlaybackState::Stopped);
    assert_eq!(state.speed, 1.0);
    assert_eq!(state.mode, PlaybackMode::Loop);
    assert_eq!(state.start_time, AnimationTime::zero());
    assert_eq!(state.offset, AnimationTime::zero());
    assert_eq!(state.end_time, None);
    assert_eq!(state.last_update_time, AnimationTime::zero());
}

#[test]
fn test_engine_multiple_instances_per_player() {
    let mut engine = AnimationEngine::default();
    let animation1 = create_simple_animation();
    let animation2 = create_multi_track_animation();

    let animation1_id = engine.load_animation_data(animation1.clone()).unwrap();
    let animation2_id = engine.load_animation_data(animation2.clone()).unwrap();
    let player_id = engine.create_player();

    let player = engine.get_player_mut(&player_id).unwrap();

    // Add multiple instances to the same player
    let instance1 = Animation::new(
        animation1_id,
        AnimationSettings::new(),
        animation1.metadata.duration,
    );
    let instance2 = Animation::new(
        animation2_id,
        AnimationSettings {
            instance_start_time: Duration::from_secs(1).into(),
            ..AnimationSettings::default()
        },
        animation2.metadata.duration,
    );

    player.add_instance(instance1);
    player.add_instance(instance2);

    engine.play_player(&player_id).unwrap();

    // Update and check that we get combined values
    let result = engine.update(Duration::from_millis(1500)); // 1.5 seconds - should activate both instances
    assert!(result.is_ok());

    let values = result.unwrap();
    assert!(values.contains_key(&player_id));

    let player_values = &values[&player_id];
    // Should have values from both animations (second instance starts at 1.0s)
    assert!(player_values.contains_key("transform.position"));
}

#[test]
fn test_engine_instance_time_offsets() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();

    let animation_id = engine.load_animation_data(animation.clone()).unwrap();
    let player_id = engine.create_player();

    let player = engine.get_player_mut(&player_id).unwrap();

    // Instance that starts 1 second into player timeline
    let instance = Animation::new(
        animation_id,
        AnimationSettings {
            instance_start_time: Duration::from_secs(1).into(),
            ..AnimationSettings::default()
        },
        animation.metadata.duration,
    );

    player.add_instance(instance);
    engine.play_player(&player_id).unwrap();

    // Update to 0.5 seconds - instance shouldn't be active yet
    let result = engine.update(Duration::from_millis(500));
    assert!(result.is_ok());

    let values = result.unwrap();
    // Should be empty since instance hasn't started yet
    assert!(values.get(&player_id).map_or(true, |v| v.is_empty()));

    // Update to 1.5 seconds - instance should now be active
    let result = engine.update(Duration::from_millis(1000)); // Additional 1.0 second
    assert!(result.is_ok());

    let values = result.unwrap();
    assert!(values.contains_key(&player_id));

    let player_values = &values[&player_id];
    assert!(player_values.contains_key("transform.position"));
}
