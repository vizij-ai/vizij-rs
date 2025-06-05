//! Tests for animation baking functionality

use animation_player::{
    baking::{AnimationBaking, BakingConfig},
    value::Vector3,
    AnimationData, AnimationKeypoint, AnimationTime, AnimationTrack, InterpolationRegistry,
    TimeRange, Value,
};

#[test]
fn test_basic_baking() {
    // Create a simple animation with a position track
    let mut animation = AnimationData::new("test", "Test Animation");

    let mut position_track = AnimationTrack::new("position", "transform.position");
    position_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Vector3(Vector3::new(0.0, 0.0, 0.0)),
        ))
        .unwrap();
    position_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Vector3(Vector3::new(10.0, 5.0, 0.0)),
        ))
        .unwrap();
    position_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Vector3(Vector3::new(20.0, 0.0, 0.0)),
        ))
        .unwrap();

    animation.add_track(position_track);

    // Configure baking
    let config = BakingConfig::new(30.0); // 30 FPS
    let mut interpolation_registry = InterpolationRegistry::default();

    // Bake the animation
    let baked = animation
        .bake(&config, &mut interpolation_registry)
        .expect("Baking should succeed");

    // Verify the baked data
    assert_eq!(baked.frame_rate, 30.0);
    assert_eq!(baked.duration, AnimationTime::from_seconds(2.0).unwrap());
    assert_eq!(baked.frame_count, 61); // 2 seconds * 30 FPS + 1 = 61 frames

    // Check that we have the position track
    assert!(baked.tracks.contains_key("transform.position"));
    let position_data = &baked.tracks["transform.position"];
    assert_eq!(position_data.len(), 61);

    // 2 seconds, 30 fps,

    // Verify first frame
    if let Value::Vector3(first_pos) = &position_data[0].1 {
        assert!((first_pos.x - 0.0).abs() < 0.001);
        assert!((first_pos.y - 0.0).abs() < 0.001);
        assert!((first_pos.z - 0.0).abs() < 0.001);
    } else {
        panic!("Expected Vector3 value");
    }

    // Verify middle frame (at 1 second, frame 30)
    if let Value::Vector3(mid_pos) = &position_data[30].1 {
        assert!((mid_pos.x - 10.0).abs() < 0.001);
        assert!((mid_pos.y - 5.0).abs() < 0.001);
        assert!((mid_pos.z - 0.0).abs() < 0.001);
    } else {
        panic!("Expected Vector3 value");
    }

    // Verify last frame
    if let Value::Vector3(last_pos) = &position_data[60].1 {
        assert!((last_pos.x - 20.0).abs() < 0.001);
        assert!((last_pos.y - 0.0).abs() < 0.001);
        assert!((last_pos.z - 0.0).abs() < 0.001);
    } else {
        panic!("Expected Vector3 value");
    }
}

#[test]
fn test_baking_with_custom_time_range() {
    // Create a simple animation
    let mut animation = AnimationData::new("test", "Test Animation");

    let mut float_track = AnimationTrack::new("intensity", "light.intensity");
    float_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Float(0.0),
        ))
        .unwrap();
    float_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Float(1.0),
        ))
        .unwrap();

    animation.add_track(float_track);

    // Configure baking with custom time range (bake only the second half)
    let time_range = TimeRange::new(
        AnimationTime::from_seconds(1.0).unwrap(),
        AnimationTime::from_seconds(2.0).unwrap(),
    )
    .unwrap();

    let config = BakingConfig::new(10.0).with_time_range(time_range);
    let mut interpolation_registry = InterpolationRegistry::default();

    // Bake the animation
    let baked = animation
        .bake(&config, &mut interpolation_registry)
        .expect("Baking should succeed");

    // Verify the baked data
    assert_eq!(baked.frame_rate, 10.0);
    assert_eq!(baked.duration, AnimationTime::from_seconds(1.0).unwrap()); // Only 1 second duration
    assert_eq!(baked.frame_count, 11); // 1 second * 10 FPS + 1 = 11 frames

    // Check first and last values
    let intensity_data = &baked.tracks["light.intensity"];
    if let Value::Float(first_val) = intensity_data[0].1 {
        assert!((first_val - 0.5).abs() < 0.001); // At t=1.0, should be 0.5
    }
    if let Value::Float(last_val) = intensity_data[10].1 {
        assert!((last_val - 1.0).abs() < 0.001); // At t=2.0, should be 1.0
    }
}

