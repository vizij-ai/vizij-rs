//! Tests for numerical differentiation functionality

use std::time::Duration;

use animation_player::{
    animation::{Animation, AnimationKeypoint, AnimationSettings, AnimationTrack},
    value::euler::Euler, // Corrected import for Euler
    value::{Color, Transform, Vector3, Vector4},
    AnimationData,
    AnimationEngine,
    AnimationEngineConfig,
    AnimationTime,
    Value,
};

// Helper to setup a player given animation data and settings
fn setup_player_for_animation(
    engine: &mut AnimationEngine,
    animation_data: AnimationData,
    custom_duration: impl Into<AnimationTime>,
) -> String {
    // Load animation data
    let animation_id = engine.load_animation_data(animation_data).unwrap();

    // Create player and instance
    let player_id = engine.create_player();

    let animation_instance = Animation::new(
        animation_id,
        AnimationSettings::default(),
        custom_duration.into(),
    );

    let player = engine.get_player_mut(&player_id).unwrap();
    player.add_instance(animation_instance);

    player_id
}

#[test]
fn test_float_derivative_calculation() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Create animation with float track
    let mut animation = AnimationData::new("test_float_derivative", "Float derivative test");
    let mut track = AnimationTrack::new("position_x", "transform.position.x");

    // Add keypoints: linear increase from 0 to 10 over 2 seconds
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

    // Create player and instance
    let player_id = setup_player_for_animation(&mut engine, animation, Duration::from_secs(2));

    // Set time to middle of animation
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(1.0).unwrap())
        .unwrap();

    // Calculate derivatives
    let derivatives = engine
        .calculate_player_derivatives(&player_id, None)
        .unwrap();

    // For linear motion from 0 to 10 over 2 seconds, derivative should be ~5.0 units/second
    if let Some(Value::Float(derivative)) = derivatives.get("transform.position.x") {
        assert!(
            (*derivative - 5.0).abs() < 3.0,
            "Expected derivative ~5.0, got {}",
            derivative
        );
    } else {
        panic!("Expected float derivative value");
    }
}

#[test]
fn test_vector3_derivative_calculation() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Create animation with Vector3 track
    let mut animation = AnimationData::new("test_vector3_derivative", "Vector3 derivative test");
    let mut track = AnimationTrack::new("position", "transform.position");

    // Add keypoints: motion from (0,0,0) to (6,3,9) over 3 seconds
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Vector3(Vector3::new(0.0, 0.0, 0.0)),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(3.0).unwrap(),
            Value::Vector3(Vector3::new(6.0, 3.0, 9.0)),
        ))
        .unwrap();

    animation.add_track(track);

    // Create player and instance
    let player_id = setup_player_for_animation(&mut engine, animation, Duration::from_secs(3));

    // Set time to middle of animation
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(1.5).unwrap())
        .unwrap();

    // Calculate derivatives
    let derivatives = engine
        .calculate_player_derivatives(&player_id, None)
        .unwrap();

    // Expected velocity: (2, 1, 3) units/second (with numerical tolerance)
    if let Some(Value::Vector3(velocity)) = derivatives.get("transform.position") {
        assert!(
            (velocity.x - 2.0).abs() < 1.5,
            "Expected x velocity ~2.0, got {}",
            velocity.x
        );
        assert!(
            (velocity.y - 1.0).abs() < 1.0,
            "Expected y velocity ~1.0, got {}",
            velocity.y
        );
        assert!(
            (velocity.z - 3.0).abs() < 1.5,
            "Expected z velocity ~3.0, got {}",
            velocity.z
        );
    } else {
        panic!("Expected Vector3 derivative value");
    }
}

