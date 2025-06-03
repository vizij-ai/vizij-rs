//! Integration tests for WASM bindings

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
mod wasm_tests {
    use animation_player::wasm::*;
    use animation_player::*;
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
        assert!(engine.load_animation(animation_json).is_ok());

        // Create player
        assert!(engine.create_player("test_player").is_ok());

        // Add instance
        assert!(engine
            .add_instance("test_player", "instance1", "test_animation")
            .is_ok());

        // Test playback controls
        assert!(engine.play("test_player").is_ok());
        assert_eq!(engine.get_player_state("test_player").unwrap(), "playing");

        assert!(engine.pause("test_player").is_ok());
        assert_eq!(engine.get_player_state("test_player").unwrap(), "paused");

        assert!(engine.stop("test_player").is_ok());
        assert_eq!(engine.get_player_state("test_player").unwrap(), "stopped");
    }

    #[wasm_bindgen_test]
    fn test_wasm_utility_functions() {
        assert!(!get_version().is_empty());

        let value_json = r#"{"Float": 42.5}"#;
        assert!(value_to_js(value_json).is_ok());
    }
}

#[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
mod non_wasm_tests {
    #[test]
    fn test_placeholder() {
        // Placeholder test for non-WASM builds
        assert!(true);
    }
}
