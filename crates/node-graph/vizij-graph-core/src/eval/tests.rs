//! Behavioural coverage for the evaluation pipeline.

use super::*;
use crate::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, InputDefault, NodeParams, NodeSpec,
    NodeType, SelectorSegment,
};
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
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    }
}

fn link(from: &str, to: &str, input: &str) -> EdgeSpec {
    link_with_output(from, "out", to, input)
}

fn link_with_output(from: &str, output_key: &str, to: &str, input: &str) -> EdgeSpec {
    EdgeSpec {
        from: EdgeOutputEndpoint {
            node_id: from.to_string(),
            output: output_key.to_string(),
        },
        to: EdgeInputEndpoint {
            node_id: to.to_string(),
            input: input.to_string(),
        },
        selector: None,
    }
}

fn link_with_selector(
    from: &str,
    output_key: &str,
    to: &str,
    input: &str,
    selector: Vec<SelectorSegment>,
) -> EdgeSpec {
    EdgeSpec {
        selector: Some(selector),
        ..link_with_output(from, output_key, to, input)
    }
}

// --- Shape validation ----------------------------------------------------

#[test]
fn it_should_respect_declared_shape() {
    let mut node = constant_node("a", Value::Float(1.0));
    node.output_shapes
        .insert("out".to_string(), Shape::new(ShapeId::Scalar));

    let spec = GraphSpec {
        nodes: vec![node],
        ..Default::default()
    };
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

    let spec = GraphSpec {
        nodes: vec![node],
        ..Default::default()
    };
    let mut rt = GraphRuntime::default();
    let err = evaluate_all(&mut rt, &spec).expect_err("should fail due to mismatch");
    assert!(err.contains("does not match declared shape"));
}

// --- Runtime outputs -----------------------------------------------------

#[test]
fn it_should_emit_write_for_output_nodes() {
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
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link("src", "out", "in")],
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
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::Vec3([1.0, 2.0, 3.0])),
            NodeSpec {
                id: "out".to_string(),
                kind: NodeType::Output,
                params: NodeParams {
                    path: Some(TypedPath::parse("robot/pose.translation").expect("valid path")),
                    ..Default::default()
                },
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link("src", "out", "in")],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    let original = rt.writes.clone();

    let json = serde_json::to_string(&original).expect("serialize writes");
    let parsed: WriteBatch = serde_json::from_str(&json).expect("parse writes");
    assert_eq!(original, parsed, "writes batch should roundtrip via JSON");
}

#[test]
fn input_defaults_supply_missing_connections() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "rhs".to_string(),
        InputDefault {
            value: Value::Float(2.0),
            shape: None,
        },
    );

    let graph = GraphSpec {
        nodes: vec![
            constant_node("numerator", Value::Float(10.0)),
            NodeSpec {
                id: "div".to_string(),
                kind: NodeType::Divide,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: defaults,
            },
        ],
        edges: vec![link("numerator", "div", "lhs")],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("defaults should evaluate");
    let outputs = rt.outputs.get("div").expect("divide outputs present");
    let value = outputs.get("out").expect("out port").value.clone();
    match value {
        Value::Float(f) => assert_eq!(f, 5.0),
        other => panic!("expected float result, got {:?}", other),
    }
}

