use crate::ecs::components::AnimationInstance;
use crate::ecs::resources::IdMapping;
use bevy::prelude::*;
use wasm_bindgen::prelude::*;

use super::WasmAnimationEngine;

#[wasm_bindgen]
impl WasmAnimationEngine {
    /// Adds an animation instance to a player, with optional configuration.
    #[wasm_bindgen(js_name = addInstance)]
    pub fn add_instance(
        &mut self,
        player_id: &str,
        animation_id: &str,
        config_json: Option<String>,
    ) -> Result<String, JsValue> {
        let id_mapping = self.app.world.resource::<IdMapping>();
        let player_entity = id_mapping
            .players
            .get(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        let animation_handle = id_mapping
            .animations
            .get(animation_id)
            .ok_or_else(|| JsValue::from_str("Animation not found"))?;

        let settings: crate::AnimationInstanceSettings = if let Some(json) = config_json {
            serde_json::from_str(&json)
                .map_err(|e| JsValue::from_str(&format!("Instance config parse error: {}", e)))?
        } else {
            Default::default()
        };

        let instance_component = AnimationInstance {
            animation: animation_handle.clone(),
            weight: settings.weight as f32,
            time_scale: settings.time_scale as f32,
            start_time: settings.instance_start_time,
        };

        let instance_entity = self.app.world.spawn(instance_component).id();
        self.app
            .world
            .entity_mut(*player_entity)
            .add_child(instance_entity);

        let mut id_mapping = self.app.world.resource_mut::<IdMapping>();
        let id = uuid::Uuid::new_v4().to_string();
        id_mapping.instances.insert(id.clone(), instance_entity);

        Ok(id)
    }

    /// Removes an animation instance from the world.
    #[wasm_bindgen(js_name = removeInstance)]
    pub fn remove_instance(&mut self, instance_id: &str) -> Result<(), JsValue> {
        let entity = {
            let mut id_mapping = self.app.world.resource_mut::<IdMapping>();
            id_mapping
                .instances
                .remove(instance_id)
                .ok_or_else(|| JsValue::from_str("Instance not found"))?
        };

        self.app.world.despawn(entity);
        Ok(())
    }

    /// Updates the configuration of an existing animation instance.
    #[wasm_bindgen(js_name = updateInstanceConfig)]
    pub fn update_instance_config(
        &mut self,
        _player_id: &str,
        instance_id: &str,
        config_json: &str,
    ) -> Result<(), JsValue> {
        let id_mapping = self.app.world.resource::<IdMapping>();
        let entity = id_mapping
            .instances
            .get(instance_id)
            .ok_or_else(|| JsValue::from_str("Instance not found"))?;

        let config: serde_json::Value = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("Config JSON parse error: {}", e)))?;

        if let Some(mut instance) = self.app.world.get_mut::<AnimationInstance>(*entity) {
            if let Some(weight) = config.get("weight").and_then(|v| v.as_f64()) {
                instance.weight = weight as f32;
            }
            if let Some(time_scale) = config.get("timeScale").and_then(|v| v.as_f64()) {
                instance.time_scale = time_scale as f32;
            }
            if let Some(start_time) = config.get("instanceStartTime").and_then(|v| v.as_f64()) {
                instance.start_time = crate::AnimationTime::from_seconds(start_time)
                    .map_err(|e| JsValue::from_str(&format!("Invalid start time: {:?}", e)))?;
            }
        }

        Ok(())
    }

    /// Removes an animation instance from its player.
    #[wasm_bindgen(js_name = removeInstance)]
    pub fn remove_instance(
        &mut self,
        _player_id: &str,
        instance_id: &str,
    ) -> Result<(), JsValue> {
        // Remove instance ID and get entity
        let entity = {
            let mut id_mapping = self.app.world.resource_mut::<IdMapping>();
            id_mapping
                .instances
                .remove(instance_id)
                .ok_or_else(|| JsValue::from_str("Instance not found"))?
        };

        // Despawn the instance entity
        let _ = self
            .app
            .world
            .entity_mut(entity)
            .despawn_recursive();

        Ok(())
    }
}
