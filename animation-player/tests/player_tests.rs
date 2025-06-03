use animation_player::animation::{
    instance::PlaybackMode, AnimationData, AnimationInstance, InstanceSettings,
};
use animation_player::player::{AnimationEngine, AnimationPlayer, PlaybackState};
use animation_player::{AnimationConfig, AnimationTime};

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
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    let player_id = "test_player";
    let player = engine.create_player(player_id).unwrap();
    assert_eq!(player.id, player_id);
    assert!(engine.get_player(player_id).is_some());
    assert!(engine.get_player_state(player_id).is_some());
}

#[test]
fn test_animation_engine_load_animation_data() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    let anim_data = AnimationData::new("test_anim", "Test Animation");
    engine.load_animation_data(anim_data.clone()).unwrap();
    assert!(engine.get_animation_data("test_anim").is_some());
}

#[test]
fn test_animation_player_add_instance() {
    let mut player = AnimationPlayer::new("player1");
    let anim_instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("anim1").with_playback_mode(PlaybackMode::Loop),
        AnimationTime::new(10.0).unwrap(),
    );
    player.add_instance(anim_instance.clone()).unwrap();
    assert!(player.instances.contains_key("instance1"));
}

#[test]
fn test_animation_player_get_effective_time() {
    let anim_duration = AnimationTime::new(10.0).unwrap();
    let settings = InstanceSettings::new("anim1")
        .with_start_offset(AnimationTime::new(2.0).unwrap())
        .with_instance_start_time(AnimationTime::new(5.0).unwrap())
        .with_timescale(0.5);
    let instance = AnimationInstance::new("instance1", settings.clone(), anim_duration);

    // Player time 5.0s, instance starts at 5.0s, offset 2.0s, timescale 0.5
    // Relative time = 0.0s, scaled = 0.0s, effective = 0.0s + 2.0s = 2.0s
    assert_eq!(
        instance.get_effective_time(AnimationTime::new(5.0).unwrap()),
        AnimationTime::new(2.0).unwrap()
    );

    // Player time 7.0s, instance starts at 5.0s, offset 2.0s, timescale 0.5
    // Relative time = 2.0s, scaled = 1.0s, effective = 1.0s + 2.0s = 3.0s
    assert_eq!(
        instance.get_effective_time(AnimationTime::new(7.0).unwrap()),
        AnimationTime::new(3.0).unwrap()
    );

    // Player time 15.0s (well past end of animation data duration 10s + offset 2s = 12s)
    // Relative time = 10.0s, scaled = 5.0s, effective = 5.0s + 2.0s = 7.0s (clamped to 10s for Once mode)
    // For PlaybackMode::Once, it clamps to effective_duration.
    // Effective duration is 10s. Scaled time is 5s. Looped time is 5s. Add offset 2s. Result 7s.
    let instance_once = AnimationInstance::new(
        "instance_once",
        settings.clone().with_playback_mode(PlaybackMode::Once),
        anim_duration,
    );
    assert_eq!(
        instance_once.get_effective_time(AnimationTime::new(25.0).unwrap()),
        AnimationTime::new(12.0).unwrap()
    );

    // Test loop mode
    let instance_loop = AnimationInstance::new(
        "instance_loop",
        settings.clone().with_playback_mode(PlaybackMode::Loop),
        anim_duration,
    );
    // Player time 25.0s
    // Relative time = 20.0s, scaled = 10.0s.
    // Looped time (10.0 % 10.0) = 0.0s. Add offset 2.0s. Result 2.0s.
    assert_eq!(
        instance_loop.get_effective_time(AnimationTime::new(25.0).unwrap()),
        AnimationTime::new(2.0).unwrap()
    );

    // Test ping-pong mode
    let instance_pingpong = AnimationInstance::new(
        "instance_pingpong",
        settings.clone().with_playback_mode(PlaybackMode::PingPong),
        anim_duration,
    );
    // Player time 25.0s
    // Relative time = 20.0s, scaled = 10.0s.
    // Cycle duration = 20.0s. Cycle time = 10.0s % 20.0s = 10.0s.
    // time_in_half_cycle = cycle_duration - cycle_time = 20.0 - 10.0 = 10.0s.
    // Add offset 2.0s. Result 12.0s.
    assert_eq!(
        instance_pingpong.get_effective_time(AnimationTime::new(25.0).unwrap()),
        AnimationTime::new(12.0).unwrap()
    );
}

