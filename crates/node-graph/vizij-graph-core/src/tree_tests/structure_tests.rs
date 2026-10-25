//! The structural tests: a graph's topological order (`topo`), how a
//! changed graph keeps what it shares with the last one, and its selections
//! (`eval::shape_helpers`). A changed graph is loaded again, and each node it
//! keeps keeps its state.
use hashbrown::HashMap;
use vizij_api_core::value as vocab;
use vizij_api_core::{TypedPath, Value};

use crate::tree_runtime::{evaluate_all, TreeRuntime};
use crate::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeSpec, NodeType,
    SelectorSegment,
};

fn node(id: &str, kind: NodeType, params: NodeParams) -> NodeSpec {
    NodeSpec {
        id: id.into(),
        kind,
        params,
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    }
}

fn constant(id: &str, value: Value) -> NodeSpec {
    node(
        id,
        NodeType::Constant,
        NodeParams {
            value: Some(value),
            ..Default::default()
        },
    )
}

fn add(id: &str) -> NodeSpec {
    node(id, NodeType::Add, NodeParams::default())
}

fn output(id: &str, path: &str) -> NodeSpec {
    node(
        id,
        NodeType::Output,
        NodeParams {
            path: Some(TypedPath::parse(path).expect("a path")),
            ..Default::default()
        },
    )
}

fn edge(from: &str, to: &str, input: &str) -> EdgeSpec {
    EdgeSpec {
        from: EdgeOutputEndpoint {
            node_id: from.into(),
            output: "out".into(),
        },
        to: EdgeInputEndpoint {
            node_id: to.into(),
            input: input.into(),
        },
        selector: None,
    }
}

fn selected(from: &str, to: &str, selector: Vec<SelectorSegment>) -> EdgeSpec {
    EdgeSpec {
        selector: Some(selector),
        ..edge(from, to, "in")
    }
}

fn field(name: &str) -> SelectorSegment {
    SelectorSegment::Field(name.to_string())
}

fn written(rt: &TreeRuntime) -> Vec<(String, Value)> {
    rt.writes
        .iter()
        .map(|op| (op.path.to_string(), op.value.clone()))
        .collect()
}

fn spec(nodes: Vec<NodeSpec>, edges: Vec<EdgeSpec>) -> GraphSpec {
    GraphSpec {
        nodes,
        edges,
        ..GraphSpec::default()
    }
    .with_cache()
}

/// A node reads what the node it is wired from computed at the same tick.
#[test]
fn simple_topo() {
    let g = spec(
        vec![constant("a", vocab::float(1.0)), add("b")],
        vec![edge("a", "b", "operand_0")],
    );
    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &g).expect("the graph evaluates");
    assert_eq!(rt.outputs.len(), 2);
    assert_eq!(rt.outputs["b"]["out"].value, vocab::float(1.0));
}

/// Of two outputs writing one path, the later-listed one wins, however deep
/// the earlier one's component is, at every tick; outputs of independent
/// nodes write in their listed order.
#[test]
fn the_order_is_the_listed_precedence() {
    let g = spec(
        vec![
            constant("base/a", vocab::float(1.0)),
            add("base/b"),
            add("base/c"),
            output("base/out", "shared/path"),
            constant("late", vocab::float(9.0)),
            output("late/out", "shared/path"),
        ],
        vec![
            edge("base/a", "base/b", "operand_0"),
            edge("base/b", "base/c", "operand_0"),
            edge("base/c", "base/out", "in"),
            edge("late", "late/out", "in"),
        ],
    );
    let mut rt = TreeRuntime::default();
    for _ in 0..4 {
        evaluate_all(&mut rt, &g).expect("the graph evaluates");
        assert_eq!(
            written(&rt),
            vec![
                ("shared/path".to_string(), vocab::float(1.0)),
                ("shared/path".to_string(), vocab::float(9.0)),
            ]
        );
        let key = arora_types::data::Key::from("shared/path");
        use arora_types::data::DataStore;
        assert_eq!(
            rt.store().read(&[key]).remove(0),
            Some(vocab::float(9.0)),
            "the later writer wins"
        );
    }

    let independent = spec(
        vec![
            constant("z", vocab::float(1.0)),
            output("z/out", "z"),
            constant("a", vocab::float(2.0)),
            output("a/out", "a"),
            constant("m", vocab::float(3.0)),
            output("m/out", "m"),
        ],
        vec![
            edge("z", "z/out", "in"),
            edge("a", "a/out", "in"),
            edge("m", "m/out", "in"),
        ],
    );
    evaluate_all(&mut rt, &independent).expect("the graph evaluates");
    let paths: Vec<String> = written(&rt).into_iter().map(|(path, _)| path).collect();
    assert_eq!(paths, ["z", "a", "m"]);
}

