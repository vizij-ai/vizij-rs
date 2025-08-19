//! Animation data management for WebAssembly.
use super::engine::WasmAnimationEngine;
use crate::{
    animation::{
        baking::{AnimationBaking, BakingConfig},
        transition::AnimationTransition,
        AnimationMetadata,
        TransitionVariant,
    },
    ecs::{components::AnimationInstance, resources::IdMapping},
    interpolation::InterpolationRegistry,
    loaders::studio_animation::load_test_animation_from_json,
    value::{Color, Vector3, Vector4},
    AnimationData, AnimationKeypoint, AnimationTime, AnimationTrack, KeypointId, Value,
};
use bevy::prelude::*;
use serde_json::Value as JsonValue;
use wasm_bindgen::prelude::*;
use std::collections::HashMap;


/// Converts a JSON string representing a `Value` into a `JsValue`.
///
/// This is useful for converting individual animation values for use in JavaScript.
///
/// # Example
///
/// ```javascript
/// const valueJson = `{"Vector3":[1, 2, 3]}`;
/// const jsValue = value_to_js(valueJson);
/// console.log(jsValue); // { "Vector3": [1, 2, 3] }
/// ```
///
/// @param {string} value_json - A JSON string of a `Value` enum.
/// @returns {any} The JavaScript representation of the value.
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
///
/// This function generates a complex animation with tracks for position, rotation, scale,
/// color, and intensity, demonstrating the engine's ability to handle different data types.
///
/// @returns {string} A JSON string representing the test animation.
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
///
/// This function generates an animation that uses every available transition type (Step, Linear,
/// Cubic, Bezier, etc.) to allow for visual testing and verification.
///
/// @returns {string} A JSON string representing the test animation.
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

#[wasm_bindgen]
impl WasmAnimationEngine {
    /// Loads animation data from a JSON string (snake_case) with test_animation fallback.
    #[wasm_bindgen]
    pub fn load_animation_wasm(&mut self, animation_json: &str) -> Result<String, JsValue> {
        // Try to parse native format, fallback to test animation format
        let data: AnimationData = match serde_json::from_str(animation_json) {
            Ok(d) => d,
            Err(primary_err) => match load_test_animation_from_json(animation_json) {
                Ok(converted) => converted,
                Err(fallback_err) => {
                    return Err(JsValue::from_str(&format!(
                        "Animation JSON parse error: {}. Fallback loader error: {}",
                        primary_err, fallback_err
                    )))
                }
            },
        };

        let handle = {
            let mut assets = self.app.world_mut().resource_mut::<Assets<AnimationData>>();
            assets.add(data)
        };

        let id = uuid::Uuid::new_v4().to_string();
        {
            let mut id_mapping = self.app.world_mut().resource_mut::<IdMapping>();
            id_mapping.animations.insert(id.clone(), handle);
        }

        Ok(id)
    }

    /// Adds an animation instance to a player, with optional configuration.
    ///
    /// Config JSON keys:
    /// - weight: number (default 1.0)
    /// - timeScale: number (default 1.0)
    /// - instanceStartTime: number seconds (default 0.0)
    /// - enabled: boolean (default true)
    #[wasm_bindgen]
    pub fn add_instance(
        &mut self,
        player_id: &str,
        animation_id: &str,
        config_json: Option<String>,
    ) -> Result<String, JsValue> {
        let player_entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        // Fetch animation handle
        let anim_handle = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            id_mapping
                .animations
                .get(animation_id)
                .cloned()
                .ok_or_else(|| JsValue::from_str("Animation not found"))?
        };

        // Defaults
        let mut weight: f32 = 1.0;
        let mut time_scale: f32 = 1.0;
        let mut start_time = AnimationTime::zero();
        let mut enabled = true;

