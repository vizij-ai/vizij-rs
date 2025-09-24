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

// --- Blend helpers ------------------------------------------------------

#[test]
fn vector_weighted_sum_matches_manual_math() {
    let values = vec![1.0, 2.0, 3.0];
    let weights = vec![0.5, 0.25, 0.25];
    let mask = vec![1.0, 0.0, 1.0];

    let expected_weighted_sum: f32 = values
        .iter()
        .zip(weights.iter())
        .zip(mask.iter())
        .map(|((value, weight), mask)| value * weight * mask)
        .sum();
    let expected_total_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| weight * mask)
        .sum();
    let expected_max_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| weight * mask)
        .fold(0.0, |acc, value| acc.max(value));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values.clone())),
            constant_node("weights", Value::Vector(weights.clone())),
            constant_node("mask", Value::Vector(mask.clone())),
            NodeSpec {
                id: "sum".to_string(),
                kind: NodeType::VectorWeightedSum,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    ("values".to_string(), connection("values", "out")),
                    ("weights".to_string(), connection("weights", "out")),
                    ("mask".to_string(), connection("mask", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("weighted sum should evaluate");

    let outputs = rt.outputs.get("sum").expect("weighted sum outputs");
    let weighted_sum = match outputs
        .get("weighted_sum")
        .expect("weighted_sum port")
        .value
    {
        Value::Float(f) => f,
        ref other => panic!("expected float, got {:?}", other),
    };
    let total_weight = match outputs
        .get("total_weight")
        .expect("total_weight port")
        .value
    {
        Value::Float(f) => f,
        ref other => panic!("expected float, got {:?}", other),
    };
    let max_weight = match outputs.get("max_weight").expect("max_weight port").value {
        Value::Float(f) => f,
        ref other => panic!("expected float, got {:?}", other),
    };

    assert!((weighted_sum - expected_weighted_sum).abs() < 1e-6);
    assert!((total_weight - expected_total_weight).abs() < 1e-6);
    assert!((max_weight - expected_max_weight).abs() < 1e-6);
}

#[test]
fn blend_weighted_average_and_additive_match_manual_math() {
    let values = vec![1.0, 2.0, 3.0];
    let weights = vec![0.5, 0.25, 0.25];
    let mask = vec![1.0, 0.0, 1.0];
    let fallback_base = 10.0;

    let weighted_sum: f32 = values
        .iter()
        .zip(weights.iter())
        .zip(mask.iter())
        .map(|((value, weight), mask)| value * weight * mask)
        .sum();
    let total_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| weight * mask)
        .sum();
    let max_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| weight * mask)
        .fold(0.0, |acc, value| acc.max(value));
    let expected_average = weighted_sum / (total_weight / max_weight);

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values)),
            constant_node("weights", Value::Vector(weights)),
            constant_node("mask", Value::Vector(mask)),
            constant_node("base", Value::Float(fallback_base)),
            NodeSpec {
                id: "sum".to_string(),
                kind: NodeType::VectorWeightedSum,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    ("values".to_string(), connection("values", "out")),
                    ("weights".to_string(), connection("weights", "out")),
                    ("mask".to_string(), connection("mask", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "average".to_string(),
                kind: NodeType::BlendWeightedAverage,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    (
                        "weighted_sum".to_string(),
                        connection("sum", "weighted_sum"),
                    ),
                    (
                        "total_weight".to_string(),
                        connection("sum", "total_weight"),
                    ),
                    ("max_weight".to_string(), connection("sum", "max_weight")),
                    ("fallback".to_string(), connection("base", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "additive".to_string(),
                kind: NodeType::BlendAdditive,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    (
                        "weighted_sum".to_string(),
                        connection("sum", "weighted_sum"),
                    ),
                    (
                        "total_weight".to_string(),
                        connection("sum", "total_weight"),
                    ),
                    ("fallback".to_string(), connection("base", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("blend nodes should evaluate");

    let average_value = match rt
        .outputs
        .get("average")
        .and_then(|map| map.get("out"))
        .expect("average output")
        .value
    {
        Value::Float(f) => f,
        ref other => panic!("expected float, got {:?}", other),
    };
    let additive_value = match rt
        .outputs
        .get("additive")
        .and_then(|map| map.get("out"))
        .expect("additive output")
        .value
    {
        Value::Float(f) => f,
        ref other => panic!("expected float, got {:?}", other),
    };

    assert!((average_value - expected_average).abs() < 1e-6);
    assert!((additive_value - weighted_sum).abs() < 1e-6);
}

#[test]
fn blend_weighted_overlay_combines_with_base() {
    let values = vec![1.5, 0.5];
    let weights = vec![0.6, 0.4];
    let mask = vec![1.0, 1.0];
    let base_value = 2.0;

    let weighted_sum: f32 = values
        .iter()
        .zip(weights.iter())
        .zip(mask.iter())
        .map(|((value, weight), mask)| value * weight * mask)
        .sum();
    let max_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| weight * mask)
        .fold(0.0, |acc, value| acc.max(value));
    let expected = base_value * (1.0 - max_weight) + weighted_sum * max_weight;

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values)),
            constant_node("weights", Value::Vector(weights)),
            constant_node("mask", Value::Vector(mask)),
            constant_node("base", Value::Float(base_value)),
            NodeSpec {
                id: "sum".to_string(),
                kind: NodeType::VectorWeightedSum,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    ("values".to_string(), connection("values", "out")),
                    ("weights".to_string(), connection("weights", "out")),
                    ("mask".to_string(), connection("mask", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "overlay".to_string(),
                kind: NodeType::BlendWeightedOverlay,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    (
                        "weighted_sum".to_string(),
                        connection("sum", "weighted_sum"),
                    ),
                    ("max_weight".to_string(), connection("sum", "max_weight")),
                    ("base".to_string(), connection("base", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("overlay should evaluate");

    let overlay_value = match rt
        .outputs
        .get("overlay")
        .and_then(|map| map.get("out"))
        .expect("overlay output")
        .value
    {
        Value::Float(f) => f,
        ref other => panic!("expected float, got {:?}", other),
    };

    assert!((overlay_value - expected).abs() < 1e-6);
}

#[test]
fn blend_weighted_average_overlay_matches_manual_math() {
    let values = vec![0.4, -0.1, 0.2];
    let weights = vec![0.5, 0.25, 0.25];
    let mask = vec![1.0, 1.0, 0.0];
    let base_value = 1.5;

    let weighted_sum: f32 = values
        .iter()
        .zip(weights.iter())
        .zip(mask.iter())
        .map(|((value, weight), mask)| value * weight * mask)
        .sum();
    let total_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| weight * mask)
        .sum();
    let max_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| weight * mask)
        .fold(0.0, |acc, value| acc.max(value));
    let expected_average = weighted_sum / (total_weight / max_weight);
    let expected = base_value + expected_average;

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values)),
            constant_node("weights", Value::Vector(weights)),
            constant_node("mask", Value::Vector(mask)),
            constant_node("base", Value::Float(base_value)),
            NodeSpec {
                id: "sum".to_string(),
                kind: NodeType::VectorWeightedSum,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    ("values".to_string(), connection("values", "out")),
                    ("weights".to_string(), connection("weights", "out")),
                    ("mask".to_string(), connection("mask", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "overlay".to_string(),
                kind: NodeType::BlendWeightedAverageOverlay,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    (
                        "weighted_sum".to_string(),
                        connection("sum", "weighted_sum"),
                    ),
                    (
                        "total_weight".to_string(),
                        connection("sum", "total_weight"),
                    ),
                    ("max_weight".to_string(), connection("sum", "max_weight")),
                    ("base".to_string(), connection("base", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("average overlay should evaluate");

    let overlay_value = match rt
        .outputs
        .get("overlay")
        .and_then(|map| map.get("out"))
        .expect("overlay output")
        .value
    {
        Value::Float(f) => f,
        ref other => panic!("expected float, got {:?}", other),
    };

    assert!((overlay_value - expected).abs() < 1e-6);
}

#[test]
fn blend_multiply_matches_manual_math() {
    let values = vec![0.5, 0.5];
    let weights = vec![0.2, 0.4];
    let mask = vec![1.0, 1.0];

    let expected_product: f32 = values.iter().zip(weights.iter()).zip(mask.iter()).fold(
        1.0,
        |acc, ((value, weight), mask)| {
            let factor = 1.0 - weight + value * weight * mask;
            acc * factor
        },
    );

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values)),
            constant_node("weights", Value::Vector(weights)),
            constant_node("mask", Value::Vector(mask)),
            NodeSpec {
                id: "multiply".to_string(),
                kind: NodeType::BlendMultiply,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    ("values".to_string(), connection("values", "out")),
                    ("weights".to_string(), connection("weights", "out")),
                    ("mask".to_string(), connection("mask", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("multiply should evaluate");

    let product = match rt
        .outputs
        .get("multiply")
        .and_then(|map| map.get("out"))
        .expect("multiply output")
        .value
    {
        Value::Float(f) => f,
        ref other => panic!("expected float, got {:?}", other),
    };

    assert!((product - expected_product).abs() < 1e-6);
}

#[test]
fn blend_max_respects_mask_and_fallback() {
    let values = vec![1.0, 2.0, 3.0];
    let weights = vec![0.2, 0.8, 0.6];
    let mask_primary = vec![1.0, 1.0, 0.0];
    let mask_none = vec![0.0, 0.0, 1.0];
    let fallback_value = 5.0;

    let expected_primary = values[1] * weights[1];

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values)),
            constant_node("weights", Value::Vector(weights)),
            constant_node("mask_primary", Value::Vector(mask_primary)),
            constant_node("mask_none", Value::Vector(mask_none)),
            constant_node("fallback", Value::Float(fallback_value)),
            NodeSpec {
                id: "max_primary".to_string(),
                kind: NodeType::BlendMax,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    ("values".to_string(), connection("values", "out")),
                    ("weights".to_string(), connection("weights", "out")),
                    ("mask".to_string(), connection("mask_primary", "out")),
                    ("fallback".to_string(), connection("fallback", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "max_fallback".to_string(),
                kind: NodeType::BlendMax,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    ("values".to_string(), connection("values", "out")),
                    ("weights".to_string(), connection("weights", "out")),
                    ("mask".to_string(), connection("mask_none", "out")),
                    ("fallback".to_string(), connection("fallback", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("max nodes should evaluate");

    let primary = match rt
        .outputs
        .get("max_primary")
        .and_then(|map| map.get("out"))
        .expect("primary max output")
        .value
    {
        Value::Float(f) => f,
        ref other => panic!("expected float, got {:?}", other),
    };
    let fallback = match rt
        .outputs
        .get("max_fallback")
        .and_then(|map| map.get("out"))
        .expect("fallback max output")
        .value
    {
        Value::Float(f) => f,
        ref other => panic!("expected float, got {:?}", other),
    };

    assert!((primary - expected_primary).abs() < 1e-6);
    assert!((fallback - fallback_value).abs() < 1e-6);
}

#[test]
fn case_node_routes_named_ports() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("selector_match", Value::Text("additive".to_string())),
            constant_node("selector_unknown", Value::Text("unknown".to_string())),
            constant_node("weighted_average", Value::Float(1.5)),
            constant_node("additive", Value::Float(2.0)),
            constant_node("multiply", Value::Float(3.0)),
            constant_node("fallback", Value::Float(10.0)),
            NodeSpec {
                id: "case_match".to_string(),
                kind: NodeType::Case,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    ("selector".to_string(), connection("selector_match", "out")),
                    ("default".to_string(), connection("fallback", "out")),
                    (
                        "weighted_average".to_string(),
                        connection("weighted_average", "out"),
                    ),
                    ("additive".to_string(), connection("additive", "out")),
                    ("multiply".to_string(), connection("multiply", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "case_default".to_string(),
                kind: NodeType::Case,
                params: NodeParams::default(),
                inputs: HashMap::from([
                    (
                        "selector".to_string(),
                        connection("selector_unknown", "out"),
                    ),
                    ("default".to_string(), connection("fallback", "out")),
                    (
                        "weighted_average".to_string(),
                        connection("weighted_average", "out"),
                    ),
                    ("additive".to_string(), connection("additive", "out")),
                ]),
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("case nodes should evaluate");

    let matched = match rt
        .outputs
        .get("case_match")
        .and_then(|map| map.get("out"))
        .expect("case match output")
        .value
    {
        Value::Float(f) => f,
        ref other => panic!("expected float, got {:?}", other),
    };
    let fallback = match rt
        .outputs
        .get("case_default")
        .and_then(|map| map.get("out"))
        .expect("case default output")
        .value
    {
        Value::Float(f) => f,
        ref other => panic!("expected float, got {:?}", other),
    };

    assert!((matched - 2.0).abs() < 1e-6);
    assert!((fallback - 10.0).abs() < 1e-6);
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
