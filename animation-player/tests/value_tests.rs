use animation_player::value::{
    color::Color,
    transform::slerp_quaternion,
    transform::Transform,
    value_enum::{Value, ValueType},
    vector3::Vector3,
};

#[test]
fn test_vector_operations() {
    let v1 = Vector3::new(1.0, 2.0, 3.0);
    let v2 = Vector3::new(4.0, 5.0, 6.0);

    assert_eq!(v1.length(), (14.0_f64).sqrt());
    assert_eq!(v1.dot(&v2), 32.0);

    let cross = v1.cross(&v2);
    assert_eq!(cross, Vector3::new(-3.0, 6.0, -3.0));
}

#[test]
fn test_color_conversion() {
    let color = Color::rgb(1.0, 0.5, 0.0);
    let rgba = color.to_rgba();
    assert_eq!(rgba, (1.0, 0.5, 0.0, 1.0));

    let hex_color = Color::hex("#FF8000");
    let hex_rgba = hex_color.to_rgba();
    assert!((hex_rgba.0 - 1.0).abs() < 0.01);
    assert!((hex_rgba.1 - 0.502).abs() < 0.01);
    assert!((hex_rgba.2 - 0.0).abs() < 0.01);
}

#[test]
fn test_value_interpolation_components() {
    let v3 = Value::Vector3(Vector3::new(1.0, 2.0, 3.0));
    let components = v3.interpolatable_components();
    assert_eq!(components, vec![1.0, 2.0, 3.0]);

    let color = Value::Color(Color::rgba(1.0, 0.5, 0.0, 0.8));
    let color_components = color.interpolatable_components();
    assert_eq!(color_components, vec![1.0, 0.5, 0.0, 0.8]);
}

#[test]
fn test_value_from_components() {
    let result = Value::from_components(ValueType::Vector3, &[1.0, 2.0, 3.0]).unwrap();

    if let Value::Vector3(v) = result {
        assert_eq!(v, Vector3::new(1.0, 2.0, 3.0));
    } else {
        panic!("Expected Vector3");
    }
}

#[test]
fn test_value_conversions() {
    let float_val: Value = 42.5.into();
    assert!(matches!(float_val, Value::Float(42.5)));

    let extracted: f64 = float_val.try_into().unwrap();
    assert_eq!(extracted, 42.5);
}

#[test]
fn test_transform() {
    let transform = Transform::identity();
    assert_eq!(transform.position, Vector3::zero());
    assert_eq!(transform.scale, Vector3::one());
}

#[test]
fn test_hash_consistency() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let v1 = Vector3::new(1.0, 2.0, 3.0);
    let v2 = Vector3::new(1.0, 2.0, 3.0);

    let mut hasher1 = DefaultHasher::new();
    let mut hasher2 = DefaultHasher::new();

    v1.hash(&mut hasher1);
    v2.hash(&mut hasher2);

    assert_eq!(hasher1.finish(), hasher2.finish());
}

#[test]
fn test_slerp_quaternion_identity() {
    let q1 = [0.0, 0.0, 0.0, 1.0]; // Identity
    let q2 = [0.0, 0.0, 0.0, 1.0]; // Identity
    let result = slerp_quaternion(&q1, &q2, 0.5);
    assert_eq!(result, [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn test_slerp_quaternion_halfway() {
    // 90 degrees around X axis
    let q1 = [0.7071068, 0.0, 0.0, 0.7071068];
    // 0 degrees around X axis (identity)
    let q2 = [0.0, 0.0, 0.0, 1.0];

    // Halfway should be 45 degrees around X axis
    let expected = [0.3826834, 0.0, 0.0, 0.9238795]; // sin(22.5), 0, 0, cos(22.5)
    let result = slerp_quaternion(&q2, &q1, 0.5);

    assert!((result[0] - expected[0]).abs() < 1e-6);
    assert!((result[1] - expected[1]).abs() < 1e-6);
    assert!((result[2] - expected[2]).abs() < 1e-6);
    assert!((result[3] - expected[3]).abs() < 1e-6);
}

#[test]
fn test_slerp_quaternion_shortest_path() {
    // q1: 0 degrees
    let q1 = [0.0, 0.0, 0.0, 1.0];
    // q2: 350 degrees (long way around from 0)
    let q2 = [0.0, 0.0, 0.0871557, -0.9961947]; // sin(175 deg), cos(175 deg) for 350 deg rotation
                                                // q2_alt: -10 degrees (shortest path from 0)
    let q2_alt = [0.0, 0.0, -0.0871557, 0.9961947]; // sin(-5/2), cos(-5/2)

    // SLERP should take the shortest path, so interpolating from q1 to q2
    // should be equivalent to interpolating from q1 to q2_alt (which is q2 negated)
    let result = slerp_quaternion(&q1, &q2, 0.5);
    let expected_shortest_path = slerp_quaternion(&q1, &q2_alt, 0.5);

    assert!((result[0] - expected_shortest_path[0]).abs() < 1e-6);
    assert!((result[1] - expected_shortest_path[1]).abs() < 1e-6);
    assert!((result[2] - expected_shortest_path[2]).abs() < 1e-6);
    assert!((result[3] - expected_shortest_path[3]).abs() < 1e-6);
}