#[test]
fn test_transform_derivative_calculation() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Create animation with Transform track
    let mut animation =
        AnimationData::new("test_transform_derivative", "Transform derivative test");
    let mut track = AnimationTrack::new("transform", "object.transform");

    // Add keypoints with changing position and scale
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Transform(Transform::new(
                Vector3::new(0.0, 0.0, 0.0),
                Vector4::new(0.0, 0.0, 0.0, 1.0), // rotation (quaternion)
                Vector3::new(1.0, 1.0, 1.0),
            )),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Transform(Transform::new(
                Vector3::new(4.0, 2.0, 0.0),
                Vector4::new(0.0, 0.7071, 0.0, 0.7071), // 90 degrees Y rotation (quaternion)
                Vector3::new(2.0, 1.0, 1.0),
            )),
        ))
        .unwrap();

    animation.add_track(track);

    // Create player and instance
    let player_id = setup_player_for_animation(&mut engine, animation, Duration::from_secs(2));

    // Set time to middle of animation
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(1.0).unwrap())
        .unwrap();

    // Calculate derivatives
    let derivatives = engine
        .calculate_player_derivatives(&player_id, None)
        .unwrap();

    if let Some(Value::Transform(transform_derivative)) = derivatives.get("object.transform") {
        // Check position velocity (should be (2, 1, 0) units/second)
        let pos_vel = &transform_derivative.position;
        assert!(
            (pos_vel.x - 2.0).abs() < 1.5,
            "Expected position x velocity ~2.0, got {}",
            pos_vel.x
        );
        assert!(
            (pos_vel.y - 1.0).abs() < 1.0,
            "Expected position y velocity ~1.0, got {}",
            pos_vel.y
        );
        assert!(
            pos_vel.z.abs() < 0.5,
            "Expected position z velocity ~0.0, got {}",
            pos_vel.z
        );

        // Check angular velocity (numerical approximation of quaternion derivative)
        let ang_vel = &transform_derivative.rotation;
        assert!(
            ang_vel.x.abs() < 0.5,
            "Expected angular x velocity ~0.0, got {}",
            ang_vel.x
        );
        assert!(
            ang_vel.y > 0.0 && ang_vel.y < 1.0,
            "Expected positive angular y velocity, got {}",
            ang_vel.y
        );
        assert!(
            ang_vel.z.abs() < 0.5,
            "Expected angular z velocity ~0.0, got {}",
            ang_vel.z
        );

        // Check scale rate (should be (0.5, 0, 0) units/second)
        let scale_rate = &transform_derivative.scale;
        assert!(
            (scale_rate.x - 0.5).abs() < 0.5,
            "Expected scale x rate ~0.5, got {}",
            scale_rate.x
        );
        assert!(
            scale_rate.y.abs() < 0.2,
            "Expected scale y rate ~0.0, got {}",
            scale_rate.y
        );
        assert!(
            scale_rate.z.abs() < 0.2,
            "Expected scale z rate ~0.0, got {}",
            scale_rate.z
        );
    } else {
        panic!("Expected Transform derivative value");
    }
}

#[test]
fn test_color_derivative_calculation() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Create animation with Color track
    let mut animation = AnimationData::new("test_color_derivative", "Color derivative test");
    let mut track = AnimationTrack::new("color", "material.color");

    // Add keypoints: fade from red to blue over 1 second
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Color(Color::rgba(1.0, 0.0, 0.0, 1.0)), // Red
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Color(Color::rgba(0.0, 0.0, 1.0, 1.0)), // Blue
        ))
        .unwrap();

    animation.add_track(track);

    // Create player and instance
    let player_id = setup_player_for_animation(&mut engine, animation, Duration::from_secs(1));

    // Set time to middle of animation
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(0.5).unwrap())
        .unwrap();

    // Calculate derivatives
    let derivatives = engine
        .calculate_player_derivatives(&player_id, None)
        .unwrap();

    if let Some(Value::Color(color_derivative)) = derivatives.get("material.color") {
        let (r, g, b, a) = color_derivative.to_rgba();
        // Expected color rate: (-1, 0, 1, 0) per second (with numerical tolerance)
        assert!((r + 1.0).abs() < 1.0, "Expected red rate ~-1.0, got {}", r);
        assert!(g.abs() < 0.5, "Expected green rate ~0.0, got {}", g);
        assert!((b - 1.0).abs() < 1.0, "Expected blue rate ~1.0, got {}", b);
        assert!(a.abs() < 0.5, "Expected alpha rate ~0.0, got {}", a);
    } else {
        panic!("Expected Color derivative value");
    }
}

