#![cfg(target_arch = "wasm32")]
use js_sys::{Array, Object, Reflect};
use serde_wasm_bindgen as swb;
use vizij_animation_wasm::VizijAnimation;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

use serde_json::json;
use vizij_animation_core::{
    data::{AnimationData, Keypoint, Track},
    value::Value,
};

wasm_bindgen_test_configure!(run_in_browser);

fn get_change_value(outputs: &JsValue, want_key: &str) -> Option<JsValue> {
    let obj = Object::from(outputs.clone());
    let changes = Reflect::get(&obj, &JsValue::from_str("changes")).ok()?;
    let arr = Array::from(&changes);
    for i in 0..arr.length() {
        let ch = arr.get(i);
        let key = Reflect::get(&ch, &JsValue::from_str("key"))
            .ok()?
            .as_string()?;
        if key == want_key {
            return Reflect::get(&ch, &JsValue::from_str("value")).ok();
        }
    }
    None
}

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
        id: format!("t-{}", path),
        name: path.to_string(),
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
        id: format!("t-{}", path),
        name: path.to_string(),
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

/// it should round-trip Bool/Text values through wasm outputs and respect step sampling
#[wasm_bindgen_test]
fn wasm_bool_text_outputs_and_step() {
    // Build a minimal animation with Bool/Text tracks (stamps in 0..1, duration in ms)
    let t_bool = mk_bool_track("node.flag", &[(0.0, false), (0.5, true), (1.0, true)]);
    let t_text = mk_text_track("node.label", &[(0.0, "A"), (0.5, "B"), (1.0, "B")]);
    let anim = mk_anim_ms("clip", 1000, vec![t_bool, t_text]);

    // Convert to JsValue and load via load_animation (AnimationData contract)
    let anim_js = swb::to_value(&anim).expect("to js");
    let mut eng = VizijAnimation::new(JsValue::UNDEFINED).unwrap();
    let anim_id = eng.load_animation(anim_js).expect("load anim");
    let player_id = eng.create_player("p".into());
    let _inst = eng
        .add_instance(player_id, anim_id, JsValue::UNDEFINED)
        .unwrap();

    // Initial tick at 0.0
    let out0 = eng.update_values(0.0, JsValue::UNDEFINED).unwrap();
    let v_flag0 = get_change_value(&out0, "node.flag").expect("node.flag");
    let v_label0 = get_change_value(&out0, "node.label").expect("node.label");

    // Check type tags and data
    let typ_flag0 = Reflect::get(&v_flag0, &JsValue::from_str("type"))
        .unwrap()
        .as_string()
        .unwrap();
    let data_flag0 = Reflect::get(&v_flag0, &JsValue::from_str("data"))
        .unwrap()
        .as_bool()
        .unwrap();
    assert_eq!(typ_flag0, "Bool");
    assert_eq!(data_flag0, false);

    let typ_label0 = Reflect::get(&v_label0, &JsValue::from_str("type"))
        .unwrap()
        .as_string()
        .unwrap();
    let data_label0 = Reflect::get(&v_label0, &JsValue::from_str("data"))
        .unwrap()
        .as_string()
        .unwrap();
    assert_eq!(typ_label0, "Text");
    assert_eq!(data_label0, "A".to_string());

    // Advance to 0.6s (u~=0.6) -> expect true / "B" due to step (hold left)
    let _ = eng.update_values(0.6, JsValue::UNDEFINED).unwrap();
    let out1 = eng.update_values(0.0, JsValue::UNDEFINED).unwrap();
    let v_flag1 = get_change_value(&out1, "node.flag").expect("node.flag");
    let v_label1 = get_change_value(&out1, "node.label").expect("node.label");

    let data_flag1 = Reflect::get(&v_flag1, &JsValue::from_str("data"))
        .unwrap()
        .as_bool()
        .unwrap();
    let data_label1 = Reflect::get(&v_label1, &JsValue::from_str("data"))
        .unwrap()
        .as_string()
        .unwrap();
    assert_eq!(data_flag1, true);
    assert_eq!(data_label1, "B".to_string());
}
