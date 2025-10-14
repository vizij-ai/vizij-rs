use serde_json::{json, Value as JsonValue};
use vizij_api_core::{
    json::{normalize_graph_spec_json_string, parse_value},
    TypedPath, Value, WriteBatch, WriteOp,
};
use vizij_graph_core::eval::{evaluate_all, GraphRuntime};
use vizij_graph_core::types::GraphSpec;
use vizij_orchestrator::{
    AnimationControllerConfig, GraphControllerConfig, GraphMergeError, GraphMergeOptions,
    Orchestrator, OutputConflictStrategy, Schedule, Subscriptions,
};
use vizij_test_fixtures::{animations, node_graphs};

#[test]
fn single_pass_applies_graph_writes_and_merges() {
    // Setup orchestrator with single-pass schedule
    let mut orch = Orchestrator::new(Schedule::SinglePass);

    // Register a graph controller with default subscriptions
    let cfg = GraphControllerConfig {
        id: "g".into(),
        spec: GraphSpec::default(),
        subs: Subscriptions::default(),
    };
    orch = orch.with_graph(cfg);

    // Prepare a write produced by the graph runtime and attach it
    let tp = TypedPath::parse("robot/x").unwrap();
    let mut batch = WriteBatch::new();
    batch.push(WriteOp::new(tp.clone(), Value::Float(0.5)));

    // Inject the batch into the graph runtime writes so evaluate() will yield it
    let gc = orch.graphs.get_mut("g").expect("graph exists");
    gc.rt.writes = batch.clone();

    // Step orchestrator
    let frame = orch.step(0.016).expect("step ok");

    // merged_writes should contain the write
    let found = frame
        .merged_writes
        .iter()
        .any(|op| op.path.to_string() == tp.to_string() && op.value == Value::Float(0.5));
    assert!(found, "merged_writes must contain the graph write");

    // Blackboard should have the applied value
    let be = orch
        .blackboard
        .get(&tp.to_string())
        .expect("blackboard entry present");
    assert_eq!(be.value, Value::Float(0.5));
}

#[test]
fn two_pass_applies_graph_then_anim_then_graph_writes_and_merges() {
    // Two-pass schedule: graphs -> anims -> graphs
    let mut orch = Orchestrator::new(Schedule::TwoPass);

    // Register a graph controller that will produce a write in pass1
    let cfg = GraphControllerConfig {
        id: "g1".into(),
        spec: GraphSpec::default(),
        subs: Subscriptions::default(),
    };
    orch = orch.with_graph(cfg);

    // Register another graph controller that will produce a write in pass2
    let cfg2 = GraphControllerConfig {
        id: "g2".into(),
        spec: GraphSpec::default(),
        subs: Subscriptions::default(),
    };
    orch = orch.with_graph(cfg2);

    // Prepare writes for both graphs (they'll be consumed when evaluate() is called)
    let tp1 = TypedPath::parse("robot/a").unwrap();
    let mut b1 = WriteBatch::new();
    b1.push(WriteOp::new(tp1.clone(), Value::Float(1.0)));
    orch.graphs.get_mut("g1").unwrap().rt.writes = b1;

    let tp2 = TypedPath::parse("robot/b").unwrap();
    let mut b2 = WriteBatch::new();
    b2.push(WriteOp::new(tp2.clone(), Value::Float(2.0)));
    orch.graphs.get_mut("g2").unwrap().rt.writes = b2;

    // Step orchestrator
    let frame = orch.step(0.016).expect("step ok");

    // merged_writes should contain writes from both graphs in deterministic order
    let mut found_a = false;
    let mut found_b = false;
    for op in frame.merged_writes.iter() {
        if op.path.to_string() == tp1.to_string() && op.value == Value::Float(1.0) {
            found_a = true;
        }
        if op.path.to_string() == tp2.to_string() && op.value == Value::Float(2.0) {
            found_b = true;
        }
    }
    assert!(
        found_a && found_b,
        "merged_writes must include both graph writes"
    );

    // Blackboard should have both entries applied
    let be_a = orch.blackboard.get(&tp1.to_string()).expect("entry a");
    assert_eq!(be_a.value, Value::Float(1.0));
    let be_b = orch.blackboard.get(&tp2.to_string()).expect("entry b");
    assert_eq!(be_b.value, Value::Float(2.0));
}

