//! Behavioural coverage for the evaluation pipeline.

use super::*;
use crate::types::{GraphSpec, InputConnection, NodeParams, NodeSpec, NodeType, SelectorSegment};
use hashbrown::HashMap;
use vizij_api_core::shape::Field;
use vizij_api_core::{Shape, ShapeId, TypedPath, Value};

fn constant_node(id: &str, value: Value) -> NodeSpec {
    NodeSpec {
        id: id.to_string(),
        kind: NodeType::Constant,
        params: NodeParams {
            value: Some(value),
            ..Default::default()
        },
        inputs: HashMap::new(),
        output_shapes: HashMap::new(),
    }
}

fn connection(node_id: &str, output_key: &str) -> InputConnection {
    InputConnection {
        node_id: node_id.to_string(),
        output_key: output_key.to_string(),
        selector: None,
    }
}

// --- Shape validation ----------------------------------------------------

#[test]
fn it_should_respect_declared_shape() {
    let mut node = constant_node("a", Value::Float(1.0));
    node.output_shapes
        .insert("out".to_string(), Shape::new(ShapeId::Scalar));

    let spec = GraphSpec { nodes: vec![node] };
    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("shape should match");
    let outputs = rt.outputs.get("a").expect("outputs present");
    let port = outputs.get("out").expect("out port present");
    assert!(matches!(port.shape.id, ShapeId::Scalar));
}

#[test]
fn it_should_error_when_shape_mismatches() {
    let mut node = constant_node("a", Value::Float(1.0));
    node.output_shapes
        .insert("out".to_string(), Shape::new(ShapeId::Vec3));

    let spec = GraphSpec { nodes: vec![node] };
    let mut rt = GraphRuntime::default();
    let err = evaluate_all(&mut rt, &spec).expect_err("should fail due to mismatch");
    assert!(err.contains("does not match declared shape"));
}

// --- Runtime outputs -----------------------------------------------------

