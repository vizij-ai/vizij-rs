//! Integration tests for AnimationEngine

use animation_player::{
    animation::{AnimationInstance, InstanceSettings, PlaybackMode},
    player::PlayerState,
    value::Vector3,
    AnimationConfig, AnimationData, AnimationEngine, AnimationKeypoint, AnimationTime,
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
    let config = AnimationConfig::default();
    let engine = AnimationEngine::new(config);

    assert_eq!(engine.player_count(), 0);
    assert_eq!(engine.playing_player_count(), 0);
    assert!(engine.player_ids().is_empty());
}

#[test]
fn test_engine_load_unload_animation() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();
    let animation_id = animation.id.clone();

    // Load animation
    assert!(engine.load_animation_data(animation).is_ok());
    assert!(engine.get_animation_data(&animation_id).is_some());

    // Try to load duplicate
    let duplicate_animation = create_simple_animation();
    assert!(engine.load_animation_data(duplicate_animation).is_err());

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
    assert!(engine.create_player("player1").is_ok());
    assert!(engine.create_player("player2").is_ok());
    assert_eq!(engine.player_count(), 2);

    // Try to create duplicate
    assert!(engine.create_player("player1").is_err());

    // Get players
    assert!(engine.get_player("player1").is_some());
    assert!(engine.get_player("non_existent").is_none());

    // Get player states
    assert!(engine.get_player_state("player1").is_some());
    assert!(engine.get_player_state("non_existent").is_none());

    // Remove player
    assert!(engine.remove_player("player1").is_some());
    assert_eq!(engine.player_count(), 1);
    assert!(engine.get_player("player1").is_none());

    // Remove non-existent
    assert!(engine.remove_player("non_existent").is_none());
}

#[test]
fn test_engine_player_playback_control() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();

    // Load animation and create player
    engine.load_animation_data(animation.clone()).unwrap();
    engine.create_player("test_player").unwrap();

    // Add instance to player
    let player = engine.get_player_mut("test_player").unwrap();
    let instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("simple_anim"),
        animation.metadata.duration,
    );
    player.add_instance(instance).unwrap();

    // Test playback controls
    assert!(engine.play_player("test_player").is_ok());
    let state = engine.get_player_state("test_player").unwrap();
    assert_eq!(state.playback_state, PlaybackState::Playing);
    assert_eq!(engine.playing_player_count(), 1);

    assert!(engine.pause_player("test_player").is_ok());
    let state = engine.get_player_state("test_player").unwrap();
    assert_eq!(state.playback_state, PlaybackState::Paused);
    assert_eq!(engine.playing_player_count(), 0);

    assert!(engine.stop_player("test_player").is_ok());
    let state = engine.get_player_state("test_player").unwrap();
    assert_eq!(state.playback_state, PlaybackState::Stopped);

    // Test invalid state transitions
    assert!(engine.pause_player("test_player").is_err()); // Can't pause stopped player
}

#[test]
fn test_engine_player_seeking() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();

    engine.load_animation_data(animation.clone()).unwrap();
    engine.create_player("test_player").unwrap();

    let player = engine.get_player_mut("test_player").unwrap();
    let instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("simple_anim"),
        animation.metadata.duration,
    );
    player.add_instance(instance).unwrap();

    // Seek to different time
    assert!(engine
        .seek_player("test_player", AnimationTime::from(1.5))
        .is_ok());
    let player = engine.get_player("test_player").unwrap();
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

    engine.load_animation_data(animation.clone()).unwrap();
    engine.create_player("test_player").unwrap();

    let player = engine.get_player_mut("test_player").unwrap();
    let instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("multi_track"),
        animation.metadata.duration,
    );
    player.add_instance(instance).unwrap();

    // Start playback
    engine.play_player("test_player").unwrap();

    // Update with small delta time
    let delta = 1.0 / 60.0; // 60 FPS
    let result = engine.update(delta);
    assert!(result.is_ok());

    let values = result.unwrap();
    assert!(values.contains_key("test_player"));

    let player_values = &values["test_player"];
    assert!(player_values.contains_key("transform.position"));
    assert!(player_values.contains_key("transform.scale"));
    assert!(player_values.contains_key("transform.rotation"));

    // Check that player time advanced
    let player = engine.get_player("test_player").unwrap();
    assert!(player.current_time.as_seconds() > 0.0);
    assert!(player.current_time.as_seconds() <= delta);
}

