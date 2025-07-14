use std::collections::HashMap;
use std::time::Duration;

use uuid::Uuid;

use crate::animation::instance::{AnimationInstance, AnimationInstanceSettings, PlaybackMode};
use crate::event::EventDispatcher;
use crate::player::animation_player::AnimationPlayer;
use crate::player::playback_state::PlaybackState;
use crate::player::{PlayerSettings, PlayerState};
use crate::{
    AnimationData, AnimationEngineConfig, AnimationError, AnimationTime, InterpolationRegistry,
    Value,
};

/// Settings for the entire engine
#[derive(Debug, Clone)]
pub struct EngineSettings {
    pub config: AnimationEngineConfig,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            config: AnimationEngineConfig::default(),
        }
    }
}

/// Runtime properties for the engine
#[derive(Debug, Clone)]
pub struct EngineProperties {
    pub engine_metrics: HashMap<String, f64>,
    pub last_engine_update_time: Duration,
}

impl Default for EngineProperties {
    fn default() -> Self {
        Self {
            engine_metrics: HashMap::new(),
            last_engine_update_time: Duration::ZERO,
        }
    }
}

/// Animation engine managing multiple players
pub struct AnimationEngine {
    /// All animation players
    players: HashMap<String, AnimationPlayer>,
    /// Settings for each player managed by the engine
    player_settings: HashMap<String, PlayerSettings>,
    /// Properties for each player managed by the engine
    player_state: HashMap<String, PlayerState>,
    /// All loaded animation data
    animations: HashMap<String, AnimationData>,
    /// Interpolation registry
    interpolation_registry: InterpolationRegistry,
    /// Event dispatcher
    event_dispatcher: EventDispatcher,
    /// Engine settings
    settings: EngineSettings,
    /// Engine properties
    properties: EngineProperties,
}

impl AnimationEngine {
    /// Create a new animation engine
    pub fn new(config: AnimationEngineConfig) -> Self {
        Self {
            players: HashMap::new(),
            player_settings: HashMap::new(),
            player_state: HashMap::new(),
            animations: HashMap::new(),
            interpolation_registry: InterpolationRegistry::new(config.max_cache_size),
            event_dispatcher: EventDispatcher::new(),
            settings: EngineSettings { config },
            properties: EngineProperties::default(),
        }
    }

    /// Load animation data into the engine.
    /// Returns a unique ID to use as the `animation_id` in other methods.
    pub fn load_animation_data(
        &mut self,
        animation_data: AnimationData,
    ) -> Result<String, AnimationError> {
        let mut unique_id = format!("{}_{}", animation_data.id, Uuid::new_v4());
        while self.animations.contains_key(&unique_id) {
            unique_id = format!("{}_{}", animation_data.id, Uuid::new_v4());
        }
        if self.animations.contains_key(&animation_data.id) {
            return Err(AnimationError::Generic {
                message: format!(
                    "AnimationData with ID '{}' already loaded.",
                    animation_data.id
                ),
            });
        }
        self.animations.insert(unique_id.clone(), animation_data);
        Ok(unique_id)
    }

    /// Unload animation data from the engine
    pub fn unload_animation_data(
        &mut self,
        animation_id: &str,
    ) -> Result<AnimationData, AnimationError> {
        self.animations
            .remove(animation_id)
            .ok_or_else(|| AnimationError::AnimationNotFound {
                id: animation_id.to_string(),
            })
    }

    /// Get a reference to loaded animation data
    #[inline]
    pub fn get_animation_data(&self, animation_id: &str) -> Option<&AnimationData> {
        self.animations.get(animation_id)
    }

    /// Get a mutable reference to loaded animation data
    #[inline]
    pub fn get_animation_data_mut(&mut self, animation_id: &str) -> Option<&mut AnimationData> {
        self.animations.get_mut(animation_id)
    }

