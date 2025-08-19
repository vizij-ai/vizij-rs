use crate::ecs::plugin::AnimationPlayerPlugin;
use crate::{
    ecs::resources::{AnimationOutput, EngineTime, IdMapping, EngineConfigEcs},
    event::AnimationEvent,
    AnimationData,
    config::AnimationEngineConfig,
    loaders::studio_animation::load_test_animation_from_json,
};
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use wasm_bindgen::prelude::*;

/// A WebAssembly-compatible animation engine backed by a Bevy [`App`].
#[wasm_bindgen]
pub struct WasmAnimationEngine {
    /// The internal Bevy [`App`] driving the animation systems.
    #[wasm_bindgen(skip)]
    pub app: App,
}

#[wasm_bindgen]
impl WasmAnimationEngine {
    /// Creates a new [`WasmAnimationEngine`].
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: Option<String>) -> Result<WasmAnimationEngine, JsValue> {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), AnimationPlayerPlugin));

        let config = if let Some(json) = config_json {
            serde_json::from_str::<AnimationEngineConfig>(&json)
                .map_err(|e| JsValue::from_str(&format!("Config parse error: {}", e)))?
        } else {
            AnimationEngineConfig::web_optimized()
        };
        app.insert_resource(EngineConfigEcs(config));

        Ok(WasmAnimationEngine { app })
    }

    /// Loads animation data from a JSON string and returns a unique identifier.
    /// This supports both native AnimationData JSON and the test_animation.json fallback format.
    #[wasm_bindgen]
    pub fn load_animation(&mut self, animation_json: &str) -> Result<String, JsValue> {
        // Try to parse as native AnimationData first
        let animation_data: AnimationData = match serde_json::from_str(animation_json) {
            Ok(data) => data,
            Err(primary_err) => {
                // Fallback: try to interpret as test_animation.json and convert
                match load_test_animation_from_json(animation_json) {
                    Ok(converted) => converted,
                    Err(fallback_err) => {
                        return Err(JsValue::from_str(&format!(
                            "Animation JSON parse error: {}. Fallback loader error: {}",
                            primary_err, fallback_err
                        )))
                    }
                }
            }
        };

        let handle = {
            let mut assets = self.app.world_mut().resource_mut::<Assets<AnimationData>>();
            assets.add(animation_data)
        };

        let id = uuid::Uuid::new_v4().to_string();
        {
            let mut id_mapping = self.app.world_mut().resource_mut::<IdMapping>();
            id_mapping.animations.insert(id.clone(), handle);
        }

        Ok(id)
    }

    /// Loads animation data (camelCase alias).
    #[wasm_bindgen(js_name = loadAnimation)]
    pub fn load_animation_camel(&mut self, animation_json: &str) -> Result<String, JsValue> {
        self.load_animation(animation_json)
    }

    /// Unloads animation data from the engine.
    #[wasm_bindgen]
    pub fn unload_animation(&mut self, animation_id: &str) -> Result<(), JsValue> {
        let handle = {
            let mut id_mapping = self.app.world_mut().resource_mut::<IdMapping>();
            id_mapping
                .animations
                .remove(animation_id)
                .ok_or_else(|| JsValue::from_str("Animation not found"))?
        };

        // Remove the asset from the asset storage
        {
            let mut assets = self
                .app
                .world_mut()
                .resource_mut::<Assets<AnimationData>>();
            assets.remove(&handle);
        }

        Ok(())
    }

    /// Unloads animation data (camelCase alias).
    #[wasm_bindgen(js_name = unloadAnimation)]
    pub fn unload_animation_camel(&mut self, animation_id: &str) -> Result<(), JsValue> {
        self.unload_animation(animation_id)
    }

    /// Returns list of loaded animation IDs.
    #[wasm_bindgen]
    pub fn animation_ids(&mut self) -> Vec<String> {
        let id_mapping = self.app.world().resource::<IdMapping>();
        id_mapping.animations.keys().cloned().collect()
    }

    /// Returns list of loaded animation IDs (camelCase alias).
    #[wasm_bindgen(js_name = animationIds)]
    pub fn animation_ids_camel(&mut self) -> Vec<String> {
        self.animation_ids()
    }

    /// Advances the engine and returns current animation values.
    #[wasm_bindgen]
    pub fn update(&mut self, frame_delta_seconds: f64) -> Result<JsValue, JsValue> {
        {
            let mut engine_time = self.app.world_mut().resource_mut::<EngineTime>();
            engine_time.delta_seconds = frame_delta_seconds;
            engine_time.elapsed_seconds += frame_delta_seconds;
        }

        self.app.update();

        // Clear delta after update to prevent accidental reuse across multiple updates
        {
            let mut engine_time = self.app.world_mut().resource_mut::<EngineTime>();
            engine_time.delta_seconds = 0.0;
        }

        let output = self.app.world().resource::<AnimationOutput>();
        serde_wasm_bindgen::to_value(&output.values)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Returns the current engine configuration as a JSON object.
    #[wasm_bindgen]
    pub fn get_engine_config(&self) -> JsValue {
        let cfg = self.app.world().resource::<EngineConfigEcs>().0.clone();
        serde_wasm_bindgen::to_value(&cfg).unwrap_or(JsValue::NULL)
    }

    /// Returns the current engine configuration as a JSON object. (camelCase alias)
    #[wasm_bindgen(js_name = getEngineConfig)]
    pub fn get_engine_config_camel(&self) -> JsValue {
        self.get_engine_config()
    }

    /// Sets the engine configuration from a JSON string.
    #[wasm_bindgen]
    pub fn set_engine_config(&mut self, config_json: &str) -> Result<(), JsValue> {
        let config: AnimationEngineConfig = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("Config parse error: {}", e)))?;
        self.app
            .world_mut()
            .insert_resource(EngineConfigEcs(config));
        Ok(())
    }

    /// Sets the engine configuration from a JSON string. (camelCase alias)
    #[wasm_bindgen(js_name = setEngineConfig)]
    pub fn set_engine_config_camel(&mut self, config_json: &str) -> Result<(), JsValue> {
        self.set_engine_config(config_json)
    }

    /// Drains animation events produced by the engine. (snake_case)
    #[wasm_bindgen]
    pub fn drain_events(&mut self) -> Result<JsValue, JsValue> {
        let mut events = self.app.world_mut().resource_mut::<Events<AnimationEvent>>();
        let drained: Vec<AnimationEvent> = events.drain().collect();
        serde_wasm_bindgen::to_value(&drained)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Drains animation events produced by the engine. (camelCase alias)
    #[wasm_bindgen(js_name = drainEvents)]
    pub fn drain_events_camel(&mut self) -> Result<JsValue, JsValue> {
        self.drain_events()
    }
}
