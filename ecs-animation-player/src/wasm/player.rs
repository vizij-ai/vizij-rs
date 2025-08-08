use crate::ecs::components::AnimationPlayer;
use crate::ecs::resources::IdMapping;
use bevy::prelude::*;
use std::collections::HashSet;
use wasm_bindgen::prelude::*;

use super::WasmAnimationEngine;

#[wasm_bindgen]
impl WasmAnimationEngine {
    /// Creates a new animation player and returns its unique ID.
    #[wasm_bindgen(js_name = createPlayer)]
    pub fn create_player(&mut self) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let entity = self.app.world_mut().spawn(AnimationPlayer::default()).id();

        let mut id_mapping = self.app.world_mut().resource_mut::<IdMapping>();
        id_mapping.players.insert(id.clone(), entity);

        id
    }

    /// Removes a player and all of its instances.
    #[wasm_bindgen(js_name = removePlayer)]
    pub fn remove_player(&mut self, player_id: &str) -> Result<(), JsValue> {
        let entity = {
            let mut id_mapping = self.app.world.resource_mut::<IdMapping>();
            id_mapping
                .players
                .remove(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        // Collect all descendant entities to clean up instance mappings
        let mut entities: HashSet<Entity> = HashSet::new();
        let mut stack = vec![entity];
        entities.insert(entity);
        while let Some(e) = stack.pop() {
            if let Some(children) = self.app.world.get::<Children>(e) {
                for &child in children.iter() {
                    if entities.insert(child) {
                        stack.push(child);
                    }
                }
            }
        }

        {
            let mut id_mapping = self.app.world.resource_mut::<IdMapping>();
            id_mapping.instances.retain(|_, e| !entities.contains(e));
        }

        self.app.world.despawn_recursive(entity);
        Ok(())
    }

    /// Sets the target root entity for a player.
    #[wasm_bindgen(js_name = setPlayerRoot)]
    pub fn set_player_root(&mut self, player_id: &str, entity_id: &str) -> Result<(), JsValue> {
        let player_entity = {
            let id_mapping = self.app.world.resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        let bits = entity_id
            .parse::<u64>()
            .map_err(|e| JsValue::from_str(&format!("Invalid entity ID: {}", e)))?;
        let target_entity = Entity::from_bits(bits);

        if let Some(mut player) = self.app.world.get_mut::<AnimationPlayer>(player_entity) {
            player.target_root = Some(target_entity);
        }

        Ok(())
    }

    /// Starts playback for a player.
    #[wasm_bindgen]
    pub fn play(&mut self, player_id: &str) -> Result<(), JsValue> {
        let entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        if let Some(mut player) = self.app.world_mut().get_mut::<AnimationPlayer>(entity) {
            player.playback_state = crate::PlaybackState::Playing;
        }

        Ok(())
    }

    /// Pauses playback for a player.
    #[wasm_bindgen]
    pub fn pause(&mut self, player_id: &str) -> Result<(), JsValue> {
        let entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        if let Some(mut player) = self.app.world_mut().get_mut::<AnimationPlayer>(entity) {
            player.playback_state = crate::PlaybackState::Paused;
        }

        Ok(())
    }

    /// Stops playback for a player and resets its time to the beginning.
    #[wasm_bindgen]
    pub fn stop(&mut self, player_id: &str) -> Result<(), JsValue> {
        let entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        if let Some(mut player) = self.app.world_mut().get_mut::<AnimationPlayer>(entity) {
            player.playback_state = crate::PlaybackState::Stopped;
            player.current_time = crate::AnimationTime::zero();
        }

        Ok(())
    }

    /// Seeks a player to a specific time in seconds.
    #[wasm_bindgen]
    pub fn seek(&mut self, player_id: &str, time_seconds: f64) -> Result<(), JsValue> {
        let entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        if let Some(mut player) = self.app.world_mut().get_mut::<AnimationPlayer>(entity) {
            player.current_time = crate::AnimationTime::from_seconds(time_seconds)
                .map_err(|e| JsValue::from_str(&format!("Invalid time: {:?}", e)))?;
        }

        Ok(())
    }

    /// Removes a player and all of its instances.
    #[wasm_bindgen(js_name = removePlayer)]
    pub fn remove_player(&mut self, player_id: &str) -> Result<(), JsValue> {
        // Remove player ID and fetch entity
        let entity = {
            let mut id_mapping = self.app.world.resource_mut::<IdMapping>();
            id_mapping
                .players
                .remove(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        // Collect child entities for instance ID cleanup
        let children: Vec<Entity> = self
            .app
            .world
            .get::<Children>(entity)
            .map(|c| c.iter().copied().collect())
            .unwrap_or_default();

        // Remove instance IDs associated with this player
        {
            let mut id_mapping = self.app.world.resource_mut::<IdMapping>();
            id_mapping.instances.retain(|_, e| !children.contains(e));
        }

        // Despawn the player and its children
        let _ = self
            .app
            .world
            .entity_mut(entity)
            .despawn_recursive();

        Ok(())
    }
}
