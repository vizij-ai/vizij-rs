use vizij_animation_core::{
    config::Config,
    data::{AnimationData, Keypoint, Track},
    engine::{Engine, InstanceCfg},
    inputs::{Inputs, LoopMode, PlayerCommand},
    parse_stored_animation_json,
    value::Value,
};

fn approx(a: f32, b: f32, eps: f32) {
    assert!(
        (a - b).abs() <= eps,
        "approx failed: left={a} right={b} eps={eps}"
    );
}

fn approx3(a: [f32; 3], b: [f32; 3], eps: f32) {
    approx(a[0], b[0], eps);
    approx(a[1], b[1], eps);
    approx(a[2], b[2], eps);
}

fn norm4(q: [f32; 4]) -> f32 {
    (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt()
}

/// it should match scalar ramp values across fixed dt steps
#[test]
fn parity_scalar_ramp_values() {
    // Load ramp fixture
    let json_str = include_str!("../../test_fixtures/ramp.json");
    let anim: AnimationData = parse_stored_animation_json(json_str).expect("parse ramp.json");

    let mut eng = Engine::new(Config::default());
    let aid = eng.load_animation(anim);
    let pid = eng.create_player("p");
    let _iid = eng.add_instance(pid, aid, InstanceCfg::default());

    // Force Once mode so boundary at t=1.0 clamps rather than wraps.
    let mut init = Inputs::default();
    init.player_cmds.push(PlayerCommand::SetLoopMode {
        player: pid,
        mode: LoopMode::Once,
    });
    let _ = eng.update(0.0, init);

    // Initial sample at t=0.0
    let out0 = eng.update(0.0, Inputs::default());
    let v0 = out0
        .changes
        .iter()
        .find(|c| c.key == "node.t")
        .expect("node.t")
        .value
        .clone();
    if let Value::Float(s) = v0 {
        approx(s, 0.0, 1e-5);
    } else {
        panic!("expected Float");
    }

    // Step dt=0.1 for 10 ticks, expect values ~ i/10
    let mut t = 0.0f32;
    for i in 1..=10 {
        let out = eng.update(0.1, Inputs::default());
        t += 0.1;
        let v = out
            .changes
            .iter()
            .find(|c| c.key == "node.t")
            .expect("node.t")
            .value
            .clone();
        if let Value::Float(s) = v {
            approx(s, t, 1e-5);
            assert!(
                (s - (i as f32) / 10.0).abs() < 1e-5,
                "tick {i} expected ~{}, got {}",
                (i as f32) / 10.0,
                s
            );
        } else {
            panic!("expected Float");
        }
    }
}

/// it should match constant Vec3 values across ticks
#[test]
fn parity_const_vec3_values() {
    // Load const fixture
    let json_str = include_str!("../../test_fixtures/const.json");
    let anim: AnimationData = parse_stored_animation_json(json_str).expect("parse const.json");

    let mut eng = Engine::new(Config::default());
    let aid = eng.load_animation(anim);
    let pid = eng.create_player("p");
    let _iid = eng.add_instance(pid, aid, InstanceCfg::default());

    // Sample any tick; expect translation [1,2,3]
    let out = eng.update(0.016, Inputs::default());
    let v = out
        .changes
        .iter()
        .find(|c| c.key == "node/Transform.translation")
        .expect("node/Transform.translation")
        .value
        .clone();
    match v {
        Value::Vec3(v3) => approx3(v3, [1.0, 2.0, 3.0], 1e-6),
        Value::Transform { pos, .. } => approx3(pos, [1.0, 2.0, 3.0], 1e-6),
        _ => panic!("expected Vec3 or Transform"),
    }
}

/// it should respect window clamp and loop/once seek semantics
#[test]
fn parity_window_and_seek() {
    let json_str = include_str!("../../test_fixtures/loop_window.json");
    let anim: AnimationData =
        parse_stored_animation_json(json_str).expect("parse loop_window.json");

    let mut eng = Engine::new(Config::default());
    let aid = eng.load_animation(anim);
    let pid = eng.create_player("p");
    let _iid = eng.add_instance(pid, aid, InstanceCfg::default());

    // Once mode + window [0.2, 0.8], seek 1.0 clamps to 0.8
    let mut inputs = Inputs::default();
    inputs.player_cmds.push(PlayerCommand::SetLoopMode {
        player: pid,
        mode: LoopMode::Once,
    });
    inputs.player_cmds.push(PlayerCommand::SetWindow {
        player: pid,
        start_time: 0.2,
        end_time: Some(0.8),
    });
    inputs.player_cmds.push(PlayerCommand::Seek {
        player: pid,
        time: 1.0,
    });
    let out = eng.update(0.0, inputs);
    let v = out
        .changes
        .iter()
        .find(|c| c.key == "node.t")
        .expect("node.t")
        .value
        .clone();
    if let Value::Float(s) = v {
        approx(s, 0.8, 1e-5);
    } else {
        panic!("expected Float");
    }

    // Loop mode wrapping on a 10s clip: seek -0.25 wraps to 9.75 (absolute seconds)
    let mut inputs2 = Inputs::default();
    inputs2.player_cmds.push(PlayerCommand::SetLoopMode {
        player: pid,
        mode: LoopMode::Loop,
    });
    inputs2.player_cmds.push(PlayerCommand::Seek {
        player: pid,
        time: -0.25,
    });
    let out2 = eng.update(0.0, inputs2);
    let v2 = out2
        .changes
        .iter()
        .find(|c| c.key == "node.t")
        .expect("node.t")
        .value
        .clone();
    if let Value::Float(s) = v2 {
        approx(s, 9.75, 1e-5);
    } else {
        panic!("expected Float");
    }
}

/// it should produce identical Outputs for the same dt sequence (determinism)
#[test]
fn parity_determinism_same_sequence() {
    let json_str = include_str!("../../test_fixtures/ramp.json");
    let anim: AnimationData = parse_stored_animation_json(json_str).expect("parse ramp.json");

    let mut e1 = Engine::new(Config::default());
    let mut e2 = Engine::new(Config::default());
    let a1 = e1.load_animation(anim.clone());
    let a2 = e2.load_animation(anim);
    let p1 = e1.create_player("p");
    let p2 = e2.create_player("p");
    let _ = e1.add_instance(p1, a1, InstanceCfg::default());
    let _ = e2.add_instance(p2, a2, InstanceCfg::default());

    // Same dt sequence
    let seq = [0.016, 0.016, 0.032, 0.0, 0.1, 0.25];
    for dt in seq {
        let o1 = e1.update(dt, Inputs::default());
        let o2 = e2.update(dt, Inputs::default());
        let j1 = serde_json::to_string(o1).unwrap();
        let j2 = serde_json::to_string(o2).unwrap();
        assert_eq!(j1, j2, "Outputs JSON must match for dt={dt}");
    }
}

/// it should normalize quaternion when blending two instances contributing to the same key
#[test]
fn parity_quat_layered_normalized() {
    use serde_json::json;
    use vizij_animation_core::value::Value;

    // Build two constant quaternion tracks on the same target key
    let q0 = [0.0, 0.0, 0.0, 1.0];
    let q1 = [0.0, 0.38268343, 0.0, 0.9238795]; // 45 deg around Y

    let t0 = Track {
        id: "t0".into(),
        name: "t0".into(),
        animatable_id: "node.rot".into(),
        points: vec![
            Keypoint {
                id: "k0".into(),
                stamp: 0.0,
                value: Value::Quat(q0),
                transitions: None,
            },
            Keypoint {
                id: "k1".into(),
                stamp: 1.0,
                value: Value::Quat(q0),
                transitions: None,
            },
        ],
        settings: None,
    };
    let t1 = Track {
        id: "t1".into(),
        name: "t1".into(),
        animatable_id: "node.rot".into(),
        points: vec![
            Keypoint {
                id: "k0".into(),
                stamp: 0.0,
                value: Value::Quat(q1),
                transitions: None,
            },
            Keypoint {
                id: "k1".into(),
                stamp: 1.0,
                value: Value::Quat(q1),
                transitions: None,
            },
        ],
        settings: None,
    };

    let a0 = AnimationData {
        id: None,
        name: "a0".into(),
        tracks: vec![t0],
        groups: json!({}),
        duration_ms: 1000,
    };
    let a1 = AnimationData {
        id: None,
        name: "a1".into(),
        tracks: vec![t1],
        groups: json!({}),
        duration_ms: 1000,
    };

    let mut eng = Engine::new(Config::default());
    let id0 = eng.load_animation(a0);
    let id1 = eng.load_animation(a1);
    let p = eng.create_player("p");
    let _i0 = eng.add_instance(
        p,
        id0,
        InstanceCfg {
            weight: 0.3,
            ..Default::default()
        },
    );
    let _i1 = eng.add_instance(
        p,
        id1,
        InstanceCfg {
            weight: 0.7,
            ..Default::default()
        },
    );

    let out = eng.update(0.0, Inputs::default());
    let v = out
        .changes
        .iter()
        .find(|c| c.key == "node.rot")
        .expect("node.rot")
        .value
        .clone();
    if let Value::Quat(qb) = v {
        approx(norm4(qb), 1.0, 1e-4);
    } else {
        panic!("expected Quat");
    }
}
