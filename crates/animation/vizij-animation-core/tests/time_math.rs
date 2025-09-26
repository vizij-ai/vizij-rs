use serde_json::json;
use vizij_animation_core::{
    data::{AnimationData, Keypoint, Track, Transitions, Vec2},
    engine::InstanceCfg,
    Config, Engine, Value,
};

fn mk_anim(name: &str, duration_s: f32) -> AnimationData {
    // Minimal scalar track with linear transitions encoded via per-point transitions:
    // left.out=(0,0), right.in=(1,1)
    let points = vec![
        Keypoint {
            id: "k0".into(),
            stamp: 0.0,
            value: Value::Float(0.0),
            transitions: Some(Transitions {
                r#in: None,
                r#out: Some(Vec2 { x: 0.0, y: 0.0 }),
            }),
        },
        Keypoint {
            id: "k1".into(),
            stamp: 1.0,
            value: Value::Float(1.0),
            transitions: Some(Transitions {
                r#in: Some(Vec2 { x: 1.0, y: 1.0 }),
                r#out: None,
            }),
        },
    ];
    let track = Track {
        id: "t0".into(),
        name: "Dummy".into(),
        animatable_id: "Dummy".into(),
        points,
        settings: None,
    };
    AnimationData {
        id: None,
        name: name.to_string(),
        tracks: vec![track],
        groups: json!({}),
        duration_ms: (duration_s * 1000.0) as u32,
    }
}

#[test]
fn total_duration_multiple_instances_basic() {
    let mut eng = Engine::new(Config::default());
    let a1 = eng.load_animation(mk_anim("A1", 2.0)); // 2.0s long
    let a2 = eng.load_animation(mk_anim("A2", 1.0)); // 1.0s long

    let p = eng.create_player("P");

    // Instance 1: time_scale 1.0, offset 0.0 -> span = 2.0 / 1.0 = 2.0
    eng.add_instance(
        p,
        a1,
        InstanceCfg {
            weight: 1.0,
            time_scale: 1.0,
            start_offset: 0.0,
            enabled: true,
        },
    );

    // Instance 2: time_scale 2.0 (slower by multiplier semantics), offset 0.0 -> span = 1.0 * 2.0 = 2.0
    eng.add_instance(
        p,
        a2,
        InstanceCfg {
            weight: 1.0,
            time_scale: 2.0,
            start_offset: 0.0,
            enabled: true,
        },
    );

    // Effective total duration should be the max span across instances: 2.0
    let dur0 = eng.player_total_duration(p).expect("player duration");
    assert!(
        (dur0 - 2.0).abs() < 1e-6,
        "initial total duration should be 2.0 (max span)"
    );
    // Apply a window larger than spans: [0, 10] -> total_duration should remain 2.0 (min(10,2))
    eng.update_values(
        0.0,
        vizij_animation_core::Inputs {
            player_cmds: vec![vizij_animation_core::PlayerCommand::SetWindow {
                player: p,
                start_time: 0.0,
                end_time: Some(10.0),
            }],
            instance_updates: vec![],
        },
    );

    // We can't read total_duration directly; instead simulate advance and ensure
    // local time maps without panic.
    let mut eng_local = eng;
    let _out = eng_local.update_values(1.99, vizij_animation_core::Inputs::default());
    let _out2 = eng_local.update_values(0.02, vizij_animation_core::Inputs::default());

    // Verify duration accessor still returns ~2.0 after window set
    let dur1 = eng_local.player_total_duration(p).expect("player duration");
    assert!(
        (dur1 - 2.0).abs() < 1e-6,
        "total duration should remain 2.0 after large window"
    );
}

