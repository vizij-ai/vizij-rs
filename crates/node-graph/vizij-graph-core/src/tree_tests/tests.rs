//! Behavioural coverage for the evaluation pipeline.

#[allow(unused_imports)]
use crate::eval::*;
#[allow(unused_imports)]
use crate::tree_runtime::{evaluate_all, evaluate_all_with_functions, TreeRuntime};
use crate::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, InputDefault, NodeParams, NodeSpec,
    NodeType, RoundMode, SelectorSegment,
};
use hashbrown::HashMap;
use vizij_api_core::shape::Field;
use vizij_api_core::value as vocab;
use vizij_api_core::{Shape, ShapeId, TypedPath, Value, WriteBatch};

fn expect_vec3(value: &Value) -> [f32; 3] {
    vocab::as_vec3(value).unwrap_or_else(|| panic!("expected vec3, got {value:?}"))
}

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

/// A spec evaluated again keeps the tree it was built as: the tree is built
/// once for an unchanged spec.
#[test]
fn a_spec_evaluated_again_keeps_its_tree() {
    let spec = graph_spec!({
        nodes: vec![constant_node("a", Value::F32(1.0))],
        edges: vec![],
    });

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &spec).expect("initial build");
    evaluate_all(&mut rt, &spec).expect("the same spec");
    assert_eq!(rt.loads(), 1, "the tree is reused");
    assert_eq!(
        rt.tree()
            .and_then(|tree| tree.detached_calls(crate::import::tree_node_id("a"))),
        Some(1),
        "the constant is called once"
    );
}

/// A spec of another version builds the tree again.
#[test]
fn a_spec_of_another_version_builds_its_tree_again() {
    let base = graph_spec!({
        nodes: vec![constant_node("a", Value::F32(1.0))],
        edges: vec![],
    });

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &base).expect("initial build");
    assert_eq!(rt.loads(), 1);

    let bumped = base.with_cache();
    evaluate_all(&mut rt, &bumped).expect("the bumped spec");
    assert_eq!(rt.loads(), 2, "the tree is built again");
    assert_eq!(rt.outputs["a"]["out"].value, Value::F32(1.0));
}

/// A load forgets what the module kept for the nodes it removed, so an
/// editor adding and removing nodes does not grow it.
#[test]
fn a_load_forgets_the_nodes_it_removed() {
    let both = graph_spec!({
        nodes: vec![
            constant_node("a", Value::F32(1.0)),
            constant_node("b", Value::F32(2.0)),
        ],
        edges: vec![],
    });
    let one = graph_spec!({
        nodes: vec![constant_node("a", Value::F32(1.0))],
        edges: vec![],
    });

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &both).expect("both nodes");
    assert_eq!(rt.module_sites(), 2);
    rt.load(&one).expect("one node");
    rt.evaluate().expect("one node evaluates");
    assert_eq!(rt.module_sites(), 1, "b is forgotten");
    assert_eq!(rt.outputs["a"]["out"].value, Value::F32(1.0));
}

