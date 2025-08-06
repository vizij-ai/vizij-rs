use crate::animation::AnimationTransition;
use animation_player::animation::TransitionVariant;
use animation_player::interpolation::*;
use animation_player::value::{Transform, Value, Vector3, Vector4};
use animation_player::*;

#[test]
fn test_catmull_rom_basic_interpolation() {
    let mut track = AnimationTrack::new("test", "position");

    // Add 4 keypoints for Catmull-Rom
    let _k1 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(0.0).unwrap(),
            Value::Float(0.0),
        ))
        .unwrap();
    let k2 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Float(10.0),
        ))
        .unwrap();
    let k3 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Float(20.0),
        ))
        .unwrap();
    let _k4 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(3.0).unwrap(),
            Value::Float(15.0),
        ))
        .unwrap();

    let mut registry = InterpolationRegistry::default();
    registry.register_function(Box::new(CatmullRomInterpolation));

    let animation_data = AnimationData::new("test", "test");

    // Test interpolation at t=1.5 (between second and third keypoint)
    let value = track.value_at_time(
        AnimationTime::from_seconds(1.5).unwrap(),
        &mut registry,
        Some(&AnimationTransition::new(
            k2.id,
            k3.id,
            TransitionVariant::Catmullrom,
        )),
        &animation_data,
    );

    assert!(value.is_some());
    // The exact value would depend on the Catmull-Rom curve
    // but should be between 10.0 and 20.0
    if let Some(Value::Float(v)) = value {
        assert!(v > 10.0 && v < 20.0);
    }
}

#[test]
fn test_hermite_with_explicit_tangents() {
    let mut registry = InterpolationRegistry::default();
    registry.register_function(Box::new(HermiteInterpolation));

    let start = Value::Vector3(Vector3::new(0.0, 0.0, 0.0));
    let end = Value::Vector3(Vector3::new(10.0, 10.0, 10.0));

    let mut context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(1.0).unwrap(),
        AnimationTime::from_seconds(0.5).unwrap(),
        &[],
        0,
    )
    .unwrap();

    // Set explicit tangents
    context.set_property("tangent_start", Value::Vector3(Vector3::new(5.0, 0.0, 0.0)));
    context.set_property("tangent_end", Value::Vector3(Vector3::new(0.0, 5.0, 0.0)));

    let animation_data = AnimationData::new("test", "test");
    let result = registry
        .interpolate("hermite", &start, &end, &context, &animation_data)
        .unwrap();

    // Verify the result matches expected hermite spline value
    if let Value::Vector3(v) = result {
        use animation_player::interpolation::spline_helpers::hermite_spline;
        let expected_x = hermite_spline(0.0, 10.0, 5.0, 0.0, 0.5);
        let expected_y = hermite_spline(0.0, 10.0, 0.0, 5.0, 0.5);
        let expected_z = hermite_spline(0.0, 10.0, 0.0, 0.0, 0.5);
        assert!((v.x - expected_x).abs() < 1e-6);
        assert!((v.y - expected_y).abs() < 1e-6);
        assert!((v.z - expected_z).abs() < 1e-6);
    } else {
        panic!("Expected Vector3 result");
    }
}

#[test]
fn test_b_spline_curve_helper() {
    use animation_player::interpolation::spline_helpers::b_spline_curve;

    // Simple float values
    let p0 = 0.0;
    let p1 = 10.0;
    let p2 = 25.0;
    let p3 = 15.0;

    let result_t0 = b_spline_curve(p0, p1, p2, p3, 0.0);
    let result_t1 = b_spline_curve(p0, p1, p2, p3, 1.0);

    assert_ne!(result_t0, p1);
    assert_ne!(result_t1, p2);

    // Basic vector usage with floats should compile even without vector ops
}