#[test]
fn graph_uses_input_defaults_when_link_missing() {
    let graph_json = json!({
        "nodes": [
            {
                "id": "source",
                "type": "input",
                "params": {
                    "path": "demo/input/value"
                }
            },
            {
                "id": "doubler",
                "type": "multiply",
                "input_defaults": {
                    "rhs": {
                        "value": { "type": "float", "data": 2.0 }
                    }
                }
            },
            {
                "id": "out",
                "type": "output",
                "params": {
                    "path": "demo/output/value"
                }
            }
        ],
        "links": [
            {
                "from": { "node_id": "source" },
                "to": { "node_id": "doubler", "input": "lhs" }
            },
            {
                "from": { "node_id": "doubler" },
                "to": { "node_id": "out", "input": "in" }
            }
        ]
    });

    let spec: GraphSpec = serde_json::from_value(graph_json).expect("graph spec json");
    let subs = Subscriptions {
        inputs: vec![TypedPath::parse("demo/input/value").expect("typed path")],
        outputs: vec![TypedPath::parse("demo/output/value").expect("typed path")],
        mirror_writes: true,
    };

    let cfg = GraphControllerConfig {
        id: "defaults-graph".into(),
        spec,
        subs,
    };

    let mut orch = Orchestrator::new(Schedule::SinglePass).with_graph(cfg);

    orch.set_input(
        "demo/input/value",
        json!({ "type": "float", "data": 1.5 }),
        None,
    )
    .expect("set input");

    let frame = orch.step(1.0 / 60.0).expect("step ok");
    let output = read_scalar_write(&frame.merged_writes, "demo/output/value");
    assert!(
        (output - 3.0).abs() < 1e-6,
        "expected output 3.0 when using default rhs, got {output}"
    );
}

#[test]
fn merged_graph_rewires_shared_output() {
    let producer_spec: GraphSpec = serde_json::from_value(json!({
        "nodes": [
            {
                "id": "const_one",
                "type": "constant",
                "params": { "value": { "type": "float", "data": 1.0 } }
            },
            {
                "id": "publish",
                "type": "output",
                "params": { "path": "shared/value" }
            }
        ],
        "links": [
            { "from": { "node_id": "const_one" }, "to": { "node_id": "publish", "input": "in" } }
        ]
    }))
    .expect("producer graph");

    let consumer_spec: GraphSpec = serde_json::from_value(json!({
        "nodes": [
            {
                "id": "shared_input",
                "type": "input",
                "params": { "path": "shared/value" }
            },
            {
                "id": "scale",
                "type": "multiply",
                "input_defaults": {
                    "rhs": { "value": { "type": "float", "data": 2.0 } }
                }
            },
            {
                "id": "result",
                "type": "output",
                "params": { "path": "shared/doubled" }
            }
        ],
        "links": [
            { "from": { "node_id": "shared_input" }, "to": { "node_id": "scale", "input": "lhs" } },
            { "from": { "node_id": "scale" }, "to": { "node_id": "result", "input": "in" } }
        ]
    }))
    .expect("consumer graph");

    let producer_cfg = GraphControllerConfig {
        id: "producer".into(),
        spec: producer_spec,
        subs: Subscriptions {
            inputs: Vec::new(),
            outputs: vec![TypedPath::parse("shared/value").expect("typed path")],
            mirror_writes: true,
        },
    };
    let consumer_cfg = GraphControllerConfig {
        id: "consumer".into(),
        spec: consumer_spec,
        subs: Subscriptions {
            inputs: vec![TypedPath::parse("shared/value").expect("typed path")],
            outputs: vec![TypedPath::parse("shared/doubled").expect("typed path")],
            mirror_writes: true,
        },
    };

    let merged_cfg =
        GraphControllerConfig::merged("merged", vec![producer_cfg, consumer_cfg]).expect("merge");

    let mut orch = Orchestrator::new(Schedule::SinglePass).with_graph(merged_cfg);
    let frame = orch.step(1.0 / 60.0).expect("step ok");
    let doubled = read_scalar_write(&frame.merged_writes, "shared/doubled");
    assert!(
        (doubled - 2.0).abs() < 1e-6,
        "expected merged graph to output doubled value"
    );
}

