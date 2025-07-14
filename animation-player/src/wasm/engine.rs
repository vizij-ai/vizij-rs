//! The core WebAssembly animation engine.
use crate::{AnimationEngine, AnimationEngineConfig};
use std::time::Duration;
use wasm_bindgen::prelude::*;

/// A WebAssembly-compatible wrapper for the `AnimationEngine`.
///
/// This struct provides the main interface for creating and managing the animation engine
/// from a WebAssembly environment.
#[wasm_bindgen]
pub struct WasmAnimationEngine {
    pub(crate) engine: AnimationEngine,
}

#[wasm_bindgen]
impl WasmAnimationEngine {
    /// Creates a new `WasmAnimationEngine`.
    ///
    /// An optional JSON configuration string can be provided. If `None`, a default, web-optimized
    /// configuration is used.
    ///
    /// @param {string} [config_json] - Optional JSON configuration for the engine.
    /// ```json
    /// {
    ///   "timeStep": "float",
    ///   "maxUpdatesPerFrame": "integer"
    /// }
    /// # Example
    ///
    /// ```javascript
    /// // With default configuration
    /// const engine = new WasmAnimationEngine();
    ///
    /// // With custom configuration
    /// const config = {
    ///   "time_step": 0.016, // 60 FPS
    ///   "max_updates_per_frame": 10
    /// };
    /// const customEngine = new WasmAnimationEngine(JSON.stringify(config));
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: Option<String>) -> Result<WasmAnimationEngine, JsValue> {
        let config = if let Some(json) = config_json {
            serde_json::from_str::<AnimationEngineConfig>(&json)
                .map_err(|e| JsValue::from_str(&format!("Config parse error: {}", e)))?
        } else {
            AnimationEngineConfig::web_optimized()
        };

        let engine = AnimationEngine::new(config);

        Ok(WasmAnimationEngine { engine })
    }

    /// Updates the animation engine by a given time delta and returns the current animation values.
    ///
    /// The `frame_delta_seconds` is the time elapsed since the last update. This function
    /// should be called on every frame of the application's render loop.
    ///
    /// # Example
    ///
    /// ```javascript
    /// let lastTime = performance.now();
    ///
    /// function animate() {
    ///   const now = performance.now();
    ///   const delta = (now - lastTime) / 1000.0;
    ///   lastTime = now;
    ///
    ///   const values = engine.update(delta);
    ///   console.log(values); // { "target1.property": 1.23, "target2.property": 4.56 }
    ///
    ///   requestAnimationFrame(animate);
    /// }
    ///
    /// animate();
    /// ```
    ///
    /// @param {number} frame_delta_seconds - The time elapsed since the last frame in seconds.
    /// @returns {any} A JSON object mapping property paths to their current animated values.
    /// ```json
    /// {
    ///   "player_id": {
    ///     "property.path": "Value"
    ///   }
    /// }
    /// ```
    #[wasm_bindgen]
    pub fn update(&mut self, frame_delta_seconds: f64) -> Result<JsValue, JsValue> {
        let values = self
            .engine
            .update(Duration::from_secs_f64(frame_delta_seconds))
            .map_err(|e| JsValue::from_str(&format!("Update error: {:?}", e)))?;

        serde_wasm_bindgen::to_value(&values)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Returns the engine's performance metrics as a JSON object.
    ///
    /// @returns {any} A JSON object containing performance metrics.
    /// ```json
    /// {
    ///   "totalPlayers": "integer",
    ///   "activePlayers": "integer",
    ///   "updateTime": "float",
    ///   "renderTime": "float",
    ///   "totalTime": "float"
    /// }
    /// ```
    #[wasm_bindgen]
    pub fn get_metrics(&self) -> JsValue {
        let metrics = self.engine.metrics();
        serde_wasm_bindgen::to_value(metrics).unwrap_or(JsValue::NULL)
    }
}
