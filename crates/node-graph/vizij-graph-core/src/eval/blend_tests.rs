// Blend node unit tests (broadcasting, mismatch, and basic semantics)

use super::*;
use crate::types::{GraphSpec, InputConnection, NodeParams, NodeSpec, NodeType};
use hashbrown::HashMap;
use vizij_api_core::Value;

/// Very small helpers duplicated from tests.rs for local use
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

#[test]
fn weighted_sum_vector_scalar_weight_broadcasts_and_outputs_descriptive_ports() {
    // values = [1,2,3], weight = 0.5 (scalar broadcast)
    let mut inputs = HashMap::new();
    inputs.insert("values".to_string(), connection("vals", "out"));
    inputs.insert("weights".to_string(), connection("w", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("vals", Value::Vector(vec![1.0, 2.0, 3.0])),
            constant_node("w", Value::Float(0.5)),
            NodeSpec {
                id: "ws".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("weighted sum should evaluate");
    let outputs = rt.outputs.get("ws").expect("ws outputs present");

    // total_weighted_sum == (1+2+3) * 0.5 == 3.0
    match &outputs
        .get("total_weighted_sum")
        .expect("sum present")
        .value
    {
        Value::Float(f) => assert!((*f - 3.0).abs() < 1e-6),
        other => panic!("expected float sum, got {:?}", other),
    }

    // total_weight == 0.5 * 3 == 1.5
    match &outputs.get("total_weight").expect("total present").value {
        Value::Float(f) => assert!((*f - 1.5).abs() < 1e-6),
        other => panic!("expected float total weight, got {:?}", other),
    }

    // max_effective_weight == 0.5
    match &outputs
        .get("max_effective_weight")
        .expect("max present")
        .value
    {
        Value::Float(f) => assert!((*f - 0.5).abs() < 1e-6),
        other => panic!("expected float max weight, got {:?}", other),
    }

    // input_count == 3.0
    match &outputs.get("input_count").expect("count present").value {
        Value::Float(f) => assert!((*f - 3.0).abs() < 1e-6),
        other => panic!("expected float count, got {:?}", other),
    }
}

#[test]
fn weighted_sum_vector_length_mismatch_returns_nans() {
    // values length 3, weights length 2 -> mismatch after broadcasting
    let mut inputs = HashMap::new();
    inputs.insert("values".to_string(), connection("vals", "out"));
    inputs.insert("weights".to_string(), connection("w", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("vals", Value::Vector(vec![1.0, 2.0, 3.0])),
            constant_node("w", Value::Vector(vec![0.5, 0.5])), // length 2
            NodeSpec {
                id: "ws".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("weighted sum should evaluate");
    let outputs = rt.outputs.get("ws").expect("ws outputs present");

    // sum should be NaN
    match &outputs
        .get("total_weighted_sum")
        .expect("sum present")
        .value
    {
        Value::Float(f) => assert!(f.is_nan()),
        other => panic!("expected float sum NaN, got {:?}", other),
    }
    match &outputs.get("total_weight").expect("total present").value {
        Value::Float(f) => assert!(f.is_nan()),
        other => panic!("expected float total NaN, got {:?}", other),
    }
}

#[test]
fn blend_weighted_average_computes_normalized_average() {
    // Compose WeightedSumVector -> BlendWeightedAverage
    // values [1,2,3], weight scalar 0.5 -> sum=3.0, total_weight=1.5, max=0.5
    // normalized average computed as sum / (total_weight / max) = 3 / (1.5/0.5) = 1.0
    let mut ws_inputs = HashMap::new();
    ws_inputs.insert("values".to_string(), connection("vals", "out"));
    ws_inputs.insert("weights".to_string(), connection("w", "out"));

    // Blend node inputs map to weighted-sum outputs
    let mut b_inputs = HashMap::new();
    b_inputs.insert(
        "total_weighted_sum".to_string(),
        connection("ws", "total_weighted_sum"),
    );
    b_inputs.insert("total_weight".to_string(), connection("ws", "total_weight"));
    b_inputs.insert(
        "max_effective_weight".to_string(),
        connection("ws", "max_effective_weight"),
    );

    let graph = GraphSpec {
        nodes: vec![
            constant_node("vals", Value::Vector(vec![1.0, 2.0, 3.0])),
            constant_node("w", Value::Float(0.5)),
            NodeSpec {
                id: "ws".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                inputs: ws_inputs,
                output_shapes: HashMap::new(),
            },
            NodeSpec {
                id: "bavg".to_string(),
                kind: NodeType::BlendWeightedAverage,
                params: NodeParams::default(),
                inputs: b_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    let outputs = rt.outputs.get("bavg").expect("bavg outputs present");

    match &outputs.get("out").expect("out present").value {
        Value::Float(f) => assert!((*f - 1.0).abs() < 1e-6),
        other => panic!("expected float out, got {:?}", other),
    }
}

#[test]
fn blend_multiply_computes_product_of_terms() {
    // values [0.2,0.5], weight scalar 0.5 -> terms = (1-0.5)+v*0.5 -> 0.6 and 0.75 -> product 0.45
    let mut inputs = HashMap::new();
    inputs.insert("values".to_string(), connection("vals", "out"));
    inputs.insert("weights".to_string(), connection("w", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("vals", Value::Vector(vec![0.2, 0.5])),
            constant_node("w", Value::Float(0.5)),
            NodeSpec {
                id: "mult".to_string(),
                kind: NodeType::BlendMultiply,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("blend multiply should evaluate");
    let outputs = rt.outputs.get("mult").expect("mult outputs present");

    match &outputs.get("out").expect("out present").value {
        Value::Float(f) => assert!((*f - 0.45).abs() < 1e-6),
        other => panic!("expected float out, got {:?}", other),
    }
}

#[test]
fn blend_max_selects_value_of_highest_effective_weight() {
    // values [1,2,3], weights [0.1,0.9,0.2] -> best idx 1 => selected = 2.0 * 0.9 = 1.8
    let mut inputs = HashMap::new();
    inputs.insert("values".to_string(), connection("vals", "out"));
    inputs.insert("weights".to_string(), connection("w", "out"));

    let graph = GraphSpec {
        nodes: vec![
            constant_node("vals", Value::Vector(vec![1.0, 2.0, 3.0])),
            constant_node("w", Value::Vector(vec![0.1, 0.9, 0.2])),
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
    evaluate_all(&mut rt, &graph).expect("blend max should evaluate");
    let outputs = rt.outputs.get("max").expect("max outputs present");

    match &outputs.get("out").expect("out present").value {
        Value::Float(f) => assert!((*f - 1.8).abs() < 1e-6),
        other => panic!("expected float out, got {:?}", other),
    }
}

#[test]
fn case_node_routes_based_on_case_labels_param() {
    // Case with labels ["a","b"] and two variadic case inputs: cases_1 -> "first", cases_2 -> "second"
    let mut inputs = HashMap::new();
    inputs.insert("selector".to_string(), connection("sel", "out"));
    inputs.insert("cases_1".to_string(), connection("c1", "out"));
    inputs.insert("cases_2".to_string(), connection("c2", "out"));
    inputs.insert("default".to_string(), connection("d", "out"));

    let params = NodeParams {
        case_labels: Some(vec!["a".to_string(), "b".to_string()]),
        ..Default::default()
    };

    let graph = GraphSpec {
        nodes: vec![
            constant_node("sel", Value::Text("b".to_string())),
            constant_node("c1", Value::Text("first".to_string())),
            constant_node("c2", Value::Text("second".to_string())),
            constant_node("d", Value::Text("fallback".to_string())),
            NodeSpec {
                id: "case".to_string(),
                kind: NodeType::Case,
                params,
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("case should evaluate");
    let outputs = rt.outputs.get("case").expect("case outputs present");

    match &outputs.get("out").expect("out present").value {
        Value::Text(s) => assert_eq!(s.as_str(), "second"),
        other => panic!("expected text out, got {:?}", other),
    }
}
