// Blend node unit tests (broadcasting, mismatch, and basic semantics)

use super::*;
use crate::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeSpec, NodeType,
};
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

#[test]
fn weighted_sum_vector_scalar_weight_broadcasts_and_outputs_descriptive_ports() {
    // values = [1,2,3], weight = 0.5 (scalar broadcast)
    let graph = GraphSpec {
        nodes: vec![
            constant_node("vals", Value::Vector(vec![1.0, 2.0, 3.0])),
            constant_node("w", Value::Float(0.5)),
            NodeSpec {
                id: "ws".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link("vals", "ws", "values"), link("w", "ws", "weights")],
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
    let graph = GraphSpec {
        nodes: vec![
            constant_node("vals", Value::Vector(vec![1.0, 2.0, 3.0])),
            constant_node("w", Value::Vector(vec![0.5, 0.5])), // length 2
            NodeSpec {
                id: "ws".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link("vals", "ws", "values"), link("w", "ws", "weights")],
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
    let graph = GraphSpec {
        nodes: vec![
            constant_node("vals", Value::Vector(vec![1.0, 2.0, 3.0])),
            constant_node("w", Value::Float(0.5)),
            NodeSpec {
                id: "ws".to_string(),
                kind: NodeType::WeightedSumVector,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "bavg".to_string(),
                kind: NodeType::BlendWeightedAverage,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![
            link("vals", "ws", "values"),
            link("w", "ws", "weights"),
            link_with_output("ws", "total_weighted_sum", "bavg", "total_weighted_sum"),
            link_with_output("ws", "total_weight", "bavg", "total_weight"),
            link_with_output("ws", "max_effective_weight", "bavg", "max_effective_weight"),
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
    let graph = GraphSpec {
        nodes: vec![
            constant_node("vals", Value::Vector(vec![0.2, 0.5])),
            constant_node("w", Value::Float(0.5)),
            NodeSpec {
                id: "mult".to_string(),
                kind: NodeType::BlendMultiply,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link("vals", "mult", "values"), link("w", "mult", "weights")],
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
    let graph = GraphSpec {
        nodes: vec![
            constant_node("vals", Value::Vector(vec![1.0, 2.0, 3.0])),
            constant_node("w", Value::Vector(vec![0.1, 0.9, 0.2])),
            NodeSpec {
                id: "max".to_string(),
                kind: NodeType::BlendMax,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![link("vals", "max", "values"), link("w", "max", "weights")],
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
fn blend_max_without_values_or_base_returns_nan() {
    // No inputs connected; optional base should be treated as absent, yielding NaN.
    let graph = GraphSpec {
        nodes: vec![NodeSpec {
            id: "max".to_string(),
            kind: NodeType::BlendMax,
            params: NodeParams::default(),
            output_shapes: HashMap::new(),
            input_defaults: HashMap::new(),
        }],
        edges: vec![],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("blend max should evaluate");
    let outputs = rt.outputs.get("max").expect("max outputs present");

    match &outputs.get("out").expect("out present").value {
        Value::Float(f) => assert!(f.is_nan()),
        other => panic!("expected float out NaN, got {:?}", other),
    }
}

#[test]
fn default_blend_matches_expected_vec3_output() {
    let baseline = Value::Vec3([0.1, -0.05, 0.2]);
    let offset = Value::Vec3([0.01, 0.02, -0.03]);
    let target1 = Value::Vec3([0.5, -0.2, 0.1]);
    let target2 = Value::Vec3([-0.3, 0.4, 0.25]);
    let w1 = 0.6f32;
    let w2 = 0.3f32;

    let graph = GraphSpec {
        nodes: vec![
            constant_node("baseline", baseline.clone()),
            constant_node("offset", offset.clone()),
            constant_node("t1", target1.clone()),
            constant_node("t2", target2.clone()),
            constant_node("w1", Value::Float(w1)),
            constant_node("w2", Value::Float(w2)),
            NodeSpec {
                id: "weights".to_string(),
                kind: NodeType::Join,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
            NodeSpec {
                id: "blend".to_string(),
                kind: NodeType::DefaultBlend,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![
            link("w1", "weights", "operand_1"),
            link("w2", "weights", "operand_2"),
            link("baseline", "blend", "baseline"),
            link("offset", "blend", "offset"),
            link("weights", "blend", "weights"),
            link("t1", "blend", "operand_1"),
            link("t2", "blend", "operand_2"),
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("default blend should evaluate");
    let outputs = rt.outputs.get("blend").expect("blend outputs present");

    let expected_baseline_factor = (1.0 - (w1 + w2)).max(0.0);
    let expected = {
        let b = match baseline {
            Value::Vec3(arr) => arr,
            _ => unreachable!(),
        };
        let o = match offset {
            Value::Vec3(arr) => arr,
            _ => unreachable!(),
        };
        let t1_arr = match target1 {
            Value::Vec3(arr) => arr,
            _ => unreachable!(),
        };
        let t2_arr = match target2 {
            Value::Vec3(arr) => arr,
            _ => unreachable!(),
        };

        let mut acc = [0.0f32; 3];
        for i in 0..3 {
            let blended_targets = t1_arr[i] * w1 + t2_arr[i] * w2;
            let baseline_term = b[i] * expected_baseline_factor;
            acc[i] = blended_targets + baseline_term + o[i];
        }
        acc
    };

    match &outputs.get("out").expect("out present").value {
        Value::Vec3(actual) => {
            for i in 0..3 {
                assert!((actual[i] - expected[i]).abs() < 1e-6);
            }
        }
        other => panic!("expected vec3 out, got {:?}", other),
    }
}

#[test]
fn default_blend_emits_nan_value_when_weight_length_mismatch() {
    let graph = GraphSpec {
        nodes: vec![
            constant_node("baseline", Value::Vec3([0.0, 0.0, 0.0])),
            constant_node("t1", Value::Vec3([1.0, 0.0, 0.0])),
            constant_node("t2", Value::Vec3([0.0, 1.0, 0.0])),
            constant_node("weights", Value::Vector(vec![0.5, 0.3, 0.2])),
            NodeSpec {
                id: "blend".to_string(),
                kind: NodeType::DefaultBlend,
                params: NodeParams::default(),
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![
            link("baseline", "blend", "baseline"),
            link("t1", "blend", "operand_1"),
            link("t2", "blend", "operand_2"),
            link("weights", "blend", "weights"),
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("default blend should evaluate");
    let outputs = rt.outputs.get("blend").expect("blend outputs present");

    match &outputs.get("out").expect("out present").value {
        Value::Vec3(arr) => assert!(arr.iter().all(|v| v.is_nan())),
        other => panic!("expected vec3 out with NaNs, got {:?}", other),
    }
}

#[test]
fn case_node_routes_based_on_case_labels_param() {
    // Case with labels ["a","b"] and two variadic case inputs: cases_1 -> "first", cases_2 -> "second"
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
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![
            link("sel", "case", "selector"),
            link("c1", "case", "operand_1"),
            link("c2", "case", "operand_2"),
            link("d", "case", "default"),
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

#[test]
fn case_node_without_default_emits_nan_when_no_match() {
    let params = NodeParams {
        case_labels: Some(vec!["a".to_string(), "b".to_string()]),
        ..Default::default()
    };

    let graph = GraphSpec {
        nodes: vec![
            constant_node("sel", Value::Text("z".to_string())),
            constant_node("c1", Value::Float(1.0)),
            constant_node("c2", Value::Float(2.0)),
            NodeSpec {
                id: "case".to_string(),
                kind: NodeType::Case,
                params,
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        edges: vec![
            link("sel", "case", "selector"),
            link("c1", "case", "operand_1"),
            link("c2", "case", "operand_2"),
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("case should evaluate");
    let outputs = rt.outputs.get("case").expect("case outputs present");

    match &outputs.get("out").expect("out present").value {
        Value::Float(f) => assert!(f.is_nan()),
        other => panic!("expected float NaN out, got {:?}", other),
    }
}