#[test]
fn test_engine_update_with_multiple_players() {
    let mut engine = AnimationEngine::default();
    let animation1 = create_simple_animation();
    let animation2 = create_multi_track_animation();

    engine.load_animation_data(animation1.clone()).unwrap();
    engine.load_animation_data(animation2.clone()).unwrap();

    // Create two players
    engine.create_player("player1").unwrap();
    engine.create_player("player2").unwrap();

    // Add instances
    let player1 = engine.get_player_mut("player1").unwrap();
    let instance1 = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("simple_anim"),
        animation1.metadata.duration,
    );
    player1.add_instance(instance1).unwrap();

    let player2 = engine.get_player_mut("player2").unwrap();
    let instance2 = AnimationInstance::new(
        "instance2",
        InstanceSettings::new("multi_track"),
        animation2.metadata.duration,
    );
    player2.add_instance(instance2).unwrap();

    // Start both players
    engine.play_player("player1").unwrap();
    engine.play_player("player2").unwrap();

    // Update engine
    let result = engine.update(1.0 / 60.0);
    assert!(result.is_ok());

    let values = result.unwrap();
    assert_eq!(values.len(), 2);
    assert!(values.contains_key("player1"));
    assert!(values.contains_key("player2"));

    // Player1 should have position values
    let player1_values = &values["player1"];
    assert!(player1_values.contains_key("transform.position"));

    // Player2 should have position, scale, and rotation values
    let player2_values = &values["player2"];
    assert!(player2_values.contains_key("transform.position"));
    assert!(player2_values.contains_key("transform.scale"));
    assert!(player2_values.contains_key("transform.rotation"));
}

#[test]
fn test_engine_update_paused_players() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();

    engine.load_animation_data(animation.clone()).unwrap();
    engine.create_player("test_player").unwrap();

    let player = engine.get_player_mut("test_player").unwrap();
    let instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("simple_anim"),
        animation.metadata.duration,
    );
    player.add_instance(instance).unwrap();

    // Start then pause
    engine.play_player("test_player").unwrap();
    let initial_result = engine.update(1.0 / 60.0);
    assert!(initial_result.is_ok());

    let initial_values = initial_result.unwrap();
    engine.pause_player("test_player").unwrap();

    let initial_time = engine.get_player("test_player").unwrap().current_time;

    // Update should not advance paused player
    let result = engine.update(1.0 / 60.0);
    assert!(result.is_ok());

    let values = result.unwrap();
    assert_eq!(values, initial_values);

    let final_time = engine.get_player("test_player").unwrap().current_time;
    assert_eq!(initial_time, final_time);
}

#[test]
fn test_engine_update_with_looping() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation(); // 2 seconds duration

    engine.load_animation_data(animation.clone()).unwrap();
    engine.create_player("test_player").unwrap();

    let player = engine.get_player_mut("test_player").unwrap();
    let instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("simple_anim").with_playback_mode(PlaybackMode::Loop),
        animation.metadata.duration,
    );
    player.add_instance(instance).unwrap();

    // Enable looping for the player state
    let player_state = engine.get_player_state_mut("test_player").unwrap();
    player_state.mode = PlaybackMode::Loop;

    engine.play_player("test_player").unwrap();

    // Update past the animation duration
    let result = engine.update(2.5); // Beyond 2 second duration
    assert!(result.is_ok());

    // Player should have looped back
    let player = engine.get_player("test_player").unwrap();
    assert!(player.current_time.as_seconds() < 2.0); // Should have wrapped

    let state = engine.get_player_state("test_player").unwrap();
    assert_eq!(state.playback_state, PlaybackState::Playing); // Still playing
}