#[test]
fn linked_inputs_override_defaults() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "rhs".to_string(),
        InputDefault {
            value: Value::Float(2.0),
            shape: None,
        },
    );

    let graph = GraphSpec {
        nodes: vec![
            constant_node("numerator", Value::Float(10.0)),
            constant_node("denominator", Value::Float(5.0)),
            NodeSpec {
                id: "div".to_string(),
                kind: NodeType::Divide,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: defaults,
            },
        ],
        edges: vec![
            link("numerator", "div", "lhs"),
            link("denominator", "div", "rhs"),
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    let outputs = rt.outputs.get("div").expect("divide outputs present");
    match outputs.get("out").map(|port| &port.value) {
        Some(Value::Float(f)) => assert_eq!(*f, 2.0),
        other => panic!("expected float output, got {:?}", other),
    }
}

#[test]
fn defaults_apply_when_output_key_missing() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "rhs".to_string(),
        InputDefault {
            value: Value::Float(0.25),
            shape: None,
        },
    );

    let graph = GraphSpec {
        nodes: vec![
            constant_node("numerator", Value::Float(1.0)),
            constant_node("config", Value::Float(10.0)),
            NodeSpec {
                id: "div".to_string(),
                kind: NodeType::Divide,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: defaults,
            },
        ],
        edges: vec![
            link("numerator", "div", "lhs"),
            link_with_output("config", "missing", "div", "rhs"),
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    let outputs = rt.outputs.get("div").expect("divide outputs present");
    match outputs.get("out").map(|port| &port.value) {
        Some(Value::Float(f)) => assert_eq!(*f, 4.0),
        other => panic!("expected float output using default, got {:?}", other),
    }
}

// --- Variadic & oscillator behaviour ------------------------------------

#[test]
fn join_respects_operand_order() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("a", Value::Vector(vec![1.0, 2.0])),
            constant_node("b", Value::Vector(vec![3.0])),
            constant_node("c", Value::Vector(vec![4.0, 5.0])),
            NodeSpec {
                id: "join".to_string(),
                kind: NodeType::Join,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![
            link("a", "join", "operand_1"),
            link("b", "join", "operand_2"),
            link("c", "join", "operand_3"),
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
    let graph = GraphSpec {
        nodes: vec![
            constant_node("freq", Value::Vector(vec![1.0, 2.0, 3.0])),
            constant_node("phase", Value::Float(0.0)),
            NodeSpec {
                id: "osc".to_string(),
                kind: NodeType::Oscillator,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![
            link("freq", "osc", "frequency"),
            link("phase", "osc", "phase"),
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
    let spec = GraphSpec {
        nodes: vec![node],
        ..Default::default()
    };
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

    let spec = GraphSpec {
        nodes: vec![node],
        ..Default::default()
    };
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

    let spec = GraphSpec {
        nodes: vec![node],
        ..Default::default()
    };
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
        output_shapes,
        input_defaults: HashMap::new(),
    };

    let graph = GraphSpec {
        nodes: vec![input_node],
        ..Default::default()
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
            output_shapes,
            input_defaults: HashMap::new(),
        }],
        ..Default::default()
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
            output_shapes,
            input_defaults: HashMap::new(),
        }],
        ..Default::default()
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
            output_shapes,
            input_defaults: HashMap::new(),
        }],
        ..Default::default()
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
            output_shapes,
            input_defaults: HashMap::new(),
        }],
        ..Default::default()
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
    record.insert("translation".to_string(), Value::Vec3([3.0, 4.0, 0.0]));
    record.insert("label".to_string(), Value::Text("ignored".to_string()));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::Record(record)),
            NodeSpec {
                id: "out".to_string(),
                kind: NodeType::Output,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link_with_selector(
            "src",
            "out",
            "out",
            "in",
            vec![SelectorSegment::Field("translation".to_string())],
        )],
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
    // Source provides a Transform; downstream selects .translation then [1] (y component).
    let graph = GraphSpec {
        nodes: vec![
            constant_node(
                "src",
                Value::Transform {
                    translation: [10.0, 42.0, -1.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0, 1.0, 1.0],
                },
            ),
            NodeSpec {
                id: "out".to_string(),
                kind: NodeType::Output,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link_with_selector(
            "src",
            "out",
            "out",
            "in",
            vec![
                SelectorSegment::Field("translation".to_string()),
                SelectorSegment::Index(1),
            ],
        )],
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
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::Vec3([1.0, 2.0, 3.0])),
            NodeSpec {
                id: "out".to_string(),
                kind: NodeType::Output,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link_with_selector(
            "src",
            "out",
            "out",
            "in",
            vec![SelectorSegment::Index(5)],
        )],
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
    let spring = NodeSpec {
        id: "spring".to_string(),
        kind: NodeType::Spring,
        params: NodeParams {
            stiffness: Some(30.0),
            damping: Some(6.0),
            mass: Some(1.0),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    };

    let mut spec = GraphSpec {
        nodes: vec![constant_node("target", Value::Float(0.0)), spring],
        edges: vec![link("target", "spring", "in")],
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
    let damp = NodeSpec {
        id: "damp".to_string(),
        kind: NodeType::Damp,
        params: NodeParams {
            half_life: Some(0.2),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    };

    let mut spec = GraphSpec {
        nodes: vec![constant_node("target", Value::Float(0.0)), damp],
        edges: vec![link("target", "damp", "in")],
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
    let slew = NodeSpec {
        id: "slew".to_string(),
        kind: NodeType::Slew,
        params: NodeParams {
            max_rate: Some(2.0),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    };

    let mut spec = GraphSpec {
        nodes: vec![constant_node("target", Value::Float(0.0)), slew],
        edges: vec![link("target", "slew", "in")],
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
    // Build Input node producing a record { translation: vec3, label: text } with a declared record shape.
    let typed_path = TypedPath::parse("sensor/pose").expect("valid path");

    // Declared output shape for the Input node: Record { translation: Vec3, label: Text }
    let declared = Shape::new(ShapeId::Record(vec![
        Field {
            name: "label".to_string(),
            shape: ShapeId::Text,
        },
        Field {
            name: "translation".to_string(),
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
        output_shapes: input_output_shapes,
        input_defaults: HashMap::new(),
    };

    let add_node = NodeSpec {
        id: "sum".to_string(),
        kind: NodeType::Add,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    };

    let output_node = NodeSpec {
        id: "out".to_string(),
        kind: NodeType::Output,
        params: NodeParams {
            path: Some(TypedPath::parse("robot/calc.y2").expect("valid path")),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    };

    let graph = GraphSpec {
        nodes: vec![
            input_node,
            constant_node("two", Value::Float(2.0)),
            add_node,
            output_node,
        ],
        edges: vec![
            link_with_selector(
                "in",
                "out",
                "sum",
                "operand_1",
                vec![
                    SelectorSegment::Field("translation".to_string()),
                    SelectorSegment::Index(1),
                ],
            ),
            link("two", "sum", "operand_2"),
            link("sum", "out", "in"),
        ],
    };

    // Stage record { translation: [1, 3, 5], label: "ok" } for the Input node.
    let mut record = HashMap::new();
    record.insert("translation".to_string(), Value::Vec3([1.0, 3.0, 5.0]));
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

#[test]
fn centered_remap_handles_anchor_segments() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "in_low".to_string(),
        InputDefault {
            value: Value::Float(-1.0),
            shape: None,
        },
    );
    defaults.insert(
        "in_anchor".to_string(),
        InputDefault {
            value: Value::Float(0.0),
            shape: None,
        },
    );
    defaults.insert(
        "in_high".to_string(),
        InputDefault {
            value: Value::Float(1.0),
            shape: None,
        },
    );
    defaults.insert(
        "out_low".to_string(),
        InputDefault {
            value: Value::Float(0.0),
            shape: None,
        },
    );
    defaults.insert(
        "out_anchor".to_string(),
        InputDefault {
            value: Value::Float(9.0),
            shape: None,
        },
    );
    defaults.insert(
        "out_high".to_string(),
        InputDefault {
            value: Value::Float(10.0),
            shape: None,
        },
    );

    let graph = GraphSpec {
        nodes: vec![
            NodeSpec {
                id: "input".to_string(),
                kind: NodeType::Input,
                params: NodeParams {
                    path: Some(TypedPath::parse("demo/value").expect("typed path")),
                    ..Default::default()
                },
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "remap".to_string(),
                kind: NodeType::CenteredRemap,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: defaults,
            },
        ],
        edges: vec![link("input", "remap", "in")],
    };

    let mut rt = GraphRuntime::default();

    let cases = [
        (-1.0, 0.0),
        (0.0, 9.0),
        (1.0, 10.0),
        (2.0, 11.0),
        (-2.0, -9.0),
        (0.5, 9.5),
        (-0.5, 4.5),
    ];

    for (input_value, expected) in cases {
        rt.set_input(
            TypedPath::parse("demo/value").expect("typed path"),
            Value::Float(input_value),
            None,
        );
        evaluate_all(&mut rt, &graph).expect("centered remap should evaluate");
        let outputs = rt.outputs.get("remap").expect("remap outputs present");
        let out_port = outputs.get("out").expect("out port present");
        match &out_port.value {
            Value::Float(actual) => {
                assert!(
                    (actual - expected).abs() < 1e-6,
                    "expected {expected}, got {actual}"
                );
            }
            other => panic!("expected float, got {:?}", other),
        }
    }
}

#[test]
fn centered_remap_supports_asymmetric_ranges_and_vectors() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "in_low".to_string(),
        InputDefault {
            value: Value::Float(0.0),
            shape: None,
        },
    );
    defaults.insert(
        "in_anchor".to_string(),
        InputDefault {
            value: Value::Float(0.5),
            shape: None,
        },
    );
    defaults.insert(
        "in_high".to_string(),
        InputDefault {
            value: Value::Float(1.0),
            shape: None,
        },
    );
    defaults.insert(
        "out_low".to_string(),
        InputDefault {
            value: Value::Float(1.0),
            shape: None,
        },
    );
    defaults.insert(
        "out_anchor".to_string(),
        InputDefault {
            value: Value::Float(3.0),
            shape: None,
        },
    );
    defaults.insert(
        "out_high".to_string(),
        InputDefault {
            value: Value::Float(6.0),
            shape: None,
        },
    );

    let graph = GraphSpec {
        nodes: vec![
            NodeSpec {
                id: "input".to_string(),
                kind: NodeType::Input,
                params: NodeParams {
                    path: Some(TypedPath::parse("demo/value").expect("typed path")),
                    ..Default::default()
                },
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "remap".to_string(),
                kind: NodeType::CenteredRemap,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: defaults,
            },
        ],
        edges: vec![link("input", "remap", "in")],
    };

    let mut rt = GraphRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");

    let cases = [(0.25, 2.0), (2.0, 12.0), (-1.0, -3.0)];

    for (input_value, expected) in cases {
        rt.set_input(path.clone(), Value::Float(input_value), None);
        evaluate_all(&mut rt, &graph).expect("centered remap should evaluate");
        let outputs = rt.outputs.get("remap").expect("remap outputs present");
        let out_port = outputs.get("out").expect("out port present");
        match &out_port.value {
            Value::Float(actual) => {
                assert!(
                    (actual - expected).abs() < 1e-6,
                    "expected {expected}, got {actual}"
                );
            }
            other => panic!("expected float, got {:?}", other),
        }
    }

    // Vector input is remapped component-wise.
    rt.set_input(
        path,
        Value::Vec3([0.25, 0.5, 1.25]),
        Some(Shape::new(ShapeId::Vec3)),
    );
    evaluate_all(&mut rt, &graph).expect("centered remap should evaluate");
    let outputs = rt.outputs.get("remap").expect("remap outputs present");
    let out_port = outputs.get("out").expect("out port present");
    match &out_port.value {
        Value::Vec3(values) => {
            let expected = [2.0, 3.0, 7.5];
            for (actual, expected) in values.iter().zip(expected.iter()) {
                assert!(
                    (actual - expected).abs() < 1e-6,
                    "expected {expected}, got {actual}"
                );
            }
        }
        other => panic!("expected vec3, got {:?}", other),
    }
}