#[test]
fn two_nodes_sharing_an_id_are_refused() {
    let g = spec(vec![add("a"), add("a")], vec![]);
    let error = evaluate_all(&mut TreeRuntime::default(), &g).expect_err("refused");
    assert!(error.contains("two nodes share the id 'a'"), "{error}");
}

/// A component of `prefix`: a constant, damped toward, written to
/// `<prefix>/out`.
fn component(prefix: &str, target: f32) -> (Vec<NodeSpec>, Vec<EdgeSpec>) {
    let (value, damp, out) = (
        format!("{prefix}/value"),
        format!("{prefix}/damp"),
        format!("{prefix}/out"),
    );
    (
        vec![
            constant(&value, vocab::float(target)),
            node(
                &damp,
                NodeType::Damp,
                NodeParams {
                    half_life: Some(0.1),
                    ..Default::default()
                },
            ),
            output(&out, &out),
        ],
        vec![edge(&value, &damp, "in"), edge(&damp, &out, "in")],
    )
}

fn spec_of(parts: &[(Vec<NodeSpec>, Vec<EdgeSpec>)]) -> GraphSpec {
    spec(
        parts.iter().flat_map(|(nodes, _)| nodes.clone()).collect(),
        parts.iter().flat_map(|(_, edges)| edges.clone()).collect(),
    )
}

fn damped(rt: &TreeRuntime, prefix: &str) -> f32 {
    vocab::as_float(&rt.outputs[&format!("{prefix}/damp")]["out"].value).expect("a number")
}

/// Step `rt` over `g` once, by a tenth of a second.
fn step(rt: &mut TreeRuntime, g: &GraphSpec) {
    rt.dt = 0.1;
    rt.t += rt.dt;
    evaluate_all(rt, g).expect("the graph evaluates");
}

/// A graph grown by a component computes, for that component, what it
/// computes loaded alone, and the components it had go on from where they
/// were.
#[test]
fn appending_a_component_matches_a_rebuild() {
    let base = spec_of(&[component("base", 0.0)]);
    let mut rt = TreeRuntime::default();
    step(&mut rt, &base);
    let moving = spec_of(&[component("base", 1.0)]);
    for _ in 0..3 {
        step(&mut rt, &moving);
    }
    let before = damped(&rt, "base");
    assert!(before > 0.0 && before < 1.0, "in motion: {before}");

    let grown = spec_of(&[component("base", 1.0), component("run", 1.0)]);
    step(&mut rt, &grown);
    let mut alone = TreeRuntime::default();
    step(&mut alone, &spec_of(&[component("run", 1.0)]));
    assert_eq!(damped(&rt, "run"), damped(&alone, "run"));
    step(&mut rt, &grown);
    assert!(
        damped(&rt, "base") > before,
        "the base goes on from where it was: {} after {before}",
        damped(&rt, "base")
    );
}

/// A graph shrunk by a component computes what it computes for the
/// components it keeps, which go on from where they were.
#[test]
fn removing_a_component_matches_a_rebuild() {
    let full = spec_of(&[
        component("base", 1.0),
        component("run1", 1.0),
        component("run2", 1.0),
    ]);
    let mut rt = TreeRuntime::default();
    let mut reference = TreeRuntime::default();
    let kept = spec_of(&[component("base", 1.0), component("run2", 1.0)]);
    for _ in 0..3 {
        step(&mut rt, &full);
        step(&mut reference, &kept);
    }
    step(&mut rt, &kept);
    step(&mut reference, &kept);
    assert_eq!(rt.outputs.len(), 6, "run1 is gone");
    for prefix in ["base", "run2"] {
        assert_eq!(damped(&rt, prefix), damped(&reference, prefix), "{prefix}");
    }
}

