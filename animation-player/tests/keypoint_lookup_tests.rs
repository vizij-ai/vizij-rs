//! Tests for keypoint lookup and interpolation logic
//! These tests focus on the surrounding_keypoints() method and track value calculations

use animation_player::{
    value::Vector3, AnimationKeypoint, AnimationTime, AnimationTrack, InterpolationRegistry,
    TimeRange, Value,
};

/// Helper function to create a test track with float keypoints
fn create_test_float_track() -> AnimationTrack {
    let mut track = AnimationTrack::new("test_track", "test.value");
    track.settings = None; // Explicitly set settings to None

    // Add keypoints at times 0, 2, 4, 6
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(0.0),
            Value::Float(0.0),
        ))
        .unwrap();

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(2.0),
            Value::Float(10.0),
        ))
        .unwrap();

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(4.0),
            Value::Float(5.0),
        ))
        .unwrap();

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(6.0),
            Value::Float(15.0),
        ))
        .unwrap();

    track
}

/// Helper function to create a test track with Vector3 keypoints  
fn create_test_vector3_track() -> AnimationTrack {
    let mut track = AnimationTrack::new("position_track", "transform.position");
    track.settings = None; // Explicitly set settings to None

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(0.0),
            Value::Vector3(Vector3::new(0.0, 0.0, 0.0)),
        ))
        .unwrap();

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(2.0),
            Value::Vector3(Vector3::new(10.0, 5.0, 2.0)),
        ))
        .unwrap();

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(4.0),
            Value::Vector3(Vector3::new(0.0, 10.0, -5.0)),
        ))
        .unwrap();

    track
}

/// Test surrounding_keypoints with exact keypoint matches
#[test]
fn test_surrounding_keypoints_exact_matches() {
    let track = create_test_float_track();

    // Test exact matches for each keypoint
    let test_cases = vec![
        (0.0, false, true), // At first keypoint: prev=None, next=Some(self)
        (2.0, true, true),  // At middle keypoint: prev=Some, next=Some(self)
        (4.0, true, true),  // At another middle keypoint: prev=Some, next=Some(self)
        (6.0, true, true),  // At last keypoint: prev=Some, next=Some(self)
    ];

    for (time, should_have_prev, should_have_next) in test_cases {
        let result = track.surrounding_keypoints(AnimationTime::from(time));
        assert!(
            result.is_some(),
            "Should find surrounding keypoints for time {}",
            time
        );

        let (prev, next) = result.unwrap();

        if should_have_prev {
            assert!(
                prev.is_some(),
                "Should have previous keypoint at time {}",
                time
            );
        } else {
            assert!(
                prev.is_none(),
                "Should not have previous keypoint at time {}",
                time
            );
        }

        if should_have_next {
            assert!(next.is_some(), "Should have next keypoint at time {}", time);
        } else {
            assert!(
                next.is_none(),
                "Should not have next keypoint at time {}",
                time
            );
        }
    }
}

/// Test surrounding_keypoints between keypoints
#[test]
fn test_surrounding_keypoints_between_keypoints() {
    let track = create_test_float_track();

    let test_cases = vec![
        (1.0, 0.0, 2.0), // Between first and second keypoints
        (3.0, 2.0, 4.0), // Between second and third keypoints
        (5.0, 4.0, 6.0), // Between third and fourth keypoints
    ];

    for (time, expected_prev_time, expected_next_time) in test_cases {
        let result = track.surrounding_keypoints(AnimationTime::from(time));
        assert!(
            result.is_some(),
            "Should find surrounding keypoints for time {}",
            time
        );

        let (prev, next) = result.unwrap();
        assert!(
            prev.is_some(),
            "Should have previous keypoint at time {}",
            time
        );
        assert!(next.is_some(), "Should have next keypoint at time {}", time);

        let prev_kp = prev.unwrap();
        let next_kp = next.unwrap();

        assert!(
            (prev_kp.time.as_seconds() - expected_prev_time).abs() < 0.001,
            "Previous keypoint time should be {}, got {}",
            expected_prev_time,
            prev_kp.time.as_seconds()
        );
        assert!(
            (next_kp.time.as_seconds() - expected_next_time).abs() < 0.001,
            "Next keypoint time should be {}, got {}",
            expected_next_time,
            next_kp.time.as_seconds()
        );
    }
}

