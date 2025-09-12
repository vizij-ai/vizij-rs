use serde::Deserialize;

use crate::data::{AnimationData, Keypoint, Track, TrackSettings, Transitions};
use crate::ids::AnimId;
use crate::value::Value;

/// Public API: parse StoredAnimation-style JSON (see types/animation.ts and tests/fixtures/new_format.json)
/// into vizij-animation-core's canonical AnimationData (data.rs).
///
/// Notes:
/// - Duration is provided in milliseconds in the JSON and kept as milliseconds (duration_ms).
/// - Keypoint stamps are normalized [0,1] and kept normalized.
/// - Per-keypoint transitions { in?, out? } are preserved; defaults are applied at sampling time.
/// - Values are converted from untagged RawValue shapes into core Value enum.
pub fn parse_stored_animation_json(s: &str) -> Result<AnimationData, String> {
    let sa: StoredAnimation = serde_json::from_str(s).map_err(|e| format!("parse error: {e}"))?;

    let mut tracks: Vec<Track> = Vec::with_capacity(sa.tracks.len());
    for st in sa.tracks {
        let mut points: Vec<Keypoint> = Vec::with_capacity(st.points.len());
        for p in st.points {
            let value = to_core_value(&p.value)?;
            let transitions = p.transitions.map(|t| Transitions {
                r#in: t.r#in.map(|v| crate::data::Vec2 {
                    x: v.x as f32,
                    y: v.y as f32,
                }),
                r#out: t.r#out.map(|v| crate::data::Vec2 {
                    x: v.x as f32,
                    y: v.y as f32,
                }),
            });
            points.push(Keypoint {
                id: p.id,
                stamp: p.stamp as f32,
                value,
                transitions,
            });
        }

        tracks.push(Track {
            id: st.id,
            name: st.name,
            animatable_id: st.animatable_id,
            points,
            settings: st.settings.map(|s| TrackSettings { color: s.color }),
        });
    }

    let data = AnimationData {
        id: None::<AnimId>,
        name: sa.name,
        tracks,
        groups: sa.groups,
        duration_ms: sa.duration as u32,
    };
    // Basic validation (stamps in [0,1], non-decreasing, duration_ms > 0)
    data.validate_basic()?;
    Ok(data)
}

fn to_core_value(v: &RawValue) -> Result<Value, String> {
    match v {
        RawValue::Boolean(b) => Ok(Value::Bool(*b)),
        RawValue::Number(n) => Ok(Value::Scalar(*n as f32)),
        RawValue::String(s) => Ok(Value::Text(s.clone())),
        RawValue::Vector3 { x, y, z } => Ok(Value::Vec3([*x as f32, *y as f32, *z as f32])),
        RawValue::Vector2 { x, y } => Ok(Value::Vec2([*x as f32, *y as f32])),
        // Euler (r,p,y) mapped to Vec3 [r,p,y]; adapters can remap axes if needed.
        RawValue::Euler { r, p, y } => Ok(Value::Vec3([*r as f32, *p as f32, *y as f32])),
        RawValue::RGB { r, g, b } => Ok(Value::Color([*r as f32, *g as f32, *b as f32, 1.0])),
        RawValue::HSL { h, s, l } => {
            let (r, g, b) = hsl_to_rgb(*h as f32, *s as f32, *l as f32);
            Ok(Value::Color([r, g, b, 1.0]))
        }
    }
}

/// HSL (0..1) to RGB (0..1)
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let h = ((h % 1.0) + 1.0) % 1.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);

    if s == 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    (r, g, b)
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

// ----- JSON schema (serde) -----

#[derive(Debug, Deserialize)]
struct StoredAnimation {
    pub id: String,
    pub name: String,
    pub tracks: Vec<SaTrack>,
    pub groups: serde_json::Value,
    pub duration: u64, // milliseconds
}

#[derive(Debug, Deserialize)]
struct SaTrack {
    pub id: String,
    pub name: String,
    #[serde(rename = "animatableId")]
    pub animatable_id: String,
    pub points: Vec<SaPoint>,
    pub settings: Option<SaSettings>,
}

#[derive(Debug, Deserialize)]
struct SaSettings {
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SaPoint {
    pub id: String,
    pub stamp: f64, // 0..1
    pub value: RawValue,
    pub transitions: Option<PointTransitions>,
}

#[derive(Debug, Copy, Clone, Deserialize)]
struct Vec2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Deserialize)]
struct PointTransitions {
    #[serde(default)]
    #[serde(rename = "in")]
    pub r#in: Option<Vec2>,
    #[serde(default)]
    #[serde(rename = "out")]
    pub r#out: Option<Vec2>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawValue {
    Boolean(bool),
    Number(f64),
    String(String),
    // Put more specific shapes BEFORE less specific to avoid untagged matching pitfalls.
    Vector3 { x: f64, y: f64, z: f64 },
    Vector2 { x: f64, y: f64 },
    Euler { r: f64, p: f64, y: f64 },
    RGB { r: f64, g: f64, b: f64 },
    HSL { h: f64, s: f64, l: f64 },
}
