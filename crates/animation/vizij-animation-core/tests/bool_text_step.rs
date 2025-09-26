use serde_json::json;
use vizij_animation_core::{
    config::Config,
    data::{AnimationData, Keypoint, Track},
    engine::{Engine, InstanceCfg},
    inputs::Inputs,
    sampling::sample_track,
    value::Value,
};

fn mk_bool_track(path: &str, keys: &[(f32, bool)]) -> Track {
    let mut points = Vec::with_capacity(keys.len());
    for (i, (stamp, v)) in keys.iter().enumerate() {
        points.push(Keypoint {
            id: format!("k{i}"),
            stamp: *stamp,
            value: Value::Bool(*v),
            transitions: None,
        });
    }
    Track {
        id: "t-bool".into(),
        name: "Bool".into(),
        animatable_id: path.to_string(),
        points,
        settings: None,
    }
}

fn mk_text_track(path: &str, keys: &[(f32, &str)]) -> Track {
    let mut points = Vec::with_capacity(keys.len());
    for (i, (stamp, s)) in keys.iter().enumerate() {
        points.push(Keypoint {
            id: format!("k{i}"),
            stamp: *stamp,
            value: Value::Text((*s).to_string()),
            transitions: None,
        });
    }
    Track {
        id: "t-text".into(),
        name: "Text".into(),
        animatable_id: path.to_string(),
        points,
        settings: None,
    }
}

fn mk_anim_ms(name: &str, duration_ms: u32, tracks: Vec<Track>) -> AnimationData {
    AnimationData {
        id: None,
        name: name.to_string(),
        tracks,
        groups: json!({}),
        duration_ms,
    }
}

#[test]
fn step_sampling_for_bool_and_text_tracks() {
    // Bool: false at 0.0, true at 0.5, true at 1.0
    let t_bool = mk_bool_track("node.flag", &[(0.0, false), (0.5, true), (1.0, true)]);
    // Text: "A" at 0.0, "B" at 0.5, "B" at 1.0
    let t_text = mk_text_track("node.label", &[(0.0, "A"), (0.5, "B"), (1.0, "B")]);

    // Check sampler directly (u is normalized 0..1)
    // At u=0.25 -> left of [0.0,0.5] so expect first key values
    match sample_track(&t_bool, 0.25) {
        Value::Bool(b) => assert!(!b),
        _ => panic!(),
    }
    match sample_track(&t_text, 0.25) {
        Value::Text(s) => assert_eq!(s, "A"),
        _ => panic!(),
    }
    // At u=0.6 -> in segment [0.5,1.0], expect second key values due to step (hold left)
    match sample_track(&t_bool, 0.6) {
        Value::Bool(b) => assert!(b),
        _ => panic!(),
    }
    match sample_track(&t_text, 0.6) {
        Value::Text(s) => assert_eq!(s, "B"),
        _ => panic!(),
    }

    // Engine integration: ensure outputs emit correct types/values
    let anim = mk_anim_ms("clip", 1000, vec![t_bool, t_text]);
    let mut eng = Engine::new(Config::default());
    let a = eng.load_animation(anim);
    let p = eng.create_player("p");
    let _i = eng.add_instance(p, a, InstanceCfg::default());

    // Initial tick at 0.0
    let out0 = eng.update_values(0.0, Inputs::default());
    let flag0 = out0
        .changes
        .iter()
        .find(|c| c.key == "node.flag")
        .unwrap()
        .value
        .clone();
    let label0 = out0
        .changes
        .iter()
        .find(|c| c.key == "node.label")
        .unwrap()
        .value
        .clone();
    match flag0 {
        Value::Bool(b) => assert!(!b),
        _ => panic!(),
    }
    match label0 {
        Value::Text(s) => assert_eq!(s, "A"),
        _ => panic!(),
    }

    // Advance by 0.6s (engine maps seconds via duration_ms=1.0s) -> expect B/true
    let _ = eng.update_values(0.6, Inputs::default());
    let out1 = eng.update_values(0.0, Inputs::default());
    let flag1 = out1
        .changes
        .iter()
        .find(|c| c.key == "node.flag")
        .unwrap()
        .value
        .clone();
    let label1 = out1
        .changes
        .iter()
        .find(|c| c.key == "node.label")
        .unwrap()
        .value
        .clone();
    match flag1 {
        Value::Bool(b) => assert!(b),
        _ => panic!(),
    }
    match label1 {
        Value::Text(s) => assert_eq!(s, "B"),
        _ => panic!(),
    }
}