/// Test surrounding_keypoints outside track range
#[test]
fn test_surrounding_keypoints_outside_range() {
    let track = create_test_float_track();

    // Before first keypoint
    let result = track.surrounding_keypoints(AnimationTime::from(-1.0));
    assert!(result.is_some());
    let (prev, next) = result.unwrap();
    assert!(
        prev.is_none(),
        "Should not have previous keypoint before track start"
    );
    assert!(
        next.is_some(),
        "Should have next keypoint before track start"
    );
    assert!((next.unwrap().time.as_seconds() - 0.0).abs() < 0.001);

    // After last keypoint
    let result = track.surrounding_keypoints(AnimationTime::from(7.0));
    assert!(result.is_some());
    let (prev, next) = result.unwrap();
    assert!(
        prev.is_some(),
        "Should have previous keypoint after track end"
    );
    assert!(
        next.is_none(),
        "Should not have next keypoint after track end"
    );
    assert!((prev.unwrap().time.as_seconds() - 6.0).abs() < 0.001);
}

/// Test surrounding_keypoints with empty track
#[test]
fn test_surrounding_keypoints_empty_track() {
    let mut track = AnimationTrack::new("empty_track", "test.empty");
    track.settings = None; // Explicitly set settings to None

    let result = track.surrounding_keypoints(AnimationTime::from(1.0));
    assert!(
        result.is_none(),
        "Empty track should return None for surrounding keypoints"
    );
}

/// Test surrounding_keypoints with single keypoint
#[test]
fn test_surrounding_keypoints_single_keypoint() {
    let mut track = AnimationTrack::new("single_track", "test.single");
    track.settings = None; // Explicitly set settings to None
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(2.0),
            Value::Float(5.0),
        ))
        .unwrap();

    // Before the keypoint
    let result = track.surrounding_keypoints(AnimationTime::from(1.0));
    assert!(result.is_some());
    let (prev, next) = result.unwrap();
    assert!(prev.is_none());
    assert!(next.is_some());
    assert!((next.unwrap().time.as_seconds() - 2.0).abs() < 0.001);

    // At the keypoint
    let result = track.surrounding_keypoints(AnimationTime::from(2.0));
    assert!(result.is_some());
    let (prev, next) = result.unwrap();
    assert!(prev.is_none());
    assert!(next.is_some());
    assert!((next.unwrap().time.as_seconds() - 2.0).abs() < 0.001);

    // After the keypoint
    let result = track.surrounding_keypoints(AnimationTime::from(3.0));
    assert!(result.is_some());
    let (prev, next) = result.unwrap();
    assert!(prev.is_some());
    assert!(next.is_none());
    assert!((prev.unwrap().time.as_seconds() - 2.0).abs() < 0.001);
}

/// Test value_at_time with float interpolation
#[test]
fn test_value_at_time_float_interpolation() {
    let track = create_test_float_track();

    let test_cases = vec![
        (0.0, 0.0),  // At first keypoint
        (1.0, 5.0),  // Halfway between 0.0 and 10.0 at times 0 and 2
        (2.0, 10.0), // At second keypoint
        (3.0, 7.5),  // Halfway between 10.0 and 5.0 at times 2 and 4
        (4.0, 5.0),  // At third keypoint
        (5.0, 10.0), // Halfway between 5.0 and 15.0 at times 4 and 6
        (6.0, 15.0), // At fourth keypoint
    ];
    let mut interpolation_registry = InterpolationRegistry::default();
    for (time, expected_value) in test_cases {
        let value =
            track.value_at_time(AnimationTime::from(time), &mut interpolation_registry, None);
        assert!(value.is_some(), "Should get value at time {}", time);

        if let Value::Float(actual_value) = value.unwrap() {
            assert!(
                (actual_value - expected_value).abs() < 0.001,
                "At time {}, expected {}, got {}",
                time,
                expected_value,
                actual_value
            );
        } else {
            panic!("Expected Float value at time {}", time);
        }
    }
}