#[test]
fn test_animation_player_go_to() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    let player_id = "test_player_goto";
    engine.create_player(player_id).unwrap();

    let anim_data = AnimationData::new("anim1", "Test Animation");
    engine.load_animation_data(anim_data.clone()).unwrap();

    // Add instance to the player
    {
        let player = engine.get_player_mut(player_id).unwrap();
        let anim_instance = AnimationInstance::new(
            "instance1",
            InstanceSettings::new("anim1"),
            AnimationTime::new(10.0).unwrap(),
        );
        player.add_instance(anim_instance).unwrap();
    } // player mutable borrow ends here

    // Use the engine's seek_player method
    engine
        .seek_player(player_id, AnimationTime::new(5.0).unwrap())
        .unwrap();

    // Verify the player's time was set correctly
    let player = engine.get_player(player_id).unwrap();
    assert_eq!(player.current_time, AnimationTime::new(5.0).unwrap());
}

#[test]
fn test_animation_engine_update_playback() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    let player_id = "test_player_update";
    engine.create_player(player_id).unwrap();

    let anim_data = AnimationData::new("anim1", "Test Animation");
    engine.load_animation_data(anim_data.clone()).unwrap();

    let player = engine.get_player_mut(player_id).unwrap();
    let anim_instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("anim1").with_playback_mode(PlaybackMode::Loop),
        AnimationTime::new(10.0).unwrap(),
    );
    player.add_instance(anim_instance).unwrap();

    // Play the player
    engine.play_player(player_id).unwrap();
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );

    // Update engine by 1 second
    let values = engine.update(1.0).unwrap();
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(1.0).unwrap()
    );
    assert!(values.contains_key(player_id));
    assert!(values.get(player_id).unwrap().is_empty()); // Still empty as no tracks

    // Update until end
    engine.update(9.0).unwrap(); // Total 10 seconds
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(0.0).unwrap() // Should wrap around to 0.0
    );
    // Since default playback mode is Loop, it should wrap around
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );

    // Test with PlaybackMode::Once
    let player_id_once = "test_player_once";
    engine.create_player(player_id_once).unwrap();
    let player_state_once = engine.get_player_state_mut(player_id_once).unwrap();
    player_state_once.mode = PlaybackMode::Once;

    let player_once = engine.get_player_mut(player_id_once).unwrap();
    let anim_instance_once = AnimationInstance::new(
        "instance_once",
        InstanceSettings::new("anim1"),
        AnimationTime::new(5.0).unwrap(),
    );
    player_once.add_instance(anim_instance_once).unwrap();

    engine.play_player(player_id_once).unwrap();
    engine.update(6.0).unwrap(); // Exceeds 5s duration
    assert_eq!(
        engine.get_player(player_id_once).unwrap().current_time,
        AnimationTime::new(5.0).unwrap()
    );
    assert_eq!(
        engine
            .get_player_state(player_id_once)
            .unwrap()
            .playback_state,
        PlaybackState::Ended
    );
}

// Comprehensive playback mode tests

#[test]
fn test_playback_mode_once_forward() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    let player_id = "once_forward_player";
    engine.create_player(player_id).unwrap();

    let anim_data = AnimationData::new("test_anim", "Test Animation");
    engine.load_animation_data(anim_data).unwrap();

    // Set up instance with PlaybackMode::Once
    let player = engine.get_player_mut(player_id).unwrap();
    let anim_instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("test_anim").with_playback_mode(PlaybackMode::Once),
        AnimationTime::new(10.0).unwrap(),
    );
    player.add_instance(anim_instance).unwrap();
    let player_state_once = engine.get_player_state_mut(player_id).unwrap();
    player_state_once.mode = PlaybackMode::Once;
    // Start playback
    engine.play_player(player_id).unwrap();
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(0.0).unwrap()
    );

    // Update several times, approaching the end
    engine.update(3.0).unwrap();
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(3.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );

    engine.update(5.0).unwrap(); // Total: 8 seconds
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(8.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );

    // Cross the boundary
    engine.update(3.0).unwrap(); // Total: 11 seconds, exceeds duration
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(10.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Ended
    );

    // Further updates should not change time or state
    engine.update(2.0).unwrap();
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(10.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Ended
    );
}

