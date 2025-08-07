use crate::ecs::AnimationPlayerPlugin;
use bevy::prelude::*;

use crate::{
    ecs::resources::{AnimationOutput, IdMapping},
    AnimationData,
};
use bevy::prelude::*;
/// A WebAssembly-compatible wrapper for the `AnimationEngine`.
///
/// This struct provides the main interface for creating and managing the animation engine
/// from a WebAssembly environment.
use bevy::reflect::TypePath;
use wasm_bindgen::prelude::*;

use super::WasmAnimationEngine;

#[wasm_bindgen]
impl WasmAnimationEngine {
    /// Loads animation data from a JSON string.
    #[wasm_bindgen(js_name = loadAnimation)]
    pub fn load_animation(&mut self, animation_json: &str) -> Result<String, JsValue> {
        let animation_data: AnimationData = serde_json::from_str(animation_json)
            .map_err(|e| JsValue::from_str(&format!("Animation JSON parse error: {}", e)))?;

        let mut assets = self.app.world.resource_mut::<Assets<AnimationData>>();
        let handle = assets.add(animation_data);

        let mut id_mapping = self.app.world.resource_mut::<IdMapping>();
        let id = uuid::Uuid::new_v4().to_string();
        id_mapping.animations.insert(id.clone(), handle);

        Ok(id)
    }

    /// Updates the animation engine by a given time delta and returns the current animation values.
    #[wasm_bindgen]
    pub fn update(&mut self, frame_delta_seconds: f64) -> Result<JsValue, JsValue> {
        let mut time = self.app.world.resource_mut::<Time>();
        time.advance_by(std::time::Duration::from_secs_f64(frame_delta_seconds));

        self.app.update();

        let output = self.app.world.resource::<AnimationOutput>();
        serde_wasm_bindgen::to_value(&output.values)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }
}