        if let Some(json) = config_json {
            let cfg: JsonValue = serde_json::from_str(&json)
                .map_err(|e| JsValue::from_str(&format!("Instance config parse error: {}", e)))?;
            if let Some(w) = cfg.get("weight").and_then(|v| v.as_f64()) {
                if w.is_finite() {
                    weight = w as f32;
                }
            }
            if let Some(ts) = cfg.get("timeScale").and_then(|v| v.as_f64()) {
                if ts.is_finite() {
                    time_scale = ts as f32;
                }
            }
            if let Some(st) = cfg.get("instanceStartTime").and_then(|v| v.as_f64()) {
                start_time = AnimationTime::from_seconds(st)
                    .map_err(|e| JsValue::from_str(&format!("Invalid instanceStartTime: {:?}", e)))?;
            }
            if let Some(en) = cfg.get("enabled").and_then(|v| v.as_bool()) {
                enabled = en;
            }
        }

        // Spawn instance entity
        let instance_entity = self
            .app
            .world_mut()
            .spawn(AnimationInstance {
                animation: anim_handle,
                weight,
                time_scale,
                start_time,
                enabled,
            })
            .id();

        // Parent to player
        self.app
            .world_mut()
            .entity_mut(player_entity)
            .add_child(instance_entity);

        // Register instance ID mapping
        let instance_id = uuid::Uuid::new_v4().to_string();
        {
            let mut id_mapping = self.app.world_mut().resource_mut::<IdMapping>();
            id_mapping.instances.insert(instance_id.clone(), instance_entity);
        }