#[test]
fn test_derivative_with_custom_width() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Create animation with rapid changes
    let mut animation = AnimationData::new("test_custom_width", "Custom width derivative test");
    let mut track = AnimationTrack::new("value", "test.value");

    // Add keypoints with non-linear motion (quadratic-like)
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Float(0.0),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(0.5).unwrap(),
            Value::Float(1.0),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Float(4.0),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.5).unwrap(),
            Value::Float(9.0),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Float(16.0),
        ))
        .unwrap();

    animation.add_track(track);

    // Create player and instance
    let player_id = setup_player_for_animation(&mut engine, animation, Duration::from_secs(2));

    // Set time to middle of animation
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(1.0).unwrap())
        .unwrap();

    // Calculate derivatives with different widths
    let derivative_wide = engine
        .calculate_player_derivatives(
            &player_id,
            Some(AnimationTime::from_seconds(0.2).unwrap()), // 200ms width
        )
        .unwrap();

    let derivative_narrow = engine
        .calculate_player_derivatives(
            &player_id,
            Some(AnimationTime::from_seconds(0.02).unwrap()), // 20ms width
        )
        .unwrap();

    // Both should give reasonable derivative values, but narrow might be more accurate for smooth curves
    if let (Some(Value::Float(wide)), Some(Value::Float(narrow))) = (
        derivative_wide.get("test.value"),
        derivative_narrow.get("test.value"),
    ) {
        // Both derivatives should be positive (increasing function)
        assert!(
            *wide > 0.0,
            "Wide derivative should be positive, got {}",
            wide
        );
        assert!(
            *narrow > 0.0,
            "Narrow derivative should be positive, got {}",
            narrow
        );

        // For this quadratic-like function, derivative should be positive (increasing function)
        assert!(
            *wide > 0.0 && *wide < 15.0,
            "Wide derivative should be reasonable, got {}",
            wide
        );
        assert!(
            *narrow > 0.0 && *narrow < 15.0,
            "Narrow derivative should be reasonable, got {}",
            narrow
        );
    } else {
        panic!("Expected float derivative values");
    }
}

#[test]
fn test_derivative_at_animation_boundaries() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Create simple animation
    let mut animation = AnimationData::new("test_boundaries", "Boundary derivative test");
    let mut track = AnimationTrack::new("value", "test.value");

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Float(0.0),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Float(10.0),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Float(20.0),
        ))
        .unwrap();

    animation.add_track(track);

    // Create player and instance
    let player_id = setup_player_for_animation(&mut engine, animation, Duration::from_secs(2));

    // Test derivative at start
    engine
        .seek_player(&player_id, AnimationTime::zero())
        .unwrap();
    let start_derivatives = engine
        .calculate_player_derivatives(&player_id, None)
        .unwrap();

    // Test derivative at end
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(2.0).unwrap())
        .unwrap();
    let end_derivatives = engine
        .calculate_player_derivatives(&player_id, None)
        .unwrap();

    // Both should give reasonable values (using forward/backward difference at boundaries)
    if let Some(Value::Float(start_deriv)) = start_derivatives.get("test.value") {
        assert!(
            *start_deriv > 0.0 && *start_deriv < 20.0,
            "Start derivative should be reasonable, got {}",
            start_deriv
        );
    } else {
        panic!("Expected start derivative value");
    }

    if let Some(Value::Float(end_deriv)) = end_derivatives.get("test.value") {
        assert!(
            *end_deriv > 0.0 && *end_deriv < 20.0,
            "End derivative should be reasonable, got {}",
            end_deriv
        );
    } else {
        panic!("Expected end derivative value");
    }
}