#[test]
fn test_baking_multiple_tracks() {
    // Create an animation with multiple tracks
    let mut animation = AnimationData::new("test", "Multi-track Animation");

    // Position track
    let mut position_track = AnimationTrack::new("position", "transform.position");
    position_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Vector3(Vector3::new(0.0, 0.0, 0.0)),
        ))
        .unwrap();
    position_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Vector3(Vector3::new(1.0, 1.0, 1.0)),
        ))
        .unwrap();
    animation.add_track(position_track);

    // Scale track
    let mut scale_track = AnimationTrack::new("scale", "transform.scale");
    scale_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Vector3(Vector3::new(1.0, 1.0, 1.0)),
        ))
        .unwrap();
    scale_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Vector3(Vector3::new(2.0, 2.0, 2.0)),
        ))
        .unwrap();
    animation.add_track(scale_track);

    // Intensity track
    let mut intensity_track = AnimationTrack::new("intensity", "light.intensity");
    intensity_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Float(0.5),
        ))
        .unwrap();
    intensity_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Float(1.5),
        ))
        .unwrap();
    animation.add_track(intensity_track);

    // Configure baking
    let config = BakingConfig::new(60.0);
    let mut interpolation_registry = InterpolationRegistry::default();

    // Bake the animation
    let baked = animation
        .bake(&config, &mut interpolation_registry)
        .expect("Baking should succeed");

    // Verify we have all tracks
    assert_eq!(baked.tracks.len(), 3);
    assert!(baked.tracks.contains_key("transform.position"));
    assert!(baked.tracks.contains_key("transform.scale"));
    assert!(baked.tracks.contains_key("light.intensity"));

    // Verify all tracks have the same number of frames
    let frame_count = baked.frame_count;
    for (track_name, data) in &baked.tracks {
        assert_eq!(
            data.len(),
            frame_count,
            "Track {} has wrong frame count",
            track_name
        );
    }
}

#[test]
fn test_baking_statistics() {
    // Create a simple animation
    let mut animation = AnimationData::new("test", "Test Animation");

    let mut track = AnimationTrack::new("test", "test.value");
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Float(0.0),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Float(1.0),
        ))
        .unwrap();
    animation.add_track(track);

    // Configure baking
    let config = BakingConfig::new(30.0);
    let mut interpolation_registry = InterpolationRegistry::default();

    // Bake the animation
    let baked = animation
        .bake(&config, &mut interpolation_registry)
        .expect("Baking should succeed");

    // Get statistics
    let stats = baked.get_statistics();

    assert_eq!(stats.track_count, 1);
    assert_eq!(stats.frame_count, 61);
    assert_eq!(stats.frame_rate, 30.0);
    assert_eq!(stats.duration_seconds, 2.0);
    assert!(stats.memory_estimate_bytes > 0);
}

#[test]
fn test_baking_empty_animation() {
    // Create an empty animation
    let animation = AnimationData::new("empty", "Empty Animation");

    // Configure baking
    let config = BakingConfig::new(60.0);
    let mut interpolation_registry = InterpolationRegistry::default();

    // Bake the animation
    let baked = animation
        .bake(&config, &mut interpolation_registry)
        .expect("Baking should succeed even for empty animation");

    // Verify the baked data
    assert_eq!(baked.tracks.len(), 0);
    assert_eq!(baked.frame_count, 1); // Should have at least one frame
    assert_eq!(baked.duration, AnimationTime::zero());
}

#[test]
fn test_baking_config_validation() {
    // Test invalid frame rate
    let config = BakingConfig::new(0.0);
    let mut animation = AnimationData::new("test", "Test");
    let mut track = AnimationTrack::new("test", "test");
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Float(1.0),
        ))
        .unwrap();
    animation.add_track(track);

    let mut interpolation_registry = InterpolationRegistry::default();
    let result = animation.bake(&config, &mut interpolation_registry);
    assert!(result.is_err());
}

#[test]
fn test_baking_disabled_tracks() {
    // Create an animation with enabled and disabled tracks
    let mut animation = AnimationData::new("test", "Test Animation");

    // Enabled track
    let mut enabled_track = AnimationTrack::new("enabled", "enabled.value");
    enabled_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Float(1.0),
        ))
        .unwrap();
    enabled_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Float(2.0),
        ))
        .unwrap();
    animation.add_track(enabled_track);

    // Disabled track
    let mut disabled_track = AnimationTrack::new("disabled", "disabled.value");
    disabled_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Float(10.0),
        ))
        .unwrap();
    disabled_track.set_enabled(false);
    animation.add_track(disabled_track);

    // Configure baking
    let config = BakingConfig::new(10.0);
    let mut interpolation_registry = InterpolationRegistry::default();

    // Bake the animation
    let baked = animation
        .bake(&config, &mut interpolation_registry)
        .expect("Baking should succeed");

    // Verify only enabled track is baked
    assert_eq!(baked.tracks.len(), 1);
    assert!(baked.tracks.contains_key("enabled.value"));
    assert!(!baked.tracks.contains_key("disabled.value"));
}