/// Test value_at_time with Vector3 interpolation
#[test]
fn test_value_at_time_vector3_interpolation() {
    let track = create_test_vector3_track();
    let mut interpolation_registry = InterpolationRegistry::default();

    // Test at midpoint between first and second keypoints (time 1.0)
    let value = track.value_at_time(AnimationTime::from(1.0), &mut interpolation_registry, None);
    assert!(value.is_some());

    if let Value::Vector3(vec) = value.unwrap() {
        // Should be halfway between (0,0,0) and (10,5,2)
        assert!(
            (vec.x - 5.0).abs() < 0.001,
            "X component should be 5.0, got {}",
            vec.x
        );
        assert!(
            (vec.y - 2.5).abs() < 0.001,
            "Y component should be 2.5, got {}",
            vec.y
        );
        assert!(
            (vec.z - 1.0).abs() < 0.001,
            "Z component should be 1.0, got {}",
            vec.z
        );
    } else {
        panic!("Expected Vector3 value");
    }
}

/// Test value_at_time outside track range
#[test]
fn test_value_at_time_outside_range() {
    let track = create_test_float_track();
    let mut interpolation_registry = InterpolationRegistry::default();

    // Before first keypoint - should return first keypoint value
    let value = track.value_at_time(AnimationTime::from(-1.0), &mut interpolation_registry, None);
    assert!(value.is_some());
    if let Value::Float(val) = value.unwrap() {
        assert!(
            (val - 0.0).abs() < 0.001,
            "Before range should return first keypoint value"
        );
    }

    // After last keypoint - should return last keypoint value
    let value = track.value_at_time(AnimationTime::from(7.0), &mut interpolation_registry, None);
    assert!(value.is_some());
    if let Value::Float(val) = value.unwrap() {
        assert!(
            (val - 15.0).abs() < 0.001,
            "After range should return last keypoint value"
        );
    }
}

/// Test value_at_time with empty track
#[test]
fn test_value_at_time_empty_track() {
    let track = AnimationTrack::new("empty_track", "test.empty");
    let mut interpolation_registry = InterpolationRegistry::default();

    let value = track.value_at_time(AnimationTime::from(1.0), &mut interpolation_registry, None);
    assert!(
        value.is_none(),
        "Empty track should return None for value_at_time"
    );
}

/// Test value_at_time with disabled track
#[test]
fn test_value_at_time_disabled_track() {
    let mut track = create_test_float_track();
    track.set_enabled(false);
    let mut interpolation_registry = InterpolationRegistry::default();

    // Even though track has keypoints, it's disabled so value_at_time might still work
    // This tests the basic interpolation logic regardless of enabled state
    let value = track.value_at_time(AnimationTime::from(1.0), &mut interpolation_registry, None);
    assert!(
        value.is_some(),
        "Disabled track should still interpolate values"
    );
}

/// Test performance of keypoint lookup with many keypoints
#[test]
fn test_keypoint_lookup_performance() {
    let mut track = AnimationTrack::new("perf_track", "test.performance");
    track.settings = None; // Explicitly set settings to None

    // Add 1000 keypoints
    for i in 0..1000 {
        track
            .add_keypoint(AnimationKeypoint::new(
                AnimationTime::from(i as f64),
                Value::Float(i as f64),
            ))
            .unwrap();
    }

    // Test lookup performance
    let start = std::time::Instant::now();
    for i in 0..1000 {
        let time = i as f64 + 0.5; // Between keypoints
        let _result = track.surrounding_keypoints(AnimationTime::from(time));
    }
    let elapsed = start.elapsed();

    // Binary search should be fast even with 1000 keypoints
    assert!(
        elapsed.as_millis() < 10,
        "1000 keypoint lookups took too long: {}ms",
        elapsed.as_millis()
    );
}

/// Test binary search correctness with many keypoints
#[test]
fn test_binary_search_correctness() {
    let mut track = AnimationTrack::new("binary_track", "test.binary");
    track.settings = None; // Explicitly set settings to None

    // Add keypoints at even numbers: 0, 2, 4, 6, ..., 98
    for i in 0..50 {
        track
            .add_keypoint(AnimationKeypoint::new(
                AnimationTime::from((i * 2) as f64),
                Value::Float(i as f64),
            ))
            .unwrap();
    }

    // Test lookups at odd numbers (between keypoints)
    for i in 0..49 {
        let time = (i * 2 + 1) as f64; // 1, 3, 5, 7, ..., 97
        let result = track.surrounding_keypoints(AnimationTime::from(time));
        assert!(
            result.is_some(),
            "Should find keypoints around time {}",
            time
        );

        let (prev, next) = result.unwrap();
        assert!(
            prev.is_some(),
            "Should have previous keypoint at time {}",
            time
        );
        assert!(next.is_some(), "Should have next keypoint at time {}", time);

        let prev_time = prev.unwrap().time.as_seconds();
        let next_time = next.unwrap().time.as_seconds();

        // Verify the surrounding keypoints are correct
        let expected_prev = (i * 2) as f64;
        let expected_next = ((i + 1) * 2) as f64;

        assert!(
            (prev_time - expected_prev).abs() < 0.001,
            "At time {}, expected prev {}, got {}",
            time,
            expected_prev,
            prev_time
        );
        assert!(
            (next_time - expected_next).abs() < 0.001,
            "At time {}, expected next {}, got {}",
            time,
            expected_next,
            next_time
        );
    }
}