    /// Create a new player
    pub fn create_player(&mut self) -> String {
        // Generate a new ID until we find a unique one
        let mut id = uuid::Uuid::new_v4().to_string();
        while self.players.contains_key(&id) {
            id = format!("{}_{}", id, Uuid::new_v4());
        }

        self.players.insert(id.clone(), AnimationPlayer::new());
        self.player_settings
            .insert(id.clone(), PlayerSettings::default());
        self.player_state.insert(id.clone(), PlayerState::default());
        id
    }

    /// Get a player by ID
    #[inline]
    pub fn get_player(&self, id: &str) -> Option<&AnimationPlayer> {
        self.players.get(id)
    }

    /// Get a mutable player by ID
    #[inline]
    pub fn get_player_mut(&mut self, id: &str) -> Option<&mut AnimationPlayer> {
        self.players.get_mut(id)
    }

    /// Get a player's settings by ID (speed, mode, loop_until_target, offset, start_time, end_time)
    #[inline]
    pub fn get_player_settings(&self, id: &str) -> Option<PlayerSettings> {
        self.player_settings.get(id).map(|settings| {
            let mut updated_settings = settings.clone();
            if let Some(player) = self.players.get(id) {
                updated_settings.instance_ids = player.get_instance_ids();
            }
            updated_settings
        })
    }

    /// Get mutable player settings by ID
    #[inline]
    pub fn get_player_settings_mut(&mut self, id: &str) -> Option<&mut PlayerSettings> {
        self.player_settings.get_mut(id)
    }

    /// Get a player's properties by ID (playback_state, last_update_time, current_loop_count, is_playing_forward)
    #[inline]
    pub fn get_player_state(&self, id: &str) -> Option<&PlayerState> {
        self.player_state.get(id)
    }

    /// Get mutable player properties by ID
    #[inline]
    pub fn get_player_properties_mut(&mut self, id: &str) -> Option<&mut PlayerState> {
        self.player_state.get_mut(id)
    }

    /// Remove a player
    pub fn remove_player(&mut self, id: &str) -> Option<AnimationPlayer> {
        self.player_settings.remove(id);
        self.player_state.remove(id);
        self.players.remove(id)
    }

    /// Get all player IDs
    #[inline]
    pub fn player_ids(&self) -> Vec<&str> {
        self.players.keys().map(|s| s.as_str()).collect()
    }

    /// Get all loaded animation IDs
    #[inline]
    pub fn animation_ids(&self) -> Vec<&str> {
        self.animations.keys().map(|s| s.as_str()).collect()
    }

    /// Add an animation to a player by creating a new animation instance.
    /// Returns the ID of the created instance,
    pub fn add_animation_to_player(
        &mut self,
        player_id: &str,
        animation_id: &str,
        instance_settings: Option<AnimationInstanceSettings>,
    ) -> Result<String, AnimationError> {
        // Verify the animation data exists
        let animation_data = self.get_animation_data(animation_id).ok_or_else(|| {
            AnimationError::AnimationNotFound {
                id: animation_id.to_string(),
            }
        })?;

        // Get the animation duration for the instance
        let animation_duration = animation_data.duration();

        // Get the player
        let player = self
            .get_player_mut(player_id)
            .ok_or_else(|| AnimationError::Generic {
                message: format!("Player with ID '{}' not found.", player_id),
            })?;

        // Use provided settings or create default
        let settings = instance_settings.unwrap_or_default();

        // Create the animation instance
        let instance = AnimationInstance::new(animation_id, settings, animation_duration);

        // Add the instance to the player and return its ID
        Ok(player.add_instance(instance))
    }

