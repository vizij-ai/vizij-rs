use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use hashbrown::HashMap;
use serde_json::json;
use std::time::Duration;
use vizij_animation_core::{AnimationData, Keypoint, Track, Transitions};
use vizij_api_core::{Shape, TypedPath, Value};
use vizij_graph_core::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, InputDefault, NodeParams, NodeSpec,
    NodeType,
};
use vizij_orchestrator::{
    fixtures::{self, DemoFixture},
    AnimationControllerConfig, GraphControllerConfig, Orchestrator, Schedule, Subscriptions,
};

fn schedule_from_fixture(fixture: &DemoFixture, default: Schedule) -> Schedule {
    let schedule = fixture.schedule().unwrap_or(match default {
        Schedule::SinglePass => "SinglePass",
        Schedule::TwoPass => "TwoPass",
        Schedule::RateDecoupled => "RateDecoupled",
    });
    match schedule.to_ascii_lowercase().as_str() {
        "singlepass" | "single-pass" => Schedule::SinglePass,
        "twopass" | "two-pass" => Schedule::TwoPass,
        "ratedecoupled" | "rate-decoupled" => Schedule::RateDecoupled,
        other => panic!("unknown schedule '{other}' in fixture"),
    }
}

fn register_fixture_graphs(mut orch: Orchestrator, fixture: &DemoFixture) -> Orchestrator {
    for graph in fixture.graphs() {
        orch = orch.with_graph(graph.controller_config());
    }
    for merged in fixture.merged_graphs() {
        orch = orch.with_graph(merged.controller_config());
    }
    orch
}

fn register_fixture_animations(mut orch: Orchestrator, fixture: &DemoFixture) -> Orchestrator {
    for (idx, anim) in fixture.animations().iter().enumerate() {
        let id = anim
            .id
            .clone()
            .or_else(|| anim.key.clone())
            .unwrap_or_else(|| format!("animation-{idx}"));
        let cfg = AnimationControllerConfig {
            id,
            setup: anim.setup.clone(),
        };
        orch = orch.with_animation(cfg);
    }
    orch
}

fn apply_fixture_inputs(orch: &mut Orchestrator, fixture: &DemoFixture) {
    for input in fixture.initial_inputs() {
        orch.set_input(&input.path, input.value.clone(), input.shape.clone())
            .expect("set input from fixture");
    }
}

// --- Generated graph/animation helpers for apples-to-apples orchestrator tests ---
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

/// Lightweight kitchen-style block (simpler than graph bench) to keep orchestrator spec manageable.
/// Returns (nodes, edges, entry_node, entry_port, exit_node) so callers can chain without duplicating inputs.
fn orch_block(
    idx: usize,
    base_path: &str,
) -> (Vec<NodeSpec>, Vec<EdgeSpec>, String, String, String) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let in_a = format!("in_{idx}");
    let input_path = format!("{base_path}/{in_a}");
    nodes.push(input_node(&in_a, input_path, idx as f32));

    let c1 = format!("c1_{idx}");
    let c2 = format!("c2_{idx}");
    nodes.push(constant_node(&c1, 1.0 + idx as f32 * 0.01));
    nodes.push(constant_node(&c2, 0.5));

    let add = format!("add_{idx}");
    nodes.push(NodeSpec {
        id: add.clone(),
        kind: NodeType::Add,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&in_a, &add, "operand_1"));
    edges.push(link(&c1, &add, "operand_2"));

    let mult = format!("mult_{idx}");
    nodes.push(NodeSpec {
        id: mult.clone(),
        kind: NodeType::Multiply,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&add, &mult, "operand_1"));
    edges.push(link(&c2, &mult, "operand_2"));

    let clamp = format!("clamp_{idx}");
    let cmin = format!("cmin_{idx}");
    let cmax = format!("cmax_{idx}");
    nodes.push(constant_node(&cmin, -1.0));
    nodes.push(constant_node(&cmax, 1.0));
    nodes.push(NodeSpec {
        id: clamp.clone(),
        kind: NodeType::Clamp,
        params: NodeParams::default(),
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    });
    edges.push(link(&mult, &clamp, "in"));
    edges.push(link(&cmin, &clamp, "min"));
    edges.push(link(&cmax, &clamp, "max"));

    let out = format!("out_{idx}");
    nodes.push(output_node(&out, format!("{base_path}/out_{idx}")));
    edges.push(link(&clamp, &out, "in"));

    // Chain into an unused operand slot to avoid duplicate edges when linking blocks.
    (nodes, edges, add, "operand_3".into(), clamp)
}

fn generated_graph(blocks: usize, base_path: &str) -> GraphSpec {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut prev: Option<String> = None;
    for i in 0..blocks {
        let (mut n, mut e, entry, port, exit) = orch_block(i, base_path);
        if let Some(p) = prev {
            e.push(link(&p, &entry, &port));
        }
        nodes.append(&mut n);
        edges.append(&mut e);
        prev = Some(exit);
    }
    GraphSpec { nodes, edges }
}