/// Test edge case: very close time values
#[test]
fn test_very_close_time_values() {
    let mut track = AnimationTrack::new("close_track", "test.close");
    track.settings = None; // Explicitly set settings to None

    // Add keypoints very close together
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(1.0),
            Value::Float(10.0),
        ))
        .unwrap();

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(1.001), // 1ms later
            Value::Float(20.0),
        ))
        .unwrap();

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from(1.002), // Another 1ms later
            Value::Float(30.0),
        ))
        .unwrap();

    // Test lookup between very close keypoints
    let result = track.surrounding_keypoints(AnimationTime::from(1.0005));
    assert!(result.is_some());

    let (prev, next) = result.unwrap();
    assert!(prev.is_some());
    assert!(next.is_some());

    assert!((prev.unwrap().time.as_seconds() - 1.0).abs() < 0.0001);
    assert!((next.unwrap().time.as_seconds() - 1.001).abs() < 0.0001);
}

/// Test interpolation at keypoint boundaries
#[test]
fn test_interpolation_at_boundaries() {
    let track = create_test_float_track();
    let mut interpolation_registry = InterpolationRegistry::default();

    // Test interpolation very close to keypoints
    let epsilon = 0.0001;

    // Just before second keypoint (time 2.0)
    let value = track.value_at_time(
        AnimationTime::from(2.0 - epsilon),
        &mut interpolation_registry,
        None,
    );
    assert!(value.is_some());
    if let Value::Float(val) = value.unwrap() {
        // Should be very close to 10.0 but slightly less due to interpolation
        assert!(
            val < 10.0 && val > 9.9,
            "Value just before keypoint should be close to keypoint value, got {}",
            val
        );
    }

    // Just after second keypoint
    let value = track.value_at_time(
        AnimationTime::from(2.0 + epsilon),
        &mut interpolation_registry,
        None,
    );
    assert!(value.is_some());
    if let Value::Float(val) = value.unwrap() {
        // Should be very close to 10.0 but slightly different due to interpolation towards next keypoint
        assert!(
            val < 10.0 && val > 9.9,
            "Value just after keypoint should be close to keypoint value, got {}",
            val
        );
    }
}

/// Test track time range calculation
#[test]
fn test_track_time_range() {
    let track = create_test_float_track();

    let range = track.time_range();
    assert!(
        range.is_some(),
        "Track with keypoints should have time range"
    );

    let range = range.unwrap();
    assert!(
        (range.start.as_seconds() - 0.0).abs() < 0.001,
        "Range should start at 0.0"
    );
    assert!(
        (range.end.as_seconds() - 6.0).abs() < 0.001,
        "Range should end at 6.0"
    );
    assert!(
        (range.duration().as_seconds() - 6.0).abs() < 0.001,
        "Range duration should be 6.0"
    );
}

/// Test keypoints in range query
#[test]
fn test_keypoints_in_range() {
    let track = create_test_float_track();

    // Create a range that includes middle keypoints
    let range = TimeRange::new(AnimationTime::from(1.0), AnimationTime::from(5.0)).unwrap();

    let keypoints_in_range = track.keypoints_in_range(&range);

    // Should include keypoints at times 2.0 and 4.0
    assert_eq!(
        keypoints_in_range.len(),
        2,
        "Should find 2 keypoints in range"
    );
    assert!((keypoints_in_range[0].time.as_seconds() - 2.0).abs() < 0.001);
    assert!((keypoints_in_range[1].time.as_seconds() - 4.0).abs() < 0.001);
}