#[test]
fn test_bspline_track_interpolation() {
    let mut track = AnimationTrack::new("bspline", "value");

    let _k1 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(0.0).unwrap(),
            Value::Float(0.0),
        ))
        .unwrap();
    let k2 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Float(10.0),
        ))
        .unwrap();
    let k3 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Float(20.0),
        ))
        .unwrap();
    let _k4 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(3.0).unwrap(),
            Value::Float(30.0),
        ))
        .unwrap();

    let mut registry = InterpolationRegistry::default();
    registry.register_function(Box::new(BSplineInterpolation));

    let animation_data = AnimationData::new("test", "test");

    let value = track.value_at_time(
        AnimationTime::from_seconds(1.5).unwrap(),
        &mut registry,
        Some(&AnimationTransition::new(
            k2.id,
            k3.id,
            TransitionVariant::Bspline,
        )),
        &animation_data,
    );

    assert!(value.is_some());
    if let Some(Value::Float(v)) = value {
        assert!(v > 10.0 && v < 20.0);
    }
}

#[test]
fn test_spline_with_transform_values() {
    let mut track = AnimationTrack::new("test", "transform");

    // Create transforms with different rotations
    let t1 = Transform {
        position: Vector3::new(0.0, 0.0, 0.0),
        rotation: Vector4::new(0.0, 0.0, 0.0, 1.0), // Identity quaternion
        scale: Vector3::new(1.0, 1.0, 1.0),
    };

    let t2 = Transform {
        position: Vector3::new(10.0, 0.0, 0.0),
        rotation: Vector4::new(0.0, 0.707, 0.0, 0.707), // 90 degree Y rotation
        scale: Vector3::new(2.0, 2.0, 2.0),
    };

    let t3 = Transform {
        position: Vector3::new(20.0, 10.0, 0.0),
        rotation: Vector4::new(0.0, 1.0, 0.0, 0.0), // 180 degree Y rotation
        scale: Vector3::new(1.0, 1.0, 1.0),
    };

    let t4 = Transform {
        position: Vector3::new(30.0, 0.0, 0.0),
        rotation: Vector4::new(0.0, 0.707, 0.0, 0.707), // 90 degree Y rotation
        scale: Vector3::new(0.5, 0.5, 0.5),
    };

    let _k1 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(0.0).unwrap(),
            Value::Transform(t1),
        ))
        .unwrap();
    let k2 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Transform(t2),
        ))
        .unwrap();
    let k3 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Transform(t3),
        ))
        .unwrap();
    let _k4 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(3.0).unwrap(),
            Value::Transform(t4),
        ))
        .unwrap();

    let mut registry = InterpolationRegistry::default();
    registry.register_function(Box::new(CatmullRomInterpolation));

    let animation_data = AnimationData::new("test", "test");

    let value = track.value_at_time(
        AnimationTime::from_seconds(1.5).unwrap(),
        &mut registry,
        Some(&AnimationTransition::new(
            k2.id,
            k3.id,
            TransitionVariant::Catmullrom,
        )),
        &animation_data,
    );

    assert!(value.is_some());
    if let Some(Value::Transform(t)) = value {
        // Position should be smoothly interpolated
        assert!(t.position.x > 10.0 && t.position.x < 20.0);
        // Rotation should be normalized (quaternion length = 1)
        let rot_len = (t.rotation.x * t.rotation.x
            + t.rotation.y * t.rotation.y
            + t.rotation.z * t.rotation.z
            + t.rotation.w * t.rotation.w)
            .sqrt();
        assert!((rot_len - 1.0).abs() < 0.001);
    }
}

#[test]
fn test_edge_cases_insufficient_keypoints() {
    let mut track = AnimationTrack::new("test", "position");

    // Add only 2 keypoints (not enough for Catmull-Rom)
    let k1 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(0.0).unwrap(),
            Value::Float(0.0),
        ))
        .unwrap();
    let k2 = track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Float(10.0),
        ))
        .unwrap();

    let mut registry = InterpolationRegistry::default();
    registry.register_function(Box::new(CatmullRomInterpolation));

    let animation_data = AnimationData::new("test", "test");

    // Should still work, falling back to using the same points
    let value = track.value_at_time(
        AnimationTime::from_seconds(0.5).unwrap(),
        &mut registry,
        Some(&AnimationTransition::new(
            k1.id,
            k2.id,
            TransitionVariant::Catmullrom,
        )),
        &animation_data,
    );

    assert!(value.is_some());
}