        Ok(instance_id)
    }

    /// Removes an animation instance from a player.
    #[wasm_bindgen]
    pub fn remove_instance(&mut self, _player_id: &str, instance_id: &str) -> Result<(), JsValue> {
        let instance_entity = {
            let mut id_mapping = self.app.world_mut().resource_mut::<IdMapping>();
            let ent = id_mapping
                .instances
                .remove(instance_id)
                .ok_or_else(|| JsValue::from_str("Instance not found"))?;
            ent
        };
        // Despawn instance entity
        if self.app.world().get_entity(instance_entity).is_ok() {
            self.app.world_mut().entity_mut(instance_entity).despawn();
        }
        Ok(())
    }

    /// Updates the configuration of an existing animation instance.
    ///
    /// Supported keys: weight, timeScale, instanceStartTime, enabled
    #[wasm_bindgen]
    pub fn update_instance_config(
        &mut self,
        _player_id: &str,
        instance_id: &str,
        config_json: &str,
    ) -> Result<(), JsValue> {
        let entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .instances
                .get(instance_id)
                .ok_or_else(|| JsValue::from_str("Instance not found"))?
        };

        let cfg: JsonValue = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("Config JSON parse error: {}", e)))?;

        if let Some(mut inst) = self
            .app
            .world_mut()
            .get_mut::<AnimationInstance>(entity)
        {
            if let Some(w) = cfg.get("weight").and_then(|v| v.as_f64()) {
                if w.is_finite() {
                    inst.weight = w as f32;
                }
            }
            if let Some(ts) = cfg.get("timeScale").and_then(|v| v.as_f64()) {
                if ts.is_finite() {
                    inst.time_scale = ts as f32;
                }
            }
            if let Some(st) = cfg.get("instanceStartTime").and_then(|v| v.as_f64()) {
                inst.start_time = AnimationTime::from_seconds(st)
                    .map_err(|e| JsValue::from_str(&format!("Invalid instanceStartTime: {:?}", e)))?;
            }
            if let Some(en) = cfg.get("enabled").and_then(|v| v.as_bool()) {
                inst.enabled = en;
            }
        }

        Ok(())
    }

    /// Returns the configuration of an existing animation instance as JSON.
    #[wasm_bindgen]
    pub fn get_instance_config(
        &self,
        _player_id: &str,
        instance_id: &str,
    ) -> Result<String, JsValue> {
        let entity = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            *id_mapping
                .instances
                .get(instance_id)
                .ok_or_else(|| JsValue::from_str("Instance not found"))?
        };

        let inst = self
            .app
            .world()
            .get::<AnimationInstance>(entity)
            .ok_or_else(|| JsValue::from_str("Instance not found"))?;

        let json = serde_json::json!({
            "weight": inst.weight as f64,
            "timeScale": inst.time_scale as f64,
            "instanceStartTime": inst.start_time.as_seconds(),
            "enabled": inst.enabled
        });
        serde_json::to_string(&json)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Exports a loaded animation as a JSON string.
    #[wasm_bindgen]
    pub fn export_animation(&self, animation_id: &str) -> Result<String, JsValue> {
        let handle = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            id_mapping
                .animations
                .get(animation_id)
                .cloned()
                .ok_or_else(|| JsValue::from_str("Animation not found"))?
        };
        let assets = self.app.world().resource::<Assets<AnimationData>>();
        let anim = assets
            .get(&handle)
            .ok_or_else(|| JsValue::from_str("Animation asset not loaded"))?;
        serde_json::to_string(anim)
            .map_err(|e| JsValue::from_str(&format!("Export error: {}", e)))
    }

    /// Bakes an animation and returns baked JSON string (parity with non-ECS).
    #[wasm_bindgen]
    pub fn bake_animation(
        &mut self,
        animation_id: &str,
        config_json: Option<String>,
    ) -> Result<String, JsValue> {
        let handle = {
            let id_mapping = self.app.world().resource::<IdMapping>();
            id_mapping
                .animations
                .get(animation_id)
                .cloned()
                .ok_or_else(|| JsValue::from_str("Animation not found"))?
        };

        let assets = self.app.world().resource::<Assets<AnimationData>>();
        let anim = assets
            .get(&handle)
            .ok_or_else(|| JsValue::from_str("Animation asset not loaded"))?
            .clone();

        let config = if let Some(json) = config_json {
            serde_json::from_str::<BakingConfig>(&json)
                .map_err(|e| JsValue::from_str(&format!("Baking config parse error: {}", e)))?
        } else {
            BakingConfig::default()
        };

        let mut registry = self.app.world_mut().resource_mut::<InterpolationRegistry>();
        let baked = anim
            .bake(&config, &mut registry)
            .map_err(|e| JsValue::from_str(&format!("Baking error: {:?}", e)))?;
        baked
            .to_json()
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {:?}", e)))
    }

    // camelCase aliases for existing snake_case APIs (parity with ECS consumers)
    #[wasm_bindgen(js_name = addInstance)]
    pub fn add_instance_camel(
        &mut self,
        player_id: &str,
        animation_id: &str,
        config_json: Option<String>,
    ) -> Result<String, JsValue> {
        self.add_instance(player_id, animation_id, config_json)
    }

    #[wasm_bindgen(js_name = removeInstance)]
    pub fn remove_instance_camel(&mut self, player_id: &str, instance_id: &str) -> Result<(), JsValue> {
        self.remove_instance(player_id, instance_id)
    }

    #[wasm_bindgen(js_name = updateInstanceConfig)]
    pub fn update_instance_config_camel(
        &mut self,
        player_id: &str,
        instance_id: &str,
        config_json: &str,
    ) -> Result<(), JsValue> {
        self.update_instance_config(player_id, instance_id, config_json)
    }

    #[wasm_bindgen(js_name = getInstanceConfig)]
    pub fn get_instance_config_camel(
        &self,
        player_id: &str,
        instance_id: &str,
    ) -> Result<String, JsValue> {
        self.get_instance_config(player_id, instance_id)
    }

    #[wasm_bindgen(js_name = exportAnimation)]
    pub fn export_animation_camel(&self, animation_id: &str) -> Result<String, JsValue> {
        self.export_animation(animation_id)
    }

    #[wasm_bindgen(js_name = bakeAnimation)]
    pub fn bake_animation_camel(
        &mut self,
        animation_id: &str,
        config_json: Option<String>,
    ) -> Result<String, JsValue> {
        self.bake_animation(animation_id, config_json)
    }
}

