//! Animation data management for WebAssembly.
use crate::{
    animation::AnimationInstanceSettings, AnimationBaking, AnimationData, AnimationTime,
    BakingConfig,
};
use crate::loaders::load_test_animation_from_json;
use super::engine::WasmAnimationEngine;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl WasmAnimationEngine {
    /// Loads animation data from a JSON string.
    ///
    /// This function parses a JSON string representing an `AnimationData` object and loads it
    /// into the engine. It also has a fallback to a test animation loader for compatibility.
    ///
    /// @param {string} animation_json - The animation data in JSON format. The JSON structure is:
    /// ```json
    /// {
    ///   "id": "string",
    ///   "name": "string",
    ///   "tracks": [
    ///     {
    ///       "name": "string",
    ///       "property": "string",
    ///       "keypoints": [
    ///         { "time": "float", "value": "Value", "transition": "TransitionVariant" }
    ///       ]
    ///     }
    ///   ]
    /// }
    /// ```
    /// @returns {string} The ID of the loaded animation.
    #[wasm_bindgen]
    pub fn load_animation(&mut self, animation_json: &str) -> Result<String, JsValue> {
        let animation_data: AnimationData = match serde_json::from_str(animation_json) {
            Ok(data) => data,
            Err(parse_error) => {
                // Fallback for test animations
                match load_test_animation_from_json_wasm(animation_json) {
                    Ok(corrected_json) => serde_json::from_str(&corrected_json).map_err(|e| {
                        JsValue::from_str(&format!("Fallback JSON parse error: {}", e))
                    })?,
                    Err(fallback_error) => {
                        return Err(JsValue::from_str(&format!(
                            "Animation JSON parse error: {}. Fallback loader error: {:?}",
                            parse_error, fallback_error
                        )));
                    }
                }
            }
        };
        let animation_id = self
            .engine
            .load_animation_data(animation_data)
            .map_err(|e| JsValue::from_str(&format!("Load animation error: {:?}", e)))?;

        Ok(animation_id)
    }

    /// Returns a list of all loaded animation IDs.
    ///
    /// # Example
    ///
    /// ```javascript
    /// const ids = engine.animation_ids();
    /// console.log(ids); // ["anim_01", "anim_02"]
    /// ```
    ///
    /// @returns {string[]} An array of animation IDs.
    #[wasm_bindgen]
    pub fn animation_ids(&mut self) -> Vec<String> {
        self.engine
            .animation_ids()
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// Adds an animation instance to a player, with optional configuration.
    ///
    /// @param {string} player_id - The ID of the player.
    /// @param {string} animation_id - The ID of the animation to add.
    /// @param {string} [config_json] - Optional JSON configuration for the instance.
    /// ```json
    /// {
    ///   "weight": "float",
    ///   "timeScale": "float",
    ///   "enabled": "boolean"
    /// }
    /// ```
    /// @returns {string} The ID of the new animation instance.
    #[wasm_bindgen]
    pub fn add_instance(
        &mut self,
        player_id: &str,
        animation_id: &str,
        config_json: Option<String>,
    ) -> Result<String, JsValue> {
        let config = if let Some(json) = config_json {
            serde_json::from_str::<AnimationInstanceSettings>(&json)
                .map_err(|e| JsValue::from_str(&format!("Instance config parse error: {}", e)))?
        } else {
            AnimationInstanceSettings::default()
        };

        self.engine
            .add_animation_to_player(player_id, animation_id, Some(config))
            .map_err(|e| JsValue::from_str(&format!("Engine error: {}", e)))
    }

    /// Updates the configuration of an existing animation instance.
    ///
    /// @param {string} player_id - The ID of the player containing the instance.
    /// @param {string} instance_id - The ID of the animation instance to update.
    /// @param {string} config_json - JSON configuration with the fields to update.
    /// ```json
    /// {
    ///   "weight": "float",
    ///   "timeScale": "float",
    ///   "enabled": "boolean"
    /// }
    /// ```
    #[wasm_bindgen]
    pub fn update_instance_config(
        &mut self,
        player_id: &str,
        instance_id: &str,
        config_json: &str,
    ) -> Result<(), JsValue> {
        let player = self
            .engine
            .get_player_mut(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        let config: serde_json::Value = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("Config JSON parse error: {}", e)))?;

        if let Some(weight) = config.get("weight").and_then(|v| v.as_f64()) {
            player
                .set_instance_weight(instance_id, weight as f32)
                .map_err(|e| JsValue::from_str(&format!("Set weight error: {}", e)))?;
        }
        
        if let Some(time_scale) = config.get("timeScale").and_then(|v| v.as_f64()) {
            player
                .set_instance_time_scale(instance_id, time_scale as f32)
                .map_err(|e| JsValue::from_str(&format!("Set time scale error: {}", e)))?;
        }
        
        if let Some(enabled) = config.get("enabled").and_then(|v| v.as_bool()) {
            player
                .set_instance_enabled(instance_id, enabled)
                .map_err(|e| JsValue::from_str(&format!("Set enabled error: {}", e)))?;
        }

        Ok(())
    }

    /// Exports a loaded animation as a JSON string.
    ///
    /// # Example
    ///
    /// ```javascript
    /// const json = engine.export_animation("anim_01");
    /// console.log(json);
    /// ```
    ///
    /// @param {string} animation_id - The ID of the animation to export.
    /// @returns {string} The animation data as a JSON string.
    #[wasm_bindgen]
    pub fn export_animation(&self, animation_id: &str) -> Result<String, JsValue> {
        let animation = self
            .engine
            .get_animation_data(animation_id)
            .ok_or_else(|| JsValue::from_str("Animation not found"))?;

        serde_json::to_string(animation)
            .map_err(|e| JsValue::from_str(&format!("Export error: {}", e)))
    }

    /// Calculates the derivatives (rates of change) for all tracks of a player at the current time.
    ///
    /// The `derivative_width_ms` is the time window in milliseconds over which to calculate the rate of change.
    ///
    /// # Example
    ///
    /// ```javascript
    /// const derivatives = engine.get_derivatives("player_12345", 16.67); // ~1 frame at 60fps
    /// console.log(derivatives);
    /// // { "object.position.x": 6000, ... }
    /// ```
    ///
    /// @param {string} player_id - The ID of the player.
    /// @param {number} [derivative_width_ms] - The time window in milliseconds.
    /// @returns {any} A JSON object mapping property paths to their derivative values.
    /// ```json
    /// {
    ///   "property.path": "float"
    /// }
    /// ```
    #[wasm_bindgen]
    pub fn get_derivatives(
        &mut self,
        player_id: &str,
        derivative_width_ms: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let derivative_width =
            if let Some(width_ms) = derivative_width_ms {
                if width_ms <= 0.0 {
                    return Err(JsValue::from_str("Derivative width must be positive"));
                }
                Some(AnimationTime::from_millis(width_ms).map_err(|e| {
                    JsValue::from_str(&format!("Invalid derivative width: {:?}", e))
                })?)
            } else {
                None
            };

        let derivatives = self
            .engine
            .calculate_player_derivatives(player_id, derivative_width)
            .map_err(|e| JsValue::from_str(&format!("Calculate derivatives error: {:?}", e)))?;

        serde_wasm_bindgen::to_value(&derivatives)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Bakes an animation into a series of pre-calculated values at a specified frame rate.
    ///
    /// @param {string} animation_id - The ID of the animation to bake.
    /// @param {string} [config_json] - Optional baking configuration.
    /// ```json
    /// {
    ///   "frameRate": "float",
    ///   "startTime": "float",
    ///   "endTime": "float"
    /// }
    /// ```
    /// @returns {string} The baked animation data as a JSON string.
    #[wasm_bindgen]
    pub fn bake_animation(
        &mut self,
        animation_id: &str,
        config_json: Option<String>,
    ) -> Result<String, JsValue> {
        let animation = self
            .engine
            .get_animation_data(animation_id)
            .ok_or_else(|| JsValue::from_str("Animation not found"))?
            .clone();

        let config = if let Some(json) = config_json {
            serde_json::from_str::<BakingConfig>(&json)
                .map_err(|e| JsValue::from_str(&format!("Baking config parse error: {}", e)))?
        } else {
            BakingConfig::default()
        };

        let baked_data = animation
            .bake(&config, self.engine.interpolation_registry_mut())
            .map_err(|e| JsValue::from_str(&format!("Baking error: {:?}", e)))?;

        baked_data
            .to_json()
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {:?}", e)))
    }
}

/// Loads a test animation from a JSON string and returns it as a JSON string.
#[wasm_bindgen]
pub fn load_test_animation_from_json_wasm(json_str: &str) -> Result<String, JsValue> {
    load_test_animation_from_json(json_str)
        .map_err(|e| JsValue::from_str(&format!("Test animation load error: {:?}", e)))
        .and_then(|animation| {
            serde_json::to_string(&animation)
                .map_err(|e| JsValue::from_str(&format!("Animation serialization error: {}", e)))
        })
}
