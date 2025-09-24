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

// --- Blend helpers --------------------------------------------------------

#[test]
fn weighted_sum_vector_matches_expected_math() {
    let values = vec![2.0, 4.0, 1.0];
    let weights = vec![0.5, 0.25, 1.0];
    let mask = vec![1.0, 0.0, 1.0];

    let mut sum_inputs = HashMap::new();
    sum_inputs.insert("values".to_string(), connection("values", "out"));
    sum_inputs.insert("weights".to_string(), connection("weights", "out"));
    sum_inputs.insert("mask".to_string(), connection("mask", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values.clone())),
            constant_node("weights", Value::Vector(weights.clone())),
            constant_node("mask", Value::Vector(mask.clone())),
            NodeSpec {
                id: "sum".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                inputs: sum_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("weighted sum should evaluate");
    let outputs = rt.outputs.get("sum").expect("weighted sum outputs present");

    let total_weighted_sum = match &outputs
        .get("total_weighted_sum")
        .expect("total_weighted_sum output")
        .value
    {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    let total_weight = match &outputs
        .get("total_weight")
        .expect("total_weight output")
        .value
    {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    let max_weight = match &outputs.get("max_weight").expect("max_weight output").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    let input_count = match &outputs
        .get("input_count")
        .expect("input_count output")
        .value
    {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };

    let expected_weighted_sum: f32 = values
        .iter()
        .zip(weights.iter())
        .zip(mask.iter())
        .map(|((value, weight), mask)| *value * *weight * *mask)
        .sum();
    let expected_total_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| *weight * *mask)
        .sum();
    let expected_max_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| *weight * *mask)
        .fold(0.0f32, |acc, value| acc.max(value));
    let expected_input_count = weights.len().max(values.len()).max(mask.len()) as f32;

    assert!(
        (total_weighted_sum - expected_weighted_sum).abs() < 1e-6,
        "expected {expected_weighted_sum}, got {total_weighted_sum}"
    );
    assert!(
        (total_weight - expected_total_weight).abs() < 1e-6,
        "expected {expected_total_weight}, got {total_weight}"
    );
    assert!(
        (max_weight - expected_max_weight).abs() < 1e-6,
        "expected {expected_max_weight}, got {max_weight}"
    );
    assert!(
        (input_count - expected_input_count).abs() < 1e-6,
        "expected {expected_input_count}, got {input_count}"
    );
}

#[test]
fn blend_weighted_average_and_additive_match_expected_math() {
    let values = vec![0.8, 0.1, 1.2];
    let weights = vec![0.5, 0.25, 0.75];
    let mask = vec![1.0, 1.0, 1.0];
    let base_value = 0.2;

    let mut sum_inputs = HashMap::new();
    sum_inputs.insert("values".to_string(), connection("values", "out"));
    sum_inputs.insert("weights".to_string(), connection("weights", "out"));
    sum_inputs.insert("mask".to_string(), connection("mask", "out"));

    let mut avg_inputs = HashMap::new();
    avg_inputs.insert(
        "total_weighted_sum".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weighted_sum".to_string(),
            selector: None,
        },
    );
    avg_inputs.insert(
        "total_weight".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weight".to_string(),
            selector: None,
        },
    );
    avg_inputs.insert(
        "max_weight".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "max_weight".to_string(),
            selector: None,
        },
    );
    avg_inputs.insert("base".to_string(), connection("base", "out"));

    let mut add_inputs = HashMap::new();
    add_inputs.insert(
        "total_weighted_sum".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weighted_sum".to_string(),
            selector: None,
        },
    );
    add_inputs.insert(
        "total_weight".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weight".to_string(),
            selector: None,
        },
    );
    add_inputs.insert("base".to_string(), connection("base", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values.clone())),
            constant_node("weights", Value::Vector(weights.clone())),
            constant_node("mask", Value::Vector(mask.clone())),
            constant_node("base", Value::Float(base_value)),
            NodeSpec {
                id: "sum".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                inputs: sum_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "avg".to_string(),
                kind: NodeType::BlendWeightedAverage,
                params: NodeParams::default(),
                inputs: avg_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "add".to_string(),
                kind: NodeType::BlendAdditive,
                params: NodeParams::default(),
                inputs: add_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("blend nodes should evaluate");

    let sum_outputs = rt.outputs.get("sum").expect("sum outputs present");
    let total_weighted_sum = match &sum_outputs
        .get("total_weighted_sum")
        .expect("total_weighted_sum output")
        .value
    {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    let total_weight = match &sum_outputs
        .get("total_weight")
        .expect("total_weight output")
        .value
    {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    let max_weight = match &sum_outputs
        .get("max_weight")
        .expect("max_weight output")
        .value
    {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };

    let expected_total_weighted_sum: f32 = values
        .iter()
        .zip(weights.iter())
        .zip(mask.iter())
        .map(|((value, weight), mask)| *value * *weight * *mask)
        .sum();
    let expected_total_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| *weight * *mask)
        .sum();
    let expected_max_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| *weight * *mask)
        .fold(0.0f32, |acc, value| acc.max(value));

    assert!((total_weighted_sum - expected_total_weighted_sum).abs() < 1e-6);
    assert!((total_weight - expected_total_weight).abs() < 1e-6);
    assert!((max_weight - expected_max_weight).abs() < 1e-6);

    let expected_average =
        expected_total_weighted_sum / (expected_total_weight / expected_max_weight);
    let avg_outputs = rt.outputs.get("avg").expect("avg output present");
    let avg_value = match &avg_outputs.get("out").expect("avg out").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (avg_value - expected_average).abs() < 1e-6,
        "expected {expected_average}, got {avg_value}"
    );

    let add_outputs = rt.outputs.get("add").expect("add output present");
    let additive_value = match &add_outputs.get("out").expect("add out").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (additive_value - expected_total_weighted_sum).abs() < 1e-6,
        "expected {expected_total_weighted_sum}, got {additive_value}"
    );
}

