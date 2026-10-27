//! A stress test of a node graph run on the behavior tree: the cost of
//! loading a graph, of a step, and the calls a step makes, for a chain of
//! additions `depth` deep and for a graph read from a JSON file.
//!
//! ```text
//! cargo run --release -p vizij-graph-core --example stress -- chain 10000
//! cargo run --release -p vizij-graph-core --example stress -- file graph.json
//! ```
//!
//! A step of `moving` stages a new value on every input; one of `settled`
//! stages none, so only what the clock drives changes. `Demanded` runs what
//! the outputs read and makes the writes alone, as a device does; `Every`
//! runs every node and copies every port, as an editor does. An `edit` is an
//! editor's: one constant's value changed, then an evaluation, in place
//! ([`TreeRuntime::set_params`]) and by loading the edited spec.
use std::time::{Duration, Instant};

use vizij_api_core::value::float;
use vizij_api_core::{TypedPath, Value};
use vizij_graph_core::import::tree_node_id;
use vizij_graph_core::tree_runtime::{Evaluation, TreeRuntime};
use vizij_graph_core::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeSpec, NodeType,
};

fn node(id: String, kind: NodeType, params: NodeParams) -> NodeSpec {
    NodeSpec {
        id,
        kind,
        params,
        output_shapes: Default::default(),
        input_defaults: Default::default(),
    }
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

/// An input, `depth` additions of a constant one after the other, and an
/// output.
fn chain(depth: usize) -> GraphSpec {
    let mut nodes = vec![
        node(
            "in".into(),
            NodeType::Input,
            NodeParams {
                path: Some(TypedPath::parse("stress/in").expect("a path")),
                value: Some(float(0.0)),
                ..Default::default()
            },
        ),
        node(
            "one".into(),
            NodeType::Constant,
            NodeParams {
                value: Some(float(1.0)),
                ..Default::default()
            },
        ),
    ];
    let mut edges = Vec::new();
    let mut previous = "in".to_string();
    for i in 0..depth {
        let id = format!("add_{i}");
        nodes.push(node(id.clone(), NodeType::Add, NodeParams::default()));
        edges.push(edge(&previous, &id, "operand_0"));
        edges.push(edge("one", &id, "operand_1"));
        previous = id;
    }
    nodes.push(node(
        "out".into(),
        NodeType::Output,
        NodeParams {
            path: Some(TypedPath::parse("stress/out").expect("a path")),
            ..Default::default()
        },
    ));
    edges.push(edge(&previous, "out", "in"));
    GraphSpec {
        nodes,
        edges,
        ..Default::default()
    }
}

fn file(path: &str) -> GraphSpec {
    let text = std::fs::read_to_string(path).expect("the graph file reads");
    let (spec, _) = vizij_graph_core::import::import_json(&text, &Default::default())
        .expect("the graph imports");
    spec
}

fn input_paths(spec: &GraphSpec) -> Vec<TypedPath> {
    spec.nodes
        .iter()
        .filter(|node| node.kind == NodeType::Input)
        .filter_map(|node| node.params.path.clone())
        .collect()
}

const STEPS: u32 = 100;

fn per_step(elapsed: Duration) -> String {
    format!(
        "{:>10.1} µs",
        elapsed.as_secs_f64() * 1e6 / f64::from(STEPS)
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let spec = match (args.get(1).map(String::as_str), args.get(2)) {
        (Some("chain"), Some(depth)) => chain(depth.parse().expect("a depth")),
        (Some("file"), Some(path)) => file(path),
        _ => panic!("usage: stress chain <depth> | stress file <graph.json> [load]"),
    };
    let spec = spec.with_cache();
    if args.get(3).map(String::as_str) == Some("load") {
        // One load and one evaluation, for a profiler.
        let mut tree = TreeRuntime::new(Evaluation::Demanded);
        let start = Instant::now();
        tree.load(&spec).expect("the tree loads");
        println!("load {:.1} ms", start.elapsed().as_secs_f64() * 1e3);
        tree.evaluate().expect("the tree evaluates");
        return;
    }
    let inputs = input_paths(&spec);
    if let Some(mode) = args.get(3).and_then(|mode| mode.strip_prefix("steps-")) {
        // Steps alone, every input moving, for a profiler: `steps-demanded`
        // or `steps-every`.
        let evaluation = match mode {
            "every" => Evaluation::Every,
            _ => Evaluation::Demanded,
        };
        let mut tree = TreeRuntime::new(evaluation);
        tree.set_collect_outputs(evaluation == Evaluation::Every);
        tree.dt = 1.0 / 60.0;
        tree.load(&spec).expect("the tree loads");
        tree.evaluate().expect("the tree evaluates");
        let start = Instant::now();
        for step in 0..STEPS {
            tree.t = step as f32 * tree.dt;
            for (i, path) in inputs.iter().enumerate() {
                tree.set_input(path.clone(), float((step as usize + i) as f32 * 0.01), None);
            }
            tree.evaluate().expect("the tree evaluates");
        }
        println!("step {}", per_step(start.elapsed()));
        return;
    }
    println!(
        "{} nodes, {} edges, {} inputs",
        spec.nodes.len(),
        spec.edges.len(),
        inputs.len()
    );

    for moving in [false, true] {
        let label = if moving { "moving " } else { "settled" };
        let stage = |step: u32, set: &mut dyn FnMut(TypedPath, f32)| {
            if moving {
                for (i, path) in inputs.iter().enumerate() {
                    set(path.clone(), (step as usize + i) as f32 * 0.01);
                }
            }
        };

        for evaluation in [Evaluation::Demanded, Evaluation::Every] {
            let mut tree = TreeRuntime::new(evaluation);
            // A device applies the writes alone; an editor shows every port.
            tree.set_collect_outputs(evaluation == Evaluation::Every);
            tree.dt = 1.0 / 60.0;
            let start = Instant::now();
            tree.load(&spec).expect("the tree loads");
            let load = start.elapsed();
            let start = Instant::now();
            tree.evaluate().expect("the tree evaluates");
            let first = start.elapsed();
            let calls_before: u64 = spec
                .nodes
                .iter()
                .filter_map(|node| tree.tree()?.detached_calls(tree_node_id(&node.id)))
                .sum();
            let start = Instant::now();
            for step in 0..STEPS {
                tree.t = step as f32 * tree.dt;
                stage(step, &mut |path, value| {
                    tree.set_input(path, float(value), None)
                });
                tree.evaluate().expect("the tree evaluates");
            }
            let elapsed = start.elapsed();
            let calls: u64 = spec
                .nodes
                .iter()
                .filter_map(|node| tree.tree()?.detached_calls(tree_node_id(&node.id)))
                .sum();
            println!(
                "{label} tree {:<9}    load {:>10.1} ms   first {:>8.1} ms   step {}   calls/step {:>8.1}",
                format!("{evaluation:?}"),
                load.as_secs_f64() * 1e3,
                first.as_secs_f64() * 1e3,
                per_step(elapsed),
                (calls - calls_before) as f64 / f64::from(STEPS)
            );
        }
    }

    let constant = spec.nodes.iter().position(|node| {
        matches!(node.params.value, Some(Value::F32(_))) && node.kind == NodeType::Constant
    });
    if let Some(at) = constant {
        let mut tree = TreeRuntime::new(Evaluation::Every);
        tree.dt = 1.0 / 60.0;
        tree.load(&spec).expect("the tree loads");
        tree.evaluate().expect("the tree evaluates");
        let mut params = spec.nodes[at].params.clone();
        if let Some(Value::F32(value)) = &mut params.value {
            *value += 1.0;
        }
        let start = Instant::now();
        let in_place = tree
            .set_params(&spec.nodes[at].id, &params)
            .expect("an edit");
        assert!(in_place, "a constant's value is edited in place");
        tree.evaluate().expect("the tree evaluates");
        let in_place = start.elapsed();
        let mut edited = spec.clone();
        edited.nodes[at].params = params;
        let start = Instant::now();
        tree.load(&edited).expect("the tree loads");
        tree.evaluate().expect("the tree evaluates");
        let reload = start.elapsed();
        println!(
            "edit    tree Every        in place + eval {:>8.1} ms   load + eval {:>8.1} ms",
            in_place.as_secs_f64() * 1e3,
            reload.as_secs_f64() * 1e3
        );
    }
}
