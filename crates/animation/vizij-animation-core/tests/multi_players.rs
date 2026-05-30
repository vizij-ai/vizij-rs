use serde_json::json;
use vizij_animation_core::{
    data::{AnimationData, AuthoredTransition, Keypoint, Track, Transitions},
    engine::InstanceCfg,
    Config, Engine, Value,
};

fn mk_anim(name: &str, duration_s: f32) -> AnimationData {
    let duration_ms = (duration_s * 1000.0) as u32;
    // Minimal canonical Studio v2 scalar track with millisecond stamps.
    let points = vec![
        Keypoint {
            id: "k0".into(),
            stamp: 0.0,
            value: Value::Float(0.0),
            transitions: Some(Transitions {
                r#in: None,
                r#out: Some(AuthoredTransition::name("linear")),
                pairing: None,
            }),
        },
        Keypoint {
            id: "k1".into(),
            stamp: duration_ms as f32,
            value: Value::Float(1.0),
            transitions: Some(Transitions {
                r#in: Some(AuthoredTransition::name("linear")),
                r#out: None,
                pairing: None,
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
        duration_ms,
    }
}

#[test]
fn durations_are_independent_per_player() {
    let mut eng = Engine::new(Config::default());
    let a = eng.load_animation(mk_anim("A", 3.0)); // 3s clip

    let p1 = eng.create_player("P1");
    let p2 = eng.create_player("P2");

    // P1: single instance, scale 1.0, offset 0 => span 3.0
    eng.add_instance(
        p1,
        a,
        InstanceCfg {
            weight: 1.0,
            time_scale: 1.0,
            start_offset: 0.0,
            enabled: true,
        },
    );
    // P2: two instances with different spans (Studio speed semantics):
    // (3.0 / (1/3)) = 9.0 and (3.0 / 2.0) = 1.5 => max span 9.0
    eng.add_instance(
        p2,
        a,
        InstanceCfg {
            weight: 1.0,
            time_scale: 1.0 / 3.0,
            start_offset: 0.0,
            enabled: true,
        },
    );
    eng.add_instance(
        p2,
        a,
        InstanceCfg {
            weight: 1.0,
            time_scale: 2.0,
            start_offset: 0.0,
            enabled: true,
        },
    );

    let d1 = eng.player_total_duration(p1).unwrap();
    let d2 = eng.player_total_duration(p2).unwrap();

    assert!((d1 - 3.0).abs() < 1e-6, "P1 total_duration should be 3.0");
    assert!((d2 - 9.0).abs() < 1e-6, "P2 total_duration should be 9.0");

    // Apply window to P2 that is smaller than span; duration should clamp to window
    let _ = eng.update(
        0.0,
        vizij_animation_core::Inputs {
            player_cmds: vec![vizij_animation_core::PlayerCommand::SetWindow {
                player: p2,
                start_time: 0.0,
                end_time: Some(2.5),
            }],
            instance_updates: vec![],
        },
    );
    let d2w = eng.player_total_duration(p2).unwrap();
    assert!(
        (d2w - 2.5).abs() < 1e-6,
        "P2 duration should clamp to window 2.5"
    );
}
