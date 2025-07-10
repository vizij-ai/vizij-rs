//! WebAssembly bindings for the animation engine.
use crate::{
    animation::PlaybackMode, AnimationBaking, AnimationData, AnimationEngine,
    AnimationEngineConfig, AnimationTime, BakingConfig, Value,
};
use crate::{
    animation::{transition::AnimationTransition, AnimationMetadata, TransitionVariant},
    loaders::load_test_animation_from_json,
    value::{Color, Vector3, Vector4},
    AnimationKeypoint, AnimationTrack, KeypointId,
};
use std::time::Duration;
use wasm_bindgen::prelude::*;

/// Sets up a panic hook to log panic messages to the browser console.
#[wasm_bindgen(start)]
pub fn on_start() {
    console_error_panic_hook::set_once();
}

/// A WebAssembly-compatible wrapper for the `AnimationEngine`.
#[wasm_bindgen]
pub struct WasmAnimationEngine {
    engine: AnimationEngine,
}

#[wasm_bindgen]
impl WasmAnimationEngine {
    /// Creates a new `WasmAnimationEngine`.
    ///
    /// An optional JSON configuration string can be provided. If `None`, a default, web-optimized
    /// configuration is used.
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: Option<String>) -> Result<WasmAnimationEngine, JsValue> {
        let config = if let Some(json) = config_json {
            serde_json::from_str::<AnimationEngineConfig>(&json)
                .map_err(|e| JsValue::from_str(&format!("Config parse error: {}", e)))?
        } else {
            AnimationEngineConfig::web_optimized()
        };

        let engine = AnimationEngine::new(config);

        Ok(WasmAnimationEngine { engine })
    }

    /// Loads animation data from a JSON string.
    ///
    /// This function first attempts to parse the JSON directly into an `AnimationData` struct.
    /// If that fails, it falls back to a test animation loader for compatibility.
    #[wasm_bindgen]
    pub fn load_animation(&mut self, animation_json: &str) -> Result<String, JsValue> {
        let animation_data: AnimationData = match serde_json::from_str(animation_json) {
            Ok(data) => data,
            Err(parse_error) => {
                // Fallback for test animations
                match load_test_animation_from_json_wasm(animation_json) {
                    Ok(corrected_json) => serde_json::from_str(&corrected_json).map_err(|e| {
                        JsValue::from_str(&format!("Fallback JSON parse error: {}", e))
                    })?,
                    Err(fallback_error) => {
                        return Err(JsValue::from_str(&format!(
                            "Animation JSON parse error: {}. Fallback loader error: {:?}",
                            parse_error, fallback_error
                        )));
                    }
                }
            }
        };
        let animation_id = self
            .engine
            .load_animation_data(animation_data)
            .map_err(|e| JsValue::from_str(&format!("Load animation error: {:?}", e)))?;

        Ok(animation_id)
    }

    /// Creates a new animation player and returns its unique ID.
    #[wasm_bindgen]
    pub fn create_player(&mut self) -> String {
        self.engine.create_player()
    }

    /// Returns a list of all loaded animation IDs.
    #[wasm_bindgen]
    pub fn animation_ids(&mut self) -> Vec<String> {
        self.engine
            .animation_ids()
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// Adds an animation instance to a player.
    #[wasm_bindgen]
    pub fn add_instance(&mut self, player_id: &str, animation_id: &str) -> Result<String, JsValue> {
        self.engine
            .add_animation_to_player(player_id, animation_id, None)
            .map_err(|e| JsValue::from_str(&format!("Engine lock poisoned: {}", e)))
    }

    /// Starts playback for a player.
    #[wasm_bindgen]
    pub fn play(&mut self, player_id: &str) -> Result<(), JsValue> {
        self.engine
            .play_player(player_id)
            .map_err(|e| JsValue::from_str(&format!("Play error: {:?}", e)))?;
        Ok(())
    }

    /// Pauses playback for a player.
    #[wasm_bindgen]
    pub fn pause(&mut self, player_id: &str) -> Result<(), JsValue> {
        self.engine
            .pause_player(player_id)
            .map_err(|e| JsValue::from_str(&format!("Pause error: {:?}", e)))?;
        Ok(())
    }

    /// Stops playback for a player and resets its time to the beginning.
    #[wasm_bindgen]
    pub fn stop(&mut self, player_id: &str) -> Result<(), JsValue> {
        self.engine
            .stop_player(player_id)
            .map_err(|e| JsValue::from_str(&format!("Stop error: {:?}", e)))?;
        Ok(())
    }

    /// Seeks a player to a specific time in seconds.
    #[wasm_bindgen]
    pub fn seek(&mut self, player_id: &str, time_seconds: f64) -> Result<(), JsValue> {
        let time = AnimationTime::from_seconds(time_seconds)
            .map_err(|e| JsValue::from_str(&format!("Invalid time: {:?}", e)))?;

        self.engine
            .seek_player(player_id, time)
            .map_err(|e| JsValue::from_str(&format!("Seek error: {:?}", e)))?;
        Ok(())
    }

    /// Updates the animation engine by a given time delta and returns the current animation values.
    ///
    /// The `frame_delta_seconds` is the time elapsed since the last update.
    #[wasm_bindgen]
    pub fn update(&mut self, frame_delta_seconds: f64) -> Result<JsValue, JsValue> {
        let values = self
            .engine
            .update(Duration::from_secs_f64(frame_delta_seconds))
            .map_err(|e| JsValue::from_str(&format!("Update error: {:?}", e)))?;

        serde_wasm_bindgen::to_value(&values)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Returns the current playback state of a player as a JSON object.
    #[wasm_bindgen]
    pub fn get_player_state(&self, player_id: &str) -> Result<JsValue, JsValue> {
        let state = self
            .engine
            .get_player_state(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;
        serde_wasm_bindgen::to_value(&state)
            .map_err(|e| JsValue::from_str(&format!("State serialization error: {}", e)))
    }

    /// Returns the current time of a player in seconds.
    #[wasm_bindgen]
    pub fn get_player_time(&self, player_id: &str) -> Result<f64, JsValue> {
        let player = self
            .engine
            .get_player(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        Ok(player.current_time.as_seconds())
    }

    /// Returns the playback progress of a player as a value between 0.0 and 1.0.
    #[wasm_bindgen]
    pub fn get_player_progress(&self, player_id: &str) -> Result<f64, JsValue> {
        let player = self
            .engine
            .get_player(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        Ok(player.progress())
    }

    /// Returns a list of all player IDs.
    #[wasm_bindgen]
    pub fn get_player_ids(&self) -> Result<Vec<String>, JsValue> {
        Ok(self
            .engine
            .player_ids()
            .into_iter()
            .map(|s| s.to_string())
            .collect())
    }

    /// Returns the engine's performance metrics as a JSON object.
    #[wasm_bindgen]
    pub fn get_metrics(&self) -> JsValue {
        let metrics = self.engine.metrics();
        serde_wasm_bindgen::to_value(metrics).unwrap_or(JsValue::NULL)
    }

    /// Updates a player's configuration from a JSON string.
    #[wasm_bindgen]
    pub fn update_player_config(
        &mut self,
        player_id: &str,
        config_json: &str,
    ) -> Result<(), JsValue> {
        let player_config = self
            .engine
            .get_player_state_mut(player_id)
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

        if let Some(start_time_val) = config.get("start_time").and_then(|v| v.as_f64()) {
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

        if let Some(end_time_val) = config.get("end_time") {
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

    /// Exports a loaded animation as a JSON string.
    #[wasm_bindgen]
    pub fn export_animation(&self, animation_id: &str) -> Result<String, JsValue> {
        let animation = self
            .engine
            .get_animation_data(animation_id)
            .ok_or_else(|| JsValue::from_str("Animation not found"))?;

        serde_json::to_string(animation)
            .map_err(|e| JsValue::from_str(&format!("Export error: {}", e)))
    }

    /// Calculates the derivatives (rates of change) for all tracks of a player at the current time.
    ///
    /// The `derivative_width_ms` is the time window in milliseconds over which to calculate the rate of change.
    #[wasm_bindgen]
    pub fn get_derivatives(
        &mut self,
        player_id: &str,
        derivative_width_ms: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let derivative_width =
            if let Some(width_ms) = derivative_width_ms {
                if width_ms <= 0.0 {
                    return Err(JsValue::from_str("Derivative width must be positive"));
                }
                Some(AnimationTime::from_millis(width_ms).map_err(|e| {
                    JsValue::from_str(&format!("Invalid derivative width: {:?}", e))
                })?)
            } else {
                None
            };

        let derivatives = self
            .engine
            .calculate_player_derivatives(player_id, derivative_width)
            .map_err(|e| JsValue::from_str(&format!("Calculate derivatives error: {:?}", e)))?;

        serde_wasm_bindgen::to_value(&derivatives)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Bakes an animation into a series of pre-calculated values at a specified frame rate.
    #[wasm_bindgen]
    pub fn bake_animation(
        &mut self,
        animation_id: &str,
        config_json: Option<String>,
    ) -> Result<String, JsValue> {
        let animation = self
            .engine
            .get_animation_data(animation_id)
            .ok_or_else(|| JsValue::from_str("Animation not found"))?
            .clone();

        let config = if let Some(json) = config_json {
            serde_json::from_str::<BakingConfig>(&json)
                .map_err(|e| JsValue::from_str(&format!("Baking config parse error: {}", e)))?
        } else {
            BakingConfig::default()
        };

        let baked_data = animation
            .bake(&config, self.engine.interpolation_registry_mut())
            .map_err(|e| JsValue::from_str(&format!("Baking error: {:?}", e)))?;

        baked_data
            .to_json()
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {:?}", e)))
    }
}

/// Logs a message to the browser console.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

/// A convenience function for logging from Rust to the browser console.
#[wasm_bindgen]
pub fn console_log(message: &str) {
    log(message);
}

/// Converts a JSON string representing a `Value` into a `JsValue`.
#[wasm_bindgen]
pub fn value_to_js(value_json: &str) -> Result<JsValue, JsValue> {
    let value: Value = serde_json::from_str(value_json)
        .map_err(|e| JsValue::from_str(&format!("Value parse error: {}", e)))?;

    serde_wasm_bindgen::to_value(&value)
        .map_err(|e| JsValue::from_str(&format!("Value conversion error: {}", e)))
}

/// Creates an `AnimationTime` from seconds.
#[inline]
fn time(t: f64) -> AnimationTime {
    if t.abs() < f64::EPSILON {
        AnimationTime::zero()
    } else {
        AnimationTime::from_seconds(t).expect("invalid time")
    }
}

/// A helper function to build an `AnimationTrack` from arrays of times and values.
#[inline]
fn build_track<F>(
    name: &str,
    property: &str,
    times: &[f64],
    make_value: F,
) -> (AnimationTrack, Vec<KeypointId>)
where
    F: Fn(usize) -> Value,
{
    let mut track = AnimationTrack::new(name, property);
    let mut ids: Vec<KeypointId> = Vec::with_capacity(times.len());

    for (i, &t) in times.iter().enumerate() {
        let kp = track
            .add_keypoint(AnimationKeypoint::new(time(t), make_value(i)))
            .unwrap();
        ids.push(kp.id);
    }

    (track, ids)
}

/// Creates a test animation with various value types.
#[wasm_bindgen]
pub fn create_animation_test_type() -> String {
    const POS_TIME: [f64; 5] = [0.0, 1.0, 2.0, 3.0, 4.1];
    const ROT_TIME: [f64; 5] = [0.0, 1.0, 2.0, 3.0, 4.0];
    const SCALE_TIME: [f64; 9] = [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0];
    const COL_TIME: [f64; 5] = [0.0, 1.0, 2.0, 3.0, 4.0];
    const INT_TIME: [f64; 8] = [0.0, 0.25, 0.5, 0.75, 1.0, 2.0, 3.0, 4.0];

    let mut animation = AnimationData::new("test_animation", "Robot Wave Animation");

    let (track, _) = build_track("position", "transform.position", &POS_TIME, |i| {
        let coords = [
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 1.5, 0.0),
            Vector3::new(4.0, 0.0, 0.0),
            Vector3::new(6.0, 1.0, 0.5),
            Vector3::new(8.0, 0.0, 0.0),
        ][i];
        Value::Vector3(coords)
    });
    animation.add_track(track);

    let (track, _) = build_track("rotation", "transform.rotation", &ROT_TIME, |i| {
        let q = [
            Vector4::new(0.0, 0.0, 0.0, 1.0),
            Vector4::new(0.0, 0.3827, 0.0, 0.9239),
            Vector4::new(0.0, 0.7071, 0.0, 0.7071),
            Vector4::new(0.0, 0.9239, 0.0, 0.3827),
            Vector4::new(0.0, 1.0, 0.0, 0.0),
        ][i];
        Value::Vector4(q)
    });
    animation.add_track(track);

    let (track, _) = build_track("scale", "transform.scale", &SCALE_TIME, |i| {
        let s = [
            Vector3::new(1.0, 1.0, 1.0),
            Vector3::new(1.2, 1.1, 1.2),
            Vector3::new(1.0, 1.0, 1.0),
            Vector3::new(0.9, 1.1, 0.9),
            Vector3::new(1.0, 1.0, 1.0),
            Vector3::new(1.3, 0.9, 1.3),
            Vector3::new(1.0, 1.0, 1.0),
            Vector3::new(1.1, 1.2, 1.1),
            Vector3::new(1.0, 1.0, 1.0),
        ][i];
        Value::Vector3(s)
    });
    animation.add_track(track);

    let (track, _) = build_track("color", "material.color", &COL_TIME, |i| {
        let c = &[
            Color::rgba(1.0, 0.2, 0.2, 1.0),
            Color::rgba(1.0, 0.8, 0.2, 1.0),
            Color::rgba(0.2, 1.0, 0.2, 1.0),
            Color::rgba(0.2, 0.5, 1.0, 1.0),
            Color::rgba(0.8, 0.2, 1.0, 1.0),
        ][i];
        Value::Color(c.clone())
    });
    animation.add_track(track);

    let (track, _) = build_track("intensity", "light.easing", &INT_TIME, |i| {
        let v = [0.5, 1.0, 0.3, 0.8, 0.5, 1.2, 0.2, 0.5][i];
        Value::Float(v)
    });
    animation.add_track(track);

    animation.metadata = AnimationMetadata {
        author: Some("WASM Animation Player Demo For Different types".to_string()),
        description: Some(
            "A complex robot animation showcasing position, rotation, scale, color, and intensity changes over time"
                .to_string(),
        ),
        frame_rate: 60f64,
        tags: vec!["demo".to_string(), "robot".to_string(), "complex".to_string()],
        ..animation.metadata
    };

    serde_json::to_string(&animation).unwrap_or_else(|_| "{}".to_owned())
}

/// Creates a test animation with various transition types.
#[wasm_bindgen]
pub fn create_test_animation() -> String {
    const TIMES: [f64; 8] = [0.0, 0.25, 0.5, 0.75, 1.0, 2.0, 3.0, 4.0];
    const VALUES: [f32; 8] = [0.5, 1.0, 0.3, 0.8, 0.5, 1.2, 0.2, 0.5];

    const TRACKS: &[(&str, &str, TransitionVariant)] = &[
        ("a", "a.step", TransitionVariant::Step),
        ("b", "b.cubic", TransitionVariant::Cubic),
        ("c", "c.linear", TransitionVariant::Linear),
        ("d", "d.bezier", TransitionVariant::Bezier),
        ("e", "e.spring", TransitionVariant::Spring),
        ("f", "f.hermite", TransitionVariant::Hermite),
        ("g", "g.catmullrom", TransitionVariant::Catmullrom),
        ("h", "h.bspline", TransitionVariant::Bspline),
    ];

    let mut animation = AnimationData::new("test_animation", "Transition Testing Animation");

    for &(name, property, variant) in TRACKS {
        let (track, ids) = build_track(name, property, &TIMES, |i| Value::Float(VALUES[i].into()));

        for pair in ids.windows(2) {
            animation.add_transition(AnimationTransition::new(pair[0], pair[1], variant));
        }

        animation.add_track(track);
    }

    animation.metadata = AnimationMetadata {
        author: Some("WASM Animation Player Demo".to_string()),
        description: Some(
            "A complex robot animation showcasing different transition changes over time"
                .to_string(),
        ),
        frame_rate: 60f64,
        tags: vec!["demo".to_string(), "complex".to_string()],
        ..animation.metadata
    };

    serde_json::to_string(&animation).unwrap_or_else(|_| "{}".to_owned())
}

/// A simple test function that returns a greeting.
#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Animation Player WASM is ready.", name)
}

/// Loads a test animation from a JSON string and returns it as a JSON string.
#[wasm_bindgen]
pub fn load_test_animation_from_json_wasm(json_str: &str) -> Result<String, JsValue> {
    load_test_animation_from_json(json_str)
        .map_err(|e| JsValue::from_str(&format!("Test animation load error: {:?}", e)))
        .and_then(|animation| {
            serde_json::to_string(&animation)
                .map_err(|e| JsValue::from_str(&format!("Animation serialization error: {}", e)))
        })
}
