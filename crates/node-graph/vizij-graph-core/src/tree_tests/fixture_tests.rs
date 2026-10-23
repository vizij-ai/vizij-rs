//! Every fixture evaluated on the tree two ways over the same inputs and
//! clock, and the two compared at every tick:
//!
//! - evaluating what the outputs read makes the writes evaluating every node
//!   makes;
//! - a parameter edited in place computes what a tree loading the edited
//!   spec computes, on every port of every node.
use vizij_api_core::{TypedPath, Value};

use crate::tree_runtime::{Evaluation, TreeRuntime};
use crate::types::GraphSpec;

/// The fixture `name`'s graph, and its input nodes' paths with the value
/// each reads when nothing is staged.
fn fixture(name: &str) -> (GraphSpec, Vec<(String, Option<Value>)>) {
    let document: serde_json::Value = crate::fixtures::spec(name).expect("the fixture parses");
    let mut spec = match document.get("spec") {
        Some(spec) => spec.clone(),
        None => document.clone(),
    };
    vizij_api_core::json::normalize_graph_spec_value(&mut spec).expect("the fixture normalizes");
    let spec: GraphSpec = serde_json::from_value(spec).expect("a graph");
    let mut inputs: Vec<(String, Option<Value>)> = spec
        .nodes
        .iter()
        .filter(|node| node.kind == crate::types::NodeType::Input)
        .filter_map(|node| {
            Some((
                node.params.path.as_ref()?.to_string(),
                node.params.value.clone(),
            ))
        })
        .collect();
    inputs.sort_by(|a, b| a.0.cmp(&b.0));
    inputs.dedup_by(|a, b| a.0 == b.0);
    (spec, inputs)
}

