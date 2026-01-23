use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use hashbrown::HashMap;
use std::str::FromStr;
use std::time::Duration;
use vizij_api_core::json::normalize_graph_spec_value;
use vizij_api_core::Shape;
use vizij_api_core::Value;
use vizij_graph_core::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, InputDefault, NodeParams, NodeSpec,
    NodeType,
};
use vizij_graph_core::{evaluate_all, GraphRuntime};
use vizij_test_fixtures::node_graphs;

fn link(from: &str, to: &str, input: &str) -> EdgeSpec {
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

fn constant_node(id: impl Into<String>, value: f32) -> NodeSpec {
    NodeSpec {
        id: id.into(),
        kind: NodeType::Constant,
        params: NodeParams {
            value: Some(Value::Float(value)),
            ..Default::default()
        },
        output_shapes: HashMap::<String, Shape>::new(),
        input_defaults: HashMap::<String, InputDefault>::new(),
    }
}

fn vector_constant_node(id: impl Into<String>, xyz: [f32; 3]) -> NodeSpec {
    NodeSpec {
        id: id.into(),
        kind: NodeType::VectorConstant,
        params: NodeParams {
            x: Some(xyz[0]),
            y: Some(xyz[1]),
            z: Some(xyz[2]),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    }
}

fn oscillator_node(id: impl Into<String>, freq: f32) -> NodeSpec {
    NodeSpec {
        id: id.into(),
        kind: NodeType::Oscillator,
        params: NodeParams {
            frequency: Some(freq),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    }
}

fn input_node(id: impl Into<String>, path: impl Into<String>, default: f32) -> NodeSpec {
    NodeSpec {
        id: id.into(),
        kind: NodeType::Input,
        params: NodeParams {
            path: Some(
                path.into()
                    .parse()
                    .expect("bench input path parses to TypedPath"),
            ),
            value: Some(Value::Float(default)),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    }
}

fn output_node(id: impl Into<String>, path: impl Into<String>) -> NodeSpec {
    NodeSpec {
        id: id.into(),
        kind: NodeType::Output,
        params: NodeParams {
            path: Some(
                path.into()
                    .parse()
                    .expect("bench output path parses to TypedPath"),
            ),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    }
}

fn add_node(id: impl Into<String>) -> NodeSpec {
    NodeSpec {
        id: id.into(),
        kind: NodeType::Add,
        params: NodeParams::default(),
        output_shapes: HashMap::<String, Shape>::new(),
        input_defaults: HashMap::<String, InputDefault>::new(),
    }
}

/// Chain of Add nodes fed by a shared constant; length controls depth.
fn chain_graph(length: usize) -> GraphSpec {
    let length = length.max(1);
    let mut nodes = Vec::with_capacity(length + 2);
    let mut edges = Vec::with_capacity(length * 2);

    nodes.push(constant_node("const_base", 1.0));
    let mut prev = "const_base".to_string();

    for i in 0..length {
        let const_id = format!("bias_{i}");
        let add_id = format!("add_{i}");
        nodes.push(constant_node(&const_id, 0.5));
        nodes.push(add_node(&add_id));
        edges.push(link(&prev, &add_id, "operand_1"));
        edges.push(link(&const_id, &add_id, "operand_2"));
        prev = add_id;
    }

    GraphSpec {
        nodes,
        edges,
        ..Default::default()
    }
    .with_cache()
}

/// Construct a “kitchen sink” block that hits many node types (excluding robotics/blend).
/// Returns (nodes, edges, entry_target_id, entry_port, exit_id).
fn kitchen_block(idx: usize) -> (Vec<NodeSpec>, Vec<EdgeSpec>, String, String, String) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Inputs & seeds
    let in_a = format!("in_a_{idx}");
    let in_b = format!("in_b_{idx}");
    nodes.push(input_node(&in_a, format!("bench/{idx}/a"), idx as f32));
    nodes.push(input_node(
        &in_b,
        format!("bench/{idx}/b"),
        (idx * 2) as f32,
    ));

    let const_gain = format!("c_gain_{idx}");
    nodes.push(constant_node(&const_gain, 1.5));
    let const_bias = format!("c_bias_{idx}");
    nodes.push(constant_node(&const_bias, 0.25));
    let const_div = format!("c_div_{idx}");
    nodes.push(constant_node(&const_div, 3.0));
    let const_min = format!("c_min_{idx}");
    nodes.push(constant_node(&const_min, -1.0));
    let const_max = format!("c_max_{idx}");
    nodes.push(constant_node(&const_max, 1.0));
    let in_min = format!("in_min_{idx}");
    nodes.push(constant_node(&in_min, -2.0));
    let in_max = format!("in_max_{idx}");
    nodes.push(constant_node(&in_max, 2.0));
    let out_min = format!("out_min_{idx}");
    nodes.push(constant_node(&out_min, 0.0));
    let out_max = format!("out_max_{idx}");
    nodes.push(constant_node(&out_max, 10.0));
    let const_scalar = format!("c_scalar_{idx}");
    nodes.push(constant_node(&const_scalar, 0.75));
    let const_freq = format!("c_freq_{idx}");
    nodes.push(constant_node(&const_freq, 2.0 + idx as f32 * 0.01));

    // Arithmetic chain
    let sum = format!("sum_{idx}");
    nodes.push(add_node(&sum));
    edges.push(link(&in_a, &sum, "operand_1"));
    edges.push(link(&in_b, &sum, "operand_2"));

    let mult = format!("mult_{idx}");
    nodes.push(NodeSpec {
        id: mult.clone(),
        kind: NodeType::Multiply,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&sum, &mult, "operand_1"));
    edges.push(link(&const_gain, &mult, "operand_2"));

    let div = format!("div_{idx}");
    nodes.push(NodeSpec {
        id: div.clone(),
        kind: NodeType::Divide,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&mult, &div, "lhs"));
    edges.push(link(&const_div, &div, "rhs"));

    // Trig + clamp/remap
    let sin = format!("sin_{idx}");
    nodes.push(NodeSpec {
        id: sin.clone(),
        kind: NodeType::Sin,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&div, &sin, "in"));

    let clamp = format!("clamp_{idx}");
    nodes.push(NodeSpec {
        id: clamp.clone(),
        kind: NodeType::Clamp,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&sin, &clamp, "in"));
    edges.push(link(&const_min, &clamp, "min"));
    edges.push(link(&const_max, &clamp, "max"));

    let remap = format!("remap_{idx}");
    nodes.push(NodeSpec {
        id: remap.clone(),
        kind: NodeType::Remap,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&clamp, &remap, "in"));
    edges.push(link(&in_min, &remap, "in_min"));
    edges.push(link(&in_max, &remap, "in_max"));
    edges.push(link(&out_min, &remap, "out_min"));
    edges.push(link(&out_max, &remap, "out_max"));

    // Vector ops
    let vconst = format!("vconst_{idx}");
    nodes.push(vector_constant_node(&vconst, [1.0, 2.0, 3.0]));

    let join = format!("join_{idx}");
    nodes.push(NodeSpec {
        id: join.clone(),
        kind: NodeType::Join,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&sin, &join, "operand_1"));
    edges.push(link(&clamp, &join, "operand_2"));
    edges.push(link(&remap, &join, "operand_3"));

    let vadd = format!("vadd_{idx}");
    nodes.push(NodeSpec {
        id: vadd.clone(),
        kind: NodeType::VectorAdd,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&vconst, &vadd, "a"));
    edges.push(link(&join, &vadd, "b"));

    let vscale = format!("vscale_{idx}");
    nodes.push(NodeSpec {
        id: vscale.clone(),
        kind: NodeType::VectorScale,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&vadd, &vscale, "v"));
    edges.push(link(&const_scalar, &vscale, "scalar"));

    let vnorm = format!("vnorm_{idx}");
    nodes.push(NodeSpec {
        id: vnorm.clone(),
        kind: NodeType::VectorNormalize,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&vscale, &vnorm, "in"));

    let vdot = format!("vdot_{idx}");
    nodes.push(NodeSpec {
        id: vdot.clone(),
        kind: NodeType::VectorDot,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&vnorm, &vdot, "a"));
    edges.push(link(&vconst, &vdot, "b"));

    // Logic/comparisons
    let gt = format!("gt_{idx}");
    nodes.push(NodeSpec {
        id: gt.clone(),
        kind: NodeType::GreaterThan,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&clamp, &gt, "lhs"));
    edges.push(link(&remap, &gt, "rhs"));

    let lt = format!("lt_{idx}");
    nodes.push(NodeSpec {
        id: lt.clone(),
        kind: NodeType::LessThan,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&sin, &lt, "lhs"));
    edges.push(link(&clamp, &lt, "rhs"));

    let and = format!("and_{idx}");
    nodes.push(NodeSpec {
        id: and.clone(),
        kind: NodeType::And,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&gt, &and, "lhs"));
    edges.push(link(&lt, &and, "rhs"));

    let not = format!("not_{idx}");
    nodes.push(NodeSpec {
        id: not.clone(),
        kind: NodeType::Not,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&lt, &not, "in"));

    // Time + oscillator + transitions
    let time = format!("time_{idx}");
    nodes.push(NodeSpec {
        id: time.clone(),
        kind: NodeType::Time,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });

    let osc = format!("osc_{idx}");
    nodes.push(oscillator_node(&osc, 1.0));
    edges.push(link(&const_freq, &osc, "frequency"));

    let slew = format!("slew_{idx}");
    nodes.push(NodeSpec {
        id: slew.clone(),
        kind: NodeType::Slew,
        params: NodeParams {
            max_rate: Some(5.0),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&osc, &slew, "in"));

    let damp = format!("damp_{idx}");
    nodes.push(NodeSpec {
        id: damp.clone(),
        kind: NodeType::Damp,
        params: NodeParams {
            half_life: Some(0.2),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&remap, &damp, "in"));

    let spring = format!("spring_{idx}");
    nodes.push(NodeSpec {
        id: spring.clone(),
        kind: NodeType::Spring,
        params: NodeParams {
            stiffness: Some(80.0),
            damping: Some(12.0),
            mass: Some(1.0),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&clamp, &spring, "in"));

    // If/Case style routing
    let sel = format!("sel_{idx}");
    nodes.push(NodeSpec {
        id: sel.clone(),
        kind: NodeType::If,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&and, &sel, "cond"));
    edges.push(link(&spring, &sel, "then"));
    edges.push(link(&damp, &sel, "else"));

    // Output nodes
    let out_scalar = format!("out_scalar_{idx}");
    nodes.push(output_node(&out_scalar, format!("bench/{idx}/out_scalar")));
    edges.push(link(&sel, &out_scalar, "in"));

    let out_vec = format!("out_vec_{idx}");
    nodes.push(output_node(&out_vec, format!("bench/{idx}/out_vec")));
    edges.push(link(&vnorm, &out_vec, "in"));

    let out_bool = format!("out_bool_{idx}");
    nodes.push(output_node(&out_bool, format!("bench/{idx}/out_bool")));
    edges.push(link(&not, &out_bool, "in"));

    // Expose a single scalar for chaining (use remap result).
    let exit_id = remap.clone();
    let entry_target = sum.clone();

    // For chaining, feed prior block into a variadic operand slot not otherwise used.
    (nodes, edges, entry_target, "operand_3".into(), exit_id)
}

fn kitchen_chain(blocks: usize) -> GraphSpec {
    let blocks = blocks.max(1);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let mut prev_out: Option<String> = None;
    for i in 0..blocks {
        let (mut bn, mut be, entry_node, entry_port, exit_id) = kitchen_block(i);
        if let Some(prev) = prev_out {
            be.push(link(&prev, &entry_node, &entry_port));
        }
        nodes.append(&mut bn);
        edges.append(&mut be);
        prev_out = Some(exit_id);
    }

    GraphSpec {
        nodes,
        edges,
        ..Default::default()
    }
    .with_cache()
}

fn load_fixture_spec(name: &str) -> GraphSpec {
    let raw: serde_json::Value =
        node_graphs::spec(name).unwrap_or_else(|_| panic!("load graph fixture {name}"));
    let mut spec_value = raw
        .get("spec")
        .cloned()
        .unwrap_or_else(|| panic!("graph fixture {name} missing 'spec' field"));
    normalize_graph_spec_value(&mut spec_value).expect("normalize graph spec");
    serde_json::from_value(spec_value).expect("fixture spec deserializes into GraphSpec")
}

// Tunables (override via env, comma-separated aligned lists):
//   GRAPH_KITCHEN_SIZES=1,20,200
//   GRAPH_COLD_SAMPLES=100,100,10
//   GRAPH_AMORT_STEPS=500,100,100
//   GRAPH_AMORT_SAMPLES=100,50,10   (Criterion needs >=10)
const DEFAULT_KITCHEN_SIZES: &[usize] = &[1, 50, 500];
const DEFAULT_COLD_SAMPLES: &[usize] = &[100, 10, 10];
const DEFAULT_AMORT_STEPS: &[u32] = &[500, 100, 100];
const DEFAULT_AMORT_SAMPLES: &[usize] = &[100, 10, 10];

fn warm_runtime(rt: &mut GraphRuntime, spec: &GraphSpec) {
    rt.advance_epoch();
    // First eval builds the plan cache; exclude it from timed loops.
    evaluate_all(rt, spec).expect("graph evaluation");
}

fn bench_amortized_per_step(b: &mut criterion::Bencher, spec: &GraphSpec, steps: u32) {
    b.iter_custom(|iters| {
        let mut total = Duration::ZERO;
        // Reuse a single runtime per iteration to avoid counting plan rebuilds.
        for _ in 0..iters {
            let mut rt = GraphRuntime {
                dt: 1.0 / 60.0,
                t: 0.0,
                ..Default::default()
            };
            warm_runtime(&mut rt, spec);

            let start = std::time::Instant::now();
            for step in 0..steps {
                rt.t = step as f32 * rt.dt;
                rt.advance_epoch();
                evaluate_all(&mut rt, spec).expect("graph evaluation");
            }
            total += start.elapsed() / steps;
        }
        total
    });
}

fn bench_graphs(c: &mut Criterion) {
    let kitchen_sizes = parse_list("GRAPH_KITCHEN_SIZES", DEFAULT_KITCHEN_SIZES);
    let cold_samples = parse_list("GRAPH_COLD_SAMPLES", DEFAULT_COLD_SAMPLES);
    let amort_steps = parse_list("GRAPH_AMORT_STEPS", DEFAULT_AMORT_STEPS);
    let amort_samples = parse_list("GRAPH_AMORT_SAMPLES", DEFAULT_AMORT_SAMPLES);
    let len = kitchen_sizes
        .len()
        .min(cold_samples.len())
        .min(amort_steps.len())
        .min(amort_samples.len());

    let mut group = c.benchmark_group("graph_eval");

    // Tiny smoke
    {
        group.sample_size(50);
        let spec = chain_graph(8);
        group.bench_with_input(
            BenchmarkId::new("cold", "tiny-smoke-8"),
            &spec,
            |b, spec| {
                b.iter(|| {
                    let mut rt = GraphRuntime {
                        dt: 1.0 / 60.0,
                        t: 0.0,
                        ..Default::default()
                    };
                    rt.advance_epoch();
                    evaluate_all(&mut rt, black_box(spec)).expect("graph evaluation");
                });
            },
        );
        group.sample_size(10);
        group.bench_with_input(
            BenchmarkId::new("amortized_per_step", "tiny-smoke-8"),
            &spec,
            |b, spec| bench_amortized_per_step(b, black_box(spec), 100),
        );
    }

    // Fixture
    {
        let spec = load_fixture_spec("simple-gain-offset");
        group.sample_size(50);
        group.bench_with_input(
            BenchmarkId::new("cold", "fixture/simple-gain-offset"),
            &spec,
            |b, spec| {
                b.iter(|| {
                    let mut rt = GraphRuntime {
                        dt: 1.0 / 60.0,
                        t: 0.0,
                        ..Default::default()
                    };
                    rt.advance_epoch();
                    evaluate_all(&mut rt, black_box(spec)).expect("graph evaluation");
                });
            },
        );
        group.sample_size(10);
        group.bench_with_input(
            BenchmarkId::new("amortized_per_step", "fixture/simple-gain-offset"),
            &spec,
            |b, spec| bench_amortized_per_step(b, black_box(spec), 100),
        );
    }

    // Kitchen variants
    for i in 0..len {
        let size = kitchen_sizes[i];
        let cold_sample = cold_samples[i].max(1);
        let steps = amort_steps[i].max(1);
        let amort_sample = amort_samples[i].max(10); // Criterion min
        let name = format!("kitchen-{}-blocks", size);
        let spec = kitchen_chain(size);

        group.sample_size(cold_sample);
        group.bench_with_input(BenchmarkId::new("cold", &name), &spec, |b, spec| {
            b.iter(|| {
                let mut rt = GraphRuntime {
                    dt: 1.0 / 60.0,
                    t: 0.0,
                    ..Default::default()
                };
                rt.advance_epoch();
                evaluate_all(&mut rt, black_box(spec)).expect("graph evaluation");
            });
        });

        group.sample_size(amort_sample);
        group.bench_with_input(
            BenchmarkId::new("amortized_per_step", &name),
            &spec,
            |b, spec| bench_amortized_per_step(b, black_box(spec), steps),
        );
    }

    group.finish();
}

fn parse_list<T: FromStr + Copy>(env: &str, default: &[T]) -> Vec<T> {
    if let Ok(val) = std::env::var(env) {
        let parsed: Vec<T> = val
            .split(',')
            .filter_map(|s| s.trim().parse::<T>().ok())
            .collect();
        if !parsed.is_empty() {
            return parsed;
        }
    }
    default.to_vec()
}

criterion_group!(benches, bench_graphs);
criterion_main!(benches);