#[test]
fn test_playback_mode_once_reverse() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    let player_id = "once_reverse_player";
    engine.create_player(player_id).unwrap();

    let anim_data = AnimationData::new("test_anim", "Test Animation");
    engine.load_animation_data(anim_data).unwrap();

    // Set up instance with PlaybackMode::Once
    let player = engine.get_player_mut(player_id).unwrap();
    let anim_instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("test_anim"),
        AnimationTime::new(10.0).unwrap(),
    );
    player.add_instance(anim_instance).unwrap();
    // Start at end and set negative speed
    engine
        .seek_player(player_id, AnimationTime::new(10.0).unwrap())
        .unwrap();
    let player_state = engine.get_player_state_mut(player_id).unwrap();
    player_state.speed = -1.0;
    player_state.playback_state = PlaybackState::Playing;
    player_state.mode = PlaybackMode::Once;

    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(10.0).unwrap()
    );
    assert_eq!(engine.get_player_state(player_id).unwrap().speed, -1.0);

    // Update several times, approaching the start
    engine.update(3.0).unwrap();
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(7.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );

    engine.update(5.0).unwrap(); // Total moved: 8 seconds back
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(2.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );

    // Cross the boundary
    engine.update(3.0).unwrap(); // Would go to -1.0, but should clamp and end
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(0.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Ended
    );
}

#[test]
fn test_playback_mode_loop_forward() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    let player_id = "loop_forward_player";
    engine.create_player(player_id).unwrap();

    let anim_data = AnimationData::new("test_anim", "Test Animation");
    engine.load_animation_data(anim_data).unwrap();

    // Set up instance with PlaybackMode::Loop
    let player = engine.get_player_mut(player_id).unwrap();
    let anim_instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("test_anim"),
        AnimationTime::new(10.0).unwrap(),
    );
    player.add_instance(anim_instance).unwrap();

    // Start playback
    engine.play_player(player_id).unwrap();
    let player_state_mut = engine.get_player_state_mut(player_id).unwrap();
    player_state_mut.mode = PlaybackMode::Loop;

    // Update to near the end
    engine.update(9.5).unwrap();
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(9.5).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );

    // Cross the boundary - should loop back to start
    engine.update(1.0).unwrap(); // Would be 10.5, should wrap to 0.5
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(0.5).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );

    // Continue and loop again
    engine.update(12.0).unwrap(); // Should wrap around again
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(2.5).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );
}

#[test]
fn test_playback_mode_loop_reverse() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    let player_id = "loop_reverse_player";
    engine.create_player(player_id).unwrap();

    let anim_data = AnimationData::new("test_anim", "Test Animation");
    engine.load_animation_data(anim_data).unwrap();

    // Set up instance with PlaybackMode::Loop
    let player = engine.get_player_mut(player_id).unwrap();
    let anim_instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("test_anim"),
        AnimationTime::new(10.0).unwrap(),
    );
    player.add_instance(anim_instance).unwrap();

    // Start near beginning with reverse speed
    engine
        .seek_player(player_id, AnimationTime::new(0.5).unwrap())
        .unwrap();
    let player_state_mut = engine.get_player_state_mut(player_id).unwrap();
    player_state_mut.mode = PlaybackMode::Loop;
    player_state_mut.speed = -1.0;
    player_state_mut.playback_state = PlaybackState::Playing;

    // Cross the boundary - should loop back to end
    engine.update(1.0).unwrap(); // Would be -0.5, should wrap to end
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(10.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );

    // Continue with another update to verify loop behavior
    engine.update(2.0).unwrap(); // Should go to 8.0 (10.0 - 2.0)
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(8.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );
}

#[test]
fn test_playback_mode_pingpong_forward_to_reverse() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    let player_id = "pingpong_player";
    engine.create_player(player_id).unwrap();

    let anim_data = AnimationData::new("test_anim", "Test Animation");
    engine.load_animation_data(anim_data).unwrap();

    // Set up instance with PlaybackMode::PingPong
    let player = engine.get_player_mut(player_id).unwrap();
    let anim_instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("test_anim"),
        AnimationTime::new(10.0).unwrap(),
    );
    player.add_instance(anim_instance).unwrap();
    let player_state_mut = engine.get_player_state_mut(player_id).unwrap();
    player_state_mut.mode = PlaybackMode::PingPong;
    // Start playback
    engine.play_player(player_id).unwrap();
    assert_eq!(engine.get_player_state(player_id).unwrap().speed, 1.0);

    // Update to near the end
    engine.update(9.5).unwrap();
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(9.5).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );
    assert_eq!(engine.get_player_state(player_id).unwrap().speed, 1.0);

    // Cross the boundary - should reverse direction
    engine.update(1.0).unwrap(); // Would be 10.5, should clamp to 10.0 and reverse speed
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(10.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );
    assert_eq!(engine.get_player_state(player_id).unwrap().speed, -1.0); // Speed should be reversed

    // Continue in reverse direction
    engine.update(3.0).unwrap();
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(7.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );
    assert_eq!(engine.get_player_state(player_id).unwrap().speed, -1.0);
}

