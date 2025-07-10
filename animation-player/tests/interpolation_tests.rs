use animation_player::interpolation::parameters::{InterpolationParams, StepParams};
use animation_player::interpolation::InterpolationType;
use animation_player::interpolation::{
    context::InterpolationContext,
    functions::{
        BezierInterpolation, CubicInterpolation, EaseInInterpolation, EaseInOutInterpolation,
        EaseOutInterpolation, LinearInterpolation, SpringInterpolation, StepInterpolation,
    },
    registry::InterpolationRegistry,
};
use animation_player::value::{ValueType, Vector3};
use animation_player::AnimationError;
use animation_player::Interpolator;
use animation_player::{AnimationData, AnimationTime, Value};

#[test]
fn test_linear_interpolation_float() {
    let start = Value::Float(0.0);
    let end = Value::Float(10.0);
    let context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(1.0).unwrap(),
        AnimationTime::from_seconds(0.5).unwrap(),
        &[],
        0,
    )
    .unwrap();
    let linear = LinearInterpolation;
    let animation_data = AnimationData::new("test", "test");

    let result = linear
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(f) = result {
        assert!((f - 5.0).abs() < 0.001);
    } else {
        panic!("Expected float result");
    }
}

#[test]
fn test_cubic_interpolation_float() {
    let start = Value::Float(0.0);
    let end = Value::Float(10.0);
    let context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(1.0).unwrap(),
        AnimationTime::from_seconds(0.5).unwrap(),
        &[],
        0,
    )
    .unwrap();
    let cubic = CubicInterpolation;
    let animation_data = AnimationData::new("test", "test");

    let result = cubic
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(f) = result {
        // Cubic interpolation should give a different result than linear
        assert!(f > 4.0 && f < 6.0);
    } else {
        panic!("Expected float result");
    }
}

#[test]
fn test_step_interpolation() {
    let step = StepInterpolation;
    let start = Value::Float(0.0);
    let end = Value::Float(10.0);
    let animation_data = AnimationData::new("test", "test");

    // Before threshold
    let context1 = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(2.0).unwrap(),
        AnimationTime::from_seconds(1.0).unwrap(),
        &[],
        0,
    )
    .unwrap();

    let result1 = step
        .interpolate(&start, &end, &context1, &animation_data)
        .unwrap();
    if let Value::Float(f) = result1 {
        assert_eq!(f, 0.0);
    }

    // At threshold
    let context2 = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(1.0).unwrap(),
        AnimationTime::from_seconds(1.0).unwrap(),
        &[],
        0,
    )
    .unwrap();

    let result2 = step
        .interpolate(&start, &end, &context2, &animation_data)
        .unwrap();
    if let Value::Float(f) = result2 {
        assert_eq!(f, 10.0);
    }
}

#[test]
fn test_vector_interpolation() {
    let linear = LinearInterpolation;
    let start = Value::Vector3(Vector3::new(0.0, 0.0, 0.0));
    let end = Value::Vector3(Vector3::new(10.0, 20.0, 30.0));
    let context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(2.0).unwrap(),
        AnimationTime::from_seconds(1.0).unwrap(),
        &[],
        0,
    )
    .unwrap();
    let animation_data = AnimationData::new("test", "test");

    let result = linear
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Vector3(v) = result {
        assert!((v.x - 5.0).abs() < 0.001);
        assert!((v.y - 10.0).abs() < 0.001);
        assert!((v.z - 15.0).abs() < 0.001);
    } else {
        panic!("Expected Vector3 result");
    }
}

