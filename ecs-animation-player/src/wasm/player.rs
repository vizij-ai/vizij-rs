use crate::ecs::components::AnimationPlayer;
use crate::ecs::resources::IdMapping;
use bevy::prelude::*;
use wasm_bindgen::prelude::*;

use super::WasmAnimationEngine;

#[wasm_bindgen]
impl WasmAnimationEngine {
    /// Creates a new animation player and returns its unique ID.
    #[wasm_bindgen(js_name = createPlayer)]
    pub fn create_player(&mut self) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let entity = self.app.world.spawn(AnimationPlayer::default()).id();

        let mut id_mapping = self.app.world.resource_mut::<IdMapping>();
        id_mapping.players.insert(id.clone(), entity);

        id
    }

    /// Starts playback for a player.
    #[wasm_bindgen]
    pub fn play(&mut self, player_id: &str) -> Result<(), JsValue> {
        let mut id_mapping = self.app.world.resource_mut::<IdMapping>();
        let entity = id_mapping
            .players
            .get(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        if let Some(mut player) = self.app.world.get_mut::<AnimationPlayer>(*entity) {
            player.playback_state = crate::PlaybackState::Playing;
        }

        Ok(())
    }

    /// Pauses playback for a player.
    #[wasm_bindgen]
    pub fn pause(&mut self, player_id: &str) -> Result<(), JsValue> {
        let id_mapping = self.app.world.resource::<IdMapping>();
        let entity = id_mapping
            .players
            .get(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        if let Some(mut player) = self.app.world.get_mut::<AnimationPlayer>(*entity) {
            player.playback_state = crate::PlaybackState::Paused;
        }

        Ok(())
    }

    /// Stops playback for a player and resets its time to the beginning.
    #[wasm_bindgen]
    pub fn stop(&mut self, player_id: &str) -> Result<(), JsValue> {
        let id_mapping = self.app.world.resource::<IdMapping>();
        let entity = id_mapping
            .players
            .get(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        if let Some(mut player) = self.app.world.get_mut::<AnimationPlayer>(*entity) {
            player.playback_state = crate::PlaybackState::Stopped;
            player.current_time = crate::AnimationTime::zero();
        }

        Ok(())
    }

    /// Seeks a player to a specific time in seconds.
    #[wasm_bindgen]
    pub fn seek(&mut self, player_id: &str, time_seconds: f64) -> Result<(), JsValue> {
        let id_mapping = self.app.world.resource::<IdMapping>();
        let entity = id_mapping
            .players
            .get(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        if let Some(mut player) = self.app.world.get_mut::<AnimationPlayer>(*entity) {
            player.current_time = crate::AnimationTime::from_seconds(time_seconds)
                .map_err(|e| JsValue::from_str(&format!("Invalid time: {:?}", e)))?;
        }

        Ok(())
    }
}
