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

#[wasm_bindgen_test]
fn bake_animation_with_derivatives_bundle() {
    let mut eng = VizijAnimation::new(JsValue::NULL).unwrap();
    let anim_id = eng.load_animation(test_animation_json()).unwrap();
    let bundle = eng
        .bake_animation_with_derivatives(anim_id, JsValue::UNDEFINED)
        .unwrap();
    let obj = js_sys::Object::from(bundle);
    let values = js_sys::Reflect::get(&obj, &JsValue::from_str("values")).unwrap();
    assert!(values.is_object());
    let derivatives = js_sys::Reflect::get(&obj, &JsValue::from_str("derivatives")).unwrap();
    assert!(derivatives.is_object());
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

    // Update with derivatives
    let outputs_with_derivatives = eng
        .update_values_with_derivatives(0.016, JsValue::UNDEFINED)
        .unwrap();
    let obj = js_sys::Object::from(outputs_with_derivatives);
    let changes = js_sys::Reflect::get(&obj, &JsValue::from_str("changes")).unwrap();
    let first = js_sys::Array::from(&changes).get(0);
    let derivative =
        js_sys::Reflect::get(&first, &JsValue::from_str("derivative")).unwrap_or(JsValue::NULL);
    assert!(derivative.is_null() || derivative.is_object());
}