fn synthetic_animation(track_count: usize, keyframes: usize, base_path: &str) -> AnimationData {
    let keyframes = keyframes.max(2);
    let mut tracks = Vec::new();
    for i in 0..track_count {
        let mut points = Vec::new();
        let last = (keyframes - 1) as f32;
        for k in 0..keyframes {
            let t = k as f32 / last;
            let val = match i % 3 {
                0 => Value::Float(k as f32),
                1 => Value::Vec3([t, t * 0.5, t * 0.25]),
                _ => Value::Quat([0.0, 0.0, t, 1.0]),
            };
            points.push(Keypoint {
                id: format!("k{k}"),
                stamp: t,
                value: val,
                transitions: Some(Transitions {
                    r#in: None,
                    r#out: None,
                }),
            });
        }
        tracks.push(Track {
            id: format!("track_{i}"),
            name: format!("Track {i}"),
            animatable_id: format!("{base_path}/in_{i}"),
            points,
            settings: None,
        });
    }
    AnimationData {
        id: None,
        name: format!("{track_count}x{keyframes} synthetic"),
        tracks,
        groups: serde_json::Value::Null,
        duration_ms: 2_000,
    }
}

fn animation_to_setup(anim: &AnimationData) -> serde_json::Value {
    fn raw_value_json(v: &Value) -> serde_json::Value {
        match v {
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Float(f) => {
                serde_json::Value::Number(serde_json::Number::from_f64(*f as f64).unwrap())
            }
            Value::Vec2(v) => json!({ "x": v[0], "y": v[1] }),
            Value::Vec3(v) => json!({ "x": v[0], "y": v[1], "z": v[2] }),
            Value::Vec4(v) => json!({ "x": v[0], "y": v[1], "z": v[2], "w": v[3] }),
            Value::Quat(q) => json!({ "x": q[0], "y": q[1], "z": q[2], "w": q[3] }),
            Value::ColorRgba(c) => json!({ "r": c[0], "g": c[1], "b": c[2] }),
            Value::Transform {
                translation,
                rotation,
                scale,
            } => json!({
                "translation": { "x": translation[0], "y": translation[1], "z": translation[2] },
                "rotation": { "x": rotation[0], "y": rotation[1], "z": rotation[2], "w": rotation[3] },
                "scale": { "x": scale[0], "y": scale[1], "z": scale[2] }
            }),
            _ => serde_json::Value::Number(serde_json::Number::from(0)),
        }
    }

    // StoredAnimation schema equivalent.
    let tracks: Vec<serde_json::Value> = anim
        .tracks
        .iter()
        .map(|t| {
            let points: Vec<serde_json::Value> = t
                .points
                .iter()
                .map(|p| {
                    json!({
                        "id": p.id,
                        "stamp": p.stamp,
                        "value": raw_value_json(&p.value),
                        "transitions": {
                            "in": p.transitions.as_ref().and_then(|t| t.r#in.as_ref()).map(|v| json!({"x": v.x, "y": v.y})),
                            "out": p.transitions.as_ref().and_then(|t| t.r#out.as_ref()).map(|v| json!({"x": v.x, "y": v.y})),
                        }
                    })
                })
                .collect();
            json!({
                "id": t.id,
                "name": t.name,
                "animatableId": t.animatable_id,
                "points": points,
            })
        })
        .collect();

    json!({
        "animation": {
            "id": anim.name.clone(),
            "name": anim.name,
            "duration": anim.duration_ms,
            "tracks": tracks,
            "groups": anim.groups,
        },
        "player": { "name": "bench-player", "loop_mode": "loop", "speed": 1.0 },
        "instance": { "weight": 1.0, "time_scale": 1.0, "start_offset": 0.0, "enabled": true }
    })
}

fn typed(path: &str) -> TypedPath {
    path.parse().expect("bench path parses to TypedPath")
}

fn graph_cfg_with_subs(id: &str, blocks: usize, base_path: &str) -> GraphControllerConfig {
    let spec = generated_graph(blocks, base_path);
    let inputs: Vec<TypedPath> = (0..blocks)
        .map(|i| format!("{base_path}/in_{i}"))
        .map(|p| typed(&p))
        .collect();
    let outputs: Vec<TypedPath> = (0..blocks)
        .map(|i| format!("{base_path}/out_{i}"))
        .map(|p| typed(&p))
        .collect();
    GraphControllerConfig {
        id: id.to_string(),
        spec,
        subs: Subscriptions {
            inputs,
            outputs,
            mirror_writes: false,
        },
    }
}

fn generated_case_cfg(
    id: &str,
    blocks: usize,
    track_count: usize,
    keys: usize,
    base_path: &str,
) -> (GraphControllerConfig, Vec<serde_json::Value>) {
    let graph_cfg = graph_cfg_with_subs(id, blocks, base_path);
    let anim = synthetic_animation(track_count, keys, base_path);
    (graph_cfg, vec![animation_to_setup(&anim)])
}

enum Case {
    Empty,
    Fixture(Box<DemoFixture>),
    Generated {
        graph_cfg: GraphControllerConfig,
        animations: Vec<serde_json::Value>,
    },
}

fn bench_orchestrator(c: &mut Criterion) {
    let (blend_graph_cfg, blend_anims) =
        generated_case_cfg("bench-graph-blend", 20, 64, 16, "bench/blend");

    // Merged case: two 20-block graphs with matching 64x16 animations; animations write to the
    // graph input paths (bench/merged/g{1,2}/in_*).
    let g1_cfg = graph_cfg_with_subs("bench-graph-g1", 20, "bench/merged/g1");
    let g2_cfg = graph_cfg_with_subs("bench-graph-g2", 20, "bench/merged/g2");
    let merged_graph_cfg =
        GraphControllerConfig::merged("bench-graph-merged", vec![g1_cfg.clone(), g2_cfg.clone()])
            .expect("merge generated graphs");
    let merged_anims = vec![
        animation_to_setup(&synthetic_animation(64, 16, "bench/merged/g1")),
        animation_to_setup(&synthetic_animation(64, 16, "bench/merged/g2")),
    ];

    let cases: Vec<(String, Case)> = vec![
        ("empty".into(), Case::Empty),
        (
            "light-scalar-ramp".into(),
            Case::Fixture(Box::new(fixtures::demo_single_pass())),
        ),
        (
            "20bx64tx16k-blend".into(),
            Case::Generated {
                graph_cfg: blend_graph_cfg.clone(),
                animations: blend_anims.clone(),
            },
        ),
        (
            "merged-2x20b-2x64tx16k".into(),
            Case::Generated {
                graph_cfg: merged_graph_cfg,
                animations: merged_anims,
            },
        ),
    ];

    let mut group = c.benchmark_group("orchestrator_tick");
    group.sample_size(50);
    for (name, case) in cases {
        // Cold: construct orchestrator + one step
        group.bench_with_input(BenchmarkId::new("cold", &name), &case, |b, case| {
            b.iter(|| {
                let mut orch = match case {
                    Case::Empty => Orchestrator::new(Schedule::SinglePass),
                    Case::Fixture(fixture) => {
                        let fixture = fixture.as_ref();
                        let schedule = schedule_from_fixture(fixture, Schedule::SinglePass);
                        let mut orch = Orchestrator::new(schedule);
                        orch = register_fixture_graphs(orch, fixture);
                        orch = register_fixture_animations(orch, fixture);
                        apply_fixture_inputs(&mut orch, fixture);
                        orch
                    }
                    Case::Generated {
                        graph_cfg,
                        animations,
                    } => {
                        let mut orch = Orchestrator::new(Schedule::SinglePass);
                        orch = orch.with_graph(graph_cfg.clone());
                        for (idx, anim_setup) in animations.iter().enumerate() {
                            orch = orch.with_animation(AnimationControllerConfig {
                                id: format!("bench-anim-{idx}"),
                                setup: anim_setup.clone(),
                            });
                        }
                        orch
                    }
                };
                let _ = orch.step(black_box(1.0 / 60.0)).expect("orchestrator step");
            });
        });

        group.sample_size(10);
        // Amortized per-step: construct once per iteration, run 100 ticks; return per-step time
        group.bench_with_input(
            BenchmarkId::new("amortized_per_step", &name),
            &case,
            |b, case| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        let mut orch = match case {
                            Case::Empty => Orchestrator::new(Schedule::SinglePass),
                            Case::Fixture(fixture) => {
                                let fixture = fixture.as_ref();
                                let schedule = schedule_from_fixture(fixture, Schedule::SinglePass);
                                let mut orch = Orchestrator::new(schedule);
                                orch = register_fixture_graphs(orch, fixture);
                                orch = register_fixture_animations(orch, fixture);
                                apply_fixture_inputs(&mut orch, fixture);
                                orch
                            }
                            Case::Generated {
                                graph_cfg,
                                animations,
                            } => {
                                let mut orch = Orchestrator::new(Schedule::SinglePass);
                                orch = orch.with_graph(graph_cfg.clone());
                                for (idx, anim_setup) in animations.iter().enumerate() {
                                    orch = orch.with_animation(AnimationControllerConfig {
                                        id: format!("bench-anim-{idx}"),
                                        setup: anim_setup.clone(),
                                    });
                                }
                                orch
                            }
                        };

                        let start = std::time::Instant::now();
                        for _ in 0..100 {
                            let _ = orch.step(black_box(1.0 / 60.0)).expect("orchestrator step");
                        }
                        total += start.elapsed() / 100;
                    }
                    total
                });
            },
        );
        group.sample_size(50);
    }
    group.finish();
}

criterion_group!(benches, bench_orchestrator);
criterion_main!(benches);