#[test]
fn piecewise_remap_matches_linear_case() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "input_breakpoints".to_string(),
        InputDefault {
            value: Value::ArrayF32(vec![0.0, 1.0]),
            shape: None,
        },
    );
    defaults.insert(
        "output_breakpoints".to_string(),
        InputDefault {
            value: Value::ArrayF32(vec![10.0, 20.0]),
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

    let mut rt = TreeRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");

    let cases = [(0.0, 10.0), (0.5, 15.0), (1.0, 20.0)];

    for (input_value, expected) in cases {
        rt.set_input(path.clone(), Value::F32(input_value), None);
        evaluate_all(&mut rt, &graph).expect("piecewise remap should evaluate");
        let outputs = rt.outputs.get("remap").expect("remap outputs present");
        let out_port = outputs.get("out").expect("out port present");
        match &out_port.value {
            Value::F32(actual) => assert!(
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
            value: Value::ArrayF32(vec![0.0, 1.0, 2.0, 3.0]),
            shape: None,
        },
    );
    defaults.insert(
        "output_breakpoints".to_string(),
        InputDefault {
            value: Value::ArrayF32(vec![0.0, 10.0, 15.0, 30.0]),
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

    let mut rt = TreeRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");

    let cases = [
        (0.5, 5.0),
        (1.5, 12.5),
        (2.5, 22.5),
        (-0.5, -5.0),
        (4.0, 45.0),
    ];

    for (input_value, expected) in cases {
        rt.set_input(path.clone(), Value::F32(input_value), None);
        evaluate_all(&mut rt, &graph).expect("piecewise remap should evaluate");
        let outputs = rt.outputs.get("remap").expect("remap outputs present");
        let out_port = outputs.get("out").expect("out port present");
        match &out_port.value {
            Value::F32(actual) => assert!(
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
            value: Value::ArrayF32(vec![0.0, 0.5, 0.5, 1.0]),
            shape: None,
        },
    );
    defaults.insert(
        "output_breakpoints".to_string(),
        InputDefault {
            value: Value::ArrayF32(vec![0.0, 0.5, 1.0, 2.0]),
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

    let mut rt = TreeRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");

    let cases = [(0.25, 0.25), (0.5, 0.5), (0.75, 1.5)];

    for (input_value, expected) in cases {
        rt.set_input(path.clone(), Value::F32(input_value), None);
        evaluate_all(&mut rt, &graph).expect("piecewise remap should evaluate");
        let outputs = rt.outputs.get("remap").expect("remap outputs present");
        let out_port = outputs.get("out").expect("out port present");
        match &out_port.value {
            Value::F32(actual) => assert!(
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
            value: Value::ArrayF32(vec![0.0, 0.5, 0.5, 1.0]),
            shape: None,
        },
    );
    defaults.insert(
        "output_breakpoints".to_string(),
        InputDefault {
            value: Value::ArrayF32(vec![0.0, 0.5, 0.75, 2.0]),
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

    let mut rt = TreeRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");

    let cases = [(0.5, 0.5), (0.75, 1.375)];

    for (input_value, expected) in cases {
        rt.set_input(path.clone(), Value::F32(input_value), None);
        evaluate_all(&mut rt, &graph).expect("duplicate breakpoints should evaluate");
        let outputs = rt.outputs.get("remap").expect("outputs present");
        let out_port = outputs.get("out").expect("out port present");
        match &out_port.value {
            Value::F32(actual) => assert!(
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
            value: Value::ArrayF32(vec![0.0, 0.5, 0.5, 1.0]),
            shape: None,
        },
    );
    defaults.insert(
        "output_breakpoints".to_string(),
        InputDefault {
            value: Value::ArrayF32(vec![0.0, 0.5, 0.5, 2.0]),
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

    let mut rt = TreeRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");

    rt.set_input(path.clone(), Value::F32(0.5), None);
    evaluate_all(&mut rt, &graph).expect("duplicate plateau should evaluate");
    let outputs = rt.outputs.get("remap").expect("outputs present");
    let out_port = outputs.get("out").expect("out port present");
    match &out_port.value {
        Value::F32(actual) => assert!(
            (actual - 0.5).abs() < 1e-6,
            "expected plateau value 0.5, got {actual}"
        ),
        other => panic!("expected float, got {:?}", other),
    }

    rt.set_input(path, Value::F32(0.75), None);
    evaluate_all(&mut rt, &graph).expect("piecewise remap should evaluate");
    let outputs = rt.outputs.get("remap").expect("outputs present");
    let out_port = outputs.get("out").expect("out port present");
    match &out_port.value {
        Value::F32(actual) => assert!(
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
            value: Value::ArrayF32(vec![0.0, 0.6, 0.5, 1.0]),
            shape: None,
        },
    );
    defaults.insert(
        "output_breakpoints".to_string(),
        InputDefault {
            value: Value::ArrayF32(vec![0.0, 0.5, 0.75, 2.0]),
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

    let mut rt = TreeRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");
    rt.set_input(path, Value::F32(0.5), None);

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
    let mut node = constant_node("a", Value::F32(1.0));
    node.output_shapes
        .insert("out".to_string(), Shape::new(ShapeId::Scalar));

    let spec = GraphSpec {
        nodes: vec![node],
        ..Default::default()
    }
    .with_cache();
    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &spec).expect("shape should match");
    let outputs = rt.outputs.get("a").expect("outputs present");
    let port = outputs.get("out").expect("out port present");
    assert!(matches!(port.shape.id, ShapeId::Scalar));
}

#[test]
fn it_should_error_when_shape_mismatches() {
    let mut node = constant_node("a", Value::F32(1.0));
    node.output_shapes
        .insert("out".to_string(), Shape::new(ShapeId::Vec3));

    let spec = GraphSpec {
        nodes: vec![node],
        ..Default::default()
    }
    .with_cache();
    let mut rt = TreeRuntime::default();
    let err = evaluate_all(&mut rt, &spec).expect_err("should fail due to mismatch");
    assert!(err.contains("does not match declared shape"));
}

#[test]
fn abs_node_handles_negative_values() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", vocab::vec3([-1.0, 2.0, -3.0])),
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("abs should evaluate");
    let outputs = rt.outputs.get("abs").expect("abs outputs present");
    let value = &outputs.get("out").expect("out port present").value;
    assert_eq!(expect_vec3(value), [1.0, 2.0, 3.0]);
}

#[test]
fn modulo_node_handles_division() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("lhs", Value::F32(7.5)),
            constant_node("rhs", Value::F32(2.0)),
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("modulo should evaluate");
    let outputs = rt.outputs.get("mod").expect("mod outputs present");
    match outputs.get("out").map(|pv| pv.value.clone()) {
        Some(Value::F32(result)) => assert!((result - 1.5).abs() < 1e-6),
        other => panic!("expected float, got {:?}", other),
    }
}

#[test]
fn sqrt_node_handles_vectors() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::ArrayF32(vec![4.0, 9.0])),
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("sqrt should evaluate");
    let outputs = rt.outputs.get("sqrt").expect("sqrt outputs present");
    match outputs.get("out").map(|pv| pv.value.clone()) {
        Some(Value::ArrayF32(values)) => assert_eq!(values, vec![2.0, 3.0]),
        other => panic!("expected vector, got {:?}", other),
    }
}

#[test]
fn sign_node_outputs_signum() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::ArrayF32(vec![-2.0, 0.0, 5.0])),
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("sign should evaluate");
    let outputs = rt.outputs.get("sign").expect("sign outputs present");
    match outputs.get("out").map(|pv| pv.value.clone()) {
        Some(Value::ArrayF32(values)) => assert_eq!(values, vec![-1.0, 0.0, 1.0]),
        other => panic!("expected vector, got {:?}", other),
    }
}

#[test]
fn min_max_nodes_select_expected_values() {
    let mut rt = TreeRuntime::default();
    let graph = GraphSpec {
        nodes: vec![
            constant_node("a", Value::F32(3.0)),
            constant_node("b", Value::F32(-1.0)),
            constant_node("c", Value::F32(5.0)),
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
        Some(Value::F32(v)) => v,
        other => panic!("expected float, got {:?}", other),
    };
    assert!((min_val + 1.0).abs() < 1e-6);

    let max_val = match rt
        .outputs
        .get("maxx")
        .and_then(|ports| ports.get("out"))
        .map(|pv| pv.value.clone())
    {
        Some(Value::F32(v)) => v,
        other => panic!("expected float, got {:?}", other),
    };
    assert!((max_val - 5.0).abs() < 1e-6);
}

#[test]
fn round_node_respects_modes() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::ArrayF32(vec![-1.2, 0.5, 2.8])),
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("round nodes should evaluate");

    let floor_vals = match rt
        .outputs
        .get("floor")
        .and_then(|ports| ports.get("out"))
        .map(|pv| pv.value.clone())
    {
        Some(Value::ArrayF32(vals)) => vals,
        other => panic!("expected vector, got {:?}", other),
    };
    assert_eq!(floor_vals, vec![-2.0, 0.0, 2.0]);

    let ceil_vals = match rt
        .outputs
        .get("ceil")
        .and_then(|ports| ports.get("out"))
        .map(|pv| pv.value.clone())
    {
        Some(Value::ArrayF32(vals)) => vals,
        other => panic!("expected vector, got {:?}", other),
    };
    assert_eq!(ceil_vals, vec![-1.0, 1.0, 3.0]);

    let trunc_vals = match rt
        .outputs
        .get("trunc")
        .and_then(|ports| ports.get("out"))
        .map(|pv| pv.value.clone())
    {
        Some(Value::ArrayF32(vals)) => vals,
        other => panic!("expected vector, got {:?}", other),
    };
    assert_eq!(trunc_vals, vec![-1.0, 0.0, 2.0]);
}

// --- Runtime outputs -----------------------------------------------------

#[test]
fn it_should_emit_write_for_output_nodes() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::F32(2.0)),
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    assert_eq!(rt.writes.iter().count(), 1);
    let op = rt.writes.iter().next().expect("write present");
    assert_eq!(op.path.to_string(), "robot1/Arm/Joint.angle");
    assert!(matches!(
        op.shape.as_ref().map(|s| &s.id),
        Some(ShapeId::Scalar)
    ));
    match op.value {
        Value::F32(f) => assert_eq!(f, 2.0),
        _ => panic!("expected float write"),
    }
}

#[test]
fn writes_batch_json_roundtrip_from_graph() {
    // Build a trivial graph that emits a write.
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", vocab::vec3([1.0, 2.0, 3.0])),
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

    let mut rt = TreeRuntime::default();
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
            value: Value::F32(2.0),
            shape: None,
        },
    );

    let graph = GraphSpec {
        nodes: vec![
            constant_node("numerator", Value::F32(10.0)),
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("defaults should evaluate");
    let outputs = rt.outputs.get("div").expect("divide outputs present");
    let value = outputs.get("out").expect("out port").value.clone();
    match value {
        Value::F32(f) => assert_eq!(f, 5.0),
        other => panic!("expected float result, got {:?}", other),
    }
}

#[test]
fn linked_inputs_override_defaults() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "rhs".to_string(),
        InputDefault {
            value: Value::F32(2.0),
            shape: None,
        },
    );

    let graph = GraphSpec {
        nodes: vec![
            constant_node("numerator", Value::F32(10.0)),
            constant_node("denominator", Value::F32(5.0)),
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    let outputs = rt.outputs.get("div").expect("divide outputs present");
    match outputs.get("out").map(|port| &port.value) {
        Some(Value::F32(f)) => assert_eq!(*f, 2.0),
        other => panic!("expected float output, got {:?}", other),
    }
}

#[test]
fn defaults_apply_when_output_key_missing() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "rhs".to_string(),
        InputDefault {
            value: Value::F32(0.25),
            shape: None,
        },
    );

    let graph = GraphSpec {
        nodes: vec![
            constant_node("numerator", Value::F32(1.0)),
            constant_node("config", Value::F32(10.0)),
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    let outputs = rt.outputs.get("div").expect("divide outputs present");
    match outputs.get("out").map(|port| &port.value) {
        Some(Value::F32(f)) => assert_eq!(*f, 4.0),
        other => panic!("expected float output using default, got {:?}", other),
    }
}

// --- Variadic & oscillator behaviour ------------------------------------

#[test]
fn join_respects_operand_order() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("a", Value::ArrayF32(vec![1.0, 2.0])),
            constant_node("b", Value::ArrayF32(vec![3.0])),
            constant_node("c", Value::ArrayF32(vec![4.0, 5.0])),
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("join should evaluate");
    let outputs = rt.outputs.get("join").expect("join outputs present");
    let port = outputs.get("out").expect("out port present");
    match &port.value {
        Value::ArrayF32(vec) => assert_eq!(vec, &vec![1.0, 2.0, 3.0, 4.0, 5.0]),
        other => panic!("expected vector output, got {:?}", other),
    }
}

#[test]
fn oscillator_broadcasts_vector_inputs() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("freq", Value::ArrayF32(vec![1.0, 2.0, 3.0])),
            constant_node("phase", Value::F32(0.0)),
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

    let mut rt = TreeRuntime::default();
    rt.t = 0.5;
    evaluate_all(&mut rt, &graph).expect("oscillator should evaluate");

    let outputs = rt.outputs.get("osc").expect("osc outputs present");
    let port = outputs.get("out").expect("osc out port present");
    let expected: Vec<f32> = vec![1.0, 2.0, 3.0]
        .into_iter()
        .map(|f| (std::f32::consts::TAU * f * rt.t).sin())
        .collect();

    match &port.value {
        Value::ArrayF32(vec) => {
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
    let node = constant_node("vec", Value::ArrayF32(vec![1.0, 2.0, 3.0]));
    let spec = GraphSpec {
        nodes: vec![node],
        ..Default::default()
    }
    .with_cache();
    let mut rt = TreeRuntime::default();
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
    let mut node = constant_node("a", Value::F32(1.0));
    node.output_shapes
        .insert("secondary".to_string(), Shape::new(ShapeId::Scalar));

    let spec = GraphSpec {
        nodes: vec![node],
        ..Default::default()
    }
    .with_cache();
    let mut rt = TreeRuntime::default();
    let err = evaluate_all(&mut rt, &spec).expect_err("missing declared output should error");
    assert!(err.contains("missing declared output"));
}

#[test]
fn it_should_validate_vector_length_against_declared_shape() {
    let mut node = constant_node("a", Value::ArrayF32(vec![1.0, 2.0, 3.0]));
    node.output_shapes.insert(
        "out".to_string(),
        Shape::new(ShapeId::Vector { len: Some(4) }),
    );

    let spec = GraphSpec {
        nodes: vec![node],
        ..Default::default()
    };
    let mut rt = TreeRuntime::default();
    let err = evaluate_all(&mut rt, &spec).expect_err("vector length mismatch should error");
    assert!(err.contains("does not match declared shape"));
}

/// What a device runs, the nodes its outputs read, checks each one's
/// declared shapes as an evaluation of every node does.
#[test]
fn a_declared_shape_is_checked_where_an_output_reads_the_node() {
    let mut node = constant_node("a", Value::ArrayF32(vec![1.0, 2.0, 3.0]));
    node.output_shapes.insert(
        "out".to_string(),
        Shape::new(ShapeId::Vector { len: Some(4) }),
    );
    let sink = NodeSpec {
        id: "sink".to_string(),
        kind: NodeType::Output,
        params: NodeParams {
            path: Some(TypedPath::parse("robot/x").expect("valid path")),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    };
    let spec = GraphSpec {
        nodes: vec![node, sink],
        edges: vec![link("a", "sink", "in")],
        ..Default::default()
    };
    let mut rt = TreeRuntime::new(crate::tree_runtime::Evaluation::Demanded);
    rt.load(&spec).expect("the graph loads");
    let err = rt
        .evaluate()
        .expect_err("vector length mismatch should error");
    assert!(err.contains("does not match declared shape"), "{err}");
    assert!(rt.writes.is_empty(), "nothing is written");
}

/// An `input` or `output` node has the one port `out`: a shape it declares
/// of another port is refused when the graph loads.
#[test]
fn an_input_declaring_the_shape_of_another_port_is_refused() {
    let input = NodeSpec {
        id: "in".to_string(),
        kind: NodeType::Input,
        params: NodeParams {
            path: Some(TypedPath::parse("robot/x").expect("valid path")),
            ..Default::default()
        },
        output_shapes: HashMap::from([("extra".to_string(), Shape::new(ShapeId::Scalar))]),
        input_defaults: HashMap::new(),
    };
    let spec = GraphSpec {
        nodes: vec![input],
        ..Default::default()
    };
    let err = TreeRuntime::default()
        .load(&spec)
        .expect_err("the graph is refused");
    assert_eq!(
        err,
        "node 'in' declares the shape of an output 'extra' it does not have"
    );
}

/// An `input`'s value fitted to its declared shape is checked against it: a
/// shape's `NaN`-filled value is not always of the shape.
#[test]
fn an_input_s_fitted_value_is_checked_against_its_shape() {
    let input = NodeSpec {
        id: "in".to_string(),
        kind: NodeType::Input,
        params: NodeParams {
            path: Some(TypedPath::parse("robot/x").expect("valid path")),
            ..Default::default()
        },
        output_shapes: HashMap::from([("out".to_string(), Shape::new(ShapeId::Enum(vec![])))]),
        input_defaults: HashMap::new(),
    };
    let spec = GraphSpec {
        nodes: vec![input],
        ..Default::default()
    };
    let err = evaluate_all(&mut TreeRuntime::default(), &spec).expect_err("the shape is refused");
    assert!(err.contains("does not match declared shape"), "{err}");
}

/// A declared shape is checked against the node's return as it is: an
/// option is not the number it holds.
#[test]
fn a_declared_shape_is_checked_against_the_return_as_it_is() {
    struct ReturnsAnOption;
    impl NodeFunctions for ReturnsAnOption {
        fn call(&mut self, _: uuid::Uuid, _: &[(uuid::Uuid, Value)]) -> Result<Value, String> {
            Ok(Value::Option(Some(Box::new(Value::F32(1.0)))))
        }
    }
    let spec = GraphSpec {
        nodes: vec![NodeSpec {
            id: "call".to_string(),
            kind: NodeType::ExternalFunction,
            params: NodeParams {
                function: Some(uuid::Uuid::from_u128(0x1)),
                ..Default::default()
            },
            output_shapes: HashMap::from([("out".to_string(), Shape::new(ShapeId::Scalar))]),
            input_defaults: HashMap::new(),
        }],
        ..Default::default()
    };
    let err = evaluate_all_with_functions(&mut TreeRuntime::default(), &spec, &mut ReturnsAnOption)
        .expect_err("the shape is refused");
    assert!(err.contains("does not match declared shape"), "{err}");
}

/// A graph's node ids are any strings: one named as the importer would name
/// a node it adds (a check, a selection, the root) is a node of its own.
#[test]
fn a_node_named_like_an_added_one_is_a_node_of_its_own() {
    let mut checked = constant_node("a", Value::F32(1.0));
    checked
        .output_shapes
        .insert("out".to_string(), Shape::new(ShapeId::Scalar));
    let spec = GraphSpec {
        nodes: vec![
            checked,
            constant_node("check:a", Value::F32(2.0)),
            constant_node("root", Value::F32(3.0)),
            constant_node("select:sink:in", Value::F32(4.0)),
            constant_node("pose", vocab::record([("x", vocab::float(5.0))])),
            NodeSpec {
                id: "sink".to_string(),
                kind: NodeType::Output,
                params: NodeParams {
                    path: Some(TypedPath::parse("robot/x").expect("valid path")),
                    ..Default::default()
                },
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link_with_selector(
            "pose",
            "out",
            "sink",
            "in",
            vec![SelectorSegment::Field("x".to_string())],
        )],
        ..Default::default()
    };
    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &spec).expect("the graph evaluates");
    for (id, value) in [
        ("a", 1.0),
        ("check:a", 2.0),
        ("root", 3.0),
        ("select:sink:in", 4.0),
    ] {
        assert_eq!(
            rt.outputs[id]["out"].value,
            Value::F32(value),
            "node '{id}'"
        );
    }
    let written: Vec<Value> = rt.writes.iter().map(|op| op.value.clone()).collect();
    assert_eq!(
        written,
        [vocab::float(5.0)],
        "the selection reaches the sink"
    );
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

    let mut rt = TreeRuntime::default();
    rt.set_input(
        typed_path,
        vocab::vec3([1.0, 2.0, 3.0]),
        Some(Shape::new(ShapeId::Vec3)),
    );

    evaluate_all(&mut rt, &graph).expect("input node should evaluate");

    let port = rt
        .outputs
        .get("input")
        .and_then(|outputs| outputs.get("out"))
        .expect("input output present");

    assert_eq!(expect_vec3(&port.value), [1.0, 2.0, 3.0]);
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

    let mut rt = TreeRuntime::default();
    // Staged value is a generic vector of the right length to coerce to Vec3.
    rt.set_input(
        typed_path,
        Value::ArrayF32(vec![9.0, 8.0, 7.0]),
        Some(Shape::new(ShapeId::Vector { len: Some(3) })),
    );

    evaluate_all(&mut rt, &graph).expect("coercion should succeed");

    let port = rt
        .outputs
        .get("input")
        .and_then(|outputs| outputs.get("out"))
        .expect("input output present");

    assert_eq!(expect_vec3(&port.value), [9.0, 8.0, 7.0]);
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("numeric shape should fall back to NaNs");

    let port = rt
        .outputs
        .get("input")
        .and_then(|outputs| outputs.get("out"))
        .expect("input output present");

    assert!(expect_vec3(&port.value).iter().all(|v| v.is_nan()));
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

    let mut rt = TreeRuntime::default();
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

    let mut rt = TreeRuntime::default();
    rt.set_input(
        typed_path.clone(),
        vocab::vec3([1.0, 2.0, 3.0]),
        Some(Shape::new(ShapeId::Vec3)),
    );

    evaluate_all(&mut rt, &graph).expect("first frame should read staged value");
    let first = rt
        .outputs
        .get("input")
        .and_then(|outputs| outputs.get("out"))
        .expect("input output present");
    assert_eq!(expect_vec3(&first.value), [1.0, 2.0, 3.0]);

    evaluate_all(&mut rt, &graph).expect("second frame should evaluate");
    let second = rt
        .outputs
        .get("input")
        .and_then(|outputs| outputs.get("out"))
        .expect("input output present");
    assert!(expect_vec3(&second.value).iter().all(|v| v.is_nan()));
}

#[test]
fn selector_projects_record_field() {
    let record = vocab::record([
        ("translation", vocab::vec3([3.0, 4.0, 0.0])),
        ("label", vocab::text("ignored")),
    ]);

    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", record),
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("selector projection should succeed");

    let port = rt
        .outputs
        .get("out")
        .and_then(|outputs| outputs.get("out"))
        .expect("output node present");

    assert_eq!(expect_vec3(&port.value), [3.0, 4.0, 0.0]);
}

#[test]
fn selector_projects_transform_field_and_nested_index() {
    // Source provides a Transform; downstream selects .translation then [1] (y component).
    let graph = GraphSpec {
        nodes: vec![
            constant_node(
                "src",
                vocab::transform(vocab::Transform {
                    translation: [10.0, 42.0, -1.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0, 1.0, 1.0],
                }),
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

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("selector projection should succeed");

    let port = rt
        .outputs
        .get("out")
        .and_then(|outputs| outputs.get("out"))
        .expect("output node present");

    match &port.value {
        Value::F32(f) => assert_eq!(*f, 42.0),
        other => panic!("expected scalar, got {:?}", other),
    }
    assert!(matches!(port.shape.id, ShapeId::Scalar));
}

#[test]
fn selector_index_out_of_bounds_errors() {
    // Select index 5 from a vec3; should error.
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", vocab::vec3([1.0, 2.0, 3.0])),
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

    let mut rt = TreeRuntime::default();
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
        nodes: vec![constant_node("target", Value::F32(0.0)), spring],
        edges: vec![link("target", "spring", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &spec).expect("initial evaluate");

    spec.nodes[0].params.value = Some(Value::F32(10.0));

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
        Value::F32(f) => f,
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
        Value::F32(f) => f,
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
        nodes: vec![constant_node("target", Value::F32(0.0)), damp],
        edges: vec![link("target", "damp", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &spec).expect("initial evaluate");

    spec.nodes[0].params.value = Some(Value::F32(1.0));
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
        Value::F32(f) => f,
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
        Value::F32(f) => f,
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
        nodes: vec![constant_node("target", Value::F32(0.0)), slew],
        edges: vec![link("target", "slew", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &spec).expect("initial evaluate");

    spec.nodes[0].params.value = Some(Value::F32(5.0));
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
        Value::F32(f) => f,
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
        Value::F32(f) => f,
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
            constant_node("two", Value::F32(2.0)),
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
    let record = vocab::record([
        ("translation", vocab::vec3([1.0, 3.0, 5.0])),
        ("label", vocab::text("ok")),
    ]);

    let mut rt = TreeRuntime::default();
    rt.set_input(typed_path, record, Some(declared));

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
        Value::F32(f) => assert!((f - 5.0).abs() < 1e-6),
        ref other => panic!("expected scalar value, got {:?}", other),
    }
}

/// An evaluation of what the outputs read writes what a full one writes,
/// frame after frame, and computes the same ports for every node an output
/// reads; a node nothing reads is not evaluated.
#[test]
fn an_evaluation_of_what_the_outputs_read_writes_what_a_full_one_writes() {
    let path = TypedPath::parse("robot/calc.y").expect("valid path");
    let graph = GraphSpec {
        nodes: vec![
            NodeSpec {
                id: "t".to_string(),
                kind: NodeType::Time,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            constant_node(
                "pose",
                vocab::record([("gain", vocab::float(2.0)), ("unused", vocab::text("x"))]),
            ),
            NodeSpec {
                id: "scaled".to_string(),
                kind: NodeType::Multiply,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "out".to_string(),
                kind: NodeType::Output,
                params: NodeParams {
                    path: Some(path.clone()),
                    ..Default::default()
                },
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            constant_node("unread", vocab::float(9.0)),
        ],
        edges: vec![
            link("t", "scaled", "operand_1"),
            link_with_selector(
                "pose",
                "out",
                "scaled",
                "operand_2",
                vec![SelectorSegment::Field("gain".to_string())],
            ),
            link("scaled", "out", "in"),
        ],
        ..Default::default()
    }
    .with_cache();

    let mut full = TreeRuntime::default();
    let mut for_writes = TreeRuntime::new(crate::tree_runtime::Evaluation::Demanded);
    for frame in 1..=3 {
        for rt in [&mut full, &mut for_writes] {
            rt.dt = 0.25;
            rt.t = 0.25 * frame as f32;
        }
        evaluate_all(&mut full, &graph).expect("full");
        evaluate_all(&mut for_writes, &graph).expect("for writes");
        let written = |rt: &TreeRuntime| -> Vec<(String, Value)> {
            rt.writes
                .iter()
                .map(|op| (op.path.to_string(), op.value.clone()))
                .collect()
        };
        assert_eq!(written(&for_writes), written(&full));
        assert_eq!(
            written(&full),
            vec![(path.to_string(), vocab::float(0.5 * frame as f32))]
        );
        assert_eq!(full.outputs.len(), 5, "a full evaluation shows every node");
        let mut expected = full.outputs.clone();
        expected.remove("unread");
        let values = |outputs: &std::collections::HashMap<
            String,
            std::collections::HashMap<String, PortValue>,
        >| {
            let mut values: Vec<(String, String, Value)> = outputs
                .iter()
                .flat_map(|(node, ports)| {
                    ports
                        .iter()
                        .map(move |(port, value)| (node.clone(), port.clone(), value.value.clone()))
                })
                .collect();
            values.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
            values
        };
        assert_eq!(values(&for_writes.outputs), values(&expected));
    }
}

/// A constant's port carries over from one evaluation to the next, and
/// follows its `value` when that changes in place, without a new plan, and
/// its node when a new plan moves it to another index.
#[test]
fn a_constant_follows_its_value_and_its_node() {
    let out = |id: &str, path: &str| NodeSpec {
        id: id.to_string(),
        kind: NodeType::Output,
        params: NodeParams {
            path: Some(TypedPath::parse(path).expect("valid path")),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    };
    let written = |rt: &TreeRuntime| -> Vec<(String, Value)> {
        rt.writes
            .iter()
            .map(|op| (op.path.to_string(), op.value.clone()))
            .collect()
    };
    let mut graph = GraphSpec {
        nodes: vec![
            constant_node("a", vocab::float(1.0)),
            constant_node("b", vocab::record([("x", vocab::float(2.0))])),
            out("out_a", "robot/a"),
            out("out_b", "robot/b"),
        ],
        edges: vec![link("a", "out_a", "in"), link("b", "out_b", "in")],
        ..Default::default()
    };
    let mut rt = TreeRuntime::default();
    let b = vocab::record([("x", vocab::float(2.0))]);
    for _ in 0..2 {
        evaluate_all(&mut rt, &graph).expect("evaluation");
        assert_eq!(
            written(&rt),
            vec![
                ("robot/a".into(), vocab::float(1.0)),
                ("robot/b".into(), b.clone())
            ]
        );
        let port = &rt.outputs["b"]["out"];
        assert_eq!(port.shape, shape_helpers::infer_shape(&b));
    }

    // The value changes in place: the plan stands, the port follows.
    graph.nodes[0].params.value = Some(vocab::float(3.0));
    evaluate_all(&mut rt, &graph).expect("evaluation");
    assert_eq!(written(&rt)[0], ("robot/a".into(), vocab::float(3.0)));

    // A new plan puts other nodes at the constants' indices.
    graph.nodes.swap(0, 2);
    graph.nodes.swap(1, 3);
    evaluate_all(&mut rt, &graph).expect("evaluation");
    let mut writes = written(&rt);
    writes.sort_by(|l, r| l.0.cmp(&r.0));
    assert_eq!(
        writes,
        vec![
            ("robot/a".into(), vocab::float(3.0)),
            ("robot/b".into(), b.clone())
        ]
    );
}

/// A selector reading a constant reads the constant once: the constant is
/// called at its first evaluation and not again while its value holds. A
/// new value is selected from then on, and a constant also read whole is
/// read whole too.
#[test]
fn a_selection_from_a_constant_reads_it_once_and_follows_its_value() {
    let out = |id: &str, path: &str| NodeSpec {
        id: id.to_string(),
        kind: NodeType::Output,
        params: NodeParams {
            path: Some(TypedPath::parse(path).expect("valid path")),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    };
    let record = |x: f32| vocab::record([("values", vocab::record([("x", vocab::float(x))]))]);
    let picked = || {
        link_with_selector(
            "rec",
            "out",
            "out_x",
            "in",
            vec![
                SelectorSegment::Field("values".to_string()),
                SelectorSegment::Field("x".to_string()),
            ],
        )
    };
    let written = |rt: &TreeRuntime| -> Vec<(String, Value)> {
        let mut writes: Vec<(String, Value)> = rt
            .writes
            .iter()
            .map(|op| (op.path.to_string(), op.value.clone()))
            .collect();
        writes.sort_by(|l, r| l.0.cmp(&r.0));
        writes
    };
    let calls = |rt: &TreeRuntime| {
        rt.tree()
            .and_then(|tree| tree.detached_calls(crate::import::tree_node_id("rec")))
    };

    // Read through a selector only.
    let mut graph = GraphSpec {
        nodes: vec![constant_node("rec", record(1.0)), out("out_x", "robot/x")],
        edges: vec![picked()],
        ..Default::default()
    }
    .with_cache();
    let mut rt = TreeRuntime::new(crate::tree_runtime::Evaluation::Demanded);
    for _ in 0..3 {
        evaluate_all(&mut rt, &graph).expect("evaluation");
        assert_eq!(written(&rt), vec![("robot/x".into(), vocab::float(1.0))]);
    }
    assert_eq!(calls(&rt), Some(1), "the record is evaluated once");

    // A new value in a new version is selected.
    graph.nodes[0].params.value = Some(record(2.0));
    graph = graph.with_cache();
    evaluate_all(&mut rt, &graph).expect("evaluation");
    assert_eq!(written(&rt), vec![("robot/x".into(), vocab::float(2.0))]);

    // Read whole as well.
    let mut both = GraphSpec {
        nodes: vec![
            constant_node("rec", record(3.0)),
            out("out_x", "robot/x"),
            out("out_rec", "robot/rec"),
        ],
        edges: vec![picked(), link("rec", "out_rec", "in")],
        ..Default::default()
    };
    let mut rt = TreeRuntime::new(crate::tree_runtime::Evaluation::Demanded);
    evaluate_all(&mut rt, &both).expect("evaluation");
    assert_eq!(
        written(&rt),
        vec![
            ("robot/rec".into(), record(3.0)),
            ("robot/x".into(), vocab::float(3.0))
        ]
    );

    // Unversioned, edited in place: the new value is read.
    both.nodes[0].params.value = Some(record(4.0));
    evaluate_all(&mut rt, &both).expect("evaluation");
    assert_eq!(
        written(&rt),
        vec![
            ("robot/rec".into(), record(4.0)),
            ("robot/x".into(), vocab::float(4.0))
        ]
    );
}

#[test]
fn centered_remap_handles_anchor_segments() {
    let mut defaults = HashMap::new();
    defaults.insert(
        "in_low".to_string(),
        InputDefault {
            value: Value::F32(-1.0),
            shape: None,
        },
    );
    defaults.insert(
        "in_anchor".to_string(),
        InputDefault {
            value: Value::F32(0.0),
            shape: None,
        },
    );
    defaults.insert(
        "in_high".to_string(),
        InputDefault {
            value: Value::F32(1.0),
            shape: None,
        },
    );
    defaults.insert(
        "out_low".to_string(),
        InputDefault {
            value: Value::F32(0.0),
            shape: None,
        },
    );
    defaults.insert(
        "out_anchor".to_string(),
        InputDefault {
            value: Value::F32(9.0),
            shape: None,
        },
    );
    defaults.insert(
        "out_high".to_string(),
        InputDefault {
            value: Value::F32(10.0),
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

    let mut rt = TreeRuntime::default();

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
            Value::F32(input_value),
            None,
        );
        evaluate_all(&mut rt, &graph).expect("centered remap should evaluate");
        let outputs = rt.outputs.get("remap").expect("remap outputs present");
        let out_port = outputs.get("out").expect("out port present");
        match &out_port.value {
            Value::F32(actual) => {
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
            value: Value::F32(0.0),
            shape: None,
        },
    );
    defaults.insert(
        "in_anchor".to_string(),
        InputDefault {
            value: Value::F32(0.5),
            shape: None,
        },
    );
    defaults.insert(
        "in_high".to_string(),
        InputDefault {
            value: Value::F32(1.0),
            shape: None,
        },
    );
    defaults.insert(
        "out_low".to_string(),
        InputDefault {
            value: Value::F32(1.0),
            shape: None,
        },
    );
    defaults.insert(
        "out_anchor".to_string(),
        InputDefault {
            value: Value::F32(3.0),
            shape: None,
        },
    );
    defaults.insert(
        "out_high".to_string(),
        InputDefault {
            value: Value::F32(6.0),
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

    let mut rt = TreeRuntime::default();
    let path = TypedPath::parse("demo/value").expect("typed path");

    let cases = [(0.25, 2.0), (2.0, 12.0), (-1.0, -3.0)];

    for (input_value, expected) in cases {
        rt.set_input(path.clone(), Value::F32(input_value), None);
        evaluate_all(&mut rt, &graph).expect("centered remap should evaluate");
        let outputs = rt.outputs.get("remap").expect("remap outputs present");
        let out_port = outputs.get("out").expect("out port present");
        match &out_port.value {
            Value::F32(actual) => {
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
        vocab::vec3([0.25, 0.5, 1.25]),
        Some(Shape::new(ShapeId::Vec3)),
    );
    evaluate_all(&mut rt, &graph).expect("centered remap should evaluate");
    let outputs = rt.outputs.get("remap").expect("remap outputs present");
    let out_port = outputs.get("out").expect("out port present");
    let values = expect_vec3(&out_port.value);
    let expected = [2.0, 3.0, 7.5];
    for (actual, expected) in values.iter().zip(expected.iter()) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }
}

// --- ToVector / FromVector -------------------------------------------------

#[test]
fn to_vector_packs_floats() {
    let spec = graph_spec!({
        nodes: vec![
            constant_node("a", Value::F32(1.0)),
            constant_node("b", Value::F32(2.0)),
            constant_node("c", Value::F32(3.0)),
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
    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &spec).expect("to_vector should evaluate");
    let port = rt
        .outputs
        .get("tv")
        .and_then(|o| o.get("out"))
        .expect("out port");
    match &port.value {
        Value::ArrayF32(v) => assert_eq!(v, &vec![1.0, 2.0, 3.0]),
        other => panic!("expected Vector, got {:?}", other),
    }
}

#[test]
fn from_vector_unpacks_with_nan_padding() {
    let spec = graph_spec!({
        nodes: vec![
            constant_node("src", Value::ArrayF32(vec![10.0, 20.0, 30.0])),
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
    let mut rt = TreeRuntime::default();
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
            (Value::F32(f), Some(e)) => {
                assert!((f - e).abs() < 1e-6, "{key}: expected {e}, got {f}")
            }
            (Value::F32(f), None) => assert!(f.is_nan(), "{key}: expected NaN, got {f}"),
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
            constant_node("src", Value::ArrayF32(vec![10.0, 20.0, 30.0])),
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
    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &spec).expect("from_vector should evaluate");

    let fv_out = rt.outputs.get("fv").expect("fv outputs");
    for (key, expected) in [
        ("elements_0", 10.0f32),
        ("elements_1", 20.0),
        ("elements_2", 30.0),
    ] {
        let port = fv_out.get(key).unwrap_or_else(|| panic!("missing {key}"));
        match &port.value {
            Value::F32(f) => {
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
                value: Value::F32(1.5),
                shape: None,
            },
        );
        defaults.insert(
            "y".to_string(),
            InputDefault {
                value: Value::F32(2.5),
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

        let mut rt = TreeRuntime::default();
        evaluate_all(&mut rt, &spec).expect("noise should evaluate");
        let v1 = match rt
            .outputs
            .get("n")
            .and_then(|o| o.get("out"))
            .map(|p| &p.value)
        {
            Some(Value::F32(f)) => *f,
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
            Some(Value::F32(f)) => *f,
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

/// Far from the origin, over the 16 octaves a noise node allows, the
/// lattice indices of the last octave approach and reach `i32::MAX`: every
/// noise still evaluates, in range.
#[test]
fn noise_nodes_evaluate_far_from_the_origin_over_sixteen_octaves() {
    for kind in [
        NodeType::SimpleNoise,
        NodeType::PerlinNoise,
        NodeType::SimplexNoise,
    ] {
        for at in [32768.0, 65536.0] {
            let defaults: HashMap<String, InputDefault> = ["x", "y"]
                .into_iter()
                .map(|port| {
                    let value = Value::F32(at);
                    (port.to_string(), InputDefault { value, shape: None })
                })
                .collect();
            let spec = graph_spec!({
                nodes: vec![NodeSpec {
                    id: "n".to_string(),
                    kind: kind.clone(),
                    params: NodeParams {
                        octaves: Some(16.0),
                        ..Default::default()
                    },
                    output_shapes: HashMap::new(),
                    input_defaults: defaults,
                }],
                edges: vec![],
            });

            let mut rt = TreeRuntime::default();
            evaluate_all(&mut rt, &spec).expect("noise should evaluate");
            let value = match rt.outputs.get("n").and_then(|o| o.get("out")) {
                Some(port) => match port.value {
                    Value::F32(f) => f,
                    ref other => panic!("expected Float, got {other:?}"),
                },
                None => panic!("no output for {kind:?}"),
            };
            assert!((-1.0..=1.0).contains(&value), "{kind:?} at {at}: {value}");
        }
    }
}

// --- path-less Output: keyed record batches ----------------------------------

use uuid::Uuid;
use vizij_api_core::value::{StructureField, StructureWithoutId};

const KEY_FIELD: Uuid = Uuid::from_u128(0x11);
const VALUE_FIELD: Uuid = Uuid::from_u128(0x22);

fn record_field(id: Uuid, value: Value) -> StructureField {
    StructureField {
        id,
        value: Box::new(value),
    }
}

fn keyed_record(key: Value, value: Value) -> StructureWithoutId {
    StructureWithoutId {
        fields: vec![
            record_field(KEY_FIELD, key),
            record_field(VALUE_FIELD, value),
        ],
    }
}

fn keyed_batch(elements: Vec<StructureWithoutId>) -> Value {
    Value::ArrayStructure {
        id: Uuid::from_u128(0xBA7C4),
        elements,
    }
}

fn pathless_output_node(id: &str) -> NodeSpec {
    NodeSpec {
        id: id.to_string(),
        kind: NodeType::Output,
        params: NodeParams {
            key_field: Some(KEY_FIELD),
            value_field: Some(VALUE_FIELD),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    }
}

#[test]
fn pathless_output_expands_a_keyed_batch_into_per_key_writes() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node(
                "src",
                keyed_batch(vec![
                    keyed_record(Value::String("anim/x".into()), Value::F32(0.25)),
                    keyed_record(Value::String("anim/y".into()), Value::F32(0.75)),
                ]),
            ),
            pathless_output_node("sink"),
        ],
        edges: vec![link("src", "sink", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    let writes: Vec<_> = rt.writes.iter().collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].path.to_string(), "anim/x");
    assert!(matches!(writes[0].value, Value::F32(v) if v == 0.25));
    assert_eq!(writes[1].path.to_string(), "anim/y");
    assert!(matches!(writes[1].value, Value::F32(v) if v == 0.75));
}

#[test]
fn pathless_output_writes_nothing_for_an_empty_batch() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", keyed_batch(vec![])),
            pathless_output_node("sink"),
        ],
        edges: vec![link("src", "sink", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("an empty batch is not an error");
    assert_eq!(rt.writes.iter().count(), 0);
}

#[test]
fn pathless_output_keeps_batch_order_for_a_repeated_key() {
    // Batch order is preserved in the write set; the per-tick flush (a map)
    // keeps the last write. Explicit combination is VIZ-76's ground.
    let graph = GraphSpec {
        nodes: vec![
            constant_node(
                "src",
                keyed_batch(vec![
                    keyed_record(Value::String("anim/x".into()), Value::F32(1.0)),
                    keyed_record(Value::String("anim/x".into()), Value::F32(2.0)),
                ]),
            ),
            pathless_output_node("sink"),
        ],
        edges: vec![link("src", "sink", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    let writes: Vec<_> = rt.writes.iter().collect();
    assert_eq!(writes.len(), 2);
    assert!(matches!(writes[0].value, Value::F32(v) if v == 1.0));
    assert!(matches!(writes[1].value, Value::F32(v) if v == 2.0));
}

#[test]
fn pathless_output_errors_on_a_malformed_record() {
    // A record without the declared value field is a miswired batch: loud.
    let graph = GraphSpec {
        nodes: vec![
            constant_node(
                "src",
                keyed_batch(vec![StructureWithoutId {
                    fields: vec![record_field(KEY_FIELD, Value::String("anim/x".into()))],
                }]),
            ),
            pathless_output_node("sink"),
        ],
        edges: vec![link("src", "sink", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = TreeRuntime::default();
    let err = evaluate_all(&mut rt, &graph).expect_err("a malformed record is an error");
    assert!(err.contains("sink"), "error names the node: {err}");
    assert!(err.contains("missing"), "error names the gap: {err}");
}

#[test]
fn pathless_output_errors_on_a_non_batch_value() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::F32(1.0)),
            pathless_output_node("sink"),
        ],
        edges: vec![link("src", "sink", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = TreeRuntime::default();
    let err = evaluate_all(&mut rt, &graph).expect_err("a scalar is not a record batch");
    assert!(err.contains("record batch"), "error names the shape: {err}");
}

#[test]
fn output_path_takes_precedence_over_key_fields() {
    let batch = keyed_batch(vec![keyed_record(
        Value::String("anim/x".into()),
        Value::F32(0.5),
    )]);
    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", batch.clone()),
            NodeSpec {
                id: "sink".to_string(),
                kind: NodeType::Output,
                params: NodeParams {
                    path: Some(TypedPath::parse("whole/batch").expect("valid path")),
                    key_field: Some(KEY_FIELD),
                    value_field: Some(VALUE_FIELD),
                    ..Default::default()
                },
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link("src", "sink", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    let writes: Vec<_> = rt.writes.iter().collect();
    assert_eq!(writes.len(), 1, "an explicit path writes the value whole");
    assert_eq!(writes[0].path.to_string(), "whole/batch");
    assert_eq!(writes[0].value, batch);
}

// --- numbers in structures of any type ---------------------------------------

/// A structure no node names: three fields, declared out of id order, of
/// their own float types.
fn foreign_structure(a: f64, b: f64, c: f32) -> Value {
    use vizij_api_core::value::{Structure, StructureField};
    let field = |id: u128, value: Value| StructureField {
        id: Uuid::from_u128(id),
        value: Box::new(value),
    };
    Value::Structure(Structure {
        id: Uuid::from_u128(0xF0),
        fields: vec![
            field(0xF3, Value::F64(a)),
            field(0xF1, Value::F64(b)),
            field(0xF2, Value::F32(c)),
        ],
    })
}

/// What a node of `kind` reading the constants `inputs` on its ports outputs.
fn output_of(kind: NodeType, inputs: Vec<(&str, Value)>) -> Value {
    let mut nodes: Vec<NodeSpec> = inputs
        .iter()
        .map(|(port, value)| constant_node(port, value.clone()))
        .collect();
    nodes.push(NodeSpec {
        id: "n".to_string(),
        kind,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    let edges = inputs
        .iter()
        .map(|(port, _)| link(port, "n", port))
        .collect();
    let spec = graph_spec!({ nodes, edges, });

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &spec).expect("the graph should evaluate");
    rt.outputs
        .get("n")
        .and_then(|outputs| outputs.get("out"))
        .map(|port| port.value.clone())
        .expect("the node should output")
}

fn sum_of(lhs: Value, rhs: Value) -> Value {
    output_of(NodeType::Add, vec![("operand_1", lhs), ("operand_2", rhs)])
}

#[test]
fn arithmetic_on_any_structure_keeps_its_form_and_field_types() {
    // A scalar broadcasts.
    assert_eq!(
        sum_of(foreign_structure(1.5, 2.0, 3.0), Value::F32(0.5)),
        foreign_structure(2.0, 2.5, 3.5)
    );
    assert_eq!(
        sum_of(
            foreign_structure(1.0, 2.0, 3.0),
            foreign_structure(0.25, 0.5, -1.0)
        ),
        foreign_structure(1.25, 2.5, 2.0)
    );
}

#[test]
fn a_transform_adds_component_by_component() {
    let transform = |offset: f32| {
        vocab::transform(vocab::Transform {
            translation: [1.0 + offset, 2.0, 3.0],
            rotation: [0.0, 0.0, offset, 1.0],
            scale: [1.0, 1.0, 1.0 + offset],
        })
    };
    let sum = vocab::as_transform(&sum_of(transform(0.0), transform(0.5)))
        .expect("the sum should be a transform");
    assert_eq!(sum.translation, [2.5, 4.0, 6.0]);
    assert_eq!(sum.rotation, [0.0, 0.0, 0.5, 2.0]);
    assert_eq!(sum.scale, [2.0, 2.0, 2.5]);
}

#[test]
fn structures_combine_only_with_their_own_form() {
    // A vec4 and a quaternion each hold four numbers, and do not combine.
    let mixed = sum_of(
        vocab::vec4([1.0, 2.0, 3.0, 4.0]),
        vocab::quat([0.0, 0.0, 0.0, 1.0]),
    );
    let mixed = vocab::as_vec4(&mixed).unwrap_or_else(|| panic!("expected vec4, got {mixed:?}"));
    assert!(mixed.iter().all(|x| x.is_nan()), "{mixed:?}");

    // Three numbers each, of two structures: the left one's form, NaN-filled.
    let Value::Structure(mixed) = sum_of(
        foreign_structure(1.0, 2.0, 3.0),
        vocab::vec3([1.0, 2.0, 3.0]),
    ) else {
        panic!("expected a structure");
    };
    assert_eq!(mixed.id, Uuid::from_u128(0xF0));
    assert!(matches!(*mixed.fields[0].value, Value::F64(x) if x.is_nan()));
    assert!(matches!(*mixed.fields[1].value, Value::F64(x) if x.is_nan()));
    assert!(matches!(*mixed.fields[2].value, Value::F32(x) if x.is_nan()));
}

#[test]
fn a_structure_reads_its_fields_in_declaration_order() {
    for (index, expected) in [(0.0, 7.0), (1.0, 8.0), (2.0, 9.0)] {
        let read = output_of(
            NodeType::VectorIndex,
            vec![
                ("v", foreign_structure(7.0, 8.0, 9.0)),
                ("index", Value::F32(index)),
            ],
        );
        assert_eq!(read, Value::F32(expected), "index {index}");
    }
}

#[test]
fn a_structure_holding_more_than_floats_has_no_numeric_form() {
    use vizij_api_core::value::{Structure, StructureField};
    // A text field, and an integer field: a node's intermediate results would
    // round it at every step.
    for other in [Value::String("label".to_string()), Value::I32(5)] {
        let mixed = Value::Structure(Structure {
            id: Uuid::from_u128(0xF4),
            fields: vec![
                StructureField {
                    id: Uuid::from_u128(0xF5),
                    value: Box::new(Value::F32(1.0)),
                },
                StructureField {
                    id: Uuid::from_u128(0xF6),
                    value: Box::new(other.clone()),
                },
            ],
        });
        let sum = sum_of(mixed, Value::F32(1.0));
        assert!(
            matches!(sum, Value::F32(x) if x.is_nan()),
            "{other:?}: {sum:?}"
        );
    }
}

#[test]
fn a_slew_on_a_structure_keeps_its_progress_between_steps() {
    use vizij_api_core::value::{Structure, StructureField};
    let at = |x: f64| {
        Value::Structure(Structure {
            id: Uuid::from_u128(0xF7),
            fields: vec![StructureField {
                id: Uuid::from_u128(0xF8),
                value: Box::new(Value::F64(x)),
            }],
        })
    };
    let slew = NodeSpec {
        id: "slew".to_string(),
        kind: NodeType::Slew,
        params: NodeParams {
            max_rate: Some(1.0),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    };
    let mut spec = GraphSpec {
        nodes: vec![constant_node("target", at(0.0)), slew],
        edges: vec![link("target", "slew", "in")],
        ..Default::default()
    }
    .with_cache();

    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &spec).expect("initial evaluate");
    spec.nodes[0].params.value = Some(at(1.0));
    rt.dt = 1.0 / 60.0;
    for _ in 0..30 {
        rt.t += rt.dt;
        evaluate_all(&mut rt, &spec).expect("slew step");
    }

    let out = rt
        .outputs
        .get("slew")
        .and_then(|outputs| outputs.get("out"))
        .map(|port| port.value.clone())
        .expect("slew output");
    let Value::Structure(out) = out else {
        panic!("expected a structure, got {out:?}");
    };
    // Thirty steps of a sixtieth of a second at one unit a second.
    assert!(
        matches!(*out.fields[0].value, Value::F64(x) if (x - 0.5).abs() < 1e-5),
        "{out:?}"
    );
}
