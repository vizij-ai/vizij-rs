#![cfg(target_arch = "wasm32")]
use serde_wasm_bindgen as swb;
use vizij_animation_wasm::{abi_version, VizijAnimation};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

use serde_json::json;
use vizij_animation_core::data::{AnimationData, Keypoint, Track};
use vizij_animation_core::value::Value;

// Minimal AnimationData JSON matching the new vizij-animation-core schema
fn test_animation_json() -> JsValue {
    let track = Track {
        id: "t0".into(),
        name: "scalar".into(),
        animatable_id: "node.s".into(),
        points: vec![
            Keypoint {
                id: "k0".into(),
                stamp: 0.0,
                value: Value::Scalar(0.0),
                transitions: None,
            },
            Keypoint {
                id: "k1".into(),
                stamp: 1.0,
                value: Value::Scalar(1.0),
                transitions: None,
            },
        ],
        settings: None,
    };
    let anim = AnimationData {
        id: None,
        name: "clip".into(),
        tracks: vec![track],
        groups: json!({}),
        duration_ms: 1000,
    };
    swb::to_value(&anim).unwrap()
}

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn abi_is_2() {
    assert_eq!(abi_version(), 2);
}

#[wasm_bindgen_test]
fn construct_with_defaults() {
    let eng = VizijAnimation::new(JsValue::UNDEFINED);
    assert!(eng.is_ok());
}

#[wasm_bindgen_test]
fn load_create_add_and_update() {
    let mut eng = VizijAnimation::new(JsValue::NULL).unwrap();

    // Load animation JSON
    let anim_id = eng.load_animation(test_animation_json()).unwrap();
    // Create player
    let player_id = eng.create_player("p".to_string());
    assert_eq!(player_id, 0);

    // Add instance with default cfg (undefined)
    let _inst_id = eng
        .add_instance(player_id, anim_id, JsValue::UNDEFINED)
        .unwrap();

    // Prebind with a resolver that returns canonical path uppercased as handle
    let resolver = js_sys::Function::new_with_args("path", "return String(path).toUpperCase();");
    eng.prebind(resolver);

    // Update with no inputs (undefined) at small dt
    let outputs = eng.update_values(0.016, JsValue::UNDEFINED).unwrap();
    // Outputs should be an object with { changes, events }
    let obj = js_sys::Object::from(outputs);
    let changes = js_sys::Reflect::get(&obj, &JsValue::from_str("changes")).unwrap();
    assert!(changes.is_object() || changes.is_array());
}

// Negative/error-path tests

/// it should error cleanly when loading malformed JSON
#[wasm_bindgen_test]
fn load_animation_malformed_json_errors() {
    let mut eng = VizijAnimation::new(JsValue::NULL).unwrap();
    // Not a valid AnimationData shape (string)
    let bad_json = JsValue::from_str("not-json-anim");
    let res = eng.load_animation(bad_json);
    assert!(res.is_err());
}

/// it should error cleanly when add_instance receives invalid cfg JSON
#[wasm_bindgen_test]
fn add_instance_invalid_cfg_errors() {
    let mut eng = VizijAnimation::new(JsValue::NULL).unwrap();
    let anim_id = eng.load_animation(test_animation_json()).unwrap();
    let player_id = eng.create_player("p".into());
    // cfg must be an object; pass a number instead
    let bad_cfg = JsValue::from_f64(123.0);
    let res = eng.add_instance(player_id, anim_id, bad_cfg);
    assert!(res.is_err());
}

/// it should tolerate resolver throwing and treat as unresolved (no panic)
#[wasm_bindgen_test]
fn prebind_resolver_throwing_is_ignored() {
    let mut eng = VizijAnimation::new(JsValue::NULL).unwrap();
    let anim_id = eng.load_animation(test_animation_json()).unwrap();
    let player_id = eng.create_player("p".into());
    let _inst_id = eng
        .add_instance(player_id, anim_id, JsValue::UNDEFINED)
        .unwrap();

    // Resolver that throws
    let resolver = js_sys::Function::new_with_args("path", "throw new Error('boom');");
    // Should not panic
    eng.prebind(resolver);

    // Update should still succeed
    let _outputs = eng.update_values(0.016, JsValue::UNDEFINED).unwrap();
}

