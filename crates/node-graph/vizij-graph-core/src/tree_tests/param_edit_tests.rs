//! A node's parameters edited in place on a loaded graph
//! ([`TreeRuntime::set_params`]): a value reaches the node at the next
//! evaluation with the tree kept, and an edit that changes what the tree is
//! is refused, the graph left as it was.
use vizij_api_core::Value;

use crate::import::tree_node_id;
use crate::tree_runtime::TreeRuntime;
use crate::types::{GraphSpec, NodeParams};

/// The graph `json` describes, loaded and evaluated once.
fn loaded(json: serde_json::Value) -> (GraphSpec, TreeRuntime) {
    let mut json = json;
    vizij_api_core::json::normalize_graph_spec_value(&mut json).expect("the spec normalizes");
    let spec: GraphSpec = serde_json::from_value(json).expect("a graph");
    let mut rt = TreeRuntime::default();
    rt.dt = 1.0 / 60.0;
    rt.load(&spec).expect("the graph loads");
    rt.evaluate().expect("the graph evaluates");
    (spec, rt)
}

/// `id`'s parameters in `spec`.
fn params(spec: &GraphSpec, id: &str) -> NodeParams {
    spec.nodes
        .iter()
        .find(|node| node.id == id)
        .expect("the node")
        .params
        .clone()
}

fn written(rt: &TreeRuntime, path: &str) -> Option<Value> {
    rt.writes
        .iter()
        .find(|op| op.path.to_string() == path)
        .map(|op| op.value.clone())
}

#[test]
fn a_value_edited_in_place_reaches_the_outputs_with_the_tree_kept() {
    let (spec, mut rt) = loaded(serde_json::json!({
        "nodes": [
            { "id": "c", "type": "constant", "params": { "value": 1.0 } },
            { "id": "out", "type": "output", "params": { "path": "demo/out" } }
        ],
        "edges": [
            { "from": { "node_id": "c", "output": "out" }, "to": { "node_id": "out", "input": "in" } }
        ]
    }));
    assert_eq!(written(&rt, "demo/out"), Some(Value::F32(1.0)));

    let edited = NodeParams {
        value: Some(Value::F32(2.0)),
        ..params(&spec, "c")
    };
    assert!(rt.set_params("c", &edited).expect("an edit"));
    rt.evaluate().expect("the graph evaluates");
    assert_eq!(written(&rt, "demo/out"), Some(Value::F32(2.0)));
    assert_eq!(rt.loads(), 1, "the tree is kept");
    assert_eq!(
        rt.tree()
            .and_then(|tree| tree.detached_calls(tree_node_id("c"))),
        Some(2),
        "the constant runs again for its new value"
    );
}

#[test]
fn a_spring_edited_in_place_keeps_its_state() {
    let (spec, mut rt) = loaded(serde_json::json!({
        "nodes": [
            { "id": "target", "type": "constant", "params": { "value": 1.0 } },
            { "id": "s", "type": "spring", "params": { "stiffness": 120.0, "damping": 20.0 } },
            { "id": "out", "type": "output", "params": { "path": "demo/out" } }
        ],
        "edges": [
            { "from": { "node_id": "target", "output": "out" }, "to": { "node_id": "s", "input": "in" } },
            { "from": { "node_id": "s", "output": "out" }, "to": { "node_id": "out", "input": "in" } }
        ]
    }));
    for _ in 0..5 {
        rt.evaluate().expect("the graph evaluates");
    }
    let state = rt.state_value("s").expect("the spring holds a state");

    let edited = NodeParams {
        stiffness: Some(60.0),
        ..params(&spec, "s")
    };
    assert!(rt.set_params("s", &edited).expect("an edit"));
    assert_eq!(rt.state_value("s"), Some(state), "the edit keeps the state");
    rt.evaluate().expect("the graph evaluates");
    assert_eq!(rt.loads(), 1, "the tree is kept");
}

#[test]
fn an_edit_of_a_split_s_sizes_is_refused_and_changes_nothing() {
    let (spec, mut rt) = loaded(serde_json::json!({
        "nodes": [
            { "id": "vec", "type": "constant", "params": { "value": [1, 2, 3, 4] } },
            { "id": "split", "type": "split", "params": { "sizes": [2, 2] } }
        ],
        "edges": [
            { "from": { "node_id": "vec", "output": "out" }, "to": { "node_id": "split", "input": "in" } }
        ]
    }));
    let ports = rt.outputs["split"].len();

    let edited = NodeParams {
        sizes: Some(vec![1.0, 1.0, 1.0, 1.0]),
        ..params(&spec, "split")
    };
    assert!(
        !rt.set_params("split", &edited).expect("an edit"),
        "new sizes give the split other ports"
    );
    rt.evaluate().expect("the graph evaluates");
    assert_eq!(rt.outputs["split"].len(), ports, "the split is as it was");
}

#[test]
fn an_edit_of_an_output_s_path_is_refused() {
    let (spec, mut rt) = loaded(serde_json::json!({
        "nodes": [
            { "id": "c", "type": "constant", "params": { "value": 1.0 } },
            { "id": "out", "type": "output", "params": { "path": "demo/out" } }
        ],
        "edges": [
            { "from": { "node_id": "c", "output": "out" }, "to": { "node_id": "out", "input": "in" } }
        ]
    }));
    let edited = NodeParams {
        path: Some(vizij_api_core::TypedPath::parse("demo/elsewhere").expect("a path")),
        ..params(&spec, "out")
    };
    assert!(
        !rt.set_params("out", &edited).expect("an edit"),
        "the path is the store key the tree binds"
    );
    rt.evaluate().expect("the graph evaluates");
    assert_eq!(written(&rt, "demo/out"), Some(Value::F32(1.0)));
}

#[test]
fn an_edit_setting_a_parameter_the_node_did_not_set_is_refused() {
    let (spec, mut rt) = loaded(serde_json::json!({
        "nodes": [
            { "id": "target", "type": "constant", "params": { "value": 1.0 } },
            { "id": "s", "type": "spring", "params": {} }
        ],
        "edges": [
            { "from": { "node_id": "target", "output": "out" }, "to": { "node_id": "s", "input": "in" } }
        ]
    }));
    let edited = NodeParams {
        stiffness: Some(60.0),
        ..params(&spec, "s")
    };
    assert!(
        !rt.set_params("s", &edited).expect("an edit"),
        "the tree has no argument for a parameter the node did not set"
    );
    assert!(!rt
        .set_params("absent", &NodeParams::default())
        .expect("an edit"));
}
