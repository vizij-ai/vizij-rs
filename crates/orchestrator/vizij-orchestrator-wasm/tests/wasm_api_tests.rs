#![cfg(target_arch = "wasm32")]
use js_sys::Function;
use js_sys::Object;
use js_sys::Reflect;
use serde_wasm_bindgen as swb;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

use vizij_orchestrator_wasm::{abi_version, VizijOrchestrator};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn abi_is_1() {
    assert_eq!(abi_version(), 1);
}

#[wasm_bindgen_test]
fn construct_with_defaults() {
    let inst = VizijOrchestrator::new(JsValue::UNDEFINED);
    assert!(inst.is_ok());
}

#[wasm_bindgen_test]
fn register_graph_and_step_returns_frame() {
    let mut o = VizijOrchestrator::new(JsValue::NULL).unwrap();

    // Minimal GraphSpec: empty nodes array is valid for deserialization
    let spec = serde_json::json!({
        "nodes": []
    });
    // register_graph accepts either JSON string or object
    let id = o.register_graph(swb::to_value(&spec).unwrap()).unwrap();
    assert!(!id.is_empty());

    // register a simple animation controller (no setup)
    let anim_id = o
        .register_animation(swb::to_value(&serde_json::json!({})).unwrap())
        .unwrap();
    assert!(!anim_id.is_empty());

    // set a blackboard input
    let value = serde_json::json!({ "float": 0.5 });
    o.set_input(
        "robot/x".into(),
        swb::to_value(&value).unwrap(),
        JsValue::UNDEFINED,
    )
    .expect("set_input should succeed");

    // step the orchestrator by a small dt
    let frame_js = o.step(0.016).expect("step should succeed");
    let frame_obj = Object::from(frame_js);

    // Check some expected fields exist: epoch, dt, merged_writes, timings_ms
    assert!(Reflect::has(&frame_obj, &JsValue::from_str("epoch")).unwrap());
    assert!(Reflect::has(&frame_obj, &JsValue::from_str("dt")).unwrap());
    assert!(Reflect::has(&frame_obj, &JsValue::from_str("merged_writes")).unwrap());
    assert!(Reflect::has(&frame_obj, &JsValue::from_str("timings_ms")).unwrap());
}

#[wasm_bindgen_test]
fn prebind_resolver_throwing_is_ignored() {
    let mut o = VizijOrchestrator::new(JsValue::NULL).unwrap();
    let _g = o
        .register_graph(swb::to_value(&serde_json::json!({ "nodes": [] })).unwrap())
        .unwrap();
    let _a = o
        .register_animation(swb::to_value(&serde_json::json!({})).unwrap())
        .unwrap();

    // Resolver that throws
    let resolver = Function::new_with_args("path", "throw new Error('boom');");

    // Should not panic
    o.prebind(resolver);

    // Step should still work
    let frame_js = o
        .step(0.016)
        .expect("step should succeed even if resolver throws");
    let frame_obj = Object::from(frame_js);
    assert!(Reflect::has(&frame_obj, &JsValue::from_str("epoch")).unwrap());
}

#[wasm_bindgen_test]
fn list_and_remove_controllers() {
    let mut o = VizijOrchestrator::new(JsValue::NULL).unwrap();
    let g = o
        .register_graph(swb::to_value(&serde_json::json!({ "nodes": [] })).unwrap())
        .unwrap();
    let a = o
        .register_animation(swb::to_value(&serde_json::json!({})).unwrap())
        .unwrap();

    // list_controllers should return an object containing arrays
    let list = o.list_controllers().expect("list_controllers ok");
    let list_obj = Object::from(list);
    assert!(Reflect::has(&list_obj, &JsValue::from_str("graphs")).unwrap());
    assert!(Reflect::has(&list_obj, &JsValue::from_str("anims")).unwrap());

    // remove Graph and Animation
    assert!(o.remove_graph(&g));
    assert!(o.remove_animation(&a));
}