/// A component added beside a kept one can read it: the reader computes
/// from the kept node's value, and the kept node keeps its state. (The
/// reference refuses to extend its plan by such a component, and rebuilds.)
#[test]
fn a_component_reading_a_kept_node_reads_its_value() {
    let mut rt = TreeRuntime::default();
    step(&mut rt, &spec_of(&[component("base", 0.0)]));
    let base = spec_of(&[component("base", 1.0)]);
    for _ in 0..3 {
        step(&mut rt, &base);
    }
    let before = damped(&rt, "base");

    let (mut nodes, mut edges) = component("base", 1.0);
    nodes.push(add("run/reads_base"));
    edges.push(edge("base/damp", "run/reads_base", "operand_0"));
    let crossing = spec(nodes, edges);
    step(&mut rt, &crossing);
    let after = damped(&rt, "base");
    assert!(after > before, "the base went on: {after} after {before}");
    assert_eq!(
        rt.outputs["run/reads_base"]["out"].value,
        vocab::float(after)
    );
}

/// A record's field, an array item, a tuple entry and an enumeration's
/// payload are selected through an edge's selector.
#[test]
fn an_edge_selects_a_part_of_its_source() {
    let value = vocab::record([
        (
            "values",
            vocab::record([("a", vocab::float(1.0)), ("b", vocab::text("x"))]),
        ),
        (
            "items",
            vocab::array(vec![vocab::vec3([1.0, 2.0, 3.0]), vocab::float(4.0)]),
        ),
        ("choice", vocab::enumeration("on", vocab::float(5.0))),
    ]);
    let g = spec(
        vec![
            constant("value", value.clone()),
            output("a", "a"),
            output("b", "b"),
            output("y", "y"),
            output("payload", "payload"),
            output("whole", "whole"),
        ],
        vec![
            selected("value", "a", vec![field("values"), field("a")]),
            selected("value", "b", vec![field("values"), field("b")]),
            selected(
                "value",
                "y",
                vec![
                    field("items"),
                    SelectorSegment::Index(0),
                    SelectorSegment::Index(1),
                ],
            ),
            selected("value", "payload", vec![field("choice"), field("on")]),
            selected("value", "whole", vec![]),
        ],
    );
    let mut rt = TreeRuntime::default();
    evaluate_all(&mut rt, &g).expect("the graph evaluates");
    assert_eq!(
        written(&rt),
        vec![
            ("a".to_string(), vocab::float(1.0)),
            ("b".to_string(), vocab::text("x")),
            ("y".to_string(), vocab::float(2.0)),
            ("payload".to_string(), vocab::float(5.0)),
            ("whole".to_string(), value),
        ]
    );
}

/// A missing field, an out-of-range index and a variant the value is not
/// fail the evaluation, saying what failed.
#[test]
fn a_failed_selection_says_why() {
    let value = vocab::record([
        ("v", vocab::vec2([1.0, 2.0])),
        ("e", vocab::enumeration("on", vocab::float(1.0))),
    ]);
    let error = |selector: Vec<SelectorSegment>| {
        let g = spec(
            vec![constant("value", value.clone()), output("out", "out")],
            vec![selected("value", "out", selector)],
        );
        evaluate_all(&mut TreeRuntime::default(), &g).expect_err("the selection fails")
    };
    let missing = error(vec![field("missing")]);
    assert!(
        missing.contains("selector field 'missing' missing in record"),
        "{missing}"
    );
    let index = error(vec![field("v"), SelectorSegment::Index(2)]);
    assert!(
        index.contains("selector index 2 out of bounds for vec2"),
        "{index}"
    );
    let variant = error(vec![field("e"), field("off")]);
    assert!(
        variant.contains("selector field 'off' does not match"),
        "{variant}"
    );
    let unsupported = error(vec![field("v"), field("x")]);
    assert!(
        unsupported.contains("selector field 'x' unsupported"),
        "{unsupported}"
    );
}