#[test]
fn test_value_arithmetic_operations() {
    // Test Add
    let v1 = Value::Vector3(Vector3::new(1.0, 2.0, 3.0));
    let v2 = Value::Vector3(Vector3::new(4.0, 5.0, 6.0));
    let sum = v1.clone() + v2.clone();
    if let Value::Vector3(v) = sum {
        assert_eq!(v.x, 5.0);
        assert_eq!(v.y, 7.0);
        assert_eq!(v.z, 9.0);
    }

    // Test Sub
    let diff = v2.clone() - v1.clone();
    if let Value::Vector3(v) = diff {
        assert_eq!(v.x, 3.0);
        assert_eq!(v.y, 3.0);
        assert_eq!(v.z, 3.0);
    }

    // Test Mul
    let scaled = v1.clone() * 2.0;
    if let Value::Vector3(v) = scaled {
        assert_eq!(v.x, 2.0);
        assert_eq!(v.y, 4.0);
        assert_eq!(v.z, 6.0);
    }

    // Test mismatched types (should return original)
    let float_val = Value::Float(5.0);
    let string_val = Value::String("test".to_string());
    let result = float_val.clone() + string_val;
    assert_eq!(result, float_val);
}

#[test]
fn test_interpolation_context_get_point() {
    let keypoints = vec![
        AnimationKeypoint::new(AnimationTime::from_seconds(0.0).unwrap(), Value::Float(0.0)),
        AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Float(10.0),
        ),
        AnimationKeypoint::new(
            AnimationTime::from_seconds(2.0).unwrap(),
            Value::Float(20.0),
        ),
        AnimationKeypoint::new(
            AnimationTime::from_seconds(3.0).unwrap(),
            Value::Float(30.0),
        ),
    ];

    let context = InterpolationContext::new(
        AnimationTime::from_seconds(1.0).unwrap(),
        AnimationTime::from_seconds(2.0).unwrap(),
        AnimationTime::from_seconds(1.5).unwrap(),
        &keypoints,
        1, // Start index
    )
    .unwrap();

    // Test relative indexing
    assert_eq!(context.get_point(-1), Some(Value::Float(0.0))); // Previous point
    assert_eq!(context.get_point(0), Some(Value::Float(10.0))); // Start point
    assert_eq!(context.get_point(1), Some(Value::Float(20.0))); // End point
    assert_eq!(context.get_point(2), Some(Value::Float(30.0))); // Next point
    assert_eq!(context.get_point(3), None); // Out of bounds
    assert_eq!(context.get_point(-2), None); // Out of bounds
}

#[test]
fn test_bezier_curve_helper() {
    use animation_player::interpolation::spline_helpers::bezier_curve;

    // Test with control points forming a simple curve
    let p0 = 0.0;
    let p1 = 0.0;
    let p2 = 10.0;
    let p3 = 10.0;

    // At t=0, should return p0
    assert_eq!(bezier_curve(p0, p1, p2, p3, 0.0), 0.0);

    // At t=1, should return p3
    assert_eq!(bezier_curve(p0, p1, p2, p3, 1.0), 10.0);

    // At t=0.5, should be somewhere in between
    let mid = bezier_curve(p0, p1, p2, p3, 0.5);
    assert!(mid > 0.0 && mid < 10.0);
}

#[test]
fn test_keypoint_indices_at_time() {
    let mut track = AnimationTrack::new("test", "value");

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Float(10.0),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(3.0).unwrap(),
            Value::Float(30.0),
        ))
        .unwrap();
    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(5.0).unwrap(),
            Value::Float(50.0),
        ))
        .unwrap();

    // Test exact match
    let (prev, next) = track.keypoint_indices_at_time(AnimationTime::from_seconds(3.0).unwrap());
    assert_eq!(prev, Some(0));
    assert_eq!(next, Some(1));

    // Test between keypoints
    let (prev, next) = track.keypoint_indices_at_time(AnimationTime::from_seconds(2.0).unwrap());
    assert_eq!(prev, Some(0));
    assert_eq!(next, Some(1));

    // Test before first keypoint
    let (prev, next) = track.keypoint_indices_at_time(AnimationTime::from_seconds(0.5).unwrap());
    assert_eq!(prev, None);
    assert_eq!(next, Some(0));

    // Test after last keypoint
    let (prev, next) = track.keypoint_indices_at_time(AnimationTime::from_seconds(6.0).unwrap());
    assert_eq!(prev, Some(2));
    assert_eq!(next, None);
}
