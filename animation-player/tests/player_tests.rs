use std::time::Duration;

use animation_player::animation::{
    instance::PlaybackMode, AnimationData, AnimationInstance, AnimationInstanceSettings,
};
use animation_player::player::{AnimationEngine, AnimationPlayer, PlaybackState};
use animation_player::{AnimationEngineConfig, AnimationTime};

#[test]
fn test_playback_state_name() {
    assert_eq!(PlaybackState::Stopped.name(), "stopped");
    assert_eq!(PlaybackState::Playing.name(), "playing");
    assert_eq!(PlaybackState::Paused.name(), "paused");
    assert_eq!(PlaybackState::Ended.name(), "ended");
    assert_eq!(PlaybackState::Error.name(), "error");
}

#[test]
fn test_playback_state_transitions() {
    let mut state = PlaybackState::Stopped;
    assert!(state.can_resume());
    assert!(!state.can_pause());
    // assert!(state.can_stop());

    state = PlaybackState::Playing;
    assert!(!state.can_resume());
    assert!(state.can_pause());
    assert!(state.can_stop());

    state = PlaybackState::Paused;
    assert!(state.can_resume());
    assert!(!state.can_pause());
    assert!(state.can_stop());

    state = PlaybackState::Ended;
    assert!(state.can_resume());
    assert!(!state.can_pause());
    assert!(state.can_stop()); // Can stop from ended

    state = PlaybackState::Error;
    assert!(!state.can_resume());
    assert!(!state.can_pause());
    assert!(!state.can_stop()); // Cannot stop from error
}

#[test]
fn test_animation_engine_create_player() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());
    let player_id = engine.create_player();
    assert!(engine.get_player(&player_id).is_some());
    assert!(engine.get_player_properties(&player_id).is_some());
}

#[test]
fn test_animation_engine_load_animation_data() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());
    let anim_data = AnimationData::new("test_anim", "Test Animation");
    let anim_id = engine.load_animation_data(anim_data.clone()).unwrap();
    assert!(engine.get_animation_data(&anim_id).is_some());
}

#[test]
fn test_animation_player_add_instance() {
    let mut player = AnimationPlayer::new();
    let anim_instance = AnimationInstance::new(
        "anim1".to_string(),
        AnimationInstanceSettings::default(),
        AnimationTime::from_seconds(10.0).unwrap(),
    );
    let anim_instance_id = player.add_instance(anim_instance.clone());
    assert!(player.instances.contains_key(&anim_instance_id));
}

#[test]
fn test_animation_player_get_effective_time() {
    let anim_duration = AnimationTime::from_seconds(10.0).unwrap();
    let settings = AnimationInstanceSettings {
        instance_start_time: AnimationTime::from_seconds(5.0).unwrap(),
        time_scale: 0.5,
        ..Default::default()
    };
    let animation_id = "fake_animation_id";
    let instance = AnimationInstance::new(animation_id, settings.clone(), anim_duration);

    // Player time 5.0s, instance starts at 5.0s, timescale 0.5
    // Relative time = 0.0s, scaled = 0.0s, effective = 0.0s
    assert_eq!(
        instance.get_effective_time(AnimationTime::from_seconds(5.0).unwrap()),
        AnimationTime::from_seconds(0.0).unwrap()
    );

    // Player time 7.0s, instance starts at 5.0s, timescale 0.5
    // Relative time = 2.0s, scaled = 1.0s, effective = 1.0s
    assert_eq!(
        instance.get_effective_time(AnimationTime::from_seconds(7.0).unwrap()),
        AnimationTime::from_seconds(1.0).unwrap()
    );

    // Player time 15.0s (well past end of animation data duration 10s)
    // Relative time = 10.0s, scaled = 5.0s, clamped to 10s
    assert_eq!(
        instance.get_effective_time(AnimationTime::from_seconds(25.0).unwrap()),
        AnimationTime::from_seconds(10.0).unwrap()
    );
}

