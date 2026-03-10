//! Behavioural coverage for the evaluation pipeline.

use super::*;
use crate::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, InputDefault, NodeParams, NodeSpec,
    NodeType, RoundMode, SelectorSegment,
};
use hashbrown::HashMap;
use vizij_api_core::shape::Field;
use vizij_api_core::{Shape, ShapeId, TypedPath, Value, WriteBatch};

macro_rules! graph_spec {
    ({ $($body:tt)* }) => {{
        GraphSpec {
            $($body)*
            version: 0,
            fingerprint: 0,
        }
        .with_cache()
    }};
}

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

#[test]
fn plan_cache_reuses_layouts_when_spec_version_matches() {
    let spec = graph_spec!({
        nodes: vec![constant_node("a", Value::Float(1.0))],
        edges: vec![],
    });

    let mut plan = PlanCache::default();
    plan.ensure_versioned(&spec).expect("initial plan build");

    // Capture stable pointers to prove layouts/order are reused, not rebuilt.
    let order_ptr = plan.order.as_ptr();
    let layouts_ptr = plan.layouts.as_ptr();
    let bindings_ptr = plan.input_bindings.as_ptr();
    let node_index_len = plan.node_index.len();

    // Second ensure with the same version should be a no-op.
    plan.ensure_versioned(&spec)
        .expect("plan should be reused for same version");

    assert_eq!(plan.order.as_ptr(), order_ptr, "order should be reused");
    assert_eq!(
        plan.layouts.as_ptr(),
        layouts_ptr,
        "layouts should be reused"
    );
    assert_eq!(
        plan.input_bindings.as_ptr(),
        bindings_ptr,
        "bindings should be reused"
    );
    assert_eq!(
        plan.node_index.len(),
        node_index_len,
        "node index should remain stable"
    );
}

#[test]
fn plan_cache_rebuilds_when_spec_version_changes() {
    let base = graph_spec!({
        nodes: vec![constant_node("a", Value::Float(1.0))],
        edges: vec![],
    });

    let mut plan = PlanCache::default();
    plan.ensure_versioned(&base).expect("initial plan build");

    let initial_version = base.version;
    let order_ptr = plan.order.as_ptr();
    let layouts_ptr = plan.layouts.as_ptr();

    // Bump the spec generation. This should force a rebuild even if the structure is otherwise
    // identical (the cache key changed).
    let bumped = base.with_cache();
    assert!(
        bumped.version > initial_version,
        "with_cache should bump version"
    );

    plan.ensure_versioned(&bumped)
        .expect("plan should rebuild for bumped version");

    // We expect at least one of these pointers to change because rebuild assigns new vectors.
    assert!(
        plan.order.as_ptr() != order_ptr || plan.layouts.as_ptr() != layouts_ptr,
        "plan rebuild should replace cached buffers"
    );
}

