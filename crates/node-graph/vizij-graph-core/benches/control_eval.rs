//! What control nodes cost and save, per tick.
//!
//! - `branches`: eight branches, each a chain of sixteen `multiply` nodes
//!   ending in a `lessthan` and a `condition`, under a `fallback` whose first
//!   branch succeeds, under a `parallel` that ticks them all, and as a plain
//!   graph with no control node, where every node runs, as today.
//! - `children`: sixty-four conditions on constants, as the children of one
//!   `sequence` and as a plain graph: the cost of ticking a child over
//!   evaluating a node.
//!
//! Each tick goes through `evaluate_writes_with_functions`, as a host that
//! reads the graph's writes does.
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use std::time::Duration;
use vizij_api_core::Value;
use vizij_graph_core::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeSpec, NodeType,
};
use vizij_graph_core::{evaluate_writes_with_functions, GraphRuntime, NodeFunctions};

/// No function: the bench graphs call none.
struct NoFunctions;

impl NodeFunctions for NoFunctions {
    fn call(&mut self, function: uuid::Uuid, _: &[(uuid::Uuid, Value)]) -> Result<Value, String> {
        Err(format!("no function {function}"))
    }
}

fn node(id: String, kind: NodeType, params: NodeParams) -> NodeSpec {
    NodeSpec {
        id,
        kind,
        params,
        output_shapes: Default::default(),
        input_defaults: Default::default(),
    }
}

fn constant(id: String, value: f32) -> NodeSpec {
    node(
        id,
        NodeType::Constant,
        NodeParams {
            value: Some(vizij_api_core::value::float(value)),
            ..Default::default()
        },
    )
}

fn edge(from: &str, to: &str, input: &str) -> EdgeSpec {
    EdgeSpec {
        from: EdgeOutputEndpoint {
            node_id: from.to_string(),
            output: "out".into(),
        },
        to: EdgeInputEndpoint {
            node_id: to.to_string(),
            input: input.to_string(),
        },
        selector: None,
    }
}

/// Eight chains of sixteen multiplies, each ending in a condition; the first
/// condition holds, the others do not. `root` takes the conditions as its
/// children; `None` writes each to an output instead.
fn branches(root: Option<NodeType>) -> GraphSpec {
    let mut spec = GraphSpec::default();
    spec.nodes.push(constant("factor".into(), 1.01));
    let mut conditions = Vec::new();
    for branch in 0..8 {
        let start = format!("b{branch}_start");
        spec.nodes.push(constant(start.clone(), 1.0));
        let mut last = start;
        for step in 0..16 {
            let id = format!("b{branch}_m{step}");
            spec.nodes
                .push(node(id.clone(), NodeType::Multiply, NodeParams::default()));
            spec.edges.push(edge(&last, &id, "operand_0"));
            spec.edges.push(edge("factor", &id, "operand_1"));
            last = id;
        }
        let limit = format!("b{branch}_limit");
        spec.nodes.push(constant(
            limit.clone(),
            if branch == 0 { 10.0 } else { 0.0 },
        ));
        let less = format!("b{branch}_less");
        spec.nodes.push(node(
            less.clone(),
            NodeType::LessThan,
            NodeParams::default(),
        ));
        spec.edges.push(edge(&last, &less, "lhs"));
        spec.edges.push(edge(&limit, &less, "rhs"));
        let condition = format!("b{branch}_condition");
        spec.nodes.push(node(
            condition.clone(),
            NodeType::Condition,
            NodeParams::default(),
        ));
        spec.edges.push(edge(&less, &condition, "in"));
        conditions.push(condition);
    }
    attach(&mut spec, root, &conditions);
    spec.with_cache()
}

/// Sixty-four conditions on a constant that holds.
fn children(root: Option<NodeType>) -> GraphSpec {
    let mut spec = GraphSpec::default();
    let mut conditions = Vec::new();
    for child in 0..64 {
        let value = format!("c{child}_value");
        spec.nodes.push(constant(value.clone(), 1.0));
        let condition = format!("c{child}");
        spec.nodes.push(node(
            condition.clone(),
            NodeType::Condition,
            NodeParams::default(),
        ));
        spec.edges.push(edge(&value, &condition, "in"));
        conditions.push(condition);
    }
    attach(&mut spec, root, &conditions);
    spec.with_cache()
}

fn attach(spec: &mut GraphSpec, root: Option<NodeType>, children: &[String]) {
    // Read by nothing without a root: a graph with no control node runs
    // every node.
    if let Some(kind) = root {
        spec.nodes
            .push(node("root".into(), kind, NodeParams::default()));
        for (at, child) in children.iter().enumerate() {
            spec.edges.push(edge(child, "root", &format!("child_{at}")));
        }
    }
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("control");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    for (name, spec) in [
        ("branches/fallback", branches(Some(NodeType::Fallback))),
        ("branches/parallel", branches(Some(NodeType::Parallel))),
        ("branches/plain", branches(None)),
        ("children/sequence", children(Some(NodeType::Sequence))),
        ("children/plain", children(None)),
    ] {
        spec.validate().expect("the bench graph is valid");
        let mut rt = GraphRuntime::default();
        let mut host = NoFunctions;
        evaluate_writes_with_functions(&mut rt, &spec, &mut host).expect("it evaluates");
        group.bench_function(name, |b| {
            b.iter(|| {
                evaluate_writes_with_functions(&mut rt, &spec, &mut host).expect("it evaluates");
                black_box(&rt.writes);
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
