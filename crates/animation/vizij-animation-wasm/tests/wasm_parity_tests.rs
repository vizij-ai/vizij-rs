#![cfg(target_arch = "wasm32")]
use js_sys::{Array, Object, Reflect};
use serde_wasm_bindgen as swb;
use vizij_animation_wasm::{abi_version, VizijAnimation};
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

/// it should mirror core ramp parity via the wasm API (tick-by-tick scalar equality)
#[wasm_bindgen_test]
fn wasm_parity_scalar_ramp() {
    // Fixture matches vizij-animation-core schema
    use vizij_animation_core::parse_stored_animation_json;
    let ramp_json = include_str!("../../test_fixtures/ramp.json");
    let anim = parse_stored_animation_json(ramp_json).unwrap();
    let ramp_js = swb::to_value(&anim).unwrap();

    let mut eng = VizijAnimation::new(JsValue::UNDEFINED).unwrap();
    let anim_id = eng.load_animation(ramp_js).unwrap();
    let player_id = eng.create_player("p".into());
    let _inst = eng
        .add_instance(player_id, anim_id, JsValue::UNDEFINED)
        .unwrap();

    // Initial tick at dt=0.0 -> value ~ 0.0
    let out0 = eng.update(0.0, JsValue::UNDEFINED).unwrap();
    let s0 = get_scalar_by_key(out0, "node.t").expect("node.t");
    approx(s0, 0.0, 1e-6);

    // Step 9 ticks of 0.1 -> expect ~ i/10 at each step (avoid wrap at exactly 1.0 in Loop mode)
    let mut t = 0.0f64;
    for i in 1..=9 {
        let out = eng.update(0.1, JsValue::UNDEFINED).unwrap();
        t += 0.1;
        let s = get_scalar_by_key(out, "node.t").expect("node.t");
        approx(s, t, 1e-6);
        assert!(
            (s - (i as f64) / 10.0).abs() < 1e-6,
            "tick {i} expected ~{}, got {}",
            (i as f64) / 10.0,
            s
        );
    }
}

/// it should validate the wasm ABI version gate for the parity suite
#[wasm_bindgen_test]
fn wasm_parity_abi_is_1() {
    assert_eq!(abi_version(), 2);
}
