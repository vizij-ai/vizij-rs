use serde_json::Value as JsonValue;
use vizij_animation_core::{
    data::{AnimationData, Keypoint, Track, Transitions, Vec2},
    Config, Engine, Inputs, Value,
};

fn mk_scalar_track_linear(path: &str, keys: &[(f32, f32)]) -> Track {
    let mut points: Vec<Keypoint> = Vec::with_capacity(keys.len());
    for (i, (stamp, v)) in keys.iter().enumerate() {
        let mut transitions: Option<Transitions> = None;
        let is_first = i == 0;
        let is_last = i + 1 == keys.len();
        if !is_last || !is_first {
            let mut t = Transitions {
                r#in: None,
                r#out: None,
            };
            if !is_last {
                t.r#out = Some(Vec2 { x: 0.0, y: 0.0 });
            }
            if !is_first {
                t.r#in = Some(Vec2 { x: 1.0, y: 1.0 });
            }
            if t.r#in.is_some() || t.r#out.is_some() {
                transitions = Some(t);
            }
        }
        points.push(Keypoint {
            id: format!("k{i}"),
            stamp: *stamp,
            value: Value::Float(*v),
            transitions,
        });
    }
    Track {
        id: format!("t-{}", path),
        name: path.to_string(),
        animatable_id: path.to_string(),
        points,
        settings: None,
    }
}

fn mk_anim(name: &str, duration_s: f32, tracks: Vec<Track>) -> AnimationData {
    AnimationData {
        id: None,
        name: name.to_string(),
        tracks,
        groups: serde_json::json!({}),
        duration_ms: (duration_s * 1000.0) as u32,
    }
}

#[test]
fn update_writebatch_serializes_to_json_shape() {
    // Build a simple animation with one scalar track
    let track = mk_scalar_track_linear("node.t", &[(0.0, 0.0), (1.0, 1.0)]);
    let anim = mk_anim("wb", 1.0, vec![track]);

    let mut eng = Engine::new(Config::default());
    let aid = eng.load_animation(anim);
    let pid = eng.create_player("p");
    let _iid = eng.add_instance(pid, aid, Default::default());

    // Request a WriteBatch from the engine for dt=0.0
    let batch = eng.update_writebatch(0.0, Inputs::default());

    // Serialize to serde_json::Value and assert shape: should be an array of objects with path+value
    let j = serde_json::to_value(&batch).expect("serialize batch");
    assert!(
        j.is_array(),
        "expected WriteBatch to serialize to JSON array"
    );

    let arr = j.as_array().unwrap();
    assert!(!arr.is_empty(), "expected at least one write entry");

    // Each entry should be an object with "path" and "value" keys
    let first = &arr[0];
    assert!(first.is_object(), "first write entry should be object");
    let obj = first.as_object().unwrap();
    assert!(obj.contains_key("path"), "write entry missing path");
    assert!(obj.contains_key("value"), "write entry missing value");

    // Basic sanity on types
    let path_val = &obj["path"];
    assert!(path_val.is_string(), "path should be string");
    let _value_obj: &JsonValue = &obj["value"];
    // Further validation of value JSON shape could be added, but this asserts the basic contract.
}