#[test]
fn it_should_emit_write_for_output_nodes() {
    let mut output_inputs = HashMap::new();
    output_inputs.insert("in".to_string(), connection("src", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::Float(2.0)),
            NodeSpec {
                id: "out".to_string(),
                kind: NodeType::Output,
                params: NodeParams {
                    path: Some(TypedPath::parse("robot1/Arm/Joint.angle").expect("valid path")),
                    ..Default::default()
                },
                inputs: output_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    assert_eq!(rt.writes.iter().count(), 1);
    let op = rt.writes.iter().next().expect("write present");
    assert_eq!(op.path.to_string(), "robot1/Arm/Joint.angle");
    assert!(matches!(
        op.shape.as_ref().map(|s| &s.id),
        Some(ShapeId::Scalar)
    ));
    match op.value {
        Value::Float(f) => assert_eq!(f, 2.0),
        _ => panic!("expected float write"),
    }
}

#[test]
fn writes_batch_json_roundtrip_from_graph() {
    // Build a trivial graph that emits a write.
    let mut output_inputs = HashMap::new();
    output_inputs.insert("in".to_string(), connection("src", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::Vec3([1.0, 2.0, 3.0])),
            NodeSpec {
                id: "out".to_string(),
                kind: NodeType::Output,
                params: NodeParams {
                    path: Some(TypedPath::parse("robot/pose.pos").expect("valid path")),
                    ..Default::default()
                },
                inputs: output_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    let original = rt.writes.clone();

    let json = serde_json::to_string(&original).expect("serialize writes");
    let parsed: WriteBatch = serde_json::from_str(&json).expect("parse writes");
    assert_eq!(original, parsed, "writes batch should roundtrip via JSON");
}

// --- Variadic & oscillator behaviour ------------------------------------

#[test]
fn join_respects_operand_order() {
    let mut inputs = HashMap::new();
    inputs.insert("operands_1".to_string(), connection("a", "out"));
    inputs.insert("operands_2".to_string(), connection("b", "out"));
    inputs.insert("operands_3".to_string(), connection("c", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("a", Value::Vector(vec![1.0, 2.0])),
            constant_node("b", Value::Vector(vec![3.0])),
            constant_node("c", Value::Vector(vec![4.0, 5.0])),
            NodeSpec {
                id: "join".to_string(),
                kind: NodeType::Join,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("join should evaluate");
    let outputs = rt.outputs.get("join").expect("join outputs present");
    let port = outputs.get("out").expect("out port present");
    match &port.value {
        Value::Vector(vec) => assert_eq!(vec, &vec![1.0, 2.0, 3.0, 4.0, 5.0]),
        other => panic!("expected vector output, got {:?}", other),
    }
}

#[test]
fn oscillator_broadcasts_vector_inputs() {
    let mut inputs = HashMap::new();
    inputs.insert("frequency".to_string(), connection("freq", "out"));
    inputs.insert("phase".to_string(), connection("phase", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("freq", Value::Vector(vec![1.0, 2.0, 3.0])),
            constant_node("phase", Value::Float(0.0)),
            NodeSpec {
                id: "osc".to_string(),
                kind: NodeType::Oscillator,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime {
        t: 0.5,
        ..Default::default()
    };
    evaluate_all(&mut rt, &graph).expect("oscillator should evaluate");

    let outputs = rt.outputs.get("osc").expect("osc outputs present");
    let port = outputs.get("out").expect("osc out port present");
    let expected: Vec<f32> = vec![1.0, 2.0, 3.0]
        .into_iter()
        .map(|f| (std::f32::consts::TAU * f * rt.t).sin())
        .collect();

    match &port.value {
        Value::Vector(vec) => {
            assert_eq!(vec.len(), expected.len());
            for (actual, expected) in vec.iter().zip(expected.iter()) {
                assert!(
                    (actual - expected).abs() < 1e-6,
                    "expected {expected}, got {actual}"
                );
            }
        }
        other => panic!("expected vector output, got {:?}", other),
    }
}

// --- Shape inference -----------------------------------------------------

#[test]
fn it_should_infer_vector_length_hints() {
    let node = constant_node("vec", Value::Vector(vec![1.0, 2.0, 3.0]));
    let spec = GraphSpec { nodes: vec![node] };
    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("graph should evaluate");
    let outputs = rt.outputs.get("vec").expect("outputs present");
    let port = outputs.get("out").expect("out port");
    match &port.shape.id {
        ShapeId::Vector { len } => assert_eq!(*len, Some(3)),
        other => panic!("expected vector shape, got {:?}", other),
    }
}

// --- Declared shape error handling --------------------------------------

#[test]
fn it_should_error_when_declared_output_missing() {
    let mut node = constant_node("a", Value::Float(1.0));
    node.output_shapes
        .insert("secondary".to_string(), Shape::new(ShapeId::Scalar));

    let spec = GraphSpec { nodes: vec![node] };
    let mut rt = GraphRuntime::default();
    let err = evaluate_all(&mut rt, &spec).expect_err("missing declared output should error");
    assert!(err.contains("missing declared output"));
}

#[test]
fn it_should_validate_vector_length_against_declared_shape() {
    let mut node = constant_node("a", Value::Vector(vec![1.0, 2.0, 3.0]));
    node.output_shapes.insert(
        "out".to_string(),
        Shape::new(ShapeId::Vector { len: Some(4) }),
    );

    let spec = GraphSpec { nodes: vec![node] };
    let mut rt = GraphRuntime::default();
    let err = evaluate_all(&mut rt, &spec).expect_err("vector length mismatch should error");
    assert!(err.contains("does not match declared shape"));
}

#[test]
fn it_should_reject_invalid_paths_during_deserialization() {
    let json = r#"{
        "id": "node",
        "type": "output",
        "params": { "path": "robot/invalid/" },
        "inputs": {},
        "output_shapes": {}
    }"#;

    let err = serde_json::from_str::<NodeSpec>(json)
        .expect_err("invalid typed path should fail to parse");
    assert!(err.to_string().contains("path"));
}

// --- Staged input nodes & selectors -------------------------------------

#[test]
fn input_node_emits_staged_value_with_declared_shape() {
    let typed_path = TypedPath::parse("sensor/imu.accel").expect("valid path");

    let params = NodeParams {
        path: Some(typed_path.clone()),
        ..Default::default()
    };

    let mut output_shapes = HashMap::new();
    output_shapes.insert("out".to_string(), Shape::new(ShapeId::Vec3));

    let input_node = NodeSpec {
        id: "input".to_string(),
        kind: NodeType::Input,
        params,
        inputs: HashMap::new(),
        output_shapes,
    };

    let graph = GraphSpec {
        nodes: vec![input_node],
    };

    let mut rt = GraphRuntime::default();
    rt.set_input(
        typed_path,
        Value::Vec3([1.0, 2.0, 3.0]),
        Some(Shape::new(ShapeId::Vec3)),
    );

    evaluate_all(&mut rt, &graph).expect("input node should evaluate");

    let port = rt
        .outputs
        .get("input")
        .and_then(|outputs| outputs.get("out"))
        .expect("input output present");

    match &port.value {
        Value::Vec3(arr) => assert_eq!(*arr, [1.0, 2.0, 3.0]),
        other => panic!("expected Vec3, got {:?}", other),
    }
    assert!(matches!(port.shape.id, ShapeId::Vec3));
}

#[test]
fn input_node_coerces_vector_to_declared_vec3() {
    // Stage a generic numeric vector and declare the Input's shape as Vec3.
    // The node should coerce the vector to a Vec3 value rather than erroring.
    let typed_path = TypedPath::parse("sensor/imu.accel").expect("valid path");

    let params = NodeParams {
        path: Some(typed_path.clone()),
        ..Default::default()
    };

    let mut output_shapes = HashMap::new();
    output_shapes.insert("out".to_string(), Shape::new(ShapeId::Vec3));

    let graph = GraphSpec {
        nodes: vec![NodeSpec {
            id: "input".to_string(),
            kind: NodeType::Input,
            params,
            inputs: HashMap::new(),
            output_shapes,
        }],
    };

    let mut rt = GraphRuntime::default();
    // Staged value is a generic vector of the right length to coerce to Vec3.
    rt.set_input(
        typed_path,
        Value::Vector(vec![9.0, 8.0, 7.0]),
        Some(Shape::new(ShapeId::Vector { len: Some(3) })),
    );

    evaluate_all(&mut rt, &graph).expect("coercion should succeed");

    let port = rt
        .outputs
        .get("input")
        .and_then(|outputs| outputs.get("out"))
        .expect("input output present");

    match &port.value {
        Value::Vec3(arr) => assert_eq!(*arr, [9.0, 8.0, 7.0]),
        other => panic!("expected Vec3 after coercion, got {:?}", other),
    }
    assert!(matches!(port.shape.id, ShapeId::Vec3));
}

#[test]
fn input_node_missing_numeric_returns_null() {
    let typed_path = TypedPath::parse("sensor/imu.accel").expect("valid path");

    let params = NodeParams {
        path: Some(typed_path.clone()),
        ..Default::default()
    };

    let mut output_shapes = HashMap::new();
    output_shapes.insert("out".to_string(), Shape::new(ShapeId::Vec3));

    let graph = GraphSpec {
        nodes: vec![NodeSpec {
            id: "input".to_string(),
            kind: NodeType::Input,
            params,
            inputs: HashMap::new(),
            output_shapes,
        }],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("numeric shape should fall back to NaNs");

    let port = rt
        .outputs
        .get("input")
        .and_then(|outputs| outputs.get("out"))
        .expect("input output present");

    match &port.value {
        Value::Vec3(arr) => assert!(arr.iter().all(|v| v.is_nan())),
        other => panic!("expected Vec3 null, got {:?}", other),
    }
    assert!(matches!(port.shape.id, ShapeId::Vec3));
}

#[test]
fn input_node_missing_non_numeric_errors() {
    let typed_path = TypedPath::parse("sensor/name").expect("valid path");

    let params = NodeParams {
        path: Some(typed_path.clone()),
        ..Default::default()
    };

    let mut output_shapes = HashMap::new();
    output_shapes.insert("out".to_string(), Shape::new(ShapeId::Text));

    let graph = GraphSpec {
        nodes: vec![NodeSpec {
            id: "input".to_string(),
            kind: NodeType::Input,
            params,
            inputs: HashMap::new(),
            output_shapes,
        }],
    };

    let mut rt = GraphRuntime::default();
    let err = evaluate_all(&mut rt, &graph).expect_err("non-numeric shape should error");
    assert!(err.contains("missing staged value"));
}

#[test]
fn input_node_requires_restaging_each_epoch() {
    let typed_path = TypedPath::parse("sensor/imu.accel").expect("valid path");

    let params = NodeParams {
        path: Some(typed_path.clone()),
        ..Default::default()
    };

    let mut output_shapes = HashMap::new();
    output_shapes.insert("out".to_string(), Shape::new(ShapeId::Vec3));

    let graph = GraphSpec {
        nodes: vec![NodeSpec {
            id: "input".to_string(),
            kind: NodeType::Input,
            params,
            inputs: HashMap::new(),
            output_shapes,
        }],
    };

    let mut rt = GraphRuntime::default();
    rt.set_input(
        typed_path.clone(),
        Value::Vec3([1.0, 2.0, 3.0]),
        Some(Shape::new(ShapeId::Vec3)),
    );

    evaluate_all(&mut rt, &graph).expect("first frame should read staged value");
    let first = rt
        .outputs
        .get("input")
        .and_then(|outputs| outputs.get("out"))
        .expect("input output present");
    match &first.value {
        Value::Vec3(arr) => assert_eq!(*arr, [1.0, 2.0, 3.0]),
        other => panic!("expected Vec3, got {:?}", other),
    }

    evaluate_all(&mut rt, &graph).expect("second frame should evaluate");
    let second = rt
        .outputs
        .get("input")
        .and_then(|outputs| outputs.get("out"))
        .expect("input output present");
    match &second.value {
        Value::Vec3(arr) => assert!(arr.iter().all(|v| v.is_nan())),
        other => panic!("expected Vec3 null, got {:?}", other),
    }
}

#[test]
fn selector_projects_record_field() {
    let mut record = HashMap::new();
    record.insert("pos".to_string(), Value::Vec3([3.0, 4.0, 0.0]));
    record.insert("label".to_string(), Value::Text("ignored".to_string()));

    let mut inputs = HashMap::new();
    let mut conn = connection("src", "out");
    conn.selector = Some(vec![SelectorSegment::Field("pos".to_string())]);
    inputs.insert("in".to_string(), conn);

    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::Record(record)),
            NodeSpec {
                id: "out".to_string(),
                kind: NodeType::Output,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("selector projection should succeed");

    let port = rt
        .outputs
        .get("out")
        .and_then(|outputs| outputs.get("out"))
        .expect("output node present");

    match &port.value {
        Value::Vec3(arr) => assert_eq!(*arr, [3.0, 4.0, 0.0]),
        other => panic!("expected Vec3, got {:?}", other),
    }
}

#[test]
fn selector_projects_transform_field_and_nested_index() {
    // Source provides a Transform; downstream selects .pos then [1] (y component).
    let mut inputs = HashMap::new();
    let mut conn = connection("src", "out");
    conn.selector = Some(vec![
        SelectorSegment::Field("pos".to_string()),
        SelectorSegment::Index(1),
    ]);
    inputs.insert("in".to_string(), conn);

    let graph = GraphSpec {
        nodes: vec![
            constant_node(
                "src",
                Value::Transform {
                    pos: [10.0, 42.0, -1.0],
                    rot: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0, 1.0, 1.0],
                },
            ),
            NodeSpec {
                id: "out".to_string(),
                kind: NodeType::Output,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("selector projection should succeed");

    let port = rt
        .outputs
        .get("out")
        .and_then(|outputs| outputs.get("out"))
        .expect("output node present");

    match &port.value {
        Value::Float(f) => assert_eq!(*f, 42.0),
        other => panic!("expected scalar, got {:?}", other),
    }
    assert!(matches!(port.shape.id, ShapeId::Scalar));
}

#[test]
fn selector_index_out_of_bounds_errors() {
    // Select index 5 from a vec3; should error.
    let mut inputs = HashMap::new();
    let mut conn = connection("src", "out");
    conn.selector = Some(vec![SelectorSegment::Index(5)]);
    inputs.insert("in".to_string(), conn);

    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::Vec3([1.0, 2.0, 3.0])),
            NodeSpec {
                id: "out".to_string(),
                kind: NodeType::Output,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    let err = evaluate_all(&mut rt, &graph).expect_err("oob selector should error");
    assert!(
        err.contains("out of bounds"),
        "unexpected error content: {err}"
    );
}

// --- Stateful nodes ------------------------------------------------------

#[test]
fn spring_node_transitions_toward_new_target() {
    let mut spring_inputs = HashMap::new();
    spring_inputs.insert("in".to_string(), connection("target", "out"));

    let spring = NodeSpec {
        id: "spring".to_string(),
        kind: NodeType::Spring,
        params: NodeParams {
            stiffness: Some(30.0),
            damping: Some(6.0),
            mass: Some(1.0),
            ..Default::default()
        },
        inputs: spring_inputs,
        output_shapes: HashMap::new(),
    };

    let mut spec = GraphSpec {
        nodes: vec![constant_node("target", Value::Float(0.0)), spring],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("initial evaluate");

    spec.nodes[0].params.value = Some(Value::Float(10.0));

    rt.dt = 1.0 / 60.0;
    rt.t += rt.dt;
    evaluate_all(&mut rt, &spec).expect("first step");
    let first = match rt
        .outputs
        .get("spring")
        .and_then(|map| map.get("out"))
        .map(|pv| pv.value.clone())
        .expect("spring output")
    {
        Value::Float(f) => f,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (first - 10.0).abs() > 0.01,
        "spring should not immediately reach target"
    );

    for _ in 0..240 {
        rt.dt = 1.0 / 60.0;
        rt.t += rt.dt;
        evaluate_all(&mut rt, &spec).expect("subsequent step");
    }

    let final_val = match rt
        .outputs
        .get("spring")
        .and_then(|map| map.get("out"))
        .map(|pv| pv.value.clone())
        .expect("spring output")
    {
        Value::Float(f) => f,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (final_val - 10.0).abs() < 0.1,
        "spring should converge to target"
    );
}

#[test]
fn damp_node_smooths_toward_target() {
    let mut damp_inputs = HashMap::new();
    damp_inputs.insert("in".to_string(), connection("target", "out"));

    let damp = NodeSpec {
        id: "damp".to_string(),
        kind: NodeType::Damp,
        params: NodeParams {
            half_life: Some(0.2),
            ..Default::default()
        },
        inputs: damp_inputs,
        output_shapes: HashMap::new(),
    };

    let mut spec = GraphSpec {
        nodes: vec![constant_node("target", Value::Float(0.0)), damp],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("initial evaluate");

    spec.nodes[0].params.value = Some(Value::Float(1.0));
    rt.dt = 0.1;
    rt.t += rt.dt;
    evaluate_all(&mut rt, &spec).expect("first step");

    let first = match rt
        .outputs
        .get("damp")
        .and_then(|map| map.get("out"))
        .map(|pv| pv.value.clone())
        .expect("damp output")
    {
        Value::Float(f) => f,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(first > 0.0 && first < 1.0, "damp should move but not snap");

    for _ in 0..20 {
        rt.dt = 0.1;
        rt.t += rt.dt;
        evaluate_all(&mut rt, &spec).expect("subsequent step");
    }

    let final_val = match rt
        .outputs
        .get("damp")
        .and_then(|map| map.get("out"))
        .map(|pv| pv.value.clone())
        .expect("damp output")
    {
        Value::Float(f) => f,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (final_val - 1.0).abs() < 0.05,
        "damp should approach target"
    );
}

#[test]
fn slew_node_limits_rate_of_change() {
    let mut slew_inputs = HashMap::new();
    slew_inputs.insert("in".to_string(), connection("target", "out"));

    let slew = NodeSpec {
        id: "slew".to_string(),
        kind: NodeType::Slew,
        params: NodeParams {
            max_rate: Some(2.0),
            ..Default::default()
        },
        inputs: slew_inputs,
        output_shapes: HashMap::new(),
    };

    let mut spec = GraphSpec {
        nodes: vec![constant_node("target", Value::Float(0.0)), slew],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("initial evaluate");

    spec.nodes[0].params.value = Some(Value::Float(5.0));
    rt.dt = 0.25;
    rt.t += rt.dt;
    evaluate_all(&mut rt, &spec).expect("slew step");

    let first = match rt
        .outputs
        .get("slew")
        .and_then(|map| map.get("out"))
        .map(|pv| pv.value.clone())
        .expect("slew output")
    {
        Value::Float(f) => f,
        other => panic!("expected float, got {:?}", other),
    };

    assert!(
        (first - 0.5).abs() < 1e-6,
        "slew should move at configured rate"
    );

    for _ in 0..10 {
        rt.dt = 0.25;
        rt.t += rt.dt;
        evaluate_all(&mut rt, &spec).expect("subsequent step");
    }

    let final_val = match rt
        .outputs
        .get("slew")
        .and_then(|map| map.get("out"))
        .map(|pv| pv.value.clone())
        .expect("slew output")
    {
        Value::Float(f) => f,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (final_val - 5.0).abs() < 0.25,
        "slew should eventually reach target"
    );
}

// --- Blend helpers -------------------------------------------------------

#[test]
fn blend_nodes_reproduce_manual_math() {
    let values = vec![0.2f32, 0.8, 0.4];
    let weights = vec![0.5f32, 0.3, 0.2];
    let masks = vec![1.0f32, 0.0, 1.0];
    let base_value = 0.25f32;
    let diff_values: Vec<f32> = values.iter().map(|v| *v - base_value).collect();

    let len = values.len().max(weights.len()).max(masks.len());
    let mut expected_sum = 0.0f32;
    let mut expected_weight_sum = 0.0f32;
    let mut expected_max_weight = f32::NEG_INFINITY;
    let mut expected_product = 1.0f32;
    for i in 0..len {
        let value = *values.get(i).unwrap_or(&0.0);
        let weight = *weights.get(i).unwrap_or(&0.0);
        let mask = *masks.get(i).unwrap_or(&0.0);
        let weighted = value * weight * mask;
        expected_sum += weighted;
        let effective_weight = weight * mask;
        expected_weight_sum += effective_weight;
        expected_max_weight = expected_max_weight.max(effective_weight);
        let factor = 1.0 - weight + value * weight * mask;
        expected_product *= factor;
    }
    if expected_max_weight == f32::NEG_INFINITY {
        expected_max_weight = 0.0;
    }
    let expected_additive = if expected_weight_sum > 0.0 {
        expected_sum
    } else {
        base_value
    };
    let expected_average = if expected_weight_sum > 0.0 && expected_max_weight > 0.0 {
        expected_sum / (expected_weight_sum / expected_max_weight)
    } else {
        base_value
    };
    let expected_overlay =
        base_value * (1.0 - expected_max_weight) + expected_sum * expected_max_weight;

    let len_diff = diff_values.len().max(weights.len()).max(masks.len());
    let mut expected_diff_sum = 0.0f32;
    let mut expected_diff_weight_sum = 0.0f32;
    let mut expected_diff_max_weight = f32::NEG_INFINITY;
    for i in 0..len_diff {
        let value = *diff_values.get(i).unwrap_or(&0.0);
        let weight = *weights.get(i).unwrap_or(&0.0);
        let mask = *masks.get(i).unwrap_or(&0.0);
        let weighted = value * weight * mask;
        expected_diff_sum += weighted;
        let effective_weight = weight * mask;
        expected_diff_weight_sum += effective_weight;
        expected_diff_max_weight = expected_diff_max_weight.max(effective_weight);
    }
    if expected_diff_max_weight == f32::NEG_INFINITY {
        expected_diff_max_weight = 0.0;
    }
    let expected_average_overlay =
        if expected_diff_weight_sum > 0.0 && expected_diff_max_weight > 0.0 {
            base_value + expected_diff_sum / (expected_diff_weight_sum / expected_diff_max_weight)
        } else {
            base_value
        };

    let mut expected_max_value = base_value;
    if let Some((idx, weight)) = weights
        .iter()
        .enumerate()
        .filter(|(_, w)| w.is_finite())
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
    {
        if let Some(value) = values.get(idx) {
            if value.is_finite() {
                expected_max_value = value * weight;
            }
        }
    }

    let mut sum_inputs = HashMap::new();
    sum_inputs.insert("values".to_string(), connection("values", "out"));
    sum_inputs.insert("weights".to_string(), connection("weights", "out"));
    sum_inputs.insert("masks".to_string(), connection("masks", "out"));

    let mut diff_inputs = HashMap::new();
    diff_inputs.insert("values".to_string(), connection("diff_values", "out"));
    diff_inputs.insert("weights".to_string(), connection("weights", "out"));
    diff_inputs.insert("masks".to_string(), connection("masks", "out"));

    let mut additive_inputs = HashMap::new();
    additive_inputs.insert("sum".to_string(), connection("wsum", "sum"));
    additive_inputs.insert("weight_sum".to_string(), connection("wsum", "weight_sum"));
    additive_inputs.insert("fallback".to_string(), connection("base", "out"));

    let mut average_inputs = HashMap::new();
    average_inputs.insert("sum".to_string(), connection("wsum", "sum"));
    average_inputs.insert("weight_sum".to_string(), connection("wsum", "weight_sum"));
    average_inputs.insert("max_weight".to_string(), connection("wsum", "max_weight"));
    average_inputs.insert("fallback".to_string(), connection("base", "out"));

    let mut overlay_inputs = HashMap::new();
    overlay_inputs.insert("sum".to_string(), connection("wsum", "sum"));
    overlay_inputs.insert("max_weight".to_string(), connection("wsum", "max_weight"));
    overlay_inputs.insert("base".to_string(), connection("base", "out"));

    let mut average_overlay_inputs = HashMap::new();
    average_overlay_inputs.insert("sum".to_string(), connection("wsum_diff", "sum"));
    average_overlay_inputs.insert(
        "weight_sum".to_string(),
        connection("wsum_diff", "weight_sum"),
    );
    average_overlay_inputs.insert(
        "max_weight".to_string(),
        connection("wsum_diff", "max_weight"),
    );
    average_overlay_inputs.insert("base".to_string(), connection("base", "out"));

    let mut multiply_inputs = HashMap::new();
    multiply_inputs.insert("values".to_string(), connection("values", "out"));
    multiply_inputs.insert("weights".to_string(), connection("weights", "out"));
    multiply_inputs.insert("masks".to_string(), connection("masks", "out"));

    let mut max_inputs = HashMap::new();
    max_inputs.insert("values".to_string(), connection("values", "out"));
    max_inputs.insert("weights".to_string(), connection("weights", "out"));
    max_inputs.insert("base".to_string(), connection("base", "out"));

    let mut max_fallback_inputs = HashMap::new();
    max_fallback_inputs.insert("values".to_string(), connection("values_nan", "out"));
    max_fallback_inputs.insert("weights".to_string(), connection("weights_nan", "out"));
    max_fallback_inputs.insert("base".to_string(), connection("base_nan", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values.clone())),
            constant_node("weights", Value::Vector(weights.clone())),
            constant_node("masks", Value::Vector(masks.clone())),
            constant_node("base", Value::Float(base_value)),
            constant_node("diff_values", Value::Vector(diff_values.clone())),
            constant_node("values_nan", Value::Vector(vec![f32::NAN, 0.7])),
            constant_node("weights_nan", Value::Vector(vec![0.9, 0.4])),
            constant_node("base_nan", Value::Float(0.5)),
            NodeSpec {
                id: "wsum".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                inputs: sum_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "wsum_diff".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                inputs: diff_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "additive".to_string(),
                kind: NodeType::BlendAdditive,
                params: NodeParams::default(),
                inputs: additive_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "average".to_string(),
                kind: NodeType::BlendWeightedAverage,
                params: NodeParams::default(),
                inputs: average_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "overlay".to_string(),
                kind: NodeType::BlendWeightedOverlay,
                params: NodeParams::default(),
                inputs: overlay_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "average_overlay".to_string(),
                kind: NodeType::BlendWeightedAverageOverlay,
                params: NodeParams::default(),
                inputs: average_overlay_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "multiply".to_string(),
                kind: NodeType::BlendMultiply,
                params: NodeParams::default(),
                inputs: multiply_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "max".to_string(),
                kind: NodeType::BlendMax,
                params: NodeParams::default(),
                inputs: max_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "max_fallback".to_string(),
                kind: NodeType::BlendMax,
                params: NodeParams::default(),
                inputs: max_fallback_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("blend graph should evaluate");

    let wsum_outputs = rt.outputs.get("wsum").expect("weighted sum outputs");
    let sum_value = match &wsum_outputs.get("sum").expect("sum output").value {
        Value::Float(f) => *f,
        other => panic!("expected float sum, got {:?}", other),
    };
    let weight_sum_value = match &wsum_outputs
        .get("weight_sum")
        .expect("weight sum output")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float weight_sum, got {:?}", other),
    };
    let max_weight_value = match &wsum_outputs
        .get("max_weight")
        .expect("max weight output")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float max_weight, got {:?}", other),
    };

    let wsum_diff_outputs = rt
        .outputs
        .get("wsum_diff")
        .expect("weighted diff sum outputs");
    let diff_sum_value = match &wsum_diff_outputs.get("sum").expect("diff sum").value {
        Value::Float(f) => *f,
        other => panic!("expected float diff sum, got {:?}", other),
    };
    let diff_weight_sum_value = match &wsum_diff_outputs
        .get("weight_sum")
        .expect("diff weight sum")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float diff weight_sum, got {:?}", other),
    };
    let diff_max_weight_value = match &wsum_diff_outputs
        .get("max_weight")
        .expect("diff max weight")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float diff max_weight, got {:?}", other),
    };

    let additive_value = match &rt
        .outputs
        .get("additive")
        .and_then(|map| map.get("out"))
        .expect("additive output")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float additive, got {:?}", other),
    };

    let average_value = match &rt
        .outputs
        .get("average")
        .and_then(|map| map.get("out"))
        .expect("average output")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float average, got {:?}", other),
    };

    let overlay_value = match &rt
        .outputs
        .get("overlay")
        .and_then(|map| map.get("out"))
        .expect("overlay output")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float overlay, got {:?}", other),
    };

    let average_overlay_value = match &rt
        .outputs
        .get("average_overlay")
        .and_then(|map| map.get("out"))
        .expect("average overlay output")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float average overlay, got {:?}", other),
    };

    let multiply_value = match &rt
        .outputs
        .get("multiply")
        .and_then(|map| map.get("out"))
        .expect("multiply output")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float multiply, got {:?}", other),
    };

    let max_value = match &rt
        .outputs
        .get("max")
        .and_then(|map| map.get("out"))
        .expect("max output")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float max, got {:?}", other),
    };

    let max_fallback_value = match &rt
        .outputs
        .get("max_fallback")
        .and_then(|map| map.get("out"))
        .expect("max fallback output")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float max fallback, got {:?}", other),
    };

    assert!(
        (sum_value - expected_sum).abs() < 1e-6,
        "expected sum {expected_sum}, got {sum_value}"
    );
    assert!(
        (weight_sum_value - expected_weight_sum).abs() < 1e-6,
        "expected weight sum {expected_weight_sum}, got {weight_sum_value}"
    );
    assert!(
        (max_weight_value - expected_max_weight).abs() < 1e-6,
        "expected max weight {expected_max_weight}, got {max_weight_value}"
    );
    assert!(
        (diff_sum_value - expected_diff_sum).abs() < 1e-6,
        "expected diff sum {expected_diff_sum}, got {diff_sum_value}"
    );
    assert!(
        (diff_weight_sum_value - expected_diff_weight_sum).abs() < 1e-6,
        "expected diff weight sum {expected_diff_weight_sum}, got {diff_weight_sum_value}"
    );
    assert!(
        (diff_max_weight_value - expected_diff_max_weight).abs() < 1e-6,
        "expected diff max weight {expected_diff_max_weight}, got {diff_max_weight_value}"
    );
    assert!(
        (additive_value - expected_additive).abs() < 1e-6,
        "expected additive {expected_additive}, got {additive_value}"
    );
    assert!(
        (average_value - expected_average).abs() < 1e-6,
        "expected average {expected_average}, got {average_value}"
    );
    assert!(
        (overlay_value - expected_overlay).abs() < 1e-6,
        "expected overlay {expected_overlay}, got {overlay_value}"
    );
    assert!(
        (average_overlay_value - expected_average_overlay).abs() < 1e-6,
        "expected average overlay {expected_average_overlay}, got {average_overlay_value}"
    );
    assert!(
        (multiply_value - expected_product).abs() < 1e-6,
        "expected multiply {expected_product}, got {multiply_value}"
    );
    assert!(
        (max_value - expected_max_value).abs() < 1e-6,
        "expected max {expected_max_value}, got {max_value}"
    );
    assert!(
        (max_fallback_value - 0.5).abs() < 1e-6,
        "expected fallback max 0.5, got {max_fallback_value}"
    );
}

#[test]
fn case_node_routes_by_label() {
    let case_params = NodeParams {
        case_labels: Some(vec!["a".to_string(), "b".to_string()]),
        ..Default::default()
    };

    let mut case_inputs = HashMap::new();
    case_inputs.insert("selector".to_string(), connection("selector_b", "out"));
    case_inputs.insert("default".to_string(), connection("default", "out"));
    case_inputs.insert("options_1".to_string(), connection("case_a", "out"));
    case_inputs.insert("options_2".to_string(), connection("case_b", "out"));

    let mut case_no_match_inputs = HashMap::new();
    case_no_match_inputs.insert("selector".to_string(), connection("selector_other", "out"));
    case_no_match_inputs.insert("default".to_string(), connection("default", "out"));
    case_no_match_inputs.insert("options_1".to_string(), connection("case_a", "out"));
    case_no_match_inputs.insert("options_2".to_string(), connection("case_b", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("selector_b", Value::Text("b".to_string())),
            constant_node("selector_other", Value::Text("c".to_string())),
            constant_node("default", Value::Float(-1.0)),
            constant_node("case_a", Value::Float(1.0)),
            constant_node("case_b", Value::Float(2.0)),
            NodeSpec {
                id: "case".to_string(),
                kind: NodeType::Case,
                params: case_params.clone(),
                inputs: case_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "case_no_match".to_string(),
                kind: NodeType::Case,
                params: case_params,
                inputs: case_no_match_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("case graph should evaluate");

    let matched = match &rt
        .outputs
        .get("case")
        .and_then(|map| map.get("out"))
        .expect("matched case output")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float case value, got {:?}", other),
    };
    let unmatched = match &rt
        .outputs
        .get("case_no_match")
        .and_then(|map| map.get("out"))
        .expect("defaulted case output")
        .value
    {
        Value::Float(f) => *f,
        other => panic!("expected float fallback, got {:?}", other),
    };

    assert!(
        (matched - 2.0).abs() < 1e-6,
        "expected matched case to yield 2.0"
    );
    assert!(
        (unmatched + 1.0).abs() < 1e-6,
        "expected unmatched case to return default -1.0"
    );
}

// --- End-to-end: Input → selector → math → Output ------------------------

#[test]
fn end_to_end_input_selector_scalar_math_output() {
    // Build Input node producing a record { pos: vec3, label: text } with a declared record shape.
    let typed_path = TypedPath::parse("sensor/pose").expect("valid path");

    // Declared output shape for the Input node: Record { pos: Vec3, label: Text }
    let declared = Shape::new(ShapeId::Record(vec![
        Field {
            name: "label".to_string(),
            shape: ShapeId::Text,
        },
        Field {
            name: "pos".to_string(),
            shape: ShapeId::Vec3,
        },
    ]));

    let input_params = NodeParams {
        path: Some(typed_path.clone()),
        ..Default::default()
    };

    let mut input_output_shapes = HashMap::new();
    input_output_shapes.insert("out".to_string(), declared.clone());

    let input_node = NodeSpec {
        id: "in".to_string(),
        kind: NodeType::Input,
        params: input_params,
        inputs: HashMap::new(),
        output_shapes: input_output_shapes,
    };

    // Add node: Add(lhs, rhs) where lhs is selector ["pos", 1] (y) and rhs is constant 2.0
    let mut add_inputs = HashMap::new();
    // Connection from Input.out with selector ["pos", 1]
    let mut lhs_conn = connection("in", "out");
    lhs_conn.selector = Some(vec![
        SelectorSegment::Field("pos".to_string()),
        SelectorSegment::Index(1),
    ]);
    add_inputs.insert("lhs".to_string(), lhs_conn);
    add_inputs.insert("rhs".to_string(), connection("two", "out"));

    let add_node = NodeSpec {
        id: "sum".to_string(),
        kind: NodeType::Add,
        params: NodeParams::default(),
        inputs: add_inputs,
        output_shapes: HashMap::new(),
    };

    // Output sink
    let mut out_inputs = HashMap::new();
    out_inputs.insert("in".to_string(), connection("sum", "out"));

    let output_node = NodeSpec {
        id: "out".to_string(),
        kind: NodeType::Output,
        params: NodeParams {
            path: Some(TypedPath::parse("robot/calc.y2").expect("valid path")),
            ..Default::default()
        },
        inputs: out_inputs,
        output_shapes: HashMap::new(),
    };

    let graph = GraphSpec {
        nodes: vec![
            input_node,
            constant_node("two", Value::Float(2.0)),
            add_node,
            output_node,
        ],
    };

    // Stage record { pos: [1, 3, 5], label: "ok" } for the Input node.
    let mut record = HashMap::new();
    record.insert("pos".to_string(), Value::Vec3([1.0, 3.0, 5.0]));
    record.insert("label".to_string(), Value::Text("ok".to_string()));

    let mut rt = GraphRuntime::default();
    rt.set_input(typed_path, Value::Record(record), Some(declared));

    evaluate_all(&mut rt, &graph).expect("end-to-end evaluation");

    // Assert final write to Output node path has scalar shape and expected value 3 + 2 = 5
    let writes: Vec<_> = rt
        .writes
        .iter()
        .filter(|op| op.path.to_string() == "robot/calc.y2")
        .collect();
    assert_eq!(
        writes.len(),
        1,
        "expected exactly one write to the Output node path"
    );
    let op = writes[0];
    assert!(matches!(
        op.shape.as_ref().map(|s| &s.id),
        Some(ShapeId::Scalar)
    ));
    match op.value {
        Value::Float(f) => assert!((f - 5.0).abs() < 1e-6),
        ref other => panic!("expected scalar value, got {:?}", other),
    }
}