#[test]
fn test_euler_interpolation() {
    let linear = LinearInterpolation;
    let start = Value::Euler(animation_player::value::euler::Euler::new(0.0, 0.0, 0.0));
    let end = Value::Euler(animation_player::value::euler::Euler::new(
        90.0, 180.0, 270.0,
    ));
    let context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(2.0).unwrap(),
        AnimationTime::from_seconds(1.0).unwrap(),
        &[],
        0,
    )
    .unwrap();
    let animation_data = AnimationData::new("test", "test");

    let result = linear
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Euler(e) = result {
        assert!((e.r - 45.0).abs() < 0.001);
        assert!((e.p - 90.0).abs() < 0.001);
        assert!((e.y - 135.0).abs() < 0.001);
    } else {
        panic!("Expected Euler result");
    }
}

#[test]
fn test_interpolation_registry() {
    let mut registry = InterpolationRegistry::new(10);

    let start = Value::Float(0.0);
    let end = Value::Float(10.0);
    let context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(2.0).unwrap(),
        AnimationTime::from_seconds(1.0).unwrap(),
        &[],
        0,
    )
    .unwrap();
    let animation_data = AnimationData::new("test", "test");

    // Test linear interpolation
    let result = registry
        .interpolate("linear", &start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(f) = result {
        assert!((f - 5.0).abs() < 0.001);
    }

    // Test cubic interpolation
    let result = registry
        .interpolate("cubic", &start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(f) = result {
        assert!(f > 4.0 && f < 6.0);
    }

    // Test unknown interpolation
    assert!(registry
        .interpolate("unknown", &start, &end, &context, &animation_data)
        .is_err());
}

#[test]
fn test_interpolation_caching() {
    let mut registry = InterpolationRegistry::new(10);

    let start = Value::Float(0.0);
    let end = Value::Float(10.0);
    let context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(2.0).unwrap(),
        AnimationTime::from_seconds(1.0).unwrap(),
        &[],
        0,
    )
    .unwrap();
    let animation_data = AnimationData::new("test", "test");

    // First call should be a cache miss
    let _ = registry
        .interpolate("linear", &start, &end, &context, &animation_data)
        .unwrap();
    assert_eq!(registry.metrics().cache_misses, 1);
    assert_eq!(registry.metrics().cache_hits, 0);

    // Second call with same parameters should be a cache hit
    let _ = registry
        .interpolate("linear", &start, &end, &context, &animation_data)
        .unwrap();
    assert_eq!(registry.metrics().cache_hits, 1);
}

#[test]
fn test_interpolation_context() {
    let mut context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(2.0).unwrap(),
        AnimationTime::from_seconds(1.0).unwrap(),
        &[],
        0,
    )
    .unwrap();

    assert_eq!(context.t, 0.5);

    context.set_property("test", 42.0);
    assert_eq!(context.get_property("test"), Some(42.0));
}

#[test]
fn test_parameter_schema_linear() {
    let linear = LinearInterpolation;
    let schema = linear.parameter_schema();
    assert!(schema.parameters.is_empty());
}

#[test]
fn test_parameter_schema_cubic() {
    let cubic = CubicInterpolation;
    let schema = cubic.parameter_schema();
    assert!(schema.parameters.is_empty());
}

#[test]
fn test_parameter_schema_step() {
    let step = StepInterpolation;
    let schema = step.parameter_schema();
    assert_eq!(schema.parameters.len(), 1);
    let param = schema.parameters.get("threshold").unwrap();
    assert_eq!(param.name, "threshold");
    assert_eq!(param.value_type, ValueType::Float);
    assert_eq!(param.default_value, Some(Value::Float(1.0)));
    assert_eq!(param.min_value, Some(Value::Float(0.0)));
    assert_eq!(param.max_value, Some(Value::Float(1.0)));
}

