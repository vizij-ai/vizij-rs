use serde_json::{json, Value as JsonValue};
use vizij_api_core::{
    json::{normalize_graph_spec_json_string, parse_value},
    TypedPath, Value, WriteBatch, WriteOp,
};
use vizij_graph_core::types::GraphSpec;
use vizij_orchestrator::{
    AnimationControllerConfig, GraphControllerConfig, Orchestrator, Schedule, Subscriptions,
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