#[test]
fn test_multiple_tracks_derivative() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Create animation with multiple tracks
    let mut animation =
        AnimationData::new("test_multiple_tracks", "Multiple tracks derivative test");

    // Position track
    let mut position_track = AnimationTrack::new("position", "transform.position");
    position_track.settings = None; // Explicitly set settings to None
    position_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Vector3(Vector3::new(0.0, 0.0, 0.0)),
        ))
        .unwrap();
    position_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Vector3(Vector3::new(2.0, 1.0, 0.0)),
        ))
        .unwrap();

    // Scale track
    let mut scale_track = AnimationTrack::new("scale", "transform.scale");
    scale_track.settings = None; // Explicitly set settings to None
    scale_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Vector3(Vector3::new(1.0, 1.0, 1.0)),
        ))
        .unwrap();
    scale_track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Vector3(Vector3::new(1.5, 2.0, 1.0)),
        ))
        .unwrap();

    animation.add_track(position_track);
    animation.add_track(scale_track);

    // Create player and instance
    let player_id = setup_player_for_animation(&mut engine, animation, Duration::from_secs(1));

    // Set time to middle of animation
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(0.5).unwrap())
        .unwrap();

    // Calculate derivatives
    let derivatives = engine
        .calculate_player_derivatives(&player_id, None)
        .unwrap();

    // Should have derivatives for both tracks
    assert!(
        derivatives.contains_key("transform.position"),
        "Should have position derivative"
    );
    assert!(
        derivatives.contains_key("transform.scale"),
        "Should have scale derivative"
    );

    // Check position velocity
    if let Some(Value::Vector3(pos_vel)) = derivatives.get("transform.position") {
        assert!(
            (pos_vel.x - 2.0).abs() < 1.5,
            "Expected position x velocity ~2.0, got {}",
            pos_vel.x
        );
        assert!(
            (pos_vel.y - 1.0).abs() < 1.0,
            "Expected position y velocity ~1.0, got {}",
            pos_vel.y
        );
    }

    // Check scale rate
    if let Some(Value::Vector3(scale_rate)) = derivatives.get("transform.scale") {
        assert!(
            (scale_rate.x - 0.5).abs() < 0.5,
            "Expected scale x rate ~0.5, got {}",
            scale_rate.x
        );
        assert!(
            (scale_rate.y - 1.0).abs() < 0.5,
            "Expected scale y rate ~1.0, got {}",
            scale_rate.y
        );
        assert!(
            scale_rate.z.abs() < 0.2,
            "Expected scale z rate ~0.0, got {}",
            scale_rate.z
        );
    }
}

#[test]
fn test_zero_derivative_for_constant_values() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Create animation with constant value
    let mut animation = AnimationData::new("test_constant", "Constant value derivative test");
    let mut track = AnimationTrack::new("constant", "test.constant");

    // All keypoints have same value
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Float(5.0),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Float(5.0),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Float(5.0),
        ))
        .unwrap();

    animation.add_track(track);

    // Create player and instance
    let player_id = setup_player_for_animation(&mut engine, animation, Duration::from_secs(2));

    // Set time to middle of animation
    engine
        .seek_player(&player_id, AnimationTime::from_seconds(1.0).unwrap())
        .unwrap();

    // Calculate derivatives
    let derivatives = engine
        .calculate_player_derivatives(&player_id, None)
        .unwrap();

    // Derivative should be zero for constant value
    if let Some(Value::Float(derivative)) = derivatives.get("test.constant") {
        assert!(
            derivative.abs() < 0.01,
            "Expected zero derivative for constant value, got {}",
            derivative
        );
    } else {
        panic!("Expected float derivative value");
    }
}
