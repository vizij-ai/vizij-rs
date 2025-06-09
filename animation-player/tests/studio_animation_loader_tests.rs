//! Tests for loading and playing test_animation.json format

use std::time::Duration;

use animation_player::{
    AnimationData, AnimationEngine, AnimationEngineConfig, AnimationKeypoint, AnimationTime,
    AnimationTrack, Value,
};
use serde_json;

#[derive(serde::Deserialize)]
struct StudioAnimationPoint {
    #[allow(dead_code)]
    id: String,
    stamp: f64,
    value: f64,
    #[serde(rename = "trackId")]
    #[allow(dead_code)]
    track_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct StudioAnimationTrack {
    #[allow(dead_code)]
    id: String,
    name: String,
    points: Vec<StudioAnimationPoint>,
    #[serde(rename = "animatableId")]
    animatable_id: String,
}

#[derive(serde::Deserialize)]
struct StudioAnimationData {
    id: String,
    name: String,
    tracks: Vec<StudioAnimationTrack>,
    #[serde(default)]
    #[allow(dead_code)]
    groups: serde_json::Value,
    #[serde(default)]
    #[allow(dead_code)]
    transitions: serde_json::Value,
    duration: u64,
}

/// Convert test animation format to internal AnimationData
fn convert_test_animation(test_data: StudioAnimationData) -> AnimationData {
    let mut animation = AnimationData::new(&test_data.id, &test_data.name);
    let duration_seconds = test_data.duration as f64 / 1000.0;

    for track_data in test_data.tracks {
        let mut track = AnimationTrack::new(&track_data.name, &track_data.animatable_id);

        for point in track_data.points {
            // Convert stamp (0.0-1.0) to time in seconds
            let time_seconds = point.stamp * duration_seconds;
            let time = AnimationTime::from_seconds(time_seconds).unwrap();
            let keypoint = AnimationKeypoint::new(time, Value::Float(point.value));
            track.add_keypoint(keypoint).unwrap();
        }

        animation.add_track(track);
    }

    // Set the duration
    animation.metadata.duration = AnimationTime::from_seconds(duration_seconds).unwrap();
    animation
}

/// Load test animation from JSON string
fn load_test_animation_from_json(
    json_str: &str,
) -> Result<AnimationData, Box<dyn std::error::Error>> {
    let test_data: StudioAnimationData = serde_json::from_str(json_str)?;
    Ok(convert_test_animation(test_data))
}

#[test]
fn test_load_test_animation_json() {
    let json_content = include_str!("../test_animation.json");

    let animation = load_test_animation_from_json(json_content)
        .expect("Should load test animation successfully");

    // Verify basic properties
    assert_eq!(animation.id, "e6dfc5cf-72af-45b2-9533-3d1f54290e8d");
    assert_eq!(animation.name, "Waking New Quori");
    assert_eq!(
        animation.metadata.duration,
        AnimationTime::from_seconds(5.0).unwrap()
    );

    // Verify tracks
    assert_eq!(animation.tracks.len(), 4);

    // Check specific tracks exist
    let track_names: Vec<&str> = animation.tracks.values().map(|t| t.name.as_str()).collect();
    assert!(track_names.contains(&"twist_joint"));
    assert!(track_names.contains(&"neck_joint"));
    assert!(track_names.contains(&"pan_joint"));
    assert!(track_names.contains(&"tilt_joint"));
}

#[test]
fn test_stamp_to_time_conversion() {
    let json_content = include_str!("../test_animation.json");
    let animation = load_test_animation_from_json(json_content).unwrap();

    // Find the neck_joint track
    let neck_track = animation
        .tracks
        .values()
        .find(|t| t.name == "neck_joint")
        .expect("Should find neck_joint track");

    // Verify keypoint times are correctly converted
    let keypoints = &neck_track.keypoints;
    assert_eq!(keypoints.len(), 3);

    // stamp: 0 -> time: 0.0s
    assert_eq!(keypoints[0].time, AnimationTime::from_seconds(0.0).unwrap());

    // stamp: 0.5 -> time: 2.5s (0.5 * 5.0)
    assert_eq!(keypoints[1].time, AnimationTime::from_seconds(2.5).unwrap());

    // stamp: 0.8333333333333334 -> time: ~4.167s
    assert!((keypoints[2].time.as_seconds() - 4.1666666666666670).abs() < 0.001);

    // Verify values
    if let Value::Float(val) = keypoints[0].value {
        assert_eq!(val, -0.125);
    }
    if let Value::Float(val) = keypoints[1].value {
        assert!((val - (-0.03133984463882411)).abs() < 0.0001);
    }
    if let Value::Float(val) = keypoints[2].value {
        assert!((val - 0.02082824057149351).abs() < 0.0001);
    }
}

fn setup_engine_and_player() -> (AnimationEngine, String) {
    let json_content = include_str!("../test_animation.json");
    let animation = load_test_animation_from_json(json_content).unwrap();
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Load animation
    let animation_id = engine.load_animation_data(animation.clone()).unwrap();

    // Create player
    let player_id = engine.create_player();

    engine
        .add_animation_to_player(&player_id, &animation_id, None)
        .unwrap();

    (engine, player_id)
}

#[test]
fn test_animation_playback_in_engine() {
    let (mut engine, player_id) = setup_engine_and_player();

    // Test playback at different timestamps
    let test_times = vec![0.0, 1.25, 2.5, 4.1666, 5.0]; // Key timestamps

    for time in test_times {
        engine
            .seek_player(&player_id, AnimationTime::from_seconds(time).unwrap())
            .unwrap();
        let result = engine.update(Duration::from_secs(0)).unwrap(); // Update without advancing time

        assert!(result.contains_key(&player_id));
        let player_values = &result[&player_id];

        // Should have values for all 4 tracks
        assert!(
            player_values.len() >= 4,
            "Should have values for all tracks at time {}",
            time
        );

        // Verify specific tracks have values
        assert!(player_values.contains_key("81e09645-89b9-4c3b-bdbd-91561e093ad4")); // twist_joint
        assert!(player_values.contains_key("e130bd45-3731-40d9-b61f-a9970e0d5842")); // neck_joint
        assert!(player_values.contains_key("032babc5-8858-4fe3-9060-7d32a4aa1b9f")); // pan_joint
        assert!(player_values.contains_key("cd5e7147-8381-49aa-9f43-c9b420e34053"));
        // tilt_joint
    }
}

#[test]
fn test_specific_value_interpolation() {
    let (mut engine, player_id) = setup_engine_and_player();

    // Test neck_joint interpolation at halfway point (stamp 0.25 -> time 1.25s)
    // Should interpolate between values -0.125 and -0.03133984463882411
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(1.25).unwrap())
        .unwrap();
    let result = engine.update(Duration::from_secs(0)).unwrap();