fn setup_animation_player(
    engine: &mut AnimationEngine,
    animation_data: AnimationData,
    settings: AnimationInstanceSettings,
    custom_duration: impl Into<AnimationTime>,
) -> (String, String) {
    let animation_id = engine.load_animation_data(animation_data).unwrap();
    let player_id = engine.create_player();
    let anim_instance = AnimationInstance::new(animation_id.clone(), settings, custom_duration);
    engine
        .get_player_mut(&player_id)
        .unwrap()
        .add_instance(anim_instance);
    (animation_id, player_id)
}

#[test]
fn test_animation_player_go_to() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());
    let (_, player_id) = setup_animation_player(
        &mut engine,
        AnimationData::new("anim1", "Test Animation"),
        AnimationInstanceSettings::default(),
        Duration::from_secs(10),
    );
    // Use the engine's seek_player method
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(5.0).unwrap())
        .unwrap();

    // Verify the player's time was set correctly
    let player = engine.get_player(&player_id).unwrap();
    assert_eq!(
        player.current_time,
        AnimationTime::from_seconds(5.0).unwrap()
    );
}

#[test]
fn test_animation_engine_update_playback() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());
    let (animation_id, player_id) = setup_animation_player(
        &mut engine,
        AnimationData::new("anim1", "Test Animation"),
        AnimationInstanceSettings::default(),
        Duration::from_secs(10),
    );

    // Play the player
    engine.play_player(&player_id).unwrap();
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );

    // Update engine by 1 second
    let values = engine.update(Duration::from_secs(1)).unwrap();
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(1.0).unwrap()
    );
    assert!(values.contains_key(&player_id));
    assert!(values.get(&player_id).unwrap().is_empty()); // Still empty as no tracks

    // Update until end
    engine.update(Duration::from_secs(9)).unwrap(); // Total 10 seconds
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(0.0).unwrap() // Should wrap around to 0.0
    );
    // Since default playback mode is Loop, it should wrap around
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );

    // Test with PlaybackMode::Once
    let player_id_once = engine.create_player();
    let player_state_once = engine.get_player_settings_mut(&player_id_once).unwrap();
    player_state_once.mode = PlaybackMode::Once;

    let player_once = engine.get_player_mut(&player_id_once).unwrap();
    let anim_instance_once = AnimationInstance::new(
        animation_id.clone(),
        AnimationInstanceSettings::default(),
        Duration::from_secs(5),
    );
    player_once.add_instance(anim_instance_once);

    engine.play_player(&player_id_once).unwrap();
    engine.update(Duration::from_secs(6)).unwrap(); // Exceeds 5s duration
    assert_eq!(
        engine.get_player(&player_id_once).unwrap().current_time,
        AnimationTime::from_seconds(5.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id_once)
            .unwrap()
            .playback_state,
        PlaybackState::Ended
    );
}

// Comprehensive playback mode tests

#[test]
fn test_playback_mode_once_forward() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());
    let (_, player_id) = setup_animation_player(
        &mut engine,
        AnimationData::new("test_anim", "Test Animation"),
        AnimationInstanceSettings::default(),
        Duration::from_secs(10),
    );

    let player_state_once = engine.get_player_settings_mut(&player_id).unwrap();
    player_state_once.mode = PlaybackMode::Once;

    // Start playback
    engine.play_player(&player_id).unwrap();
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(0.0).unwrap()
    );

    // Update several times, approaching the end
    engine.update(Duration::from_secs(3)).unwrap();
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(3.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );

    engine.update(Duration::from_secs(5)).unwrap(); // Total: 8 seconds
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(8.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );

    // Cross the boundary
    engine.update(Duration::from_secs(3)).unwrap(); // Total: 11 seconds, exceeds duration
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(10.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Ended
    );

    // Further updates should not change time or state
    engine.update(Duration::from_secs(2)).unwrap();
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(10.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Ended
    );
}