    /// Update all players
    pub fn update(
        &mut self,
        frame_delta: impl Into<Duration>,
    ) -> Result<HashMap<String, HashMap<String, Value>>, AnimationError> {
        let frame_delta: Duration = frame_delta.into();
        let mut all_values = HashMap::new();

        // Update engine's internal time
        self.properties.last_engine_update_time += frame_delta;

        // Collect player IDs to avoid mutable borrow issues
        let player_ids: Vec<String> = self.players.keys().cloned().collect();

        for player_id in player_ids {
            // Get mutable references to player and its state
            let player = self.players.get_mut(&player_id).unwrap();
            let player_property = self.player_state.get_mut(&player_id).unwrap();
            let player_setting = self.player_settings.get_mut(&player_id).unwrap();

            // Skip processing if player is not playing
            if player_property.playback_state != PlaybackState::Playing {
                let values =
                    player.calculate_values(&self.animations, &mut self.interpolation_registry)?;
                all_values.insert(player_id.clone(), values);
                continue;
            }

            // Get player duration
            let player_duration = player_setting.end_time.unwrap_or(player.duration());

            // Calculate animation delta based on frame_delta and player speed
            // std::time::Duration only supports multiplying with positive values.
            let animation_delta = frame_delta.mul_f64(player_setting.speed.abs());

            // Update player time and handle bounds/looping
            let values = if player_setting.speed >= 0.0 {
                // Forward playback
                let new_time = player.current_time + animation_delta.into();

                if new_time >= player_duration {
                    // Use the instance's playback mode, fallback to player state mode
                    // let effective_mode = player.instances.values()
                    //     .filter(|inst| inst.settings.enabled)
                    //     .map(|inst| inst.settings.playback_mode)
                    //     .next()
                    //     .unwrap_or(player_state.mode);

                    match player_setting.mode {
                        PlaybackMode::Loop => {
                            // Wrap around to start
                            let wrapped_time =
                                player_setting.start_time + (new_time - player_duration);
                            let result = player.go_to(
                                wrapped_time,
                                &self.animations,
                                &mut self.interpolation_registry,
                            )?;
                            player_property.playback_state = PlaybackState::Playing; // Ensure state remains Playing
                            result
                        }
                        PlaybackMode::PingPong => {
                            // Reverse the speed for ping pong mode
                            player_setting.speed = -player_setting.speed.abs();
                            // Clamp to the end and reverse
                            player.go_to(
                                player_duration,
                                &self.animations,
                                &mut self.interpolation_registry,
                            )?
                        }
                        PlaybackMode::Once => {
                            // End playback
                            player_property.playback_state = PlaybackState::Ended;
                            player.go_to(
                                player_duration,
                                &self.animations,
                                &mut self.interpolation_registry,
                            )?
                        }
                    }
                } else {
                    // Normal forward increment
                    player.increment(
                        animation_delta,
                        &self.animations,
                        &mut self.interpolation_registry,
                    )?
                }
            } else {
                // Reverse playback
                let new_time = player.current_time - animation_delta.into();

                if new_time <= player_setting.start_time {
                    // Use the instance's playback mode, fallback to player state mode
                    // let effective_mode = player.instances.values()
                    //     .filter(|inst| inst.settings.enabled)
                    //     .map(|inst| inst.settings.playback_mode)
                    //     .next()
                    //     .unwrap_or(player_state.mode);

                    match player_setting.mode {
                        PlaybackMode::Loop => {
                            // Wrap around to end
                            let wrapped_time =
                                player_duration - (player_setting.start_time - new_time);
                            let result = player.go_to(
                                wrapped_time,
                                &self.animations,
                                &mut self.interpolation_registry,
                            )?;
                            player_property.playback_state = PlaybackState::Playing; // Ensure state remains Playing
                            result
                        }
                        PlaybackMode::PingPong => {
                            // Reverse the speed for ping pong mode
                            player_setting.speed = player_setting.speed.abs();
                            // Clamp to the start and reverse
                            player.go_to(
                                player_setting.start_time,
                                &self.animations,
                                &mut self.interpolation_registry,
                            )?
                        }
                        PlaybackMode::Once => {
                            // End playback
                            player_property.playback_state = PlaybackState::Ended;
                            player.go_to(
                                player_setting.start_time,
                                &self.animations,
                                &mut self.interpolation_registry,
                            )?
                        }
                    }
                } else {
                    // Normal reverse increment
                    player.decrement(
                        animation_delta,
                        &self.animations,
                        &mut self.interpolation_registry,
                    )?
                }
            };

            all_values.insert(player_id.clone(), values);
        }

        // Update engine metrics
        self.update_engine_metrics();

        Ok(all_values)
    }

