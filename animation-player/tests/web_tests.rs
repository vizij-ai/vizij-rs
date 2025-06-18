//! Integration tests for WASM bindings
#![cfg(target_arch = "wasm32")]
extern crate wasm_bindgen_test;

use animation_player::wasm::{value_to_js, WasmAnimationEngine};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_wasm_engine_creation() {
    let engine = WasmAnimationEngine::new(None);
    assert!(engine.is_ok());
}

#[wasm_bindgen_test]
fn test_wasm_basic_workflow() {
    let mut engine = WasmAnimationEngine::new(None).unwrap();

    // Create a simple animation
    let animation_json = r#"
        {
            "id": "test_animation",
            "name": "Test Animation",
            "metadata": {
                "created_at": 0,
                "modified_at": 0,
                "author": null,
                "description": null,
                "tags": [],
                "version": "1.0.0",
                "duration": 2000000000,
                "frame_rate": 60.0
            },
            "tracks": {},
            "groups": {}
        }"#;

    // Load animation
    let animation_id = engine.load_animation(animation_json).unwrap();

    // Create player
    let player_id = engine.create_player();

    // Add instance
    assert!(engine.add_instance(&player_id, &animation_id).is_ok());

    // Test playback controls
    assert!(engine.play(&player_id).is_ok());
    let get_player_state_name = |engine: &WasmAnimationEngine| {
        let js_state = engine.get_player_state(&player_id).unwrap();
        let value = js_sys::Reflect::get(&js_state, &"playback_state".into()).unwrap();
        value.as_string().unwrap().to_lowercase()
    };
    assert_eq!(get_player_state_name(&engine), "playing");

    assert!(engine.pause(&player_id).is_ok());
    assert_eq!(get_player_state_name(&engine), "paused");

    assert!(engine.stop(&player_id).is_ok());
    assert_eq!(get_player_state_name(&engine), "stopped");
}

#[wasm_bindgen_test]
fn test_wasm_utility_functions() {
    let value_json = r#"{"Float": 42.5}"#;
    assert!(value_to_js(value_json).is_ok());
}