#[test]
fn piecewise_remap_matches_linear_case() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "input_breakpoints".to_string(),
        InputDefault {
            value: Value::Vector(vec![0.0, 1.0]),
            shape: None,
        },
    );
    defaults.insert(
        "output_breakpoints".to_string(),
        InputDefault {
            value: Value::Vector(vec![10.0, 20.0]),
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
                kind: NodeType::PiecewiseRemap,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: defaults,
            },
        ],
        edges: vec![link("input", "remap", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = GraphRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");

    let cases = [(0.0, 10.0), (0.5, 15.0), (1.0, 20.0)];

    for (input_value, expected) in cases {
        rt.set_input(path.clone(), Value::Float(input_value), None);
        evaluate_all(&mut rt, &graph).expect("piecewise remap should evaluate");
        let outputs = rt.outputs.get("remap").expect("remap outputs present");
        let out_port = outputs.get("out").expect("out port present");
        match &out_port.value {
            Value::Float(actual) => assert!(
                (actual - expected).abs() < 1e-6,
                "expected {expected}, got {actual}"
            ),
            other => panic!("expected float, got {:?}", other),
        }
    }
}

#[test]
fn piecewise_remap_handles_segments_and_extrapolation() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "input_breakpoints".to_string(),
        InputDefault {
            value: Value::Vector(vec![0.0, 1.0, 2.0, 3.0]),
            shape: None,
        },
    );
    defaults.insert(
        "output_breakpoints".to_string(),
        InputDefault {
            value: Value::Vector(vec![0.0, 10.0, 15.0, 30.0]),
            shape: None,
        },
    );

    let graph = graph_spec!({
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
                kind: NodeType::PiecewiseRemap,
                params: NodeParams {
                    clamp: Some(false),
                    ..Default::default()
                },
                output_shapes: HashMap::new(),
                input_defaults: defaults,
            },
        ],
        edges: vec![link("input", "remap", "in")],
    });

    let mut rt = GraphRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");

    let cases = [
        (0.5, 5.0),
        (1.5, 12.5),
        (2.5, 22.5),
        (-0.5, -5.0),
        (4.0, 45.0),
    ];

    for (input_value, expected) in cases {
        rt.set_input(path.clone(), Value::Float(input_value), None);
        evaluate_all(&mut rt, &graph).expect("piecewise remap should evaluate");
        let outputs = rt.outputs.get("remap").expect("remap outputs present");
        let out_port = outputs.get("out").expect("out port present");
        match &out_port.value {
            Value::Float(actual) => assert!(
                (actual - expected).abs() < 1e-5,
                "expected {expected}, got {actual}"
            ),
            other => panic!("expected float, got {:?}", other),
        }
    }
}

#[test]
fn piecewise_remap_preserves_plateaus_with_duplicate_inputs() {
    // Duplicate inputs with differing outputs should create a plateau segment, not collapse.
    let mut defaults = HashMap::new();
    defaults.insert(
        "input_breakpoints".to_string(),
        InputDefault {
            value: Value::Vector(vec![0.0, 0.5, 0.5, 1.0]),
            shape: None,
        },
    );
    defaults.insert(
        "output_breakpoints".to_string(),
        InputDefault {
            value: Value::Vector(vec![0.0, 0.5, 1.0, 2.0]),
            shape: None,
        },
    );

    let graph = graph_spec!({
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
                kind: NodeType::PiecewiseRemap,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: defaults,
            },
        ],
        edges: vec![link("input", "remap", "in")],
    });

    let mut rt = GraphRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");

    let cases = [(0.25, 0.25), (0.5, 0.5), (0.75, 1.5)];

    for (input_value, expected) in cases {
        rt.set_input(path.clone(), Value::Float(input_value), None);
        evaluate_all(&mut rt, &graph).expect("piecewise remap should evaluate");
        let outputs = rt.outputs.get("remap").expect("remap outputs present");
        let out_port = outputs.get("out").expect("out port present");
        match &out_port.value {
            Value::Float(actual) => assert!(
                (actual - expected).abs() < 1e-6,
                "expected {expected}, got {actual}"
            ),
            other => panic!("expected float, got {:?}", other),
        }
    }
}

#[test]
fn piecewise_remap_allows_duplicate_inputs_with_different_outputs() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "input_breakpoints".to_string(),
        InputDefault {
            value: Value::Vector(vec![0.0, 0.5, 0.5, 1.0]),
            shape: None,
        },
    );
    defaults.insert(
        "output_breakpoints".to_string(),
        InputDefault {
            value: Value::Vector(vec![0.0, 0.5, 0.75, 2.0]),
            shape: None,
        },
    );

    let graph = graph_spec!({
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
                kind: NodeType::PiecewiseRemap,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: defaults,
            },
        ],
        edges: vec![link("input", "remap", "in")],
    });

    let mut rt = GraphRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");

    let cases = [(0.5, 0.5), (0.75, 1.375)];

    for (input_value, expected) in cases {
        rt.set_input(path.clone(), Value::Float(input_value), None);
        evaluate_all(&mut rt, &graph).expect("duplicate breakpoints should evaluate");
        let outputs = rt.outputs.get("remap").expect("outputs present");
        let out_port = outputs.get("out").expect("out port present");
        match &out_port.value {
            Value::Float(actual) => assert!(
                (actual - expected).abs() < 1e-6,
                "expected {expected}, got {actual}"
            ),
            other => panic!("expected float, got {:?}", other),
        }
    }
}

