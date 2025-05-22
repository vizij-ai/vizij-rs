//! Test suite for the Web and headless browsers.
#![cfg(target_arch = "wasm32")]
extern crate wasm_bindgen_test;
use animation_player::animation_data::{AnimationData, AnimationTransition};
use serde_wasm_bindgen;
use std::collections::HashMap;
use uuid::Uuid;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn pass() {
    assert_eq!(1 + 1, 2);
}

#[wasm_bindgen_test]
fn test_load_animation_in_browser() {
    // Create a test animation
    let mut transitions = HashMap::new();
    transitions.insert(
        "transition1".to_string(),
        AnimationTransition {
            id: "transition1".to_string(),
            keypoints: ("key1".to_string(), "key2".to_string()),
            variant: "linear".to_string(),
            parameters: HashMap::new(),
        },
    );

    let animation = AnimationData {
        id: "web-test-anim".to_string(),
        name: "Web Test Animation".to_string(),
        tracks: vec![],
        transitions,
        duration: 5.0,
    };

    // Convert to JsValue for passing to load_animation
    let js_value = serde_wasm_bindgen::to_value(&animation).unwrap();

    // Call load_animation
    let uuid_str = animation_player::load_animation(js_value);

    // Check that we got a valid UUID back
    assert!(!uuid_str.is_empty());

    // Parse the UUID to ensure it's valid
    let _uuid = Uuid::parse_str(&uuid_str).expect("Failed to parse UUID");

    // Use the helper function to retrieve the animation and verify it
    let js_value = animation_player::get_animation(&uuid_str);
    assert!(!js_value.is_null(), "Animation should be stored");

    // Convert to AnimationData
    let stored_animation: AnimationData =
        serde_wasm_bindgen::from_value(js_value).expect("Failed to deserialize animation data");
    assert_eq!(stored_animation.name, "Web Test Animation");
    assert_eq!(stored_animation.duration, 5.0);
    assert_eq!(stored_animation.id, "web-test-anim");

    // Additional check - UUID should be properly formatted
    assert_eq!(uuid_str.len(), 36, "UUID string should be 36 characters");
    assert_eq!(
        uuid_str.matches('-').count(),
        4,
        "UUID should contain 4 hyphens"
    );
}

#[wasm_bindgen_test]
fn test_unload_animation_in_browser() {
    // Create a test animation
    let mut transitions = HashMap::new();
    transitions.insert(
        "transition1".to_string(),
        AnimationTransition {
            id: "transition1".to_string(),
            keypoints: ("key1".to_string(), "key2".to_string()),
            variant: "linear".to_string(),
            parameters: HashMap::new(),
        },
    );

    let animation = AnimationData {
        id: "web-test-anim-unload".to_string(),
        name: "Web Test Animation for Unloading".to_string(),
        tracks: vec![],
        transitions,
        duration: 5.0,
    };

    // Convert to JsValue for passing to load_animation
    let js_value = serde_wasm_bindgen::to_value(&animation).unwrap();

    // Load the animation
    let uuid_str = animation_player::load_animation(js_value);

    // Parse the UUID
    let _uuid = Uuid::parse_str(&uuid_str).expect("Failed to parse UUID");

    // Verify the animation is loaded
    let js_value = animation_player::get_animation(&uuid_str);
    assert!(!js_value.is_null(), "Animation should exist");

    // Unload the animation
    let result = animation_player::unload_animation(&uuid_str);

    // Verify animation was successfully unloaded
    assert!(result, "Animation should have been unloaded");
    let js_value_after = animation_player::get_animation(&uuid_str);
    assert!(
        js_value_after.is_null(),
        "Animation should not be retrievable after unloading"
    );

    // Try to unload again (should return false)
    let result = animation_player::unload_animation(&uuid_str);
    assert!(!result, "Second unload attempt should return false");

    // Try to unload with invalid UUID
    let result = animation_player::unload_animation("not-a-valid-uuid");
    assert!(!result, "Unloading with invalid UUID should return false");
}

#[wasm_bindgen_test]
fn test_get_animation_in_browser() {
    // Create a test animation
    let mut transitions = HashMap::new();
    transitions.insert(
        "transition1".to_string(),
        AnimationTransition {
            id: "transition1".to_string(),
            keypoints: ("key1".to_string(), "key2".to_string()),
            variant: "linear".to_string(),
            parameters: HashMap::new(),
        },
    );

    let animation = AnimationData {
        id: "web-test-anim-get".to_string(),
        name: "Web Test Animation for Get".to_string(),
        tracks: vec![],
        transitions,
        duration: 4.5,
    };

    // Convert to JsValue for passing to load_animation
    let js_value = serde_wasm_bindgen::to_value(&animation).unwrap();

    // Load the animation
    let uuid_str = animation_player::load_animation(js_value);

    // Get the animation using the new WebAssembly function
    let retrieved_js_value = animation_player::get_animation(&uuid_str);

    // Check that we got a non-null value back
    assert!(
        !retrieved_js_value.is_null(),
        "get_animation should not return null for a valid animation"
    );

    // Convert back to AnimationData
    let retrieved_animation: AnimationData =
        serde_wasm_bindgen::from_value(retrieved_js_value).expect("Failed to deserialize");

    // Verify it's the same animation
    assert_eq!(retrieved_animation.id, "web-test-anim-get");
    assert_eq!(retrieved_animation.name, "Web Test Animation for Get");
    assert_eq!(retrieved_animation.duration, 4.5);

    // Try getting with an invalid UUID
    let null_result = animation_player::get_animation("not-a-valid-uuid");
    assert!(
        null_result.is_null(),
        "get_animation should return null for an invalid UUID"
    );

    // Try getting with a non-existent UUID
    let random_uuid = Uuid::new_v4().to_string();
    let null_result2 = animation_player::get_animation(&random_uuid);
    assert!(
        null_result2.is_null(),
        "get_animation should return null for a non-existent UUID"
    );
}
