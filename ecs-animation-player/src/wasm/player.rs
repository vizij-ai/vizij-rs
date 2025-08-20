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
            let mut id_mapping = self.app.world_mut().resource_mut::<IdMapping>();
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
            if let Some(children) = self.app.world().get::<Children>(e) {
                for child in children.iter() {
                    if entities.insert(child) {
                        stack.push(child);
                    }
                }
            }
        }

        {
            let mut id_mapping = self.app.world_mut().resource_mut::<IdMapping>();
            id_mapping.instances.retain(|_, e| !entities.contains(e));
        }

        self.app.world_mut().entity_mut(entity).despawn();
        Ok(())
    }

    /// Sets the target root entity for a player (optional).
    /// Note: This ECS-only helper is optional. If you do not call set_player_root,
    /// the engine will still produce current values via binding-less fallback sampling
    /// in collect_animation_output_system. When bindings exist, they take precedence.
    #[wasm_bindgen(js_name = setPlayerRoot)]
    pub fn set_player_root(&mut self, player_id: &str, entity_id: &str) -> Result<(), JsValue> {
        let player_entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        let bits = entity_id
            .parse::<u64>()
            .map_err(|e| JsValue::from_str(&format!("Invalid entity ID: {}", e)))?;
        let target_entity = Entity::from_bits(bits);

        if let Some(mut player) = self.app.world_mut().get_mut::<AnimationPlayer>(player_entity) {
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
            // Clamp seek target into the configured playback window [start_time, end_time_or_duration]
            let start = player.start_time.as_seconds();
            let mut end = player
                .end_time
                .map(|t| t.as_seconds())
                .unwrap_or_else(|| player.duration.as_seconds());
            if end < start {
                end = start;
            }
            let clamped = time_seconds.clamp(start, end);
            player.current_time = crate::AnimationTime::from_seconds(clamped)
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
            .world()
            .get::<AnimationPlayer>(player_entity)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        // Collect instance IDs belonging to this player
        let mut instance_ids = Vec::new();
        {
            let id_mapping = self.app.world().resource::<IdMapping>();
            for (id, &entity) in id_mapping.instances.iter() {
                if let Some(children) = self.app.world().get::<Children>(player_entity) {
                    if children.iter().any(|c| c == entity) {
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
            start_time: player.start_time.as_seconds(),
            end_time: player.end_time.map(|t| t.as_seconds()),
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
            .world()
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
            .world()
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
            .world()
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
            .world()
            .get::<AnimationPlayer>(player_entity)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        // Progress is computed within the playback window [start_time, end_time_or_duration]
        let start = player.start_time.as_seconds();
        let end = player
            .end_time
            .map(|t| t.as_seconds())
            .unwrap_or_else(|| player.duration.as_seconds());
        let window_len = (end - start).max(0.0);
        if window_len > 0.0 {
            let local = (player.current_time.as_seconds() - start).clamp(0.0, window_len);
            Ok(local / window_len)
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
    /// Supported fields: "speed", "name", "mode" ("once"|"loop"|"ping_pong"),
    /// "startTime", "endTime" (number or null), and optional "rootEntity"
    /// (string u64 bits, number, or null) to set/clear target_root in the same call.
    /// Seeking and progress are clamped to the configured [startTime, endTime|duration] window.
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

        // Parse and apply optional start/end times
        let mut new_start = player.start_time;
        if let Some(start_time_val) = config.get("startTime").and_then(|v| v.as_f64()) {
            if start_time_val >= 0.0 {
                new_start = AnimationTime::from_seconds(start_time_val)
                    .map_err(|e| JsValue::from_str(&format!("Invalid start time: {:?}", e)))?;
            } else {
                return Err(JsValue::from_str(&format!(
                    "Start time must be positive, got: {}",
                    start_time_val
                )));
            }
        }

        let mut new_end = player.end_time;
        if let Some(end_time_val) = config.get("endTime") {
            if end_time_val.is_null() {
                new_end = None;
            } else if let Some(end_time_f64) = end_time_val.as_f64() {
                if end_time_f64 >= 0.0 {
                    new_end = Some(
                        AnimationTime::from_seconds(end_time_f64)
                            .map_err(|e| JsValue::from_str(&format!("Invalid end time: {:?}", e)))?,
                    );
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

        if let Some(end_time) = new_end {
            if new_start >= end_time {
                return Err(JsValue::from_str(&format!(
                    "Start time ({:.2}) must be less than end time ({:.2})",
                    new_start.as_seconds(),
                    end_time.as_seconds()
                )));
            }
        }

        player.start_time = new_start;
        player.end_time = new_end;

        // Optional: set player root entity via config.rootEntity (string u64 bits, number, or null)
        if let Some(root_val) = config.get("rootEntity") {
            if root_val.is_null() {
                player.target_root = None;
            } else if let Some(bits_str) = root_val.as_str() {
                let bits = bits_str.parse::<u64>()
                    .map_err(|e| JsValue::from_str(&format!("Invalid rootEntity: {}", e)))?;
                player.target_root = Some(Entity::from_bits(bits));
            } else if let Some(bits_num) = root_val.as_u64() {
                player.target_root = Some(Entity::from_bits(bits_num));
            } else {
                return Err(JsValue::from_str(
                    "rootEntity must be a string (u64) or number, or null",
                ));
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

    // snake_case export aliases for non-ECS parity
    #[wasm_bindgen(js_name = create_player)]
    pub fn create_player_snake(&mut self) -> String {
        self.create_player()
    }

    #[wasm_bindgen(js_name = remove_player)]
    pub fn remove_player_snake(&mut self, player_id: &str) -> Result<(), JsValue> {
        self.remove_player(player_id)
    }

    // ECS-only helper also exposed in snake_case as optional API
    #[wasm_bindgen(js_name = set_player_root)]
    pub fn set_player_root_snake(&mut self, player_id: &str, entity_id: &str) -> Result<(), JsValue> {
        self.set_player_root(player_id, entity_id)
    }

    #[wasm_bindgen(js_name = get_player_settings)]
    pub fn get_player_settings_snake(&self, player_id: &str) -> Result<JsValue, JsValue> {
        self.get_player_settings(player_id)
    }

    #[wasm_bindgen(js_name = get_player_state)]
    pub fn get_player_state_snake(&self, player_id: &str) -> Result<JsValue, JsValue> {
        self.get_player_state(player_id)
    }

    #[wasm_bindgen(js_name = get_player_duration)]
    pub fn get_player_duration_snake(&self, player_id: &str) -> Result<f64, JsValue> {
        self.get_player_duration(player_id)
    }

    #[wasm_bindgen(js_name = get_player_time)]
    pub fn get_player_time_snake(&self, player_id: &str) -> Result<f64, JsValue> {
        self.get_player_time(player_id)
    }

    #[wasm_bindgen(js_name = get_player_progress)]
    pub fn get_player_progress_snake(&self, player_id: &str) -> Result<f64, JsValue> {
        self.get_player_progress(player_id)
    }

    #[wasm_bindgen(js_name = get_player_ids)]
    pub fn get_player_ids_snake(&self) -> Result<Vec<String>, JsValue> {
        self.get_player_ids()
    }

    #[wasm_bindgen(js_name = update_player_config)]
    pub fn update_player_config_snake(&mut self, player_id: &str, config_json: &str) -> Result<(), JsValue> {
        self.update_player_config(player_id, config_json)
    }
}