#[test]
fn test_playback_mode_once_reverse() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());
    let (_, player_id) = setup_animation_player(
        &mut engine,
        AnimationData::new("test_anim", "Test Animation"),
        AnimationInstanceSettings::default(),
        Duration::from_secs(10),
    );

    // Start at end and set negative speed
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(10.0).unwrap())
        .unwrap();
    {
        let player_state = engine.get_player_settings_mut(&player_id).unwrap();
        player_state.speed = -1.0;
        player_state.mode = PlaybackMode::Once;
    }
    engine
        .get_player_properties_mut(&player_id)
        .unwrap()
        .playback_state = PlaybackState::Playing;

    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(10.0).unwrap()
    );
    assert_eq!(engine.get_player_settings(&player_id).unwrap().speed, -1.0);

    // Update several times, approaching the start
    engine.update(Duration::from_secs(3)).unwrap();
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(7.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );

    engine.update(Duration::from_secs(5)).unwrap(); // Total moved: 8 seconds back
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(2.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );

    // Cross the boundary
    engine.update(Duration::from_secs(3)).unwrap(); // Would go to -1.0, but should clamp and end
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(0.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Ended
    );
}

#[test]
fn test_playback_mode_loop_forward() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());
    let (_, player_id) = setup_animation_player(
        &mut engine,
        AnimationData::new("test_anim", "Test Animation"),
        AnimationInstanceSettings::default(),
        Duration::from_secs(10),
    );

    // Start playback
    engine.play_player(&player_id).unwrap();
    let player_state_mut = engine.get_player_settings_mut(&player_id).unwrap();
    player_state_mut.mode = PlaybackMode::Loop;

    // Update to near the end
    engine.update(Duration::from_millis(9500)).unwrap();
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(9.5).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );

    // Cross the boundary - should loop back to start
    engine.update(Duration::from_secs(1)).unwrap(); // Would be 10.5, should wrap to 0.5
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(0.5).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );

    // Continue and loop again
    engine.update(Duration::from_secs(12)).unwrap(); // Should wrap around again
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(2.5).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );
}

#[test]
fn test_playback_mode_loop_reverse() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());
    let (_, player_id) = setup_animation_player(
        &mut engine,
        AnimationData::new("test_anim", "Test Animation"),
        AnimationInstanceSettings::default(),
        Duration::from_secs(10),
    );

    // Start near beginning with reverse speed
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(0.5).unwrap())
        .unwrap();
    let player_state_mut = engine.get_player_settings_mut(&player_id).unwrap();
    player_state_mut.mode = PlaybackMode::Loop;
    player_state_mut.speed = -1.0;
    engine
        .get_player_properties_mut(&player_id)
        .unwrap()
        .playback_state = PlaybackState::Playing;

    // Cross the boundary - should loop back to end
    engine.update(Duration::from_secs(1)).unwrap(); // Would be -0.5, should wrap to end
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(10.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );

    // Continue with another update to verify loop behavior
    engine.update(Duration::from_secs(2)).unwrap(); // Should go to 8.0 (10.0 - 2.0)
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(8.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );
}

#[test]
fn test_playback_mode_pingpong_forward_to_reverse() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());
    let (_, player_id) = setup_animation_player(
        &mut engine,
        AnimationData::new("test_anim", "Test Animation"),
        AnimationInstanceSettings::default(),
        Duration::from_secs(10),
    );

    let player_state_mut = engine.get_player_settings_mut(&player_id).unwrap();
    player_state_mut.mode = PlaybackMode::PingPong;
    // Start playback
    engine.play_player(&player_id).unwrap();
    assert_eq!(engine.get_player_settings(&player_id).unwrap().speed, 1.0);

    // Update to near the end
    engine.update(Duration::from_millis(9500)).unwrap();
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(9.5).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );
    assert_eq!(engine.get_player_settings(&player_id).unwrap().speed, 1.0);

    // Cross the boundary - should reverse direction
    engine.update(Duration::from_secs(1)).unwrap(); // Would be 10.5, should clamp to 10.0 and reverse speed
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(10.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );
    assert_eq!(engine.get_player_settings(&player_id).unwrap().speed, -1.0); // Speed should be reversed

    // Continue in reverse direction
    engine.update(Duration::from_secs(3)).unwrap();
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(7.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );
    assert_eq!(engine.get_player_settings(&player_id).unwrap().speed, -1.0);
}