#[wasm_bindgen]
impl WasmAnimationEngine {
    /// Calculates derivatives for a player's current blended output.
    /// derivative_width_ms (if provided) is in milliseconds of PLAYER time.
    #[wasm_bindgen]
    pub fn get_derivatives(
        &mut self,
        player_id: &str,
        derivative_width_ms: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        // Resolve derivative width in player time
        let width_player: Option<AnimationTime> = match derivative_width_ms {
            Some(ms) => {
                if ms <= 0.0 {
                    return Err(JsValue::from_str("Derivative width must be positive"));
                }
                Some(
                    AnimationTime::from_millis(ms)
                        .map_err(|e| JsValue::from_str(&format!("Invalid derivative width: {:?}", e)))?,
                )
            }
            None => None,
        };

        // Access world mutably to read components/resources and interpolation registry
        let world = self.app.world_mut();

        // Resolve player entity
        let player_entity = {
            let id_mapping = world.resource::<IdMapping>();
            *id_mapping
                .players
                .get(player_id)
                .ok_or_else(|| JsValue::from_str("Player not found"))?
        };

        // Snapshot children to avoid borrow conflicts
        let children_vec: Vec<Entity> = world
            .get::<Children>(player_entity)
            .map(|c| c.to_vec())
            .unwrap_or_default();

        // Read player's current time in a short scope to avoid borrow conflicts
        let player_current_time = {
            let player = world
                .get::<crate::ecs::components::AnimationPlayer>(player_entity)
                .ok_or_else(|| JsValue::from_str("Player not found"))?;
            player.current_time.as_seconds()
        };

        // Prepare instance data without holding overlapping borrows
        struct RawPrepared {
            weight: f32,
            time_scale: f32,
            start_time_secs: f64,
            anim_handle: Handle<AnimationData>,
        }

        let mut raw_instances: Vec<RawPrepared> = Vec::new();
        for child in &children_vec {
            if let Some(instance) = world.get::<crate::ecs::components::AnimationInstance>(*child) {
                if !instance.enabled || instance.weight == 0.0 {
                    continue;
                }
                raw_instances.push(RawPrepared {
                    weight: instance.weight,
                    time_scale: instance.time_scale,
                    start_time_secs: instance.start_time.as_seconds(),
                    anim_handle: instance.animation.clone(),
                });
            }
        }

        // Compute local times and clone animations while only immutably borrowing resources
        struct Prepared {
            weight: f32,
            local_time: AnimationTime,
            width_local: Option<AnimationTime>,
            anim: AnimationData,
        }

        let mut prepared: Vec<Prepared> = Vec::new();
        {
            let assets = world.resource::<Assets<AnimationData>>();
            for r in raw_instances {
                // Local time of the instance
                let local_secs =
                    (player_current_time - r.start_time_secs) * (r.time_scale as f64);
                let local_time = AnimationTime::from_seconds(local_secs.max(0.0))
                    .map_err(|e| JsValue::from_str(&format!("Invalid local time: {:?}", e)))?;

                // Width in local time (chain rule)
                let width_local = width_player.as_ref().map(|w| {
                    AnimationTime::from_seconds(w.as_seconds() * (r.time_scale.abs() as f64))
                        .unwrap_or_else(|_| *w)
                });

                if let Some(anim) = assets.get(&r.anim_handle) {
                    prepared.push(Prepared {
                        weight: r.weight,
                        local_time,
                        width_local,
                        anim: anim.clone(),
                    });
                }
            }
        }

        // Accumulator: target -> list of (weight, Value-derivative)
        let mut acc: HashMap<String, Vec<(f32, Value)>> = HashMap::new();

        {
            let mut registry = world.resource_mut::<InterpolationRegistry>();
            for p in prepared.iter() {
                for track in p.anim.tracks.values() {
                    let transition =
                        p.anim.get_track_transition_for_time(p.local_time, &track.id);
                    if let Some(deriv) = track.derivative_at_time(
                        p.local_time,
                        &mut registry,
                        transition,
                        p.width_local,
                        &p.anim,
                    ) {
                        acc.entry(track.target.clone())
                            .or_default()
                            .push((p.weight, deriv));
                    }
                }
            }
        }

        // Blend accumulated derivatives per target (component-wise average by weight)
        let mut blended: HashMap<String, Value> = HashMap::new();
        for (target, list) in acc {
            if list.is_empty() {
                continue;
            }
            let total_w: f32 = list.iter().map(|(w, _)| *w).sum();
            if total_w == 0.0 {
                continue;
            }

            // Use interpolatable components for general cases, and handle Transform specially
            let value_type = list[0].1.value_type();
            let merged = match value_type {
                crate::value::ValueType::Transform => {
                    // For Transform derivative, rotation derivative is a Vector4 or Vector3 encoded inside Transform
                    // We can average each sub-component linearly by weight
                    let mut sum_pos = crate::value::Vector3::zero();
                    let mut sum_rot = crate::value::Vector4::new(0.0, 0.0, 0.0, 0.0);
                    let mut sum_scale = crate::value::Vector3::zero();
                    for (w, v) in &list {
                        if let Value::Transform(t) = v {
                            let wn = (*w / total_w) as f64;
                            sum_pos.x += t.position.x * wn;
                            sum_pos.y += t.position.y * wn;
                            sum_pos.z += t.position.z * wn;
                            sum_rot.x += t.rotation.x * wn;
                            sum_rot.y += t.rotation.y * wn;
                            sum_rot.z += t.rotation.z * wn;
                            sum_rot.w += t.rotation.w * wn;
                            sum_scale.x += t.scale.x * wn;
                            sum_scale.y += t.scale.y * wn;
                            sum_scale.z += t.scale.z * wn;
                        }
                    }
                    Value::Transform(crate::value::Transform::new(sum_pos, sum_rot, sum_scale))
                }
                _ => {
                    let mut comps = vec![0.0; list[0].1.interpolatable_components().len()];
                    for (w, v) in &list {
                        let wn = (*w / total_w) as f64;
                        for (i, c) in v.interpolatable_components().iter().enumerate() {
                            comps[i] += c * wn;
                        }
                    }
                    Value::from_components(value_type, &comps).unwrap_or_else(|_| list[0].1.clone())
                }
            };

            blended.insert(target, merged);
        }

        serde_wasm_bindgen::to_value(&blended)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// camelCase alias for get_derivatives
    #[wasm_bindgen(js_name = getDerivatives)]
    pub fn get_derivatives_camel(
        &mut self,
        player_id: &str,
        derivative_width_ms: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        self.get_derivatives(player_id, derivative_width_ms)
    }
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

/// Logs a message to the browser console.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

/// A convenience function for logging from Rust to the browser console.
///
/// # Example
///
/// ```javascript
/// import { console_log } from "./pkg/ecs_animation_player.js";
/// console_log("Hello from Rust!");
/// ```
///
/// @param {string} message - The message to log.
#[wasm_bindgen]
pub fn console_log(message: &str) {
    log(message);
}

/// A simple test function that returns a greeting.
///
/// # Example
///
/// ```javascript
/// import { greet } from "./pkg/ecs_animation_player.js";
/// const greeting = greet("World");
/// console.log(greeting); // "Hello, World! ECS Animation Player WASM is ready."
/// ```
///
/// @param {string} name - The name to include in the greeting.
/// @returns {string} The greeting message.
#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! ECS Animation Player WASM is ready.", name)
}

/// Sets up a panic hook to log panic messages to the browser console.
#[wasm_bindgen(start)]
pub fn on_start() {
    console_error_panic_hook::set_once();
}