#[test]
fn test_parameter_schema_bezier() {
    let bezier = BezierInterpolation::new();
    let schema = bezier.parameter_schema();
    assert_eq!(schema.parameters.len(), 4); // x1, y1, x2, y2
    let x1 = schema.parameters.get("x1").unwrap();
    assert_eq!(x1.name, "x1");
    assert_eq!(x1.value_type, ValueType::Float);
    assert_eq!(x1.default_value, Some(Value::Float(0.25)));
    assert_eq!(x1.min_value, Some(Value::Float(0.0)));
    assert_eq!(x1.max_value, Some(Value::Float(1.0)));
    let y1 = schema.parameters.get("y1").unwrap();
    assert_eq!(y1.name, "y1");
    assert_eq!(y1.value_type, ValueType::Float);
    assert_eq!(y1.default_value, Some(Value::Float(0.1)));
    assert_eq!(y1.min_value, None);
    assert_eq!(y1.max_value, None);
    let x2 = schema.parameters.get("x2").unwrap();
    assert_eq!(x2.name, "x2");
    assert_eq!(x2.value_type, ValueType::Float);
    assert_eq!(x2.default_value, Some(Value::Float(0.25)));
    assert_eq!(x2.min_value, Some(Value::Float(0.0)));
    assert_eq!(x2.max_value, Some(Value::Float(1.0)));
    let y2 = schema.parameters.get("y2").unwrap();
    assert_eq!(y2.name, "y2");
    assert_eq!(y2.value_type, ValueType::Float);
    assert_eq!(y2.default_value, Some(Value::Float(1.0)));
    assert_eq!(y2.min_value, None);
    assert_eq!(y2.max_value, None);
}

#[test]
fn test_parameter_schema_spring() {
    let spring = SpringInterpolation::new();
    let schema = spring.parameter_schema();
    assert_eq!(schema.parameters.len(), 2);
    let damping = schema.parameters.get("damping").unwrap();
    assert_eq!(damping.name, "damping");
    assert_eq!(damping.value_type, ValueType::Float);
    assert_eq!(damping.default_value, Some(Value::Float(0.8)));
    assert_eq!(damping.min_value, Some(Value::Float(0.0)));
    assert_eq!(damping.max_value, Some(Value::Float(2.0)));
    let stiffness = schema.parameters.get("stiffness").unwrap();
    assert_eq!(stiffness.name, "stiffness");
    assert_eq!(stiffness.value_type, ValueType::Float);
    assert_eq!(stiffness.default_value, Some(Value::Float(100.0)));
    assert_eq!(stiffness.min_value, Some(Value::Float(1.0)));
    assert_eq!(stiffness.max_value, Some(Value::Float(1000.0)));
}

#[test]
fn test_bezier_interpolation_specific_points() {
    let bezier = BezierInterpolation::with_control_points((0.0, 0.0), (1.0, 1.0)); // Linear bezier
    let start = Value::Float(0.0);
    let end = Value::Float(10.0);
    let context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(2.0).unwrap(),
        AnimationTime::from_seconds(1.0).unwrap(),
        &[],
        0,
    )
    .unwrap(); // t = 0.5
    let animation_data = AnimationData::new("test", "test");

    let result = bezier
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(f) = result {
        assert!((f - 5.0).abs() < 0.001); // Should be linear
    } else {
        panic!("Expected float result");
    }
}

#[test]
fn test_spring_interpolation_specific_params() {
    let spring = SpringInterpolation::with_params(0.5, 50.0); // Custom spring
    let start = Value::Float(0.0);
    let end = Value::Float(10.0);
    let context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(2.0).unwrap(),
        AnimationTime::from_seconds(1.0).unwrap(),
        &[],
        0,
    )
    .unwrap(); // t = 0.5
    let animation_data = AnimationData::new("test", "test");

    let result = spring
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(f) = result {
        // Check if it's a reasonable spring value (not linear)
        assert!((f - 5.0).abs() > 0.1); // Ensure it's not linear
                                        // Allow for overshooting/undershooting characteristic of a spring
    } else {
        panic!("Expected float result");
    }
}

