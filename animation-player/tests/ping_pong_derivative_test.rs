use std::time::Duration;

use animation_player::{
    animation::{
        instance::PlaybackMode, AnimationInstance, AnimationInstanceSettings, AnimationKeypoint,
        AnimationTrack,
    },
    player::PlaybackState,
    value::Value,
    AnimationData, AnimationEngine, AnimationEngineConfig, AnimationTime,
};

// Helper to setup a player given animation data and settings
fn setup_player_for_animation(
    engine: &mut AnimationEngine,
    animation_data: AnimationData,
    custom_duration: impl Into<AnimationTime>,
    playback_mode: PlaybackMode,
) -> String {
    // Load animation data
    let animation_id = engine.load_animation_data(animation_data).unwrap();

    // Create player and instance
    let player_id = engine.create_player();
    let player_state = engine.get_player_settings_mut(&player_id).unwrap();
    player_state.mode = playback_mode;

    let animation_instance = AnimationInstance::new(
        animation_id,
        AnimationInstanceSettings::default(),
        custom_duration.into(),
    );

    let player = engine.get_player_mut(&player_id).unwrap();
    player.add_instance(animation_instance);

    player_id
}

#[test]
fn test_ping_pong_derivative_sign() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Create a simple animation with a float track
    let mut animation =
        AnimationData::new("test_ping_pong_derivative", "Ping-Pong Derivative Test");
    let mut track = AnimationTrack::new("value", "test.value");

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Float(0.0),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Float(10.0),
        ))
        .unwrap();

    animation.add_track(track);

    // Create player with PingPong mode
    let player_id = setup_player_for_animation(
        &mut engine,
        animation,
        Duration::from_secs(2),
        PlaybackMode::PingPong,
    );

    // --- Test Forward Playback ---
    engine.play_player(&player_id).unwrap();
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(1.0).unwrap())
        .unwrap();

    let derivatives_forward = engine
        .calculate_player_derivatives(&player_id, None)
        .unwrap();

    if let Some(Value::Float(derivative)) = derivatives_forward.get("test.value") {
        assert!(
            *derivative > 0.0,
            "Derivative should be positive during forward playback, got {}",
            derivative
        );
    } else {
        panic!("Expected float derivative value during forward playback");
    }

    // --- Test Backward Playback ---
    // Advance time to trigger ping-pong reversal
    engine.update(Duration::from_secs(2)).unwrap(); // Move to the end and reverse

    // Now at time 2.0, speed is -1.0. Let's check derivative at time 1.0 again.
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(1.0).unwrap())
        .unwrap();

    let player_state = engine.get_player_settings_mut(&player_id).unwrap();
    player_state.speed = -1.0;
    engine.get_player_properties_mut(&player_id).unwrap().playback_state = PlaybackState::Playing;

    let derivatives_backward = engine
        .calculate_player_derivatives(&player_id, None)
        .unwrap();

    if let Some(Value::Float(derivative)) = derivatives_backward.get("test.value") {
        assert!(
            *derivative < 0.0,
            "Derivative should be negative during backward playback, got {}",
            derivative
        );
    } else {
        panic!("Expected float derivative value during backward playback");
    }
}