#[test]
fn test_engine_update_without_looping() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation(); // 2 seconds duration

    engine.load_animation_data(animation.clone()).unwrap();
    engine.create_player("test_player").unwrap();

    let player = engine.get_player_mut("test_player").unwrap();
    let instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("simple_anim"),
        animation.metadata.duration,
    );
    player.add_instance(instance).unwrap();
    let player_state = engine.get_player_state_mut("test_player").unwrap();
    player_state.mode = PlaybackMode::Once;

    engine.play_player("test_player").unwrap();

    // Update past the animation duration
    let result = engine.update(2.5); // Beyond 2 second duration
    assert!(result.is_ok());

    // Player should have ended
    let state = engine.get_player_state("test_player").unwrap();
    assert_eq!(state.playback_state, PlaybackState::Ended);
}

#[test]
fn test_engine_update_with_speed_variations() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();

    engine.load_animation_data(animation.clone()).unwrap();
    engine.create_player("test_player").unwrap();

    let player = engine.get_player_mut("test_player").unwrap();
    let instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("simple_anim"),
        animation.metadata.duration,
    );
    player.add_instance(instance).unwrap();

    // Set speed to 2x
    let player_state = engine.get_player_state_mut("test_player").unwrap();
    player_state.speed = 2.0;

    engine.play_player("test_player").unwrap();

    // Update with 1 second delta
    let result = engine.update(0.5);
    assert!(result.is_ok());

    // Player time should have advanced by 1 seconds (.5 * 2.0 speed)
    let player = engine.get_player("test_player").unwrap();
    assert!((player.current_time.as_seconds() - 1.0).abs() < 0.001);
}

#[test]
fn test_engine_update_with_reverse_speed() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();

    engine.load_animation_data(animation.clone()).unwrap();
    engine.create_player("test_player").unwrap();

    let player = engine.get_player_mut("test_player").unwrap();
    let instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("simple_anim"),
        animation.metadata.duration,
    );
    player.add_instance(instance).unwrap();

    // Start at end and play backwards
    engine
        .seek_player("test_player", animation.metadata.duration)
        .unwrap();

    let player_state = engine.get_player_state_mut("test_player").unwrap();
    player_state.speed = -1.0;

    engine.play_player("test_player").unwrap();

    // Update
    let result = engine.update(0.5);
    if let Err(e) = &result {
        eprintln!("Update error: {:?}", e);
    }
    assert!(result.is_ok());

    // Player time should have moved backwards
    let player = engine.get_player("test_player").unwrap();
    assert!((player.current_time.as_seconds() - 1.5).abs() < 0.001);
}

#[test]
fn test_engine_stop_all_players() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();

    engine.load_animation_data(animation.clone()).unwrap();
    engine.create_player("player1").unwrap();
    engine.create_player("player2").unwrap();

    // Add instances and start playback
    for player_id in ["player1", "player2"] {
        let player = engine.get_player_mut(player_id).unwrap();
        let instance = AnimationInstance::new(
            "instance1",
            InstanceSettings::new("simple_anim"),
            animation.metadata.duration,
        );
        player.add_instance(instance).unwrap();
        engine.play_player(player_id).unwrap();
    }

    assert_eq!(engine.playing_player_count(), 2);

    // Stop all players
    assert!(engine.stop_all_players().is_ok());
    assert_eq!(engine.playing_player_count(), 0);

    // Check that all players are stopped
    for player_id in ["player1", "player2"] {
        let state = engine.get_player_state(player_id).unwrap();
        assert_eq!(state.playback_state, PlaybackState::Stopped);
    }
}

#[test]
fn test_engine_pause_resume_all_players() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();

    engine.load_animation_data(animation.clone()).unwrap();
    engine.create_player("player1").unwrap();
    engine.create_player("player2").unwrap();

    // Add instances and start playback
    for player_id in ["player1", "player2"] {
        let player = engine.get_player_mut(player_id).unwrap();
        let instance = AnimationInstance::new(
            "instance1",
            InstanceSettings::new("simple_anim"),
            animation.metadata.duration,
        );
        player.add_instance(instance).unwrap();
        engine.play_player(player_id).unwrap();
    }

    assert_eq!(engine.playing_player_count(), 2);

    // Pause all players
    assert!(engine.pause_all_players().is_ok());
    assert_eq!(engine.playing_player_count(), 0);

    for player_id in ["player1", "player2"] {
        let state = engine.get_player_state(player_id).unwrap();
        assert_eq!(state.playback_state, PlaybackState::Paused);
    }

    // Resume all players
    assert!(engine.resume_all_players().is_ok());
    assert_eq!(engine.playing_player_count(), 2);

    for player_id in ["player1", "player2"] {
        let state = engine.get_player_state(player_id).unwrap();
        assert_eq!(state.playback_state, PlaybackState::Playing);
    }
}