    /// Update engine-level metrics
    fn update_engine_metrics(&mut self) {
        let total_players = self.players.len() as f64;
        let playing_players = self
            .player_state
            .values()
            .filter(|ps| ps.playback_state.is_playing())
            .count() as f64;

        let total_memory: usize = self
            .players
            .values()
            .map(|p| p.metrics.memory_usage_bytes)
            .sum();

        let avg_fps = if !self.players.is_empty() {
            self.players
                .values()
                .map(|p| p.metrics.frames_rendered as f64) // Use frames_rendered for FPS calculation
                .sum::<f64>()
                / total_players
        } else {
            0.0
        };

        self.properties
            .engine_metrics
            .insert("total_players".to_string(), total_players);
        self.properties
            .engine_metrics
            .insert("playing_players".to_string(), playing_players);
        self.properties.engine_metrics.insert(
            "total_memory_mb".to_string(),
            total_memory as f64 / (1024.0 * 1024.0),
        );
        self.properties
            .engine_metrics
            .insert("average_fps".to_string(), avg_fps);
        self.properties.engine_metrics.insert(
            "cache_hit_rate".to_string(),
            self.interpolation_registry.metrics().cache_hit_rate(),
        );
    }

    /// Get interpolation registry
    #[inline]
    pub fn interpolation_registry(&self) -> &InterpolationRegistry {
        &self.interpolation_registry
    }

    /// Get mutable interpolation registry
    #[inline]
    pub fn interpolation_registry_mut(&mut self) -> &mut InterpolationRegistry {
        &mut self.interpolation_registry
    }

    /// Get event dispatcher
    #[inline]
    pub fn event_dispatcher(&self) -> &EventDispatcher {
        &self.event_dispatcher
    }

    /// Get mutable event dispatcher
    #[inline]
    pub fn event_dispatcher_mut(&mut self) -> &mut EventDispatcher {
        &mut self.event_dispatcher
    }

    /// Get engine configuration
    #[inline]
    pub fn config(&self) -> &AnimationEngineConfig {
        &self.settings.config
    }

    /// Set engine configuration
    #[inline]
    pub fn set_config(&mut self, config: AnimationEngineConfig) {
        self.settings.config = config;
    }

    /// Get engine metrics
    #[inline]
    pub fn metrics(&self) -> &HashMap<String, f64> {
        &self.properties.engine_metrics
    }

    /// Get total number of players
    #[inline]
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Get number of playing players
    #[inline]
    pub fn playing_player_count(&self) -> usize {
        self.player_state
            .values()
            .filter(|ps| ps.playback_state.is_playing())
            .count()
    }

    /// Start playback for a specific player
    pub fn play_player(&mut self, player_id: &str) -> Result<(), AnimationError> {
        let player_property =
            self.player_state
                .get_mut(player_id)
                .ok_or_else(|| AnimationError::Generic {
                    message: format!("Player with ID '{}' not found.", player_id),
                })?;
        let player_setting = self.player_settings.get_mut(player_id).unwrap();
        if player_property.playback_state.can_resume() {
            player_property.playback_state = PlaybackState::Playing;
            player_property.last_update_time = AnimationTime::zero(); // Reset for new playback

            // Reset player's time if starting from stopped/ended
            let player = self.players.get_mut(player_id).unwrap();
            if player_property.playback_state == PlaybackState::Stopped
                || player_property.playback_state == PlaybackState::Ended
            {
                player.go_to(
                    player_setting.start_time,
                    &self.animations,
                    &mut self.interpolation_registry,
                )?;
            }
            Ok(())
        } else {
            Err(AnimationError::InvalidPlayerState {
                current_state: player_property.playback_state.name().to_string(),
                requested_state: "playing".to_string(),
            })
        }
    }