    let player_values = &result[&player_id];
    let neck_joint_id = "e130bd45-3731-40d9-b61f-a9970e0d5842";

    if let Some(Value::Float(interpolated_value)) = player_values.get(neck_joint_id) {
        // Should be approximately halfway between -0.125 and -0.03133984463882411
        let expected_mid = (-0.125 + (-0.03133984463882411)) / 2.0;
        assert!(
            (interpolated_value - expected_mid).abs() < 0.01,
            "Interpolated value {} should be close to expected {}",
            interpolated_value,
            expected_mid
        );
    } else {
        panic!("Should have interpolated value for neck_joint");
    }
}

#[test]
fn test_pan_joint_multiple_keypoints() {
    let json_content = include_str!("../test_animation.json");
    let animation = load_test_animation_from_json(json_content).unwrap();

    // Find pan_joint track - it has 5 keypoints
    let pan_track = animation
        .tracks
        .values()
        .find(|t| t.name == "pan_joint")
        .expect("Should find pan_joint track");

    assert_eq!(pan_track.keypoints.len(), 5);

    // Verify all keypoints were converted correctly
    let expected_stamps = vec![0.0, 0.3, 0.5, 0.6583333333333333, 0.8333333333333334];
    let expected_values = vec![
        -0.9169333932954267,
        0.28917892343844265,
        -1.3212424262563287,
        0.9437563333453869,
        0.09779342072773334,
    ];

    for (i, (expected_stamp, expected_value)) in expected_stamps
        .iter()
        .zip(expected_values.iter())
        .enumerate()
    {
        let expected_time = expected_stamp * 5.0; // duration is 5 seconds
        assert!((pan_track.keypoints[i].time.as_seconds() - expected_time).abs() < 0.001);

        if let Value::Float(actual_value) = pan_track.keypoints[i].value {
            assert!((actual_value - expected_value).abs() < 0.0001);
        }
    }
}

#[test]
fn test_edge_case_values() {
    let (mut engine, player_id) = setup_engine_and_player();

    // Test at exact start (time 0.0)
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(0.0).unwrap())
        .unwrap();
    let result = engine.update(Duration::from_secs(0)).unwrap();
    let player_values = &result[&player_id];

    // Should have exact starting values
    let twist_joint_id = "81e09645-89b9-4c3b-bdbd-91561e093ad4";
    if let Some(Value::Float(value)) = player_values.get(twist_joint_id) {
        assert!((value - 3.3370387617425767).abs() < 0.0001);
    }

    // Test at exact end (time 5.0)
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(5.0).unwrap())
        .unwrap();
    let result = engine.update(Duration::from_secs(0)).unwrap();
    let player_values = &result[&player_id];

    // Should have final values from last keypoints
    let neck_joint_id = "e130bd45-3731-40d9-b61f-a9970e0d5842";
    if let Some(Value::Float(value)) = player_values.get(neck_joint_id) {
        assert!((value - 0.02082824057149351).abs() < 0.0001);
    }
}

#[test]
fn test_animation_loop_playback() {
    let (mut engine, player_id) = setup_engine_and_player();

    // Enable looping
    let player_state = engine.get_player_state_mut(&player_id).unwrap();
    player_state.mode = animation_player::animation::PlaybackMode::Loop;

    engine.play_player(&player_id).unwrap();

    // Update past the animation duration to test looping
    engine.update(Duration::from_secs(6)).unwrap(); // 6 seconds, should loop back to 1 second

    let player = engine.get_player(&player_id).unwrap();
    assert!((player.current_time.as_seconds() - 1.0).abs() < 0.1); // Should have looped
}