#[test]
fn test_engine_metrics() {
    let mut engine = AnimationEngine::default();
    let animation = create_multi_track_animation();

    engine.load_animation_data(animation.clone()).unwrap();
    engine.create_player("test_player").unwrap();

    let player = engine.get_player_mut("test_player").unwrap();
    let instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("multi_track"),
        animation.metadata.duration,
    );
    player.add_instance(instance).unwrap();

    engine.play_player("test_player").unwrap();

    // Update to generate metrics
    engine.update(1.0 / 60.0).unwrap();

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
    let config = AnimationConfig::default()
        .with_target_fps(120.0)
        .with_max_memory_mb(1) // 1MB
        .with_max_cache_size(512);

    let mut engine = AnimationEngine::new(config.clone());

    assert_eq!(engine.config().target_fps, 120.0);
    assert_eq!(engine.config().max_memory_bytes, 1024 * 1024);
    assert_eq!(engine.config().max_cache_size, 512);

    // Update config
    let new_config = AnimationConfig::default()
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
    assert_eq!(state.end_time, None);
    assert_eq!(state.last_update_time, AnimationTime::zero());
}

#[test]
fn test_engine_multiple_instances_per_player() {
    let mut engine = AnimationEngine::default();
    let animation1 = create_simple_animation();
    let animation2 = create_multi_track_animation();

    engine.load_animation_data(animation1.clone()).unwrap();
    engine.load_animation_data(animation2.clone()).unwrap();
    engine.create_player("test_player").unwrap();

    let player = engine.get_player_mut("test_player").unwrap();

    // Add multiple instances to the same player
    let instance1 = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("simple_anim"),
        animation1.metadata.duration,
    );
    let instance2 = AnimationInstance::new(
        "instance2",
        InstanceSettings::new("multi_track").with_instance_start_time(AnimationTime::from(1.0)),
        animation2.metadata.duration,
    );

    player.add_instance(instance1).unwrap();
    player.add_instance(instance2).unwrap();

    engine.play_player("test_player").unwrap();

    // Update and check that we get combined values
    let result = engine.update(1.5); // 1.5 seconds - should activate both instances
    assert!(result.is_ok());

    let values = result.unwrap();
    assert!(values.contains_key("test_player"));

    let player_values = &values["test_player"];
    // Should have values from both animations (second instance starts at 1.0s)
    assert!(player_values.contains_key("transform.position"));
}

#[test]
fn test_engine_instance_time_offsets() {
    let mut engine = AnimationEngine::default();
    let animation = create_simple_animation();

    engine.load_animation_data(animation.clone()).unwrap();
    engine.create_player("test_player").unwrap();

    let player = engine.get_player_mut("test_player").unwrap();

    // Instance that starts 1 second into player timeline
    let instance = AnimationInstance::new(
        "delayed_instance",
        InstanceSettings::new("simple_anim").with_instance_start_time(AnimationTime::from(1.0)),
        animation.metadata.duration,
    );

    player.add_instance(instance).unwrap();
    engine.play_player("test_player").unwrap();

    // Update to 0.5 seconds - instance shouldn't be active yet
    let result = engine.update(0.5);
    assert!(result.is_ok());

    let values = result.unwrap();
    // Should be empty since instance hasn't started yet
    assert!(values.get("test_player").map_or(true, |v| v.is_empty()));

    // Update to 1.5 seconds - instance should now be active
    let result = engine.update(1.0); // Additional 1.0 second
    assert!(result.is_ok());

    let values = result.unwrap();
    assert!(values.contains_key("test_player"));

    let player_values = &values["test_player"];
    assert!(player_values.contains_key("transform.position"));
}
