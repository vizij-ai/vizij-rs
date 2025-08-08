use crate::ecs::plugin::AnimationPlayerPlugin;
use crate::ecs::plugin::AnimationPlayerPlugin;
use crate::{
    ecs::resources::{AnimationOutput, EngineTime, IdMapping},
    event::AnimationEvent,
    AnimationData,
};
use crate::{
    ecs::resources::{AnimationOutput, EngineTime, IdMapping},
    event::AnimationEvent,
    AnimationData,
};
use bevy::asset::AssetPlugin;
use bevy::{core::CorePlugin, prelude::*};
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
    pub fn new() -> WasmAnimationEngine {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_plugins((CorePlugin, AssetPlugin::default(), AnimationPlayerPlugin));
        WasmAnimationEngine { app }
    }

    /// Loads animation data from a JSON string and returns a unique identifier.
    #[wasm_bindgen(js_name = loadAnimation)]
    pub fn load_animation(&mut self, animation_json: &str) -> Result<String, JsValue> {
        let animation_data: AnimationData = serde_json::from_str(animation_json)
            .map_err(|e| JsValue::from_str(&format!("Animation JSON parse error: {}", e)))?;

        let mut assets = self.app.world_mut().resource_mut::<Assets<AnimationData>>();
        let handle = assets.add(animation_data);

        let mut id_mapping = self.app.world_mut().resource_mut::<IdMapping>();
        let id = uuid::Uuid::new_v4().to_string();
        id_mapping.animations.insert(id.clone(), handle);

        Ok(id)
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

    /// Drains animation events produced by the engine.
    #[wasm_bindgen(js_name = drainEvents)]
    pub fn drain_events(&mut self) -> Result<JsValue, JsValue> {
        let mut events = self.app.world.resource_mut::<Events<AnimationEvent>>();
        let drained: Vec<AnimationEvent> = events.drain().collect();
        serde_wasm_bindgen::to_value(&drained)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }
}