#[test]
fn merged_graph_final_overlap_still_errors_with_blend_strategy() {
    let cfg_from_json = |id: &str, spec_json: serde_json::Value| -> GraphControllerConfig {
        let mut spec_json = spec_json;
        vizij_api_core::json::normalize_graph_spec_value(&mut spec_json);
        GraphControllerConfig {
            id: id.to_string(),
            spec: serde_json::from_value(spec_json).expect("graph spec json"),
            subs: Subscriptions::default(),
        }
    };

    let producer = json!({
        "nodes": [
            { "id": "value", "type": "constant", "params": { "value": { "float": 1.0 } } },
            { "id": "out_final1", "type": "output", "params": { "path": "shared/a" } },
            { "id": "out_final2", "type": "output", "params": { "path": "final/a" } }
        ],
        "links": [
            { "from": { "node_id": "value" }, "to": { "node_id": "out_final1", "input": "in" } },
            { "from": { "node_id": "value" }, "to": { "node_id": "out_final2", "input": "in" } }
        ]
    });
    let consumer = json!({
        "nodes": [
            { "id": "value", "type": "constant", "params": { "value": { "float": 2.0 } } },
            { "id": "in_a", "type": "input", "params": { "path": "shared/a" } },
            { "id": "add", "type": "add", "params": { "path": "final/a" } },
            { "id": "publish", "type": "output", "params": { "path": "final/a" } }
        ],
        "links": [
            { "from": { "node_id": "in_a" }, "to": { "node_id": "add", "input": "in" } },
            { "from": { "node_id": "value" }, "to": { "node_id": "add", "input": "in" } },
            { "from": { "node_id": "add" }, "to": { "node_id": "publish", "input": "in" } }
        ]
    });

    let cfg_producer = cfg_from_json("producer", producer);
    let cfg_consumer = cfg_from_json("consumer", consumer);

    let err = GraphControllerConfig::merged_with_options(
        "bundle",
        vec![cfg_producer, cfg_consumer],
        GraphMergeOptions {
            output_conflicts: OutputConflictStrategy::Error,
            intermediate_conflicts: OutputConflictStrategy::BlendEqualWeights,
        },
    )
    .expect_err("merge should fail due to final output conflict");

    match err {
        GraphMergeError::ConflictingOutputs { path, .. } => {
            assert_eq!(path, "final/a");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn merge_reports_conflicting_outputs() {
    let spec_a: GraphSpec = serde_json::from_value(json!({
        "nodes": [
            { "id": "const_a", "type": "constant", "params": { "value": { "type": "float", "data": 1.0 } } },
            { "id": "out_a", "type": "output", "params": { "path": "shared/value" } }
        ],
        "links": [
            { "from": { "node_id": "const_a" }, "to": { "node_id": "out_a", "input": "in" } }
        ]
    }))
    .expect("spec a");

    let spec_b: GraphSpec = serde_json::from_value(json!({
        "nodes": [
            { "id": "const_b", "type": "constant", "params": { "value": { "type": "float", "data": 2.0 } } },
            { "id": "out_b", "type": "output", "params": { "path": "shared/value" } }
        ],
        "links": [
            { "from": { "node_id": "const_b" }, "to": { "node_id": "out_b", "input": "in" } }
        ]
    }))
    .expect("spec b");

    let cfg_a = GraphControllerConfig {
        id: "a".into(),
        spec: spec_a,
        subs: Subscriptions::default(),
    };
    let cfg_b = GraphControllerConfig {
        id: "b".into(),
        spec: spec_b,
        subs: Subscriptions::default(),
    };

    let err = GraphControllerConfig::merged("merged", vec![cfg_a, cfg_b])
        .expect_err("merge should fail due to conflicting outputs");
    match err {
        GraphMergeError::ConflictingOutputs { path, .. } => {
            assert_eq!(path, "shared/value");
        }
        other => panic!("unexpected merge error: {other:?}"),
    }
}

#[test]
fn merged_graph_parallel_blend_pipeline() {
    fn cfg_from_json(id: &str, spec_json: serde_json::Value) -> GraphControllerConfig {
        let mut spec_json = spec_json;
        vizij_api_core::json::normalize_graph_spec_value(&mut spec_json);
        GraphControllerConfig {
            id: id.to_string(),
            spec: serde_json::from_value(spec_json).expect("graph spec json"),
            subs: Subscriptions::default(),
        }
    }

    fn sanitize(path: &str) -> String {
        path.chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
            .collect()
    }

    let graph_one = json!({
        "nodes": [
            { "id": "const_a", "type": "constant", "params": { "value": { "float": 10.0 } } },
            { "id": "const_b", "type": "constant", "params": { "value": { "float": 20.0 } } },
            { "id": "const_c", "type": "constant", "params": { "value": { "float": 30.0 } } },
            { "id": "out_a", "type": "output", "params": { "path": "shared/a" } },
            { "id": "out_b", "type": "output", "params": { "path": "shared/b" } },
            { "id": "out_c", "type": "output", "params": { "path": "shared/c" } }
        ],
        "links": [
            { "from": { "node_id": "const_a" }, "to": { "node_id": "out_a", "input": "in" } },
            { "from": { "node_id": "const_b" }, "to": { "node_id": "out_b", "input": "in" } },
            { "from": { "node_id": "const_c" }, "to": { "node_id": "out_c", "input": "in" } }
        ]
    });
    let graph_two = json!({
        "nodes": [
            { "id": "const_b", "type": "constant", "params": { "value": { "float": 200.0 } } },
            { "id": "const_c", "type": "constant", "params": { "value": { "float": 300.0 } } },
            { "id": "const_d", "type": "constant", "params": { "value": { "float": 400.0 } } },
            { "id": "out_b", "type": "output", "params": { "path": "shared/b" } },
            { "id": "out_c", "type": "output", "params": { "path": "shared/c" } },
            { "id": "out_d", "type": "output", "params": { "path": "shared/d" } }
        ],
        "links": [
            { "from": { "node_id": "const_b" }, "to": { "node_id": "out_b", "input": "in" } },
            { "from": { "node_id": "const_c" }, "to": { "node_id": "out_c", "input": "in" } },
            { "from": { "node_id": "const_d" }, "to": { "node_id": "out_d", "input": "in" } }
        ]
    });
    let graph_three = json!({
        "nodes": [
            { "id": "const_c", "type": "constant", "params": { "value": { "float": 3000.0 } } },
            { "id": "const_d", "type": "constant", "params": { "value": { "float": 4000.0 } } },
            { "id": "const_e", "type": "constant", "params": { "value": { "float": 5000.0 } } },
            { "id": "const_f", "type": "constant", "params": { "value": { "float": 6000.0 } } },
            { "id": "out_c", "type": "output", "params": { "path": "shared/c" } },
            { "id": "out_d", "type": "output", "params": { "path": "shared/d" } },
            { "id": "out_e", "type": "output", "params": { "path": "shared/e" } },
            { "id": "out_f", "type": "output", "params": { "path": "shared/f" } }
        ],
        "links": [
            { "from": { "node_id": "const_c" }, "to": { "node_id": "out_c", "input": "in" } },
            { "from": { "node_id": "const_d" }, "to": { "node_id": "out_d", "input": "in" } },
            { "from": { "node_id": "const_e" }, "to": { "node_id": "out_e", "input": "in" } },
            { "from": { "node_id": "const_f" }, "to": { "node_id": "out_f", "input": "in" } }
        ]
    });
    let graph_four = json!({
        "nodes": [
            { "id": "in_a", "type": "input", "params": { "path": "shared/a" } },
            { "id": "in_b", "type": "input", "params": { "path": "shared/b" } },
            { "id": "in_c", "type": "input", "params": { "path": "shared/c" } },
            { "id": "in_d", "type": "input", "params": { "path": "shared/d" } },
            { "id": "in_e", "type": "input", "params": { "path": "shared/e" } },
            { "id": "in_f", "type": "input", "params": { "path": "shared/f" } },
            { "id": "out_a_final", "type": "output", "params": { "path": "final/a" } },
            { "id": "out_b_final", "type": "output", "params": { "path": "final/b" } },
            { "id": "out_c_final", "type": "output", "params": { "path": "final/c" } },
            { "id": "out_d_final", "type": "output", "params": { "path": "final/d" } },
            { "id": "out_e_final", "type": "output", "params": { "path": "final/e" } },
            { "id": "out_f_final", "type": "output", "params": { "path": "final/f" } }
        ],
        "links": [
            { "from": { "node_id": "in_a" }, "to": { "node_id": "out_a_final", "input": "in" } },
            { "from": { "node_id": "in_b" }, "to": { "node_id": "out_b_final", "input": "in" } },
            { "from": { "node_id": "in_c" }, "to": { "node_id": "out_c_final", "input": "in" } },
            { "from": { "node_id": "in_d" }, "to": { "node_id": "out_d_final", "input": "in" } },
            { "from": { "node_id": "in_e" }, "to": { "node_id": "out_e_final", "input": "in" } },
            { "from": { "node_id": "in_f" }, "to": { "node_id": "out_f_final", "input": "in" } }
        ]
    });

    let mut cfg1 = cfg_from_json("first", graph_one);
    cfg1.subs.outputs = vec![
        TypedPath::parse("shared/a").unwrap(),
        TypedPath::parse("shared/b").unwrap(),
        TypedPath::parse("shared/c").unwrap(),
    ];
    let mut cfg2 = cfg_from_json("second", graph_two);
    cfg2.subs.outputs = vec![
        TypedPath::parse("shared/b").unwrap(),
        TypedPath::parse("shared/c").unwrap(),
        TypedPath::parse("shared/d").unwrap(),
    ];
    let mut cfg3 = cfg_from_json("third", graph_three);
    cfg3.subs.outputs = vec![
        TypedPath::parse("shared/c").unwrap(),
        TypedPath::parse("shared/d").unwrap(),
        TypedPath::parse("shared/e").unwrap(),
        TypedPath::parse("shared/f").unwrap(),
    ];
    let mut cfg4 = cfg_from_json("fourth", graph_four);
    cfg4.subs.inputs = vec![
        TypedPath::parse("shared/a").unwrap(),
        TypedPath::parse("shared/b").unwrap(),
        TypedPath::parse("shared/c").unwrap(),
        TypedPath::parse("shared/d").unwrap(),
        TypedPath::parse("shared/e").unwrap(),
        TypedPath::parse("shared/f").unwrap(),
    ];
    cfg4.subs.outputs = vec![
        TypedPath::parse("final/a").unwrap(),
        TypedPath::parse("final/b").unwrap(),
        TypedPath::parse("final/c").unwrap(),
        TypedPath::parse("final/d").unwrap(),
        TypedPath::parse("final/e").unwrap(),
        TypedPath::parse("final/f").unwrap(),
    ];

    let merged = GraphControllerConfig::merged_with_options(
        "bundle",
        vec![cfg1, cfg2, cfg3, cfg4],
        GraphMergeOptions {
            output_conflicts: OutputConflictStrategy::Error,
            intermediate_conflicts: OutputConflictStrategy::BlendEqualWeights,
        },
    )
    .expect("merged");

    let spec = &merged.spec;

    let blend_paths = ["shared/b", "shared/c", "shared/d"];
    for path in blend_paths {
        let token = format!("blend_{}", sanitize(path));
        let blend_node = spec
            .nodes
            .iter()
            .find(|node| {
                matches!(node.kind, vizij_graph_core::types::NodeType::DefaultBlend)
                    && node.id.contains(&token)
            })
            .unwrap_or_else(|| panic!("blend node for {} missing", path));
        let target_links: Vec<_> = spec
            .links
            .iter()
            .filter(|link| link.to.node_id == blend_node.id && link.to.input.starts_with("target_"))
            .collect();
        let expected_sources = match path {
            "shared/b" => 2,
            "shared/c" => 3,
            "shared/d" => 2,
            _ => unreachable!(),
        };
        assert_eq!(
            target_links.len(),
            expected_sources,
            "blend node for {} should have {} inputs",
            path,
            expected_sources
        );
        assert!(
            !spec.nodes.iter().any(|node| matches!(
                node.kind,
                vizij_graph_core::types::NodeType::Input
            ) && node.params.path.as_ref().map(|p| p.to_string())
                == Some(path.to_string())),
            "input node for {} should have been removed",
            path
        );
    }

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, spec).expect("evaluation succeeds");
    let writes = rt
        .writes
        .iter()
        .map(|op| (op.path.to_string(), op.value.clone()))
        .collect::<std::collections::HashMap<_, _>>();

    let final_expected = [
        ("final/a", 10.0),
        ("final/b", (20.0 + 200.0) / 2.0),
        ("final/c", (30.0 + 300.0 + 3000.0) / 3.0),
        ("final/d", (400.0 + 4000.0) / 2.0),
        ("final/e", 5000.0),
        ("final/f", 6000.0),
    ];
    for (path, value) in final_expected {
        let actual = writes
            .get(path)
            .unwrap_or_else(|| panic!("expected final write for {}", path));
        match actual {
            Value::Float(v) => assert!(
                (v - value).abs() < 1e-6,
                "expected {} -> {}, got {}",
                path,
                value,
                v
            ),
            other => panic!("expected float value for {}, got {:?}", path, other),
        }
    }

    let per_graph_expected = [
        ("first/shared/b", 20.0),
        ("first/shared/c", 30.0),
        ("second/shared/b", 200.0),
        ("second/shared/c", 300.0),
        ("second/shared/d", 400.0),
        ("third/shared/c", 3000.0),
        ("third/shared/d", 4000.0),
    ];
    for (path, value) in per_graph_expected {
        let actual = writes
            .get(path)
            .unwrap_or_else(|| panic!("expected namespaced write for {}", path));
        match actual {
            Value::Float(v) => assert!(
                (v - value).abs() < 1e-6,
                "expected {} -> {}, got {}",
                path,
                value,
                v
            ),
            other => panic!("expected float value for {}, got {:?}", path, other),
        }
    }

    let blend_expected = [
        ("blend/shared/b", (20.0 + 200.0) / 2.0),
        ("blend/shared/c", (30.0 + 300.0 + 3000.0) / 3.0),
        ("blend/shared/d", (400.0 + 4000.0) / 2.0),
    ];
    for (path, value) in blend_expected {
        let actual = writes
            .get(path)
            .unwrap_or_else(|| panic!("expected blend write for {}", path));
        match actual {
            Value::Float(v) => assert!(
                (v - value).abs() < 1e-6,
                "expected {} -> {}, got {}",
                path,
                value,
                v
            ),
            other => panic!("expected float value for {}, got {:?}", path, other),
        }
    }
}

fn graph_fixture(name: &str) -> GraphControllerConfig {
    let raw = node_graphs::spec_json(name).unwrap_or_else(|_| panic!("load graph fixture {name}"));
    let value: JsonValue = serde_json::from_str(&raw).expect("graph fixture json");
    let spec_json = value.get("spec").cloned().expect("spec field");
    let normalized = normalize_graph_spec_json_string(&spec_json.to_string())
        .unwrap_or_else(|e| panic!("normalize graph spec failed: {e}"));
    let spec: GraphSpec = serde_json::from_str(&normalized).expect("graph spec");
    let subs_value = value.get("subs").cloned().unwrap_or_else(|| {
        json!({
            "inputs": [],
            "outputs": []
        })
    });
    let parse_paths = |key: &str| -> Vec<TypedPath> {
        subs_value
            .get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|entry| {
                        let path = entry
                            .as_str()
                            .unwrap_or_else(|| panic!("{key} entry must be string"));
                        TypedPath::parse(path).unwrap_or_else(|_| panic!("invalid path {path}"))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let subs = Subscriptions {
        inputs: parse_paths("inputs"),
        outputs: parse_paths("outputs"),
        mirror_writes: true,
    };
    GraphControllerConfig {
        id: name.into(),
        spec,
        subs,
    }
}

fn animation_setup(name: &str, player: (&str, &str)) -> JsonValue {
    let animation: JsonValue =
        animations::load(name).unwrap_or_else(|_| panic!("load animation {name}"));
    json!({
        "animation": animation,
        "player": {
            "name": player.0,
            "loop_mode": player.1,
        }
    })
}

fn read_scalar_write(batch: &WriteBatch, path: &str) -> f32 {
    let op = batch
        .iter()
        .find(|w| w.path.to_string() == path)
        .unwrap_or_else(|| panic!("missing write for {path}"));
    match op.value {
        Value::Float(v) => v,
        _ => panic!("expected float write for {path}"),
    }
}

fn find_write<'a>(batch: &'a WriteBatch, path: &str) -> &'a Value {
    batch
        .iter()
        .find(|w| w.path.to_string() == path)
        .map(|op| &op.value)
        .unwrap_or_else(|| panic!("missing write for {path}"))
}

fn assert_write_matches(batch: &WriteBatch, path: &str, expected: &JsonValue) {
    let actual = find_write(batch, path);
    let expected_value = parse_value(expected.clone())
        .unwrap_or_else(|e| panic!("parse expected value for {path}: {e}"));
    assert_values_close(actual, &expected_value, path);
}

fn assert_values_close(actual: &Value, expected: &Value, path: &str) {
    const EPS: f32 = 1e-3;
    match (actual, expected) {
        (Value::Float(a), Value::Float(b)) => assert!(
            (a - b).abs() <= EPS,
            "float mismatch for {path}: {a} vs {b}"
        ),
        (Value::Vec2(a), Value::Vec2(b)) => a
            .iter()
            .zip(b.iter())
            .for_each(|(aa, bb)| assert!((aa - bb).abs() <= EPS, "vec2 mismatch for {path}")),
        (Value::Vec3(a), Value::Vec3(b)) => a
            .iter()
            .zip(b.iter())
            .for_each(|(aa, bb)| assert!((aa - bb).abs() <= EPS, "vec3 mismatch for {path}")),
        (Value::Vec4(a), Value::Vec4(b)) => a
            .iter()
            .zip(b.iter())
            .for_each(|(aa, bb)| assert!((aa - bb).abs() <= EPS, "vec4 mismatch for {path}")),
        (Value::Quat(a), Value::Quat(b)) => {
            let direct = a
                .iter()
                .zip(b.iter())
                .all(|(aa, bb)| (aa - bb).abs() <= EPS);
            let neg = a
                .iter()
                .zip(b.iter())
                .all(|(aa, bb)| (aa + bb).abs() <= EPS);
            assert!(
                direct || neg,
                "quat mismatch for {path}: actual={a:?} expected={b:?}"
            );
        }
        (Value::Vector(a), Value::Vector(b)) => {
            assert_eq!(a.len(), b.len(), "vector length mismatch for {path}");
            a.iter()
                .zip(b.iter())
                .for_each(|(aa, bb)| assert!((aa - bb).abs() <= EPS, "vector mismatch for {path}"));
        }
        (
            Value::Transform {
                translation: at,
                rotation: ar,
                scale: as_,
            },
            Value::Transform {
                translation: bt,
                rotation: br,
                scale: bs,
            },
        ) => {
            at.iter().zip(bt.iter()).for_each(|(aa, bb)| {
                assert!(
                    (aa - bb).abs() <= EPS,
                    "transform.translation mismatch for {path}"
                )
            });
            let direct = ar
                .iter()
                .zip(br.iter())
                .all(|(aa, bb)| (aa - bb).abs() <= EPS);
            let neg = ar
                .iter()
                .zip(br.iter())
                .all(|(aa, bb)| (aa + bb).abs() <= EPS);
            assert!(
                direct || neg,
                "transform.rotation mismatch for {path}: actual={ar:?} expected={br:?}"
            );
            as_.iter().zip(bs.iter()).for_each(|(aa, bb)| {
                assert!(
                    (aa - bb).abs() <= EPS,
                    "transform.scale mismatch for {path}"
                )
            });
        }
        _ => assert_eq!(actual, expected, "value mismatch for {path}"),
    }
}

#[test]
fn mirror_writes_false_limits_blackboard() {
    let tp_public = TypedPath::parse("graph/public.value").expect("public path");
    let tp_internal = TypedPath::parse("graph/internal.value").expect("internal path");
    let tp_public_str = tp_public.to_string();
    let tp_internal_str = tp_internal.to_string();

    let subs = Subscriptions {
        inputs: Vec::new(),
        outputs: vec![tp_public.clone()],
        mirror_writes: false,
    };

    let cfg = GraphControllerConfig {
        id: "test-graph".into(),
        spec: GraphSpec::default(),
        subs,
    };

    let mut orch = Orchestrator::new(Schedule::SinglePass).with_graph(cfg);

    let graph = orch.graphs.get_mut("test-graph").expect("graph registered");
    let mut batch = WriteBatch::new();
    batch.push(WriteOp::new(tp_public.clone(), Value::Float(1.0)));
    batch.push(WriteOp::new(tp_internal.clone(), Value::Float(2.0)));
    graph.rt.writes = batch;

    let frame = orch.step(1.0 / 60.0).expect("step ok");

    let mut iter = frame.merged_writes.iter();
    let first = iter.next().expect("public write present");
    assert_eq!(first.path, tp_public);
    assert!(iter.next().is_none(), "only public write should be merged");

    assert!(orch.blackboard.get(&tp_public_str).is_some());
    assert!(orch.blackboard.get(&tp_internal_str).is_none());
}

#[test]
fn mirror_writes_true_mirrors_full_batch() {
    let tp_public = TypedPath::parse("graph/public.value").expect("public path");
    let tp_internal = TypedPath::parse("graph/internal.value").expect("internal path");
    let tp_public_str = tp_public.to_string();
    let tp_internal_str = tp_internal.to_string();

    let subs = Subscriptions {
        inputs: Vec::new(),
        outputs: vec![tp_public.clone()],
        mirror_writes: true,
    };

    let cfg = GraphControllerConfig {
        id: "test-graph".into(),
        spec: GraphSpec::default(),
        subs,
    };

    let mut orch = Orchestrator::new(Schedule::SinglePass).with_graph(cfg);

    let graph = orch.graphs.get_mut("test-graph").expect("graph registered");
    let mut batch = WriteBatch::new();
    batch.push(WriteOp::new(tp_public.clone(), Value::Float(1.0)));
    batch.push(WriteOp::new(tp_internal.clone(), Value::Float(2.0)));
    graph.rt.writes = batch;

    let frame = orch.step(1.0 / 60.0).expect("step ok");

    let mut iter = frame.merged_writes.iter();
    let first = iter.next().expect("public write present");
    assert_eq!(first.path, tp_public);
    assert!(
        iter.next().is_none(),
        "merged writes should still reflect filtered view"
    );

    let public_entry = orch
        .blackboard
        .get(&tp_public_str)
        .expect("public mirrored");
    assert_eq!(public_entry.value, Value::Float(1.0));

    let internal_entry = orch
        .blackboard
        .get(&tp_internal_str)
        .expect("internal mirrored when enabled");
    assert_eq!(internal_entry.value, Value::Float(2.0));
}

#[test]
fn scalar_ramp_pipeline_shared_fixture_executes() {
    let fixture = vizij_orchestrator::fixtures::demo_single_pass();

    let mut orch = Orchestrator::new(Schedule::SinglePass);
    let graph_cfg = graph_fixture("simple-gain-offset");
    orch = orch.with_graph(graph_cfg);

    let anim_cfg = AnimationControllerConfig {
        id: "anim".into(),
        setup: fixture.animation.setup.clone(),
    };
    orch = orch.with_animation(anim_cfg);

    for input in fixture.initial_inputs.iter() {
        orch.set_input(&input.path, input.value.clone(), None)
            .expect("set input");
    }

    for step in fixture.steps.iter() {
        let frame = orch.step(step.delta as f32).expect("step ok");
        let out = read_scalar_write(&frame.merged_writes, "demo/output/value");
        assert!(out.is_finite(), "output should be finite");
    }
}

#[test]
fn chain_sign_slew_pipeline_uses_shared_fixtures() {
    let mut orch = Orchestrator::new(Schedule::SinglePass);
    orch = orch.with_graph(graph_fixture("sign-graph"));
    orch = orch.with_graph(graph_fixture("slew-graph"));

    let anim_cfg = AnimationControllerConfig {
        id: "chain".into(),
        setup: animation_setup("chain-ramp", ("chain-player", "once")),
    };
    orch = orch.with_animation(anim_cfg);

    let steps = [
        (0.0_f32, -1.0_f32, -1.0_f32),
        (1.0_f32, 0.0_f32, 0.0_f32),
        (1.0_f32, 1.0_f32, 1.0_f32),
        (1.0_f32, 1.0_f32, 1.0_f32),
    ];

    let mut prev_slew = steps[0].2;
    let max_rate = 1.0_f32;

    for (idx, (dt, expected_sign, expected_slew)) in steps.iter().enumerate() {
        let frame = orch.step(*dt).expect("step ok");
        let writes = &frame.merged_writes;
        let sign = read_scalar_write(writes, "chain/sign.value");
        let slew = read_scalar_write(writes, "chain/slewed.value");
        assert!(
            (sign - expected_sign).abs() < 1e-3,
            "sign {sign} vs {expected_sign}"
        );
        if idx > 0 {
            let allowed = max_rate * dt + 1e-6;
            let delta = (slew - prev_slew).abs();
            assert!(delta <= allowed, "slew delta {delta} exceeded {allowed}");
        }
        assert!(
            (slew - expected_slew).abs() < 1e-3,
            "slew {slew} vs {expected_slew}"
        );
        prev_slew = slew;
    }
}

#[test]
fn sine_driver_graph_controls_animation_seek() {
    let mut orch = Orchestrator::new(Schedule::TwoPass);
    orch = orch.with_graph(graph_fixture("sine-driver"));

    let anim_cfg = AnimationControllerConfig {
        id: "control".into(),
        setup: animation_setup("control-linear", ("controller-player", "loop")),
    };
    orch = orch.with_animation(anim_cfg);

    let driver_frequency = 0.5_f32;
    let animation_duration = 2.0_f32;
    let tau = std::f32::consts::TAU;

    let normalized = |time: f32| (f32::sin(tau * driver_frequency * time) + 1.0) * 0.5;
    let expected_seek = |time: f32| normalized(time) * animation_duration;

    for step in 0..=4 {
        let time = step as f32 * 0.5;
        orch.set_input("driver/time.seconds", JsonValue::from(time), None)
            .expect("set time");
        let frame = orch.step(0.5).expect("step ok");
        let writes = &frame.merged_writes;
        let seek = read_scalar_write(writes, "anim/player/0/cmd/seek");
        assert!(
            (seek - expected_seek(time)).abs() < 1e-3,
            "seek mismatch at {time}"
        );
    }
}

#[test]
fn blend_pose_pipeline_shared_fixture_executes() {
    let fixture = vizij_orchestrator::fixtures::blend_pose_pipeline();

    let mut orch = Orchestrator::new(Schedule::TwoPass);
    let graph_cfg = graph_fixture("weighted-profile-blend");
    orch = orch.with_graph(graph_cfg);

    let anim_cfg = AnimationControllerConfig {
        id: "pose".into(),
        setup: fixture.animation.setup.clone(),
    };
    orch = orch.with_animation(anim_cfg);

    for input in fixture.initial_inputs.iter() {
        orch.set_input(&input.path, input.value.clone(), None)
            .expect("set input");
    }

    for step in fixture.steps.iter() {
        let frame = orch.step(step.delta as f32).expect("step ok");
        for (path, expected) in step.expect.iter() {
            assert_write_matches(&frame.merged_writes, path, expected);
        }

        let rotation = find_write(&frame.merged_writes, "rig/root.rotation");
        if let Value::Quat(_) = rotation {
        } else {
            panic!("rig/root.rotation should be quaternion, got {rotation:?}");
        }

        let translation = find_write(&frame.merged_writes, "rig/root.translation");
        match translation {
            Value::Vec3(_) => {}
            Value::Vector(v) => assert_eq!(v.len(), 3, "translation vector length"),
            other => panic!("rig/root.translation should be vec3, got {other:?}"),
        }

        let transform = find_write(&frame.merged_writes, "rig/root.transform");
        if let Value::Transform { .. } = transform {
        } else {
            panic!("rig/root.transform should be transform value, got {transform:?}");
        }
    }
}
