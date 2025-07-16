//! Player-related WebAssembly bindings.
use super::engine::WasmAnimationEngine;
use crate::{animation::PlaybackMode, AnimationTime};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl WasmAnimationEngine {
    /// Creates a new animation player and returns its unique ID.
    ///
    /// @returns {string} The unique ID of the newly created player.
    #[wasm_bindgen]
    pub fn create_player(&mut self) -> String {
        self.engine.create_player()
    }

    /// Removes a player by ID.
    #[wasm_bindgen]
    pub fn remove_player(&mut self, player_id: &str) -> Result<(), JsValue> {
        self
            .engine
            .remove_player(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;
        Ok(())
    }

    /// Starts playback for a player.
    ///
    /// @param {string} player_id - The ID of the player to start.
    #[wasm_bindgen]
    pub fn play(&mut self, player_id: &str) -> Result<(), JsValue> {
        self.engine
            .play_player(player_id)
            .map_err(|e| JsValue::from_str(&format!("Play error: {:?}", e)))?;
        Ok(())
    }

    /// Pauses playback for a player.
    ///
    /// @param {string} player_id - The ID of the player to pause.
    #[wasm_bindgen]
    pub fn pause(&mut self, player_id: &str) -> Result<(), JsValue> {
        self.engine
            .pause_player(player_id)
            .map_err(|e| JsValue::from_str(&format!("Pause error: {:?}", e)))?;
        Ok(())
    }

    /// Stops playback for a player and resets its time to the beginning.
    ///
    /// @param {string} player_id - The ID of the player to stop.
    #[wasm_bindgen]
    pub fn stop(&mut self, player_id: &str) -> Result<(), JsValue> {
        self.engine
            .stop_player(player_id)
            .map_err(|e| JsValue::from_str(&format!("Stop error: {:?}", e)))?;
        Ok(())
    }

    /// Seeks a player to a specific time in seconds.
    ///
    /// @param {string} player_id - The ID of the player to seek.
    /// @param {number} time_seconds - The time to seek to in seconds.
    #[wasm_bindgen]
    pub fn seek(&mut self, player_id: &str, time_seconds: f64) -> Result<(), JsValue> {
        let time = AnimationTime::from_seconds(time_seconds)
            .map_err(|e| JsValue::from_str(&format!("Invalid time: {:?}", e)))?;

        self.engine
            .seek_player(player_id, time)
            .map_err(|e| JsValue::from_str(&format!("Seek error: {:?}", e)))?;
        Ok(())
    }

    /// Returns the current playback properties of a player as a JSON object.
    ///
    /// @param {string} player_id - The ID of the player.
    /// @returns {any} A JSON object representing the player's properties.
    /// ```
    #[wasm_bindgen]
    pub fn get_player_settings(&self, player_id: &str) -> Result<JsValue, JsValue> {
        let props = self
            .engine
            .get_player_settings(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;
        serde_wasm_bindgen::to_value(&props)
            .map_err(|e| JsValue::from_str(&format!("State serialization error: {}", e)))
    }

    /// Returns the current playback properties of a player as a JSON object.
    ///
    /// @param {string} player_id - The ID of the player.
    /// @returns {any} A JSON object representing the player's properties.
    /// ```
    #[wasm_bindgen]
    pub fn get_player_state(&self, player_id: &str) -> Result<JsValue, JsValue> {
        let state = self
            .engine
            .get_player_state(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;
        serde_wasm_bindgen::to_value(&state)
            .map_err(|e| JsValue::from_str(&format!("State serialization error: {}", e)))
    }

    /// Returns the duration of a player in seconds.
    ///
    /// @param {string} player_id - The ID of the player.
    /// @returns {number} The duration of the player fully extended in seconds.
    #[wasm_bindgen]
    pub fn get_player_duration(&self, player_id: &str) -> Result<f64, JsValue> {
        let player = self
            .engine
            .get_player(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        Ok(player.duration().as_seconds())
    }

    /// Returns the current time of a player in seconds.
    ///
    /// @param {string} player_id - The ID of the player.
    /// @returns {number} The current time in seconds.
    #[wasm_bindgen]
    pub fn get_player_time(&self, player_id: &str) -> Result<f64, JsValue> {
        let player = self
            .engine
            .get_player(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        Ok(player.current_time.as_seconds())
    }

    /// Returns the playback progress of a player as a value between 0.0 and 1.0.
    ///
    /// @param {string} player_id - The ID of the player.
    /// @returns {number} The playback progress from 0.0 to 1.0.
    #[wasm_bindgen]
    pub fn get_player_progress(&self, player_id: &str) -> Result<f64, JsValue> {
        let player = self
            .engine
            .get_player(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        Ok(player.progress())
    }

    /// Returns a list of all player IDs.
    ///
    /// @returns {string[]} An array of all player IDs.
    #[wasm_bindgen]
    pub fn get_player_ids(&self) -> Result<Vec<String>, JsValue> {
        Ok(self
            .engine
            .player_ids()
            .into_iter()
            .map(|s| s.to_string())
            .collect())
    }

    /// Updates a player's configuration from a JSON string.
    ///
    /// # Example
    ///
    /// ```javascript
    /// const config = {
    ///   "speed": 1.5,
    ///   "mode": "ping_pong", // "once", "loop", "ping_pong"
    ///   "start_time": 0.5,
    ///   "end_time": 3.5
    /// };
    /// engine.update_player_config("player_12345", JSON.stringify(config));
    /// ```
    ///
    /// @param {string} player_id - The ID of the player to configure.
    /// @param {string} config_json - A JSON string with the new configuration values.
    /// ```json
    /// {
    ///   "speed": "float",
    ///   "mode": "string", // "once", "loop", "ping_pong"
    ///   "startTime": "float",
    ///   "endTime": "float" | null
    /// }
    /// ```
    #[wasm_bindgen]
    pub fn update_player_config(
        &mut self,
        player_id: &str,
        config_json: &str,
    ) -> Result<(), JsValue> {
        let player_config = self
            .engine
            .get_player_settings_mut(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        let config: serde_json::Value = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("Config JSON parse error: {}", e)))?;

        if let Some(speed_val) = config.get("speed").and_then(|v| v.as_f64()) {
            if speed_val >= -5.0 && speed_val <= 5.0 {
                player_config.speed = speed_val;
            } else {
                return Err(JsValue::from_str(&format!(
                    "Speed must be between -5.0 and 5.0, got: {}",
                    speed_val
                )));
            }
        }

        if let Some(name) = config.get("name").and_then(|v| v.as_str()) {
            player_config.name = name.to_string();
        }

        if let Some(mode_str) = config.get("mode").and_then(|v| v.as_str()) {
            match mode_str {
                "once" => player_config.mode = PlaybackMode::Once,
                "loop" => player_config.mode = PlaybackMode::Loop,
                "ping_pong" => player_config.mode = PlaybackMode::PingPong,
                _ => {
                    return Err(JsValue::from_str(&format!(
                        "Invalid playback mode: {}. Valid options: once, loop, ping_pong",
                        mode_str
                    )))
                }
            }
        }

        if let Some(start_time_val) = config.get("startTime").and_then(|v| v.as_f64()) {
            if start_time_val >= 0.0 {
                let start_time = AnimationTime::from_seconds(start_time_val)
                    .map_err(|e| JsValue::from_str(&format!("Invalid start time: {:?}", e)))?;
                player_config.start_time = start_time;
            } else {
                return Err(JsValue::from_str(&format!(
                    "Start time must be positive, got: {}",
                    start_time_val
                )));
            }
        }

        if let Some(end_time_val) = config.get("endTime") {
            if end_time_val.is_null() {
                player_config.end_time = None;
            } else if let Some(end_time_f64) = end_time_val.as_f64() {
                if end_time_f64 >= 0.0 {
                    let end_time = AnimationTime::from_seconds(end_time_f64)
                        .map_err(|e| JsValue::from_str(&format!("Invalid end time: {:?}", e)))?;
                    player_config.end_time = Some(end_time);
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

        if let Some(end_time) = player_config.end_time {
            if player_config.start_time >= end_time {
                return Err(JsValue::from_str(&format!(
                    "Start time ({:.2}) must be less than end time ({:.2})",
                    player_config.start_time.as_seconds(),
                    end_time.as_seconds()
                )));
            }
        }

        // Legacy support for boolean loop/ping_pong flags
        if let Some(loop_val) = config.get("loop").and_then(|v| v.as_bool()) {
            if loop_val {
                player_config.mode = PlaybackMode::Loop;
            }
        }
        if let Some(ping_pong_val) = config.get("ping_pong").and_then(|v| v.as_bool()) {
            if ping_pong_val {
                player_config.mode = PlaybackMode::PingPong;
            }
        }

        Ok(())
    }
}