#[test]
fn piecewise_remap_validates_duplicate_breakpoints() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "input_breakpoints".to_string(),
        InputDefault {
            value: Value::Vector(vec![0.0, 0.5, 0.5, 1.0]),
            shape: None,
        },
    );
    defaults.insert(
        "output_breakpoints".to_string(),
        InputDefault {
            value: Value::Vector(vec![0.0, 0.5, 0.5, 2.0]),
            shape: None,
        },
    );

    let graph = graph_spec!({
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
                kind: NodeType::PiecewiseRemap,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: defaults,
            },
        ],
        edges: vec![link("input", "remap", "in")],
    });

    let mut rt = GraphRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");

    rt.set_input(path.clone(), Value::Float(0.5), None);
    evaluate_all(&mut rt, &graph).expect("duplicate plateau should evaluate");
    let outputs = rt.outputs.get("remap").expect("outputs present");
    let out_port = outputs.get("out").expect("out port present");
    match &out_port.value {
        Value::Float(actual) => assert!(
            (actual - 0.5).abs() < 1e-6,
            "expected plateau value 0.5, got {actual}"
        ),
        other => panic!("expected float, got {:?}", other),
    }

    rt.set_input(path, Value::Float(0.75), None);
    evaluate_all(&mut rt, &graph).expect("piecewise remap should evaluate");
    let outputs = rt.outputs.get("remap").expect("outputs present");
    let out_port = outputs.get("out").expect("out port present");
    match &out_port.value {
        Value::Float(actual) => assert!(
            (actual - 1.25).abs() < 1e-6,
            "expected interpolated value 1.25, got {actual}"
        ),
        other => panic!("expected float, got {:?}", other),
    }
}

#[test]
fn piecewise_remap_errors_when_inputs_decrease() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "input_breakpoints".to_string(),
        InputDefault {
            value: Value::Vector(vec![0.0, 0.6, 0.5, 1.0]),
            shape: None,
        },
    );
    defaults.insert(
        "output_breakpoints".to_string(),
        InputDefault {
            value: Value::Vector(vec![0.0, 0.5, 0.75, 2.0]),
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
                kind: NodeType::PiecewiseRemap,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: defaults,
            },
        ],
        edges: vec![link("input", "remap", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = GraphRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");
    rt.set_input(path, Value::Float(0.5), None);

    let err = evaluate_all(&mut rt, &graph).expect_err("ordering mismatch should fail");
    assert!(
        err.contains("non-decreasing input breakpoints"),
        "error message should mention ordering, got {err}"
    );
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
    }
    .with_cache();
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
    }
    .with_cache();
    let mut rt = GraphRuntime::default();
    let err = evaluate_all(&mut rt, &spec).expect_err("should fail due to mismatch");
    assert!(err.contains("does not match declared shape"));
}

