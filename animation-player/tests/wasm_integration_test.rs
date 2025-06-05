//! Integration tests for WASM bindings

#[cfg(target_arch = "wasm32")]
mod wasm_tests {
    use animation_player::wasm::*;
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
                "duration": { "nanos": 2000000000 },
                "frame_rate": 60.0
            },
            "tracks": {}
        }"#;

        // Load animation
        let animation_id = engine.load_animation(animation_json).unwrap();

        // Create player
        let player_id = engine.create_player();

        // Add instance
        assert!(engine.add_instance(&player_id, &animation_id).is_ok());

        // Test playback controls
        assert!(engine.play(&player_id).is_ok());
        assert_eq!(engine.get_player_state(&player_id).unwrap(), "playing");

        assert!(engine.pause(&player_id).is_ok());
        assert_eq!(engine.get_player_state(&player_id).unwrap(), "paused");

        assert!(engine.stop(&player_id).is_ok());
        assert_eq!(engine.get_player_state(&player_id).unwrap(), "stopped");
    }

    #[wasm_bindgen_test]
    fn test_wasm_utility_functions() {
        let value_json = r#"{"Float": 42.5}"#;
        assert!(value_to_js(value_json).is_ok());
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod non_wasm_tests {
    #[test]
    fn test_placeholder() {
        // Placeholder test for non-WASM builds
        assert!(true);
    }
}