#[test]
fn test_playback_mode_pingpong_reverse_to_forward() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());
    let (_, player_id) = setup_animation_player(
        &mut engine,
        AnimationData::new("test_anim", "Test Animation"),
        AnimationInstanceSettings::default(),
        Duration::from_secs(10),
    );

    // Start near beginning with reverse speed (simulating after first ping-pong)
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(0.5).unwrap())
        .unwrap();
    let player_state_mut = engine.get_player_settings_mut(&player_id).unwrap();
    player_state_mut.mode = PlaybackMode::PingPong;
    player_state_mut.speed = -1.0;
    engine
        .get_player_properties_mut(&player_id)
        .unwrap()
        .playback_state = PlaybackState::Playing;

    // Cross the boundary - should reverse direction back to forward
    engine.update(Duration::from_secs(1)).unwrap(); // Would be -0.5, should clamp to 0.0 and reverse speed
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(0.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );
    assert_eq!(engine.get_player_settings(&player_id).unwrap().speed, 1.0); // Speed should be reversed to positive

    // Continue in forward direction
    engine.update(Duration::from_secs(3)).unwrap();
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(3.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );
    assert_eq!(engine.get_player_settings(&player_id).unwrap().speed, 1.0);
}

#[test]
fn test_playback_mode_pingpong_full_cycle() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());
    let (_, player_id) = setup_animation_player(
        &mut engine,
        AnimationData::new("test_anim", "Test Animation"),
        AnimationInstanceSettings::default(),
        Duration::from_secs(5), // Short duration for easier testing
    );

    let player_state_mut = engine.get_player_settings_mut(&player_id).unwrap();
    player_state_mut.mode = PlaybackMode::PingPong;
    // Start playback
    engine.play_player(&player_id).unwrap();
    assert_eq!(engine.get_player_settings(&player_id).unwrap().speed, 1.0);

    // Go to end (forward phase)
    engine.update(Duration::from_secs(6)).unwrap(); // Exceeds 5.0, should hit end and reverse
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(5.0).unwrap()
    );
    assert_eq!(engine.get_player_settings(&player_id).unwrap().speed, -1.0);

    // Go back to start (reverse phase)
    engine.update(Duration::from_secs(6)).unwrap(); // Should hit start and reverse again
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(0.0).unwrap()
    );
    assert_eq!(engine.get_player_settings(&player_id).unwrap().speed, 1.0);

    // Verify state is still playing throughout
    assert_eq!(
        engine
            .get_player_properties(&player_id)
            .unwrap()
            .playback_state,
        PlaybackState::Playing
    );
}

#[test]
fn test_mixed_playback_speeds() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());
    let (_, player_id) = setup_animation_player(
        &mut engine,
        AnimationData::new("test_anim", "Test Animation"),
        AnimationInstanceSettings::default(),
        Duration::from_secs(10),
    );

    // Test 2x speed
    let player_state_mut = engine.get_player_settings_mut(&player_id).unwrap();
    player_state_mut.mode = PlaybackMode::Loop;
    player_state_mut.speed = 2.0;
    engine
        .get_player_properties_mut(&player_id)
        .unwrap()
        .playback_state = PlaybackState::Playing;

    engine.update(Duration::from_secs(1)).unwrap(); // Should advance 2 seconds
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(2.0).unwrap()
    );

    engine.update(Duration::from_millis(4500)).unwrap(); // Should advance 9 more seconds, total 11, wrap to 1
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(1.0).unwrap()
    );

    // Test 0.5x speed
    let player_state = engine.get_player_settings_mut(&player_id).unwrap();
    player_state.speed = 0.5;

    engine.update(Duration::from_secs(2)).unwrap(); // Should advance 1 second
    assert_eq!(
        engine.get_player(&player_id).unwrap().current_time,
        AnimationTime::from_seconds(2.0).unwrap()
    );
}