#[test]
fn abs_node_handles_negative_values() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::Vec3([-1.0, 2.0, -3.0])),
            NodeSpec {
                id: "abs".to_string(),
                kind: NodeType::Abs,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link("src", "abs", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("abs should evaluate");
    let outputs = rt.outputs.get("abs").expect("abs outputs present");
    match outputs.get("out").map(|pv| pv.value.clone()) {
        Some(Value::Vec3(values)) => assert_eq!(values, [1.0, 2.0, 3.0]),
        other => panic!("expected vec3, got {:?}", other),
    }
}

#[test]
fn modulo_node_handles_division() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("lhs", Value::Float(7.5)),
            constant_node("rhs", Value::Float(2.0)),
            NodeSpec {
                id: "mod".to_string(),
                kind: NodeType::Modulo,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link("lhs", "mod", "lhs"), link("rhs", "mod", "rhs")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("modulo should evaluate");
    let outputs = rt.outputs.get("mod").expect("mod outputs present");
    match outputs.get("out").map(|pv| pv.value.clone()) {
        Some(Value::Float(result)) => assert!((result - 1.5).abs() < 1e-6),
        other => panic!("expected float, got {:?}", other),
    }
}

#[test]
fn sqrt_node_handles_vectors() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::Vector(vec![4.0, 9.0])),
            NodeSpec {
                id: "sqrt".to_string(),
                kind: NodeType::Sqrt,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link("src", "sqrt", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("sqrt should evaluate");
    let outputs = rt.outputs.get("sqrt").expect("sqrt outputs present");
    match outputs.get("out").map(|pv| pv.value.clone()) {
        Some(Value::Vector(values)) => assert_eq!(values, vec![2.0, 3.0]),
        other => panic!("expected vector, got {:?}", other),
    }
}

#[test]
fn sign_node_outputs_signum() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::Vector(vec![-2.0, 0.0, 5.0])),
            NodeSpec {
                id: "sign".to_string(),
                kind: NodeType::Sign,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link("src", "sign", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("sign should evaluate");
    let outputs = rt.outputs.get("sign").expect("sign outputs present");
    match outputs.get("out").map(|pv| pv.value.clone()) {
        Some(Value::Vector(values)) => assert_eq!(values, vec![-1.0, 0.0, 1.0]),
        other => panic!("expected vector, got {:?}", other),
    }
}

#[test]
fn min_max_nodes_select_expected_values() {
    let mut rt = GraphRuntime::default();
    let graph = GraphSpec {
        nodes: vec![
            constant_node("a", Value::Float(3.0)),
            constant_node("b", Value::Float(-1.0)),
            constant_node("c", Value::Float(5.0)),
            NodeSpec {
                id: "minn".to_string(),
                kind: NodeType::Min,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "maxx".to_string(),
                kind: NodeType::Max,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![
            link("a", "minn", "operand_1"),
            link("b", "minn", "operand_2"),
            link("c", "minn", "operand_3"),
            link("a", "maxx", "operand_1"),
            link("b", "maxx", "operand_2"),
            link("c", "maxx", "operand_3"),
        ],
        ..Default::default()
    }
    .with_cache();

    evaluate_all(&mut rt, &graph).expect("min/max should evaluate");

    let min_val = match rt
        .outputs
        .get("minn")
        .and_then(|ports| ports.get("out"))
        .map(|pv| pv.value.clone())
    {
        Some(Value::Float(v)) => v,
        other => panic!("expected float, got {:?}", other),
    };
    assert!((min_val + 1.0).abs() < 1e-6);

    let max_val = match rt
        .outputs
        .get("maxx")
        .and_then(|ports| ports.get("out"))
        .map(|pv| pv.value.clone())
    {
        Some(Value::Float(v)) => v,
        other => panic!("expected float, got {:?}", other),
    };
    assert!((max_val - 5.0).abs() < 1e-6);
}

#[test]
fn round_node_respects_modes() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::Vector(vec![-1.2, 0.5, 2.8])),
            NodeSpec {
                id: "floor".to_string(),
                kind: NodeType::Round,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "ceil".to_string(),
                kind: NodeType::Round,
                params: NodeParams {
                    round_mode: Some(RoundMode::Ceil),
                    ..Default::default()
                },
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "trunc".to_string(),
                kind: NodeType::Round,
                params: NodeParams {
                    round_mode: Some(RoundMode::Trunc),
                    ..Default::default()
                },
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![
            link("src", "floor", "in"),
            link("src", "ceil", "in"),
            link("src", "trunc", "in"),
        ],
        ..Default::default()
    }
    .with_cache();

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("round nodes should evaluate");

    let floor_vals = match rt
        .outputs
        .get("floor")
        .and_then(|ports| ports.get("out"))
        .map(|pv| pv.value.clone())
    {
        Some(Value::Vector(vals)) => vals,
        other => panic!("expected vector, got {:?}", other),
    };
    assert_eq!(floor_vals, vec![-2.0, 0.0, 2.0]);

    let ceil_vals = match rt
        .outputs
        .get("ceil")
        .and_then(|ports| ports.get("out"))
        .map(|pv| pv.value.clone())
    {
        Some(Value::Vector(vals)) => vals,
        other => panic!("expected vector, got {:?}", other),
    };
    assert_eq!(ceil_vals, vec![-1.0, 1.0, 3.0]);

    let trunc_vals = match rt
        .outputs
        .get("trunc")
        .and_then(|ports| ports.get("out"))
        .map(|pv| pv.value.clone())
    {
        Some(Value::Vector(vals)) => vals,
        other => panic!("expected vector, got {:?}", other),
    };
    assert_eq!(trunc_vals, vec![-1.0, 0.0, 2.0]);
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
        ..Default::default()
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
    }
    .with_cache();
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
    }
    .with_cache();
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
    }
    .with_cache();

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
    }
    .with_cache();

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
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
        ..Default::default()
    }
    .with_cache();

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
        version: 1,
        fingerprint: 0,
    }
    .with_cache();

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
        version: 1,
        fingerprint: 0,
    }
    .with_cache();

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