#[wasm_bindgen_test]
fn update_with_derivatives_returns_optional_derivative() {
    let mut eng = VizijAnimation::new(JsValue::NULL).unwrap();
    let anim_id = eng.load_animation(test_animation_json()).unwrap();
    let player_id = eng.create_player("p".into());
    let _ = eng
        .add_instance(player_id, anim_id, JsValue::UNDEFINED)
        .unwrap();

    let outputs = eng
        .update_values_and_derivatives(0.016, JsValue::UNDEFINED)
        .unwrap();
    let obj = js_sys::Object::from(outputs);
    let changes = js_sys::Reflect::get(&obj, &JsValue::from_str("changes")).unwrap();
    let array = js_sys::Array::from(&changes);
    assert!(array.length() > 0);
    let mut found_object = false;
    for idx in 0..array.length() {
        let entry = array.get(idx);
        let derivative = js_sys::Reflect::get(&entry, &JsValue::from_str("derivative")).unwrap();
        assert!(derivative.is_null() || derivative.is_object());
        if derivative.is_object() {
            found_object = true;
        }
    }
    assert!(
        found_object,
        "expected at least one derivative object for numeric track"
    );
}

#[wasm_bindgen_test]
fn bake_animation_with_derivatives_returns_bundle() {
    let mut eng = VizijAnimation::new(JsValue::NULL).unwrap();
    let anim_id = eng.load_animation(test_animation_json()).unwrap();

    let bundle = eng
        .bake_animation_with_derivatives(anim_id, JsValue::UNDEFINED)
        .unwrap();
    let obj = js_sys::Object::from(bundle);
    assert!(js_sys::Reflect::has(&obj, &JsValue::from_str("values")).unwrap());
    assert!(js_sys::Reflect::has(&obj, &JsValue::from_str("derivatives")).unwrap());

    let values = js_sys::Reflect::get(&obj, &JsValue::from_str("values")).unwrap();
    let derivatives = js_sys::Reflect::get(&obj, &JsValue::from_str("derivatives")).unwrap();
    let values_tracks =
        js_sys::Array::from(&js_sys::Reflect::get(&values, &JsValue::from_str("tracks")).unwrap());
    let derivatives_tracks = js_sys::Array::from(
        &js_sys::Reflect::get(&derivatives, &JsValue::from_str("tracks")).unwrap(),
    );
    assert_eq!(values_tracks.length(), derivatives_tracks.length());
    let first_values_track = values_tracks.get(0);
    let first_derivatives_track = derivatives_tracks.get(0);
    let value_samples = js_sys::Array::from(
        &js_sys::Reflect::get(&first_values_track, &JsValue::from_str("values")).unwrap(),
    );
    let derivative_samples = js_sys::Array::from(
        &js_sys::Reflect::get(&first_derivatives_track, &JsValue::from_str("values")).unwrap(),
    );
    assert_eq!(value_samples.length(), derivative_samples.length());
}

#[wasm_bindgen_test]
fn bake_animation_with_derivatives_rejects_invalid_config() {
    let mut eng = VizijAnimation::new(JsValue::NULL).unwrap();
    let anim_id = eng.load_animation(test_animation_json()).unwrap();

    let cfg = js_sys::Object::new();
    js_sys::Reflect::set(
        &cfg,
        &JsValue::from_str("frame_rate"),
        &JsValue::from_f64(0.0),
    )
    .unwrap();
    let res = eng.bake_animation_with_derivatives(anim_id, cfg.into());
    assert!(res.is_err());
}
