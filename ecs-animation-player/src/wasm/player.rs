use crate::ecs::components::AnimationPlayer;
use crate::ecs::resources::IdMapping;
use crate::{AnimationTime, PlaybackMode};
use bevy::prelude::*;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use wasm_bindgen::prelude::*;

use super::WasmAnimationEngine;

#[derive(Serialize)]
struct PlayerSettingsData {
    name: String,
    speed: f64,
    mode: String,
    loop_until_target: Option<u32>,
    offset: f64,
    start_time: f64,
    end_time: Option<f64>,
    instance_ids: Vec<String>,
}

#[derive(Serialize)]
struct PlayerStateData {
    playback_state: String,
    last_update_time: f64,
    current_loop_count: u32,
    is_playing_forward: bool,
}

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

    /// Returns the current playback properties of a player as a JSON object.
    #[wasm_bindgen(js_name = getPlayerSettings)]
    pub fn get_player_settings(&self, player_id: &str) -> Result<JsValue, JsValue> {
        let player_entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        let player = self
            .app
            .world
            .get::<AnimationPlayer>(player_entity)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        // Collect instance IDs belonging to this player
        let mut instance_ids = Vec::new();
        {
            let id_mapping = self.app.world().resource::<IdMapping>();
            for (id, &entity) in id_mapping.instances.iter() {
                if let Some(parent) = self.app.world().get::<Parent>(entity) {
                    if parent.get() == player_entity {
                        instance_ids.push(id.clone());
                    }
                }
            }
        }

        let mode = match player.mode {
            PlaybackMode::Once => "once",
            PlaybackMode::Loop => "loop",
            PlaybackMode::PingPong => "ping_pong",
        };

        let settings = PlayerSettingsData {
            name: player.name.clone(),
            speed: player.speed,
            mode: mode.to_string(),
            loop_until_target: None,
            offset: 0.0,
            start_time: 0.0,
            end_time: None,
            instance_ids,
        };

        serde_wasm_bindgen::to_value(&settings)
            .map_err(|e| JsValue::from_str(&format!("State serialization error: {}", e)))
    }

    /// Returns the runtime state of a player as a JSON object.
    #[wasm_bindgen(js_name = getPlayerState)]
    pub fn get_player_state(&self, player_id: &str) -> Result<JsValue, JsValue> {
        let player_entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        let player = self
            .app
            .world
            .get::<AnimationPlayer>(player_entity)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        let state = PlayerStateData {
            playback_state: player.playback_state.name().to_string(),
            last_update_time: player.current_time.as_seconds(),
            current_loop_count: 0,
            is_playing_forward: player.speed >= 0.0,
        };

        serde_wasm_bindgen::to_value(&state)
            .map_err(|e| JsValue::from_str(&format!("State serialization error: {}", e)))
    }

    /// Returns the duration of a player in seconds.
    #[wasm_bindgen(js_name = getPlayerDuration)]
    pub fn get_player_duration(&self, player_id: &str) -> Result<f64, JsValue> {
        let player_entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        let player = self
            .app
            .world
            .get::<AnimationPlayer>(player_entity)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        Ok(player.duration.as_seconds())
    }

    /// Returns the current time of a player in seconds.
    #[wasm_bindgen(js_name = getPlayerTime)]
    pub fn get_player_time(&self, player_id: &str) -> Result<f64, JsValue> {
        let player_entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        let player = self
            .app
            .world
            .get::<AnimationPlayer>(player_entity)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        Ok(player.current_time.as_seconds())
    }

    /// Returns the playback progress of a player as a value between 0.0 and 1.0.
    #[wasm_bindgen(js_name = getPlayerProgress)]
    pub fn get_player_progress(&self, player_id: &str) -> Result<f64, JsValue> {
        let player_entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        let player = self
            .app
            .world
            .get::<AnimationPlayer>(player_entity)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        let duration = player.duration.as_seconds();
        if duration > 0.0 {
            Ok((player.current_time.as_seconds() / duration).clamp(0.0, 1.0))
        } else {
            Ok(0.0)
        }
    }

    /// Returns a list of all player IDs.
    #[wasm_bindgen(js_name = getPlayerIds)]
    pub fn get_player_ids(&self) -> Result<Vec<String>, JsValue> {
        let id_mapping = self.app.world().resource::<IdMapping>();
        Ok(id_mapping.players.keys().cloned().collect())
    }

    /// Updates a player's configuration from a JSON string.
    #[wasm_bindgen(js_name = updatePlayerConfig)]
    pub fn update_player_config(
        &mut self,
        player_id: &str,
        config_json: &str,
    ) -> Result<(), JsValue> {
        let entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        let mut player = self
            .app
            .world_mut()
            .get_mut::<AnimationPlayer>(entity)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        let config: Value = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("Config JSON parse error: {}", e)))?;

        if let Some(speed_val) = config.get("speed").and_then(|v| v.as_f64()) {
            if (-5.0..=5.0).contains(&speed_val) {
                player.speed = speed_val;
            } else {
                return Err(JsValue::from_str(&format!(
                    "Speed must be between -5.0 and 5.0, got: {}",
                    speed_val
                )));
            }
        }

        if let Some(name) = config.get("name").and_then(|v| v.as_str()) {
            player.name = name.to_string();
        }

        if let Some(mode_str) = config.get("mode").and_then(|v| v.as_str()) {
            player.mode = match mode_str {
                "once" => PlaybackMode::Once,
                "loop" => PlaybackMode::Loop,
                "ping_pong" => PlaybackMode::PingPong,
                _ => {
                    return Err(JsValue::from_str(&format!(
                        "Invalid playback mode: {}. Valid options: once, loop, ping_pong",
                        mode_str
                    )))
                }
            };
        }

        // Validate optional start and end times even though they are not currently stored
        let mut start_time_opt: Option<AnimationTime> = None;
        if let Some(start_time_val) = config.get("startTime").and_then(|v| v.as_f64()) {
            if start_time_val >= 0.0 {
                start_time_opt = Some(
                    AnimationTime::from_seconds(start_time_val)
                        .map_err(|e| JsValue::from_str(&format!("Invalid start time: {:?}", e)))?,
                );
            } else {
                return Err(JsValue::from_str(&format!(
                    "Start time must be positive, got: {}",
                    start_time_val
                )));
            }
        }

        let mut end_time_opt: Option<AnimationTime> = None;
        if let Some(end_time_val) = config.get("endTime") {
            if end_time_val.is_null() {
                end_time_opt = None;
            } else if let Some(end_time_f64) = end_time_val.as_f64() {
                if end_time_f64 >= 0.0 {
                    end_time_opt =
                        Some(AnimationTime::from_seconds(end_time_f64).map_err(|e| {
                            JsValue::from_str(&format!("Invalid end time: {:?}", e))
                        })?);
                } else {
                    return Err(JsValue::from_str(&format!(
                        "End time must be positive, got: {}",
                        end_time_f64
                    )));
                }
            } else {
                return Err(JsValue::from_str("End time must be a number or null"));
            }
        }

        if let (Some(start_time), Some(end_time)) = (start_time_opt, end_time_opt) {
            if start_time >= end_time {
                return Err(JsValue::from_str(&format!(
                    "Start time ({:.2}) must be less than end time ({:.2})",
                    start_time.as_seconds(),
                    end_time.as_seconds()
                )));
            }
        }

        // Legacy support for boolean loop/ping_pong flags
        if let Some(loop_val) = config.get("loop").and_then(|v| v.as_bool()) {
            if loop_val {
                player.mode = PlaybackMode::Loop;
            }
        }
        if let Some(ping_pong_val) = config.get("ping_pong").and_then(|v| v.as_bool()) {
            if ping_pong_val {
                player.mode = PlaybackMode::PingPong;
            }
        }

        Ok(())
    }
}