// --- ToVector / FromVector -------------------------------------------------

#[test]
fn to_vector_packs_floats() {
    let spec = graph_spec!({
        nodes: vec![
            constant_node("a", Value::Float(1.0)),
            constant_node("b", Value::Float(2.0)),
            constant_node("c", Value::Float(3.0)),
            NodeSpec {
                id: "tv".to_string(),
                kind: NodeType::ToVector,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![
            link("a", "tv", "operand_1"),
            link("b", "tv", "operand_2"),
            link("c", "tv", "operand_3"),
        ],
    });
    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("to_vector should evaluate");
    let port = rt
        .outputs
        .get("tv")
        .and_then(|o| o.get("out"))
        .expect("out port");
    match &port.value {
        Value::Vector(v) => assert_eq!(v, &vec![1.0, 2.0, 3.0]),
        other => panic!("expected Vector, got {:?}", other),
    }
}

#[test]
fn from_vector_unpacks_with_nan_padding() {
    let spec = graph_spec!({
        nodes: vec![
            constant_node("src", Value::Vector(vec![10.0, 20.0, 30.0])),
            NodeSpec {
                id: "fv".to_string(),
                kind: NodeType::FromVector,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "s1".to_string(),
                kind: NodeType::Constant,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "s2".to_string(),
                kind: NodeType::Constant,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "s3".to_string(),
                kind: NodeType::Constant,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "s4".to_string(),
                kind: NodeType::Constant,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "s5".to_string(),
                kind: NodeType::Constant,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![
            link("src", "fv", "in"),
            link_with_output("fv", "element1", "s1", "in"),
            link_with_output("fv", "element2", "s2", "in"),
            link_with_output("fv", "element3", "s3", "in"),
            link_with_output("fv", "element4", "s4", "in"),
            link_with_output("fv", "element5", "s5", "in"),
        ],
    });
    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("from_vector should evaluate");

    let fv_out = rt.outputs.get("fv").expect("fv outputs");
    for (key, expected) in [
        ("element1", Some(10.0f32)),
        ("element2", Some(20.0)),
        ("element3", Some(30.0)),
        ("element4", None),
        ("element5", None),
    ] {
        let port = fv_out.get(key).unwrap_or_else(|| panic!("missing {key}"));
        match (&port.value, expected) {
            (Value::Float(f), Some(e)) => {
                assert!((f - e).abs() < 1e-6, "{key}: expected {e}, got {f}")
            }
            (Value::Float(f), None) => assert!(f.is_nan(), "{key}: expected NaN, got {f}"),
            (other, _) => panic!("{key}: expected Float, got {:?}", other),
        }
    }
}

#[test]
fn from_vector_underscore_naming_convention() {
    // This test uses the 0-indexed naming convention the frontend actually sends:
    // {variadic_id}_{N} = elements_0, elements_1, elements_2
    // (formatVariadicPortId in the vizij-web frontend uses 0-based indices).
    let spec = graph_spec!({
        nodes: vec![
            constant_node("src", Value::Vector(vec![10.0, 20.0, 30.0])),
            NodeSpec {
                id: "fv".to_string(),
                kind: NodeType::FromVector,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "s1".to_string(),
                kind: NodeType::Constant,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "s2".to_string(),
                kind: NodeType::Constant,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "s3".to_string(),
                kind: NodeType::Constant,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![
            link("src", "fv", "in"),
            link_with_output("fv", "elements_0", "s1", "in"),
            link_with_output("fv", "elements_1", "s2", "in"),
            link_with_output("fv", "elements_2", "s3", "in"),
        ],
    });
    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("from_vector should evaluate");

    let fv_out = rt.outputs.get("fv").expect("fv outputs");
    for (key, expected) in [
        ("elements_0", 10.0f32),
        ("elements_1", 20.0),
        ("elements_2", 30.0),
    ] {
        let port = fv_out.get(key).unwrap_or_else(|| panic!("missing {key}"));
        match &port.value {
            Value::Float(f) => {
                assert!(
                    (f - expected).abs() < 1e-6,
                    "{key}: expected {expected}, got {f}"
                );
            }
            other => panic!("{key}: expected Float, got {:?}", other),
        }
    }
}

// --- Noise -----------------------------------------------------------------

#[test]
fn noise_nodes_are_deterministic_and_in_range() {
    for kind in [
        NodeType::SimpleNoise,
        NodeType::PerlinNoise,
        NodeType::SimplexNoise,
    ] {
        let mut defaults = HashMap::new();
        defaults.insert(
            "x".to_string(),
            InputDefault {
                value: Value::Float(1.5),
                shape: None,
            },
        );
        defaults.insert(
            "y".to_string(),
            InputDefault {
                value: Value::Float(2.5),
                shape: None,
            },
        );

        let spec = graph_spec!({
            nodes: vec![NodeSpec {
                id: "n".to_string(),
                kind: kind.clone(),
                params: NodeParams {
                    noise_seed: Some(42.0),
                    frequency: Some(1.0),
                    octaves: Some(4.0),
                    lacunarity: Some(2.0),
                    persistence: Some(0.5),
                    ..Default::default()
                },
                output_shapes: HashMap::new(),
                input_defaults: defaults.clone(),
            }],
            edges: vec![],
        });

        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &spec).expect("noise should evaluate");
        let v1 = match rt
            .outputs
            .get("n")
            .and_then(|o| o.get("out"))
            .map(|p| &p.value)
        {
            Some(Value::Float(f)) => *f,
            other => panic!("expected Float, got {:?}", other),
        };

        // Evaluate again — must produce identical result
        evaluate_all(&mut rt, &spec).expect("noise second eval");
        let v2 = match rt
            .outputs
            .get("n")
            .and_then(|o| o.get("out"))
            .map(|p| &p.value)
        {
            Some(Value::Float(f)) => *f,
            other => panic!("expected Float, got {:?}", other),
        };

        assert_eq!(v1, v2, "noise must be deterministic for {:?}", kind);
        assert!(
            (-1.0..=1.0).contains(&v1),
            "noise output must be in [-1, 1], got {v1} for {:?}",
            kind
        );
    }
}