#[test]
fn test_step_custom_threshold() {
    let step = StepInterpolation;
    let start = Value::Float(0.0);
    let end = Value::Float(10.0);
    let mut context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(1.0).unwrap(),
        AnimationTime::from_seconds(0.5).unwrap(),
        &[],
        0,
    )
    .unwrap();
    context.set_property("threshold", 0.8);
    let animation_data = AnimationData::new("test", "test");

    let result = step
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(f) = result {
        assert_eq!(f, 0.0);
    } else {
        panic!("Expected float result");
    }

    context.set_property("threshold", 0.3);
    let result = step
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(f) = result {
        assert_eq!(f, 10.0);
    } else {
        panic!("Expected float result");
    }
}

#[test]
fn test_step_default_threshold_from_animation() {
    let step = StepInterpolation;
    let start = Value::Float(0.0);
    let end = Value::Float(10.0);

    let mut animation_data = AnimationData::new("test", "test");
    animation_data.default_interpolation.insert(
        InterpolationType::Step,
        InterpolationParams::Step(StepParams { point: 0.25 }),
    );

    let context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(1.0).unwrap(),
        AnimationTime::from_seconds(0.3).unwrap(),
        &[],
        0,
    )
    .unwrap();

    let result = step
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(f) = result {
        assert_eq!(f, 10.0);
    } else {
        panic!("Expected float result");
    }
}

#[test]
fn test_interpolator_type_mismatch_error() {
    let linear = LinearInterpolation;
    let start = Value::Float(0.0);
    let end = Value::Vector3(Vector3::new(1.0, 1.0, 1.0));
    let context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(1.0).unwrap(),
        AnimationTime::from_seconds(0.5).unwrap(),
        &[],
        0,
    )
    .unwrap();
    let animation_data = AnimationData::new("test", "test");

    let result = linear.interpolate(&start, &end, &context, &animation_data);
    assert!(matches!(
        result,
        Err(AnimationError::InterpolationError { .. })
    ));
}

#[test]
fn test_ease_functions_midpoint() {
    let start = Value::Float(0.0);
    let end = Value::Float(10.0);
    let context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(1.0).unwrap(),
        AnimationTime::from_seconds(0.5).unwrap(),
        &[],
        0,
    )
    .unwrap();
    let animation_data = AnimationData::new("test", "test");

    let ease_in = EaseInInterpolation;
    let ease_out = EaseOutInterpolation;
    let ease_in_out = EaseInOutInterpolation;

    let result = ease_in
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(v) = result {
        assert!((v - 2.5).abs() < 1e-6);
    } else {
        panic!("Expected float result");
    }

    let result = ease_out
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(v) = result {
        assert!((v - 7.5).abs() < 1e-6);
    } else {
        panic!("Expected float result");
    }

    let result = ease_in_out
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(v) = result {
        assert!((v - 5.0).abs() < 1e-6);
    } else {
        panic!("Expected float result");
    }
}

#[test]
fn test_ease_functions_quarter() {
    let start = Value::Float(0.0);
    let end = Value::Float(10.0);
    let context = InterpolationContext::new(
        AnimationTime::zero(),
        AnimationTime::from_seconds(1.0).unwrap(),
        AnimationTime::from_seconds(0.25).unwrap(),
        &[],
        0,
    )
    .unwrap();
    let animation_data = AnimationData::new("test", "test");

    let ease_in = EaseInInterpolation;
    let ease_out = EaseOutInterpolation;
    let ease_in_out = EaseInOutInterpolation;

    let result = ease_in
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(v) = result {
        assert!((v - 0.625).abs() < 1e-6);
    } else {
        panic!("Expected float result");
    }

    let result = ease_out
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(v) = result {
        assert!((v - 4.375).abs() < 1e-6);
    } else {
        panic!("Expected float result");
    }

    let result = ease_in_out
        .interpolate(&start, &end, &context, &animation_data)
        .unwrap();
    if let Value::Float(v) = result {
        assert!((v - 1.25).abs() < 1e-6);
    } else {
        panic!("Expected float result");
    }
}
