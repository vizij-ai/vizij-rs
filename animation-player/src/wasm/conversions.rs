//! Data conversion and test data generation.
use crate::{
    animation::{transition::AnimationTransition, AnimationMetadata, TransitionVariant},
    value::{Color, Vector3, Vector4},
    AnimationData,
    AnimationKeypoint, AnimationTrack, AnimationTime, KeypointId, Value,
};
use wasm_bindgen::prelude::*;

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
