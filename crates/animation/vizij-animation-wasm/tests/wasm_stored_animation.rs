#![cfg(target_arch = "wasm32")]
use js_sys::{Array, Object, Reflect, JSON};
use vizij_animation_wasm::VizijAnimation;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

fn approx(a: f64, b: f64, eps: f64) {
    assert!((a - b).abs() <= eps, "left={a} right={b} eps={eps}");
}

fn get_scalar_by_key(outputs: JsValue, want_key: &str) -> Option<f64> {
    let obj = Object::from(outputs);
    let changes = Reflect::get(&obj, &JsValue::from_str("changes")).ok()?;
    let arr = Array::from(&changes);
    for i in 0..arr.length() {
        let ch = arr.get(i);
        let key = Reflect::get(&ch, &JsValue::from_str("key"))
            .ok()?
            .as_string()?;
        if key == want_key {
            let val = Reflect::get(&ch, &JsValue::from_str("value")).ok()?;
            let typ = Reflect::get(&val, &JsValue::from_str("type"))
                .ok()?
                .as_string()?;
            if typ == "Scalar" {
                let data = Reflect::get(&val, &JsValue::from_str("data")).ok()?;
                return data.as_f64();
            }
        }
    }
    None
}

/// it should load fixtures/animations/vector-pose-combo.json via load_stored_animation and emit initial outputs
#[wasm_bindgen_test]
fn wasm_loads_new_format_and_samples_initial() {
    // Parse the fixture JSON into a JS object
    let raw = include_str!("../../../../fixtures/animations/vector-pose-combo.json");
    let js_obj = JSON::parse(raw).expect("parse fixture to JS object");

    // Create engine, load stored animation, and add an instance
    let mut eng = VizijAnimation::new(JsValue::UNDEFINED).unwrap();
    let anim_id = eng
        .load_stored_animation(js_obj)
        .expect("load stored animation");
    let player_id = eng.create_player("p".into());
    let _inst = eng
        .add_instance(player_id, anim_id, JsValue::UNDEFINED)
        .unwrap();

    // Initial tick at dt=0.0 -> should emit starting values
    let out0 = eng.update_values(0.0, JsValue::UNDEFINED).unwrap();

    // Track "cube-position-x" starts at -2 (see fixture)
    let s0 = get_scalar_by_key(out0, "cube-position-x").expect("cube-position-x");
    approx(s0, -2.0, 1e-6);
}