    /// Pause playback for a specific player
    pub fn pause_player(&mut self, player_id: &str) -> Result<(), AnimationError> {
        let player_property =
            self.player_state
                .get_mut(player_id)
                .ok_or_else(|| AnimationError::Generic {
                    message: format!("Player with ID '{}' not found.", player_id),
                })?;

        if player_property.playback_state.can_pause() {
            player_property.playback_state = PlaybackState::Paused;
            Ok(())
        } else {
            Err(AnimationError::InvalidPlayerState {
                current_state: player_property.playback_state.name().to_string(),
                requested_state: "paused".to_string(),
            })
        }
    }

    /// Stop playback for a specific player
    pub fn stop_player(&mut self, player_id: &str) -> Result<(), AnimationError> {
        let player_property =
            self.player_state
                .get_mut(player_id)
                .ok_or_else(|| AnimationError::Generic {
                    message: format!("Player with ID '{}' not found.", player_id),
                })?;
        let player_setting = self.player_settings.get(player_id).unwrap();

        if player_property.playback_state.can_stop() {
            player_property.playback_state = PlaybackState::Stopped;
            let player = self.players.get_mut(player_id).unwrap();
            player.go_to(
                player_setting.start_time,
                &self.animations,
                &mut self.interpolation_registry,
            )?;
            Ok(())
        } else {
            Err(AnimationError::InvalidPlayerState {
                current_state: player_property.playback_state.name().to_string(),
                requested_state: "stopped".to_string(),
            })
        }
    }

    /// Seek a specific player to a given time
    pub fn seek_player(
        &mut self,
        player_id: &str,
        time: impl Into<AnimationTime>,
    ) -> Result<(), AnimationError> {
        let player = self
            .players
            .get_mut(player_id)
            .ok_or_else(|| AnimationError::Generic {
                message: format!("Player with ID '{}' not found.", player_id),
            })?;

        player.go_to(time, &self.animations, &mut self.interpolation_registry)?;
        Ok(())
    }

    /// Stop all players
    pub fn stop_all_players(&mut self) -> Result<(), AnimationError> {
        let player_ids: Vec<String> = self.players.keys().cloned().collect();
        for player_id in player_ids {
            self.stop_player(&player_id)?;
        }
        Ok(())
    }

    /// Pause all players
    pub fn pause_all_players(&mut self) -> Result<(), AnimationError> {
        let player_ids: Vec<String> = self.players.keys().cloned().collect();
        for player_id in player_ids {
            self.pause_player(&player_id)?;
        }
        Ok(())
    }

    /// Resume all paused players
    pub fn resume_all_players(&mut self) -> Result<(), AnimationError> {
        let player_ids: Vec<String> = self.players.keys().cloned().collect();
        for player_id in player_ids {
            self.play_player(&player_id)?;
        }
        Ok(())
    }

    /// Calculate derivatives for a specific player (helper method for WASM)
    pub fn calculate_player_derivatives(
        &mut self,
        player_id: &str,
        derivative_width: Option<AnimationTime>,
    ) -> Result<HashMap<String, Value>, AnimationError> {
        // Get the player to collect animation IDs
        let animation_ids = self
            .get_player(player_id)
            .ok_or_else(|| AnimationError::Generic {
                message: format!("Player '{}' not found", player_id),
            })?
            .get_active_animation_ids();

        // Collect animation data
        let mut all_animations = HashMap::new();
        for animation_id in &animation_ids {
            if let Some(animation_data) = self.get_animation_data(animation_id) {
                all_animations.insert(animation_id.clone(), animation_data.clone());
            }
        }

        // Split self to avoid borrowing conflicts
        let player = self
            .players
            .get_mut(player_id)
            .ok_or_else(|| AnimationError::Generic {
                message: format!("Player '{}' not found", player_id),
            })?;

        let player_property =
            self.player_state
                .get(player_id)
                .ok_or_else(|| AnimationError::Generic {
                    message: format!("Player state for '{}' not found", player_id),
                })?;
        let player_setting = self.player_settings.get(player_id).unwrap();

        player.calculate_derivatives(
            &all_animations,
            &mut self.interpolation_registry,
            derivative_width,
            player_setting.speed,
        )
    }
}

impl Default for AnimationEngine {
    fn default() -> Self {
        Self::new(AnimationEngineConfig::default())
    }
}