#[test]
fn blend_weighted_average_and_additive_fall_back_to_base_when_weight_zero() {
    let values = vec![1.0, 2.0];
    let weights = vec![0.5, 0.75];
    let mask = vec![0.0, 0.0];
    let base_value = 0.4;

    let mut sum_inputs = HashMap::new();
    sum_inputs.insert("values".to_string(), connection("values", "out"));
    sum_inputs.insert("weights".to_string(), connection("weights", "out"));
    sum_inputs.insert("mask".to_string(), connection("mask", "out"));

    let mut avg_inputs = HashMap::new();
    avg_inputs.insert(
        "total_weighted_sum".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weighted_sum".to_string(),
            selector: None,
        },
    );
    avg_inputs.insert(
        "total_weight".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weight".to_string(),
            selector: None,
        },
    );
    avg_inputs.insert(
        "max_weight".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "max_weight".to_string(),
            selector: None,
        },
    );
    avg_inputs.insert("base".to_string(), connection("base", "out"));

    let mut add_inputs = HashMap::new();
    add_inputs.insert(
        "total_weighted_sum".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weighted_sum".to_string(),
            selector: None,
        },
    );
    add_inputs.insert(
        "total_weight".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weight".to_string(),
            selector: None,
        },
    );
    add_inputs.insert("base".to_string(), connection("base", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values)),
            constant_node("weights", Value::Vector(weights)),
            constant_node("mask", Value::Vector(mask)),
            constant_node("base", Value::Float(base_value)),
            NodeSpec {
                id: "sum".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                inputs: sum_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "avg".to_string(),
                kind: NodeType::BlendWeightedAverage,
                params: NodeParams::default(),
                inputs: avg_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "add".to_string(),
                kind: NodeType::BlendAdditive,
                params: NodeParams::default(),
                inputs: add_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("blend nodes should evaluate");

    let avg_outputs = rt.outputs.get("avg").expect("avg output present");
    let avg_value = match &avg_outputs.get("out").expect("avg out").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (avg_value - base_value).abs() < 1e-6,
        "expected base fallback {base_value}, got {avg_value}"
    );

    let add_outputs = rt.outputs.get("add").expect("add output present");
    let add_value = match &add_outputs.get("out").expect("add out").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (add_value - base_value).abs() < 1e-6,
        "expected base fallback {base_value}, got {add_value}"
    );
}

#[test]
fn blend_multiply_matches_expected_math() {
    let values = vec![0.2, 0.5, 1.1];
    let weights = vec![0.5, 0.25, 0.75];
    let mask = vec![1.0, 1.0, 0.5];

    let mut inputs = HashMap::new();
    inputs.insert("values".to_string(), connection("values", "out"));
    inputs.insert("weights".to_string(), connection("weights", "out"));
    inputs.insert("mask".to_string(), connection("mask", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values.clone())),
            constant_node("weights", Value::Vector(weights.clone())),
            constant_node("mask", Value::Vector(mask.clone())),
            NodeSpec {
                id: "mul".to_string(),
                kind: NodeType::BlendMultiply,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("multiply should evaluate");
    let outputs = rt.outputs.get("mul").expect("mul outputs present");
    let value = match &outputs.get("out").expect("mul out").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };

    let expected_product = values
        .iter()
        .zip(weights.iter())
        .zip(mask.iter())
        .fold(1.0f32, |acc, ((value, weight), mask)| {
            acc * (1.0 - *weight + *value * *weight * *mask)
        });

    assert!(
        (value - expected_product).abs() < 1e-6,
        "expected {expected_product}, got {value}"
    );
}

