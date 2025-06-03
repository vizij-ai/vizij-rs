//! WebAssembly bindings for the animation engine
use crate::{
    animation::transition::AnimationTransition,
    animation::TransitionVariant,
    loaders::load_test_animation_from_json,
    value::{Color, Vector3, Vector4},
    AnimationKeypoint, AnimationTrack, KeypointId,
};
use crate::{
    animation::PlaybackMode,
    baking::{AnimationBaking, BakingConfig},
    AnimationConfig, AnimationData, AnimationEngine, AnimationTime, Value,
};
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;

// Set up panic hook for better error messages in WASM
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}

/// WASM wrapper for the animation engine
#[wasm_bindgen]
pub struct WasmAnimationEngine {
    engine: Arc<Mutex<AnimationEngine>>,
}

#[wasm_bindgen]
impl WasmAnimationEngine {
    /// Create a new animation engine
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: Option<String>) -> Result<WasmAnimationEngine, JsValue> {
        let config = if let Some(json) = config_json {
            serde_json::from_str::<AnimationConfig>(&json)
                .map_err(|e| JsValue::from_str(&format!("Config parse error: {}", e)))?
        } else {
            AnimationConfig::web_optimized()
        };

        let engine = AnimationEngine::new(config);

        Ok(WasmAnimationEngine {
            engine: Arc::new(Mutex::new(engine)),
        })
    }

    #[wasm_bindgen]
    pub fn load_animation(&mut self, animation_json: &str) -> Result<(), JsValue> {
        console_log(&format!(
            "Loading animation on wasm side {:?}",
            animation_json
        ));

        // Try to parse the animation JSON directly first
        let animation_data: AnimationData = match serde_json::from_str(animation_json) {
            Ok(data) => {
                console_log("Successfully parsed animation JSON directly");
                data
            }
            Err(parse_error) => {
                console_log(&format!("Direct JSON parse failed: {}, attempting fallback with load_test_animation_from_json_wasm", parse_error));

                // Try using the test animation loader as fallback
                match load_test_animation_from_json_wasm(animation_json) {
                    Ok(corrected_json) => {
                        console_log("Fallback loader succeeded, parsing corrected JSON");
                        serde_json::from_str(&corrected_json).map_err(|e| {
                            JsValue::from_str(&format!("Fallback JSON parse error: {}", e))
                        })?
                    }
                    Err(fallback_error) => {
                        console_log(&format!(
                            "Fallback loader also failed: {:?}",
                            fallback_error
                        ));
                        return Err(JsValue::from_str(&format!(
                            "Animation JSON parse error: {}. Fallback loader error: {:?}",
                            parse_error, fallback_error
                        )));
                    }
                }
            }
        };
        console_log(&format!("Parsed data on wasm side {:?}", animation_data));
        self.engine
            .lock()
            .map_err(|e| {
                let msg = format!("Engine lock poisoned: {}", e);
                console_log(&msg);
                JsValue::from_str(&msg)
            })?
            .load_animation_data(animation_data)
            .map_err(|e| JsValue::from_str(&format!("Load animation error: {:?}", e)))?;
        console_log(&format!("Finished on wasm side"));

        Ok(())
    }

    /// Create a new player
    #[wasm_bindgen]
    pub fn create_player(&mut self, player_id: &str) -> Result<(), JsValue> {
        self.engine
            .lock()
            .map_err(|e| {
                let msg = format!("Engine lock poisoned: {}", e);
                console_log(&msg);
                JsValue::from_str(&msg)
            })?
            .create_player(player_id)
            .map_err(|e| JsValue::from_str(&format!("Create player error: {:?}", e)))?;

        Ok(())
    }

    /// Add an animation instance to a player
    #[wasm_bindgen]
    pub fn add_instance(
        &mut self,
        player_id: &str,
        animation_id: &str,
    ) -> Result<String, JsValue> {
        let mut engine = self.engine.lock().map_err(|e| {
            let msg = format!("Engine lock poisoned: {}", e);
            console_log(&msg);
            JsValue::from_str(&msg)
        })?;

        // Add instance to player
        let instance_id = engine.add_animation_to_player(
            player_id,
            animation_id,
            None
        ).map_err(|e| {
            let msg = format!("Engine lock poisoned: {}", e);
            console_log(&msg);
            JsValue::from_str(&msg)})?;

        Ok(instance_id)
    }

    /// Start playback for a player
    #[wasm_bindgen]
    pub fn play(&mut self, player_id: &str) -> Result<(), JsValue> {
        self.engine
            .lock()
            .map_err(|e| {
                let msg = format!("Engine lock poisoned: {}", e);
                console_log(&msg);
                JsValue::from_str(&msg)
            })?
            .play_player(player_id)
            .map_err(|e| JsValue::from_str(&format!("Play error: {:?}", e)))?;
        Ok(())
    }

    /// Pause playback for a player
    #[wasm_bindgen]
    pub fn pause(&mut self, player_id: &str) -> Result<(), JsValue> {
        self.engine
            .lock()
            .map_err(|e| {
                let msg = format!("Engine lock poisoned: {}", e);
                console_log(&msg);
                JsValue::from_str(&msg)
            })?
            .pause_player(player_id)
            .map_err(|e| JsValue::from_str(&format!("Pause error: {:?}", e)))?;
        Ok(())
    }

    /// Stop playback for a player
    #[wasm_bindgen]
    pub fn stop(&mut self, player_id: &str) -> Result<(), JsValue> {
        self.engine
            .lock()
            .map_err(|e| {
                let msg = format!("Engine lock poisoned: {}", e);
                console_log(&msg);
                JsValue::from_str(&msg)
            })?
            .stop_player(player_id)
            .map_err(|e| JsValue::from_str(&format!("Stop error: {:?}", e)))?;
        Ok(())
    }

    /// Seek to a specific time for a player
    #[wasm_bindgen]
    pub fn seek(&mut self, player_id: &str, time_seconds: f64) -> Result<(), JsValue> {
        let time = AnimationTime::new(time_seconds)
            .map_err(|e| JsValue::from_str(&format!("Invalid time: {:?}", e)))?;

        self.engine
            .lock()
            .map_err(|e| {
                let msg = format!("Engine lock poisoned: {}", e);
                console_log(&msg);
                JsValue::from_str(&msg)
            })?
            .seek_player(player_id, time)
            .map_err(|e| JsValue::from_str(&format!("Seek error: {:?}", e)))?;
        Ok(())
    }

    /// Update the animation engine and get current values
    #[wasm_bindgen]
    pub fn update(&mut self, frame_delta_seconds: f64) -> Result<JsValue, JsValue> {
        let mut engine = self.engine.lock().map_err(|e| {
            let msg = format!("Engine lock poisoned: {}", e);
            console_log(&msg); // Log to console for debugging
            JsValue::from_str(&msg)
        })?;

        let values = match engine.update(frame_delta_seconds) {
            Ok(values) => values,
            Err(e) => {
                console_log(&format!("Engine update failed: {:?}", e));
                return Err(JsValue::from_str(&format!("Update error: {:?}", e)));
            }
        };

        // Convert the nested HashMap to a JsValue
        match serde_wasm_bindgen::to_value(&values) {
            Ok(js_value) => Ok(js_value),
            Err(e) => {
                console_log(&format!("Serialization failed: {}", e));
                Err(JsValue::from_str(&format!("Serialization error: {}", e)))
            }
        }
    }

    /// Get current playback state for a player
    #[wasm_bindgen]
    pub fn get_player_state(&self, player_id: &str) -> Result<String, JsValue> {
        let engine = self.engine.lock().map_err(|e| {
            let msg = format!("Engine lock poisoned: {}", e);
            console_log(&msg);
            JsValue::from_str(&msg)
        })?;
        let player_state = engine
            .get_player_state(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;
        let state_string = serde_json::to_string(player_state)
            .map_err(|e| JsValue::from_str(&format!("Export error: {}", e)))?;
        Ok(state_string)
    }

    /// Get current time for a player
    #[wasm_bindgen]
    pub fn get_player_time(&self, player_id: &str) -> Result<f64, JsValue> {
        let engine = self.engine.lock().map_err(|e| {
            let msg = format!("Engine lock poisoned: {}", e);
            console_log(&msg);
            JsValue::from_str(&msg)
        })?;
        let player = engine
            .get_player(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        Ok(player.current_time.as_seconds())
    }

    /// Get progress (0.0-1.0) for a player
    #[wasm_bindgen]
    pub fn get_player_progress(&self, player_id: &str) -> Result<f64, JsValue> {
        let engine = self.engine.lock().map_err(|e| {
            let msg = format!("Engine lock poisoned: {}", e);
            console_log(&msg);
            JsValue::from_str(&msg)
        })?;
        let player = engine
            .get_player(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        Ok(player.progress())
    }

    /// Get list of all player IDs
    #[wasm_bindgen]
    pub fn get_player_ids(&self) -> Result<Vec<String>, JsValue> {
        let engine = self.engine.lock().map_err(|e| {
            let msg = format!("Engine lock poisoned: {}", e);
            console_log(&msg);
            JsValue::from_str(&msg)
        })?;
        Ok(engine
            .player_ids()
            .into_iter()
            .map(|s| s.to_string())
            .collect())
    }

    /// Get engine performance metrics
    #[wasm_bindgen]
    pub fn get_metrics(&self) -> JsValue {
        let engine = match self.engine.lock() {
            Ok(engine) => engine,
            Err(e) => {
                let msg = format!("Engine lock poisoned: {}", e);
                console_log(&msg);
                return JsValue::from_str(&msg);
            }
        };
        let metrics = engine.metrics();

        serde_wasm_bindgen::to_value(metrics).unwrap_or(JsValue::NULL)
    }

    /// Update player configuration
    #[wasm_bindgen]
    pub fn update_player_config(
        &mut self,
        player_id: &str,
        config_json: &str,
    ) -> Result<(), JsValue> {
        console_log(&format!("Updating {} Config: {:?}", player_id, config_json));
        let mut engine = self.engine.lock().map_err(|e| {
            let msg = format!("Engine lock poisoned: {}", e);
            console_log(&msg);
            JsValue::from_str(&msg)
        })?;
        let player_config = engine
            .get_player_state_mut(player_id)
            .ok_or_else(|| JsValue::from_str("Player not found"))?;

        let config: serde_json::Value = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("Config JSON parse error: {}", e)))?;
        console_log(&format!("New Config: {:?}", config));

        // Handle speed setting (-5.0 to 5.0)
        if let Some(speed_val) = config.get("speed").and_then(|v| v.as_f64()) {
            if speed_val >= -5.0 && speed_val <= 5.0 {
                console_log(&format!("Setting speed: {:?}", speed_val));
                player_config.speed = speed_val;
            } else {
                return Err(JsValue::from_str(&format!(
                    "Speed must be between -5.0 and 5.0, got: {}",
                    speed_val
                )));
            }
        }

        // Handle playback mode
        if let Some(mode_str) = config.get("mode").and_then(|v| v.as_str()) {
            console_log(&format!("Setting mode: {:?}", mode_str));
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

        // Handle start time (positive floats)
        if let Some(start_time_val) = config.get("start_time").and_then(|v| v.as_f64()) {
            if start_time_val >= 0.0 {
                console_log(&format!("Setting start_time: {:?}", start_time_val));
                let start_time = AnimationTime::new(start_time_val)
                    .map_err(|e| JsValue::from_str(&format!("Invalid start time: {:?}", e)))?;
                player_config.start_time = start_time;
            } else {
                return Err(JsValue::from_str(&format!(
                    "Start time must be positive, got: {}",
                    start_time_val
                )));
            }
        }

        // Handle end time (positive floats or null)
        if let Some(end_time_val) = config.get("end_time") {
            if end_time_val.is_null() {
                console_log("Setting end_time to None");
                player_config.end_time = None;
            } else if let Some(end_time_f64) = end_time_val.as_f64() {
                if end_time_f64 >= 0.0 {
                    console_log(&format!("Setting end_time: {:?}", end_time_f64));
                    let end_time = AnimationTime::new(end_time_f64)
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

        // Validate that start_time < end_time if both are set
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
                console_log("Setting legacy loop: true");
                player_config.mode = PlaybackMode::Loop;
            }
        }
        if let Some(ping_pong_val) = config.get("ping_pong").and_then(|v| v.as_bool()) {
            if ping_pong_val {
                console_log("Setting legacy ping_pong: true");
                player_config.mode = PlaybackMode::PingPong;
            }
        }

        Ok(())
    }

    /// Export animation data as JSON
    #[wasm_bindgen]
    pub fn export_animation(&self, animation_id: &str) -> Result<String, JsValue> {
        let engine = self.engine.lock().map_err(|e| {
            let msg = format!("Engine lock poisoned: {}", e);
            console_log(&msg);
            JsValue::from_str(&msg)
        })?;
        let animation = engine
            .get_animation_data(animation_id)
            .ok_or_else(|| JsValue::from_str("Animation not found"))?;

        serde_json::to_string(animation)
            .map_err(|e| JsValue::from_str(&format!("Export error: {}", e)))
    }

    /// Get derivatives (rates of change) for all tracks at the current time for a specific player
    #[wasm_bindgen]
    pub fn get_derivatives(
        &mut self,
        player_id: &str,
        derivative_width_ms: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        // Convert derivative width from milliseconds to AnimationTime if provided
        let derivative_width =
            if let Some(width_ms) = derivative_width_ms {
                if width_ms <= 0.0 {
                    return Err(JsValue::from_str("Derivative width must be positive"));
                }
                Some(AnimationTime::new(width_ms / 1000000.0).map_err(|e| {
                    JsValue::from_str(&format!("Invalid derivative width: {:?}", e))
                })?)
            } else {
                None
            };

        // Calculate derivatives using the engine's helper method
        let derivatives = {
            let mut engine = self.engine.lock().map_err(|e| {
                let msg = format!("Engine lock poisoned: {}", e);
                console_log(&msg);
                JsValue::from_str(&msg)
            })?;
            engine
                .calculate_player_derivatives(player_id, derivative_width)
                .map_err(|e| JsValue::from_str(&format!("Calculate derivatives error: {:?}", e)))?
        };

        // Convert to JsValue
        serde_wasm_bindgen::to_value(&derivatives)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Bake an animation to pre-calculated values at specified frame rate
    #[wasm_bindgen]
    pub fn bake_animation(
        &mut self,
        animation_id: &str,
        config_json: Option<String>,
    ) -> Result<String, JsValue> {
        let mut engine = self.engine.lock().map_err(|e| {
            let msg = format!("Engine lock poisoned: {}", e);
            console_log(&msg);
            JsValue::from_str(&msg)
        })?;

        // Get the animation data
        let animation = engine
            .get_animation_data(animation_id)
            .ok_or_else(|| JsValue::from_str("Animation not found"))?
            .clone(); // Clone to avoid borrowing issues

        console_log(&format!("Wasm Baking with animation data: {:?}", animation));

        // Parse baking configuration
        let config = if let Some(json) = config_json {
            serde_json::from_str::<BakingConfig>(&json)
                .map_err(|e| JsValue::from_str(&format!("Baking config parse error: {}", e)))?
        } else {
            BakingConfig::default()
        };
        console_log(&format!("Wasm Baking with config {:?}", config));

        // Perform the baking
        let baked_data = animation
            .bake(&config, engine.interpolation_registry_mut())
            .map_err(|e| JsValue::from_str(&format!("Baking error: {:?}", e)))?;

        // Convert to JSON
        console_log(&format!("Wasm Baked: {:?}", baked_data));
        baked_data
            .to_json()
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {:?}", e)))
    }
}

/// Utility function to set up console logging from WASM
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

/// Log a message to the browser console
#[wasm_bindgen]
pub fn console_log(message: &str) {
    log(message);
}

/// Convert a Rust Value to JsValue for easy JavaScript access
#[wasm_bindgen]
pub fn value_to_js(value_json: &str) -> Result<JsValue, JsValue> {
    let value: Value = serde_json::from_str(value_json)
        .map_err(|e| JsValue::from_str(&format!("Value parse error: {}", e)))?;

    serde_wasm_bindgen::to_value(&value)
        .map_err(|e| JsValue::from_str(&format!("Value conversion error: {}", e)))
}

// Convenience – avoid spelling out zero vs. new().
#[inline]
fn time(t: f64) -> AnimationTime {
    if t.abs() < f64::EPSILON {
        AnimationTime::zero()
    } else {
        AnimationTime::new(t).expect("invalid time")
    }
}

/// Helper that builds a track, returns the finished track together with all key-point IDs.
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

#[wasm_bindgen]
pub fn create_animation_test_type() -> String {
    const POS_TIME: [f64; 5] = [0.0, 1.0, 2.0, 3.0, 4.1];
    const ROT_TIME: [f64; 5] = [0.0, 1.0, 2.0, 3.0, 4.0];
    const SCALE_TIME: [f64; 9] = [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0];
    const COL_TIME: [f64; 5] = [0.0, 1.0, 2.0, 3.0, 4.0];
    const INT_TIME: [f64; 8] = [0.0, 0.25, 0.5, 0.75, 1.0, 2.0, 3.0, 4.0];

    let mut animation = AnimationData::new("test_animation", "Robot Wave Animation");

    // Position ────────────────────────────────────────────────────────────────
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

    // Rotation ────────────────────────────────────────────────────────────────
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

    // Scale ───────────────────────────────────────────────────────────────────
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

    // Color ───────────────────────────────────────────────────────────────────
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

    // Intensity ───────────────────────────────────────────────────────────────
    let (track, _) = build_track("intensity", "light.easing", &INT_TIME, |i| {
        let v = [0.5, 1.0, 0.3, 0.8, 0.5, 1.2, 0.2, 0.5][i];
        Value::Float(v)
    });
    animation.add_track(track);

    // Metadata
    animation.metadata = animation.metadata
        .with_author("WASM Animation Player Demo For Different types")
        .with_description("A complex robot animation showcasing position, rotation, scale, color, and intensity changes over time")
        .add_tag("demo").add_tag("robot").add_tag("complex")
        .with_frame_rate(60.0);

    serde_json::to_string(&animation).unwrap_or_else(|_| "{}".to_owned())
}

// ------------------------------------------------------------------------------------------------
// 2. create_test_animation
// ------------------------------------------------------------------------------------------------
#[wasm_bindgen]
pub fn create_test_animation() -> String {
    const TIMES: [f64; 8] = [0.0, 0.25, 0.5, 0.75, 1.0, 2.0, 3.0, 4.0];
    const VALUES: [f32; 8] = [0.5, 1.0, 0.3, 0.8, 0.5, 1.2, 0.2, 0.5];

    // (track-name, property, transition variant)
    const TRACKS: &[(&str, &str, TransitionVariant)] = &[
        ("a", "a.step", TransitionVariant::Step),
        ("b", "b.cubic", TransitionVariant::Cubic),
        ("c", "c.linear", TransitionVariant::Linear),
        ("d", "d.bezier", TransitionVariant::Bezier),
        ("e", "e.spring", TransitionVariant::Spring),
    ];

    let mut animation = AnimationData::new("test_animation", "Transition Testing Animation");

    // Build every track the same way
    for &(name, property, variant) in TRACKS {
        // Build track + remember each id
        let (track, ids) = build_track(name, property, &TIMES, |i| Value::Float(VALUES[i].into()));

        // Add transitions between consecutive key-points
        for pair in ids.windows(2) {
            animation.add_transition(AnimationTransition::new(pair[0], pair[1], variant));
        }

        animation.add_track(track);
    }

    animation.metadata = animation
        .metadata
        .with_author("WASM Animation Player Demo")
        .with_description(
            "A complex robot animation showcasing different transition changes over time",
        )
        .add_tag("demo")
        .add_tag("complex")
        .with_frame_rate(60.0);

    serde_json::to_string(&animation).unwrap_or_else(|_| "{}".to_owned())
}

/// Greet function for testing
#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Animation Player WASM is ready.", name)
}

/// Load and convert a test animation from JSON format
#[wasm_bindgen]
pub fn load_test_animation_from_json_wasm(json_str: &str) -> Result<String, JsValue> {
    load_test_animation_from_json(json_str)
        .map_err(|e| JsValue::from_str(&format!("Test animation load error: {:?}", e)))
        .and_then(|animation| {
            serde_json::to_string(&animation)
                .map_err(|e| JsValue::from_str(&format!("Animation serialization error: {}", e)))
        })
}
