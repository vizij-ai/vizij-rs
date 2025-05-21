use uuid::Uuid;
use wasm_bindgen::prelude::*;

use crate::animation_data::AnimationData;
use crate::animation_player;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen(start)]
pub fn start() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(all(feature = "console_error_panic_hook", target_arch = "wasm32"))]
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn greet() -> String {
    let greeting = "Hello, animation-player!";
    alert(greeting);
    greeting.to_string()
}

/// Loads an animation from a JsValue (for WebAssembly usage)
///
/// This function stores the animation data in a static HashMap keyed by a
/// generated UUID, which can be used to reference the animation later.
#[wasm_bindgen]
pub fn load_animation(data: JsValue) -> String {
    // Convert JsValue to AnimationData
    let animation_data: AnimationData =
        serde_wasm_bindgen::from_value(data).expect("Failed to deserialize animation data");

    // Store the animation and get UUID
    let id = animation_player::load_animation(animation_data);

    // Return the UUID as a string
    id.to_string()
}

/// Unloads (removes) an animation by its UUID
///
/// Returns true if the animation was found and removed, false otherwise
#[wasm_bindgen]
pub fn unload_animation(uuid_str: &str) -> bool {
    // Parse the UUID from the string
    match Uuid::parse_str(uuid_str) {
        Ok(uuid) => {
            // Unload the animation and return the result
            animation_player::unload_animation(&uuid)
        }
        Err(_) => {
            // Invalid UUID format
            false
        }
    }
}

/// Retrieves an animation by its UUID string
///
/// This function looks up an animation in the static HashMap by UUID
/// and returns it as a JsValue if found, or null if not found.
#[wasm_bindgen]
pub fn get_animation(uuid_str: &str) -> JsValue {
    // Parse the UUID from the string
    match Uuid::parse_str(uuid_str) {
        Ok(uuid) => {
            // Retrieve the animation using the core function
            match animation_player::get_animation(&uuid) {
                Some(animation) => {
                    // Serialize the animation to JsValue
                    serde_wasm_bindgen::to_value(&animation).unwrap_or(JsValue::NULL)
                }
                None => {
                    // Animation not found
                    JsValue::NULL
                }
            }
        }
        Err(_) => {
            // Invalid UUID format
            JsValue::NULL
        }
    }
}

#[cfg(test)]
#[cfg(target_arch = "wasm32")]
mod wasm_tests {
    use super::*;
    use crate::animation_data::AnimationTransition;
    use std::collections::HashMap;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_load_animation_wasm() {
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
            id: "test-anim-wasm".to_string(),
            name: "WASM Test Animation".to_string(),
            tracks: vec![],
            transitions,
            duration: 5.0,
        };

        // Convert to JsValue
        let js_value = serde_wasm_bindgen::to_value(&animation).unwrap();

        // Call load_animation
        let uuid_str = load_animation(js_value);

        // Parse the UUID to ensure it's valid
        let uuid = Uuid::parse_str(&uuid_str).expect("Failed to parse UUID");

        // Retrieve and check the animation
        let stored_animation =
            animation_player::get_animation(&uuid).expect("Animation should be stored");
        assert_eq!(stored_animation.name, "WASM Test Animation");
        assert_eq!(stored_animation.duration, 5.0);
        assert_eq!(stored_animation.id, "test-anim-wasm");
    }

    #[wasm_bindgen_test]
    fn test_unload_animation() {
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
            id: "test-anim-unload".to_string(),
            name: "Test Animation for Unloading".to_string(),
            tracks: vec![],
            transitions,
            duration: 5.0,
        };

        // Convert to JsValue
        let js_value = serde_wasm_bindgen::to_value(&animation).unwrap();

        // Load the animation
        let uuid_str = load_animation(js_value);

        // Verify the animation is loaded
        let uuid = Uuid::parse_str(&uuid_str).expect("Failed to parse UUID");
        assert!(animation_player::get_animation(&uuid).is_some());

        // Unload the animation
        let result = unload_animation(&uuid_str);

        // Verify the animation was successfully unloaded
        assert!(result);
        assert!(animation_player::get_animation(&uuid).is_none());

        // Try to unload again (should return false as it no longer exists)
        let result = unload_animation(&uuid_str);
        assert!(!result);

        // Try to unload with invalid UUID
        let result = unload_animation("not-a-valid-uuid");
        assert!(!result);
    }

    #[wasm_bindgen_test]
    fn test_get_animation_wasm() {
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
            id: "test-anim-get".to_string(),
            name: "WASM Test Animation for Get".to_string(),
            tracks: vec![],
            transitions,
            duration: 3.5,
        };

        // Convert to JsValue
        let js_value = serde_wasm_bindgen::to_value(&animation).unwrap();

        // Load the animation
        let uuid_str = load_animation(js_value);

        // Get the animation using get_animation
        let retrieved_js_value = get_animation(&uuid_str);

        // Check that we got a non-null value back
        assert!(!retrieved_js_value.is_null());

        // Convert back to AnimationData
        let retrieved_animation: AnimationData =
            serde_wasm_bindgen::from_value(retrieved_js_value).expect("Failed to deserialize");

        // Verify it's the same animation
        assert_eq!(retrieved_animation.id, "test-anim-get");
        assert_eq!(retrieved_animation.name, "WASM Test Animation for Get");
        assert_eq!(retrieved_animation.duration, 3.5);

        // Try getting with an invalid UUID
        let null_result = get_animation("not-a-valid-uuid");
        assert!(null_result.is_null());

        // Try getting with a non-existent UUID
        let random_uuid = Uuid::new_v4().to_string();
        let null_result2 = get_animation(&random_uuid);
        assert!(null_result2.is_null());
    }
}