#[test]
fn blend_weighted_overlay_matches_expected_math() {
    let values = vec![0.2, 0.5, 1.0];
    let weights = vec![0.25, 0.5, 0.75];
    let mask = vec![1.0, 1.0, 1.0];
    let base_value = 0.3;

    let mut sum_inputs = HashMap::new();
    sum_inputs.insert("values".to_string(), connection("values", "out"));
    sum_inputs.insert("weights".to_string(), connection("weights", "out"));
    sum_inputs.insert("mask".to_string(), connection("mask", "out"));

    let mut overlay_inputs = HashMap::new();
    overlay_inputs.insert(
        "total_weighted_sum".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weighted_sum".to_string(),
            selector: None,
        },
    );
    overlay_inputs.insert(
        "max_weight".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "max_weight".to_string(),
            selector: None,
        },
    );
    overlay_inputs.insert("base".to_string(), connection("base", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values.clone())),
            constant_node("weights", Value::Vector(weights.clone())),
            constant_node("mask", Value::Vector(mask.clone())),
            constant_node("base", Value::Float(base_value)),
            NodeSpec {
                id: "sum".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                inputs: sum_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "overlay".to_string(),
                kind: NodeType::BlendWeightedOverlay,
                params: NodeParams::default(),
                inputs: overlay_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("overlay should evaluate");

    let sum_outputs = rt.outputs.get("sum").expect("sum outputs present");
    let total_weighted_sum = match &sum_outputs
        .get("total_weighted_sum")
        .expect("total_weighted_sum output")
        .value
    {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    let max_weight = match &sum_outputs
        .get("max_weight")
        .expect("max_weight output")
        .value
    {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };

    let expected_weighted_sum: f32 = values
        .iter()
        .zip(weights.iter())
        .zip(mask.iter())
        .map(|((value, weight), mask)| *value * *weight * *mask)
        .sum();
    let expected_max_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| *weight * *mask)
        .fold(0.0f32, |acc, value| acc.max(value));

    assert!((total_weighted_sum - expected_weighted_sum).abs() < 1e-6);
    assert!((max_weight - expected_max_weight).abs() < 1e-6);

    let overlay_outputs = rt.outputs.get("overlay").expect("overlay outputs present");
    let overlay_value = match &overlay_outputs.get("out").expect("overlay out").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };

    let expected_overlay =
        base_value * (1.0 - expected_max_weight) + expected_weighted_sum * expected_max_weight;
    assert!(
        (overlay_value - expected_overlay).abs() < 1e-6,
        "expected {expected_overlay}, got {overlay_value}"
    );
}