#[test]
fn total_duration_with_offsets_and_negative_scale() {
    let mut eng = Engine::new(Config::default());
    let a = eng.load_animation(mk_anim("A", 2.0));

    let p = eng.create_player("P");

    // Instance forward near the end: remaining_local = 2.0 - 1.5 = 0.5 => span = 0.5 / 1.0
    eng.add_instance(
        p,
        a,
        InstanceCfg {
            weight: 1.0,
            time_scale: 1.0,
            start_offset: 1.5,
            enabled: true,
        },
    );

    // Instance reverse from 0.3: remaining_local = 0.3 - 0.0 = 0.3, span = 0.3 / 1.0
    eng.add_instance(
        p,
        a,
        InstanceCfg {
            weight: 1.0,
            time_scale: -1.0,
            start_offset: 0.3,
            enabled: true,
        },
    );

    // Apply small window to force total_duration to be min(window_len, max_span)
    eng.update_values(
        0.0,
        vizij_animation_core::Inputs {
            player_cmds: vec![vizij_animation_core::PlayerCommand::SetWindow {
                player: p,
                start_time: 0.0,
                end_time: Some(0.4),
            }],
            instance_updates: vec![],
        },
    );

    // After window (0.4), total duration should be 0.4 (min(0.4, max_span=0.5))
    let durw = eng.player_total_duration(p).expect("player duration");
    assert!(
        (durw - 0.4).abs() < 1e-6,
        "total duration should be 0.4 after window"
    );
    // Flip to Once mode
    eng.update_values(
        0.0,
        vizij_animation_core::Inputs {
            player_cmds: vec![vizij_animation_core::PlayerCommand::SetLoopMode {
                player: p,
                mode: vizij_animation_core::LoopMode::Once,
            }],
            instance_updates: vec![],
        },
    );

    // Run for 1.0s without panicking; local mapping should handle reverse/forward correctly
    let mut eng2 = eng;
    let _ = eng2.update_values(1.0, vizij_animation_core::Inputs::default());
}

#[test]
fn interpolation_bezier_and_bezier_ease_in_smoke() {
    use vizij_animation_core::sampling::sample_track;

    // Ease-in-out via default sampler (no transitions specified)
    let bez_points = vec![
        Keypoint {
            id: "k0".into(),
            stamp: 0.0,
            value: Value::Float(0.0),
            transitions: None, // default out (0.42,0)
        },
        Keypoint {
            id: "k1".into(),
            stamp: 1.0,
            value: Value::Float(1.0),
            transitions: None, // default in (0.58,1)
        },
    ];
    let track_bez = Track {
        id: "tb".into(),
        name: "bez".into(),
        animatable_id: "node.v".into(),
        points: bez_points,
        settings: None,
    };
    let v_mid = sample_track(&track_bez, 0.5);
    if let Value::Float(x) = v_mid {
        assert!(
            x > 0.4 && x < 0.6,
            "bezier midpoint in reasonable range, got {x}"
        );
    } else {
        panic!("expected scalar");
    }

    // Bezier ease-in: left.out=(0.42,0), right.in=(1,1) -> expect below linear at mid
    let ei_points = vec![
        Keypoint {
            id: "k0".into(),
            stamp: 0.0,
            value: Value::Float(0.0),
            transitions: Some(Transitions {
                r#in: None,
                r#out: Some(Vec2 { x: 0.42, y: 0.0 }),
            }),
        },
        Keypoint {
            id: "k1".into(),
            stamp: 1.0,
            value: Value::Float(1.0),
            transitions: Some(Transitions {
                r#in: Some(Vec2 { x: 1.0, y: 1.0 }),
                r#out: None,
            }),
        },
    ];
    let track_ease_in = Track {
        id: "te".into(),
        name: "ease-in".into(),
        animatable_id: "node.v".into(),
        points: ei_points,
        settings: None,
    };
    let v_mid_bz = sample_track(&track_ease_in, 0.5);
    if let Value::Float(x) = v_mid_bz {
        assert!(
            x < 0.5,
            "ease-in at 0.5 should be less than linear 0.5, got {x}"
        );
    } else {
        panic!("expected scalar");
    }
}