/// Every `f32` of `json`, the serde form of a value, scaled by `factor`.
fn scale(json: &mut serde_json::Value, factor: f64) {
    match json {
        serde_json::Value::Object(fields) => {
            for (key, field) in fields.iter_mut() {
                match (key.as_str(), field) {
                    ("f32", serde_json::Value::Number(number)) => {
                        *number =
                            serde_json::Number::from_f64(number.as_f64().unwrap_or(0.0) * factor)
                                .unwrap_or_else(|| serde_json::Number::from(0));
                    }
                    ("f32s", serde_json::Value::Array(numbers)) => {
                        for number in numbers {
                            *number = serde_json::json!(number.as_f64().unwrap_or(0.0) * factor);
                        }
                    }
                    (_, field) => scale(field, factor),
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(|item| scale(item, factor)),
        _ => {}
    }
}

/// The value staged on the `i`-th input at tick `tick`: its default, every
/// number of it moving; a moving number for an input with no default.
fn staged(default: &Option<Value>, i: usize, tick: usize) -> Value {
    let factor = 1.0 + 0.05 * (tick as f64 * 0.37 + i as f64).sin();
    match default {
        Some(default) => {
            let mut json = serde_json::to_value(default).expect("a value serializes");
            scale(&mut json, factor);
            serde_json::from_value(json).expect("a value deserializes")
        }
        None => Value::F32(factor as f32 * 2.0 - 2.0),
    }
}

fn close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::F32(a), Value::F32(b)) => (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-6,
        (a, b) => a == b,
    }
}

/// Whether `spec` has a node evaluating a URDF, which fails without the
/// `urdf_ik` feature.
fn needs_urdf(spec: &GraphSpec) -> bool {
    use crate::types::NodeType;
    spec.nodes.iter().any(|node| {
        matches!(
            node.kind,
            NodeType::UrdfFk | NodeType::UrdfIkPosition | NodeType::UrdfIkPose
        )
    })
}

/// What one tick of `rt` evaluating `spec` gives, over the fixture's inputs
/// at `tick`: `None` when the tick fails, as an IK target out of a chain's
/// reach does.
fn tick(
    rt: &mut TreeRuntime,
    spec: &GraphSpec,
    inputs: &[(String, Option<Value>)],
    tick: usize,
) -> Option<()> {
    rt.dt = 1.0 / 60.0;
    rt.t += rt.dt;
    for (i, (input, default)) in inputs.iter().enumerate() {
        let path = TypedPath::parse(input).expect("a path");
        rt.set_input(path, staged(default, i, tick), None);
    }
    rt.evaluate_spec(spec).ok()
}

/// `expected`'s and `actual`'s writes are the same, path by path.
fn same_writes(name: &str, tick: usize, expected: &TreeRuntime, actual: &TreeRuntime) {
    let writes = |batch: &vizij_api_core::WriteBatch| {
        batch
            .iter()
            .map(|op| (op.path.to_string(), op.value.clone()))
            .collect::<Vec<_>>()
    };
    let (expected, actual) = (writes(&expected.writes), writes(&actual.writes));
    assert_eq!(
        expected.len(),
        actual.len(),
        "{name}, tick {tick}: the writes"
    );
    for ((path, value), (got_path, got)) in expected.iter().zip(&actual) {
        assert_eq!(path, got_path, "{name}, tick {tick}");
        assert!(
            close(value, got),
            "{name}, tick {tick}: {path} is {value:?}, then {got:?}"
        );
    }
}

/// `expected` and `actual` hold the same value on every port of every node.
fn same_ports(name: &str, tick: usize, expected: &TreeRuntime, actual: &TreeRuntime) {
    for (node, ports) in &expected.outputs {
        for (port, value) in ports {
            let got = actual.outputs.get(node).and_then(|ports| ports.get(port));
            let got = got.unwrap_or_else(|| panic!("{name}, tick {tick}: no {node}.{port}"));
            assert!(
                close(&value.value, &got.value),
                "{name}, tick {tick}: {node}.{port} is {:?}, then {:?}",
                value.value,
                got.value
            );
            assert_eq!(
                value.shape, got.shape,
                "{name}, tick {tick}: {node}.{port}'s shape"
            );
        }
    }
}

#[test]
fn every_fixture_writes_the_same_evaluating_every_node_or_what_its_outputs_read() {
    for (name, _) in crate::fixtures::ALL {
        let (spec, inputs) = fixture(name);
        if needs_urdf(&spec) && !cfg!(feature = "urdf_ik") {
            // Its URDF nodes fail every evaluation that runs them.
            continue;
        }
        let spec = spec.with_cache();
        let mut every = TreeRuntime::new(Evaluation::Every);
        let mut demanded = TreeRuntime::new(Evaluation::Demanded);
        let mut compared = 0;
        for at in 0..40 {
            let expected = tick(&mut every, &spec, &inputs, at);
            let actual = tick(&mut demanded, &spec, &inputs, at);
            assert_eq!(
                expected.is_some(),
                actual.is_some(),
                "{name}, tick {at}: one evaluation fails"
            );
            if expected.is_some() {
                same_writes(name, at, &every, &demanded);
                compared += 1;
            }
        }
        assert!(compared > 0, "{name}: no tick evaluates");
    }
}

/// The parameters of a node that take any number, which an editor's slider
/// moves.
const CONTINUOUS: &[&str] = &[
    "frequency",
    "phase",
    "min",
    "max",
    "x",
    "y",
    "z",
    "in_min",
    "in_max",
    "out_min",
    "out_max",
    "in_low",
    "in_anchor",
    "in_high",
    "out_low",
    "out_anchor",
    "out_high",
    "bone1",
    "bone2",
    "bone3",
    "lacunarity",
    "persistence",
    "stiffness",
    "damping",
    "mass",
    "half_life",
    "max_rate",
];

/// `node`'s parameters with every continuous one, and a constant's value,
/// scaled by `factor`.
fn edited(node: &crate::types::NodeSpec, factor: f64) -> crate::types::NodeParams {
    let mut json = serde_json::to_value(&node.params).expect("params serialize");
    if let serde_json::Value::Object(fields) = &mut json {
        for (field, value) in fields.iter_mut() {
            match value {
                serde_json::Value::Number(number) if CONTINUOUS.contains(&field.as_str()) => {
                    *value = serde_json::json!(number.as_f64().unwrap_or(0.0) * factor);
                }
                _ if field == "value" && node.kind == crate::types::NodeType::Constant => {
                    scale(value, factor)
                }
                _ => {}
            }
        }
    }
    serde_json::from_value(json).expect("params deserialize")
}

/// Every fixture with its parameters edited halfway through, in place on
/// one tree and in the spec another tree loads: the two compute the same,
/// each node keeping its state through the edit, and the tree edited in
/// place is not built again.
#[test]
fn a_parameter_edited_in_place_computes_what_a_tree_loading_the_edited_spec_computes() {
    let mut edits = 0;
    for (name, _) in crate::fixtures::ALL {
        let (spec, inputs) = fixture(name);
        let spec = spec.with_cache();
        let mut in_place = TreeRuntime::default();
        let mut reloaded = TreeRuntime::default();
        let (mut in_place_spec, mut reloaded_spec) = (spec.clone(), spec.clone());
        for at in 0..40 {
            if at == 20 {
                for (index, node) in spec.nodes.iter().enumerate() {
                    let params = edited(node, 1.1);
                    let made = in_place.set_params(&node.id, &params).expect("an edit");
                    assert!(
                        made,
                        "{name}: the edit of {}'s parameters is made in place",
                        node.id
                    );
                    edits += 1;
                    in_place_spec.nodes[index].params = params.clone();
                    reloaded_spec.nodes[index].params = params;
                }
                // The other tree loads the edited spec, as its host does for
                // an edit it cannot tell is a value's; its nodes keep their
                // state.
                reloaded_spec = reloaded_spec.with_cache();
            }
            let expected = tick(&mut reloaded, &reloaded_spec, &inputs, at);
            let actual = tick(&mut in_place, &in_place_spec, &inputs, at);
            assert_eq!(
                expected.is_some(),
                actual.is_some(),
                "{name}, tick {at}: one evaluation fails"
            );
            if expected.is_some() {
                same_ports(name, at, &reloaded, &in_place);
                same_writes(name, at, &reloaded, &in_place);
            }
        }
        assert_eq!(in_place.loads(), 1, "{name}: the tree is built once");
        assert_eq!(reloaded.loads(), 2, "{name}: the other tree loads the edit");
    }
    assert!(edits > 0);
}