#[test]
fn blend_weighted_average_overlay_matches_expected_math() {
    let diffs = vec![0.2, 0.5];
    let weights = vec![0.5, 0.75];
    let mask = vec![1.0, 1.0];
    let base_value = 0.4;

    let mut sum_inputs = HashMap::new();
    sum_inputs.insert("values".to_string(), connection("diffs", "out"));
    sum_inputs.insert("weights".to_string(), connection("weights", "out"));
    sum_inputs.insert("mask".to_string(), connection("mask", "out"));

    let mut overlay_inputs = HashMap::new();
    overlay_inputs.insert(
        "total_weighted_sum".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weighted_sum".to_string(),
            selector: None,
        },
    );
    overlay_inputs.insert(
        "total_weight".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weight".to_string(),
            selector: None,
        },
    );
    overlay_inputs.insert(
        "max_weight".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "max_weight".to_string(),
            selector: None,
        },
    );
    overlay_inputs.insert(
        "input_count".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "input_count".to_string(),
            selector: None,
        },
    );
    overlay_inputs.insert("base".to_string(), connection("base", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("diffs", Value::Vector(diffs.clone())),
            constant_node("weights", Value::Vector(weights.clone())),
            constant_node("mask", Value::Vector(mask.clone())),
            constant_node("base", Value::Float(base_value)),
            NodeSpec {
                id: "sum".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                inputs: sum_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "overlay".to_string(),
                kind: NodeType::BlendWeightedAverageOverlay,
                params: NodeParams::default(),
                inputs: overlay_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("overlay should evaluate");

    let sum_outputs = rt.outputs.get("sum").expect("sum outputs present");
    let total_weighted_sum = match &sum_outputs
        .get("total_weighted_sum")
        .expect("total_weighted_sum output")
        .value
    {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    let total_weight = match &sum_outputs
        .get("total_weight")
        .expect("total_weight output")
        .value
    {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    let max_weight = match &sum_outputs
        .get("max_weight")
        .expect("max_weight output")
        .value
    {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    let input_count = match &sum_outputs
        .get("input_count")
        .expect("input_count output")
        .value
    {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };

    let expected_weighted_sum: f32 = diffs
        .iter()
        .zip(weights.iter())
        .zip(mask.iter())
        .map(|((value, weight), mask)| *value * *weight * *mask)
        .sum();
    let expected_total_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| *weight * *mask)
        .sum();
    let expected_max_weight: f32 = weights
        .iter()
        .zip(mask.iter())
        .map(|(weight, mask)| *weight * *mask)
        .fold(0.0f32, |acc, value| acc.max(value));
    let expected_count = weights.len().max(diffs.len()).max(mask.len()) as f32;

    assert!((total_weighted_sum - expected_weighted_sum).abs() < 1e-6);
    assert!((total_weight - expected_total_weight).abs() < 1e-6);
    assert!((max_weight - expected_max_weight).abs() < 1e-6);
    assert!((input_count - expected_count).abs() < 1e-6);

    let overlay_outputs = rt.outputs.get("overlay").expect("overlay outputs present");
    let overlay_value = match &overlay_outputs.get("out").expect("overlay out").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };

    let expected_divider = expected_total_weight / expected_max_weight;
    let expected_average = expected_weighted_sum / expected_divider;
    let expected_overlay = base_value + expected_average;
    assert!(
        (overlay_value - expected_overlay).abs() < 1e-6,
        "expected {expected_overlay}, got {overlay_value}"
    );
}

#[test]
fn blend_weighted_average_overlay_falls_back_to_base() {
    let diffs = vec![0.2, 0.5];
    let weights = vec![0.5, 0.75];
    let mask = vec![0.0, 0.0];
    let base_value = 0.4;

    let mut sum_inputs = HashMap::new();
    sum_inputs.insert("values".to_string(), connection("diffs", "out"));
    sum_inputs.insert("weights".to_string(), connection("weights", "out"));
    sum_inputs.insert("mask".to_string(), connection("mask", "out"));

    let mut overlay_inputs = HashMap::new();
    overlay_inputs.insert(
        "total_weighted_sum".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weighted_sum".to_string(),
            selector: None,
        },
    );
    overlay_inputs.insert(
        "total_weight".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "total_weight".to_string(),
            selector: None,
        },
    );
    overlay_inputs.insert(
        "max_weight".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "max_weight".to_string(),
            selector: None,
        },
    );
    overlay_inputs.insert(
        "input_count".to_string(),
        InputConnection {
            node_id: "sum".to_string(),
            output_key: "input_count".to_string(),
            selector: None,
        },
    );
    overlay_inputs.insert("base".to_string(), connection("base", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("diffs", Value::Vector(diffs)),
            constant_node("weights", Value::Vector(weights)),
            constant_node("mask", Value::Vector(mask)),
            constant_node("base", Value::Float(base_value)),
            NodeSpec {
                id: "sum".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                inputs: sum_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "overlay".to_string(),
                kind: NodeType::BlendWeightedAverageOverlay,
                params: NodeParams::default(),
                inputs: overlay_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("overlay should evaluate");
    let overlay_outputs = rt.outputs.get("overlay").expect("overlay outputs present");
    let overlay_value = match &overlay_outputs.get("out").expect("overlay out").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (overlay_value - base_value).abs() < 1e-6,
        "expected base fallback {base_value}, got {overlay_value}"
    );
}

#[test]
fn blend_max_matches_expected_math() {
    let values = vec![1.0, 0.5, 0.9];
    let weights = vec![0.2, 0.8, 0.6];
    let mask = vec![1.0, 1.0, 0.0];
    let base_value = 0.3;

    let mut inputs = HashMap::new();
    inputs.insert("values".to_string(), connection("values", "out"));
    inputs.insert("weights".to_string(), connection("weights", "out"));
    inputs.insert("mask".to_string(), connection("mask", "out"));
    inputs.insert("base".to_string(), connection("base", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values.clone())),
            constant_node("weights", Value::Vector(weights.clone())),
            constant_node("mask", Value::Vector(mask.clone())),
            constant_node("base", Value::Float(base_value)),
            NodeSpec {
                id: "max".to_string(),
                kind: NodeType::BlendMax,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("max should evaluate");
    let outputs = rt.outputs.get("max").expect("max outputs present");
    let max_value = match &outputs.get("out").expect("max out").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };

    let (expected_value, expected_weight, _) = values
        .iter()
        .zip(weights.iter())
        .zip(mask.iter())
        .map(|((value, weight), mask)| (*value, *weight, *weight * *mask))
        .fold((0.0f32, 0.0f32, f32::NEG_INFINITY), |acc, item| {
            let (value, weight, effective) = item;
            if effective > acc.2 {
                (value, weight, effective)
            } else {
                acc
            }
        });
    let expected_max = expected_value * expected_weight;

    assert!(
        (max_value - expected_max).abs() < 1e-6,
        "expected {expected_max}, got {max_value}"
    );
}

#[test]
fn blend_max_returns_base_when_all_masked() {
    let values = vec![1.0, 0.5];
    let weights = vec![0.7, 0.9];
    let mask = vec![0.0, 0.0];
    let base_value = 0.45;

    let mut inputs = HashMap::new();
    inputs.insert("values".to_string(), connection("values", "out"));
    inputs.insert("weights".to_string(), connection("weights", "out"));
    inputs.insert("mask".to_string(), connection("mask", "out"));
    inputs.insert("base".to_string(), connection("base", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("values", Value::Vector(values)),
            constant_node("weights", Value::Vector(weights)),
            constant_node("mask", Value::Vector(mask)),
            constant_node("base", Value::Float(base_value)),
            NodeSpec {
                id: "max".to_string(),
                kind: NodeType::BlendMax,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("max should evaluate");
    let outputs = rt.outputs.get("max").expect("max outputs present");
    let max_value = match &outputs.get("out").expect("max out").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (max_value - base_value).abs() < 1e-6,
        "expected base fallback {base_value}, got {max_value}"
    );
}

#[test]
fn case_node_routes_matching_label_or_default() {
    let labels = vec!["weighted_average".to_string(), "additive".to_string()];

    let mut case_inputs = HashMap::new();
    case_inputs.insert("selector".to_string(), connection("selector_match", "out"));
    case_inputs.insert("default".to_string(), connection("default", "out"));
    case_inputs.insert("cases_1".to_string(), connection("avg_val", "out"));
    case_inputs.insert("cases_2".to_string(), connection("add_val", "out"));

    let mut default_inputs = HashMap::new();
    default_inputs.insert(
        "selector".to_string(),
        connection("selector_unknown", "out"),
    );
    default_inputs.insert("default".to_string(), connection("default", "out"));
    default_inputs.insert("cases_1".to_string(), connection("avg_val", "out"));
    default_inputs.insert("cases_2".to_string(), connection("add_val", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("selector_match", Value::Text("additive".to_string())),
            constant_node("selector_unknown", Value::Text("multiply".to_string())),
            constant_node("avg_val", Value::Float(1.1)),
            constant_node("add_val", Value::Float(2.2)),
            constant_node("default", Value::Float(3.3)),
            NodeSpec {
                id: "case_match".to_string(),
                kind: NodeType::Case,
                params: NodeParams {
                    labels: Some(labels.clone()),
                    ..Default::default()
                },
                inputs: case_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "case_default".to_string(),
                kind: NodeType::Case,
                params: NodeParams {
                    labels: Some(labels),
                    ..Default::default()
                },
                inputs: default_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("case nodes should evaluate");

    let match_outputs = rt
        .outputs
        .get("case_match")
        .expect("case_match outputs present");
    let match_value = match &match_outputs.get("out").expect("case_match out").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (match_value - 2.2).abs() < 1e-6,
        "expected additive branch 2.2, got {match_value}"
    );

    let default_outputs = rt
        .outputs
        .get("case_default")
        .expect("case_default outputs present");
    let default_value = match &default_outputs.get("out").expect("case_default out").value {
        Value::Float(value) => *value,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (default_value - 3.3).abs() < 1e-6,
        "expected default branch 3.3, got {default_value}"
    );
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