#[test]
fn test_playback_mode_pingpong_reverse_to_forward() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    let player_id = "pingpong_reverse_player";
    engine.create_player(player_id).unwrap();

    let anim_data = AnimationData::new("test_anim", "Test Animation");
    engine.load_animation_data(anim_data).unwrap();

    // Set up instance with PlaybackMode::PingPong
    let player = engine.get_player_mut(player_id).unwrap();
    let anim_instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("test_anim"),
        AnimationTime::new(10.0).unwrap(),
    );
    player.add_instance(anim_instance).unwrap();

    // Start near beginning with reverse speed (simulating after first ping-pong)
    engine
        .seek_player(player_id, AnimationTime::new(0.5).unwrap())
        .unwrap();
    let player_state_mut = engine.get_player_state_mut(player_id).unwrap();
    player_state_mut.mode = PlaybackMode::PingPong;
    player_state_mut.speed = -1.0;
    player_state_mut.playback_state = PlaybackState::Playing;

    // Cross the boundary - should reverse direction back to forward
    engine.update(1.0).unwrap(); // Would be -0.5, should clamp to 0.0 and reverse speed
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(0.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );
    assert_eq!(engine.get_player_state(player_id).unwrap().speed, 1.0); // Speed should be reversed to positive

    // Continue in forward direction
    engine.update(3.0).unwrap();
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(3.0).unwrap()
    );
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );
    assert_eq!(engine.get_player_state(player_id).unwrap().speed, 1.0);
}

#[test]
fn test_playback_mode_pingpong_full_cycle() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    let player_id = "pingpong_full_cycle_player";
    engine.create_player(player_id).unwrap();

    let anim_data = AnimationData::new("test_anim", "Test Animation");
    engine.load_animation_data(anim_data).unwrap();

    // Set up instance with PlaybackMode::PingPong
    let player = engine.get_player_mut(player_id).unwrap();
    let anim_instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("test_anim"),
        AnimationTime::new(5.0).unwrap(), // Shorter duration for easier testing
    );
    player.add_instance(anim_instance).unwrap();
    let player_state_mut = engine.get_player_state_mut(player_id).unwrap();
    player_state_mut.mode = PlaybackMode::PingPong;
    // Start playback
    engine.play_player(player_id).unwrap();
    assert_eq!(engine.get_player_state(player_id).unwrap().speed, 1.0);

    // Go to end (forward phase)
    engine.update(6.0).unwrap(); // Exceeds 5.0, should hit end and reverse
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(5.0).unwrap()
    );
    assert_eq!(engine.get_player_state(player_id).unwrap().speed, -1.0);

    // Go back to start (reverse phase)
    engine.update(6.0).unwrap(); // Should hit start and reverse again
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(0.0).unwrap()
    );
    assert_eq!(engine.get_player_state(player_id).unwrap().speed, 1.0);

    // Verify state is still playing throughout
    assert_eq!(
        engine.get_player_state(player_id).unwrap().playback_state,
        PlaybackState::Playing
    );
}

#[test]
fn test_mixed_playback_speeds() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    let player_id = "speed_test_player";
    engine.create_player(player_id).unwrap();

    let anim_data = AnimationData::new("test_anim", "Test Animation");
    engine.load_animation_data(anim_data).unwrap();

    // Set up instance with Loop mode
    let player = engine.get_player_mut(player_id).unwrap();
    let anim_instance = AnimationInstance::new(
        "instance1",
        InstanceSettings::new("test_anim"),
        AnimationTime::new(10.0).unwrap(),
    );
    player.add_instance(anim_instance).unwrap();

    // Test 2x speed
    let player_state_mut = engine.get_player_state_mut(player_id).unwrap();
    player_state_mut.mode = PlaybackMode::Loop;
    player_state_mut.speed = 2.0;
    player_state_mut.playback_state = PlaybackState::Playing;

    engine.update(1.0).unwrap(); // Should advance 2 seconds
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(2.0).unwrap()
    );

    engine.update(4.5).unwrap(); // Should advance 9 more seconds, total 11, wrap to 1
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(1.0).unwrap()
    );

    // Test 0.5x speed
    let player_state = engine.get_player_state_mut(player_id).unwrap();
    player_state.speed = 0.5;

    engine.update(2.0).unwrap(); // Should advance 1 second
    assert_eq!(
        engine.get_player(player_id).unwrap().current_time,
        AnimationTime::new(2.0).unwrap()
    );
}
