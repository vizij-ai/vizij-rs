//! Parsing helpers for Vizij/Studio stored-animation JSON formats.

use serde::Deserialize;

use crate::data::{
    AnimationData, AuthoredTransition, Keypoint, Track, TrackSettings, Transitions,
    Vec2 as CoreVec2, CURRENT_ANIMATION_FORMAT_VERSION,
};
use crate::ids::AnimId;
use vizij_api_core::Value;

const LEGACY_DEFAULT_DURATION_MS: u64 = 5000;
const LEGACY_VIZIJ_DEFAULT_OUT: CoreVec2 = CoreVec2 { x: 0.42, y: 0.0 };
const LEGACY_VIZIJ_DEFAULT_IN: CoreVec2 = CoreVec2 { x: 0.58, y: 1.0 };

/// Public API: parse StoredAnimation-style JSON into vizij-animation-core's canonical
/// `AnimationData`.
///
/// Notes:
/// - Studio v2 (`formatVersion: 2`) uses millisecond stamps and anchor-relative transition deltas.
/// - Studio v1 (`formatVersion: 1`) follows Studio's migration rule: normalized stamps and
///   transition x-deltas are scaled by duration, transition y-deltas are preserved.
/// - Assets without `formatVersion` are intentionally treated as legacy Vizij assets, not Studio
///   v1. They used normalized stamps and segment-normalized cubic-bezier handles; those handles are
///   materialized into Studio v2 explicit deltas so the sampler does not need old implicit defaults.
/// - Values are converted from untagged RawValue shapes into core Value enum.
pub fn parse_stored_animation_json(s: &str) -> Result<AnimationData, String> {
    let sa: StoredAnimation = serde_json::from_str(s).map_err(|e| format!("parse error: {e}"))?;
    let duration = sa.duration.unwrap_or(LEGACY_DEFAULT_DURATION_MS);
    let mut tracks: Vec<Track> = Vec::with_capacity(sa.tracks.len());

    for st in sa.tracks {
        let points = match sa.format_version {
            Some(CURRENT_ANIMATION_FORMAT_VERSION) => parse_studio_v2_points(st.points)?,
            Some(1) => parse_studio_v1_points(st.points, duration)?,
            _ => parse_legacy_vizij_points(st.points, duration)?,
        };

        tracks.push(Track {
            id: st.id,
            name: st.name.unwrap_or_else(|| "Unnamed Track".to_string()),
            animatable_id: st.animatable_id,
            points,
            settings: st.settings.map(|s| TrackSettings { color: s.color }),
        });
    }

    let max_stamp_ms = tracks
        .iter()
        .flat_map(|track| track.points.iter().map(|point| point.stamp))
        .filter(|stamp| stamp.is_finite() && *stamp >= 0.0)
        .fold(0.0_f32, f32::max)
        .ceil() as u32;
    let viewport_extent_ms = sa
        .default_viewport_extent
        .or(sa.duration)
        .unwrap_or(LEGACY_DEFAULT_DURATION_MS) as u32;
    let duration_ms = max_stamp_ms.max(viewport_extent_ms).max(1);

    let data = AnimationData {
        id: None::<AnimId>,
        name: sa
            .name
            .unwrap_or_else(|| sa.id.unwrap_or_else(|| "Untitled Animation".into())),
        tracks,
        groups: sa.groups.unwrap_or_else(|| serde_json::json!([])),
        duration_ms,
    };
    data.validate_basic()?;
    Ok(data)
}

fn parse_studio_v2_points(points: Vec<SaPoint>) -> Result<Vec<Keypoint>, String> {
    points
        .into_iter()
        .map(|point| {
            Ok(Keypoint {
                id: point.id,
                stamp: point.stamp as f32,
                value: to_core_value(&point.value)?,
                transitions: point.transitions.map(to_core_transitions),
            })
        })
        .collect()
}

fn parse_studio_v1_points(points: Vec<SaPoint>, duration_ms: u64) -> Result<Vec<Keypoint>, String> {
    let duration = duration_ms as f32;
    points
        .into_iter()
        .map(|point| {
            Ok(Keypoint {
                id: point.id,
                stamp: (point.stamp as f32) * duration,
                value: to_core_value(&point.value)?,
                transitions: point
                    .transitions
                    .map(|transitions| scale_studio_v1_transitions(transitions, duration)),
            })
        })
        .collect()
}

fn parse_legacy_vizij_points(
    points: Vec<SaPoint>,
    duration_ms: u64,
) -> Result<Vec<Keypoint>, String> {
    let duration = duration_ms as f32;
    let mut source: Vec<(SaPoint, Value)> = Vec::with_capacity(points.len());
    for point in points {
        let value = to_core_value(&point.value)?;
        source.push((point, value));
    }

    let mut converted: Vec<Keypoint> = source
        .iter()
        .map(|(point, value)| Keypoint {
            id: point.id.clone(),
            stamp: (point.stamp as f32) * duration,
            value: value.clone(),
            transitions: point.transitions.as_ref().and_then(|t| {
                let named_in = t
                    .r#in
                    .as_ref()
                    .and_then(|transition| transition.as_name_transition());
                let named_out = t
                    .r#out
                    .as_ref()
                    .and_then(|transition| transition.as_name_transition());
                if named_in.is_none() && named_out.is_none() && t.pairing.is_none() {
                    None
                } else {
                    Some(Transitions {
                        r#in: named_in,
                        r#out: named_out,
                        pairing: t.pairing.clone(),
                    })
                }
            }),
        })
        .collect();

    for index in 0..source.len() {
        if index + 1 < source.len() {
            let (left_point, left_value) = &source[index];
            let (_, right_value) = &source[index + 1];
            let span = normalized_segment_span(left_point, &source[index + 1].0, duration);
            let authored = left_point
                .transitions
                .as_ref()
                .and_then(|transitions| transitions.r#out.as_ref())
                .and_then(|transition| transition.as_explicit_vec2())
                .unwrap_or(LEGACY_VIZIJ_DEFAULT_OUT);
            let y = value_delta(left_value, right_value)
                .map(|delta| authored.y * delta)
                .unwrap_or(authored.y);
            ensure_transitions(&mut converted[index]).r#out =
                Some(AuthoredTransition::explicit(authored.x * span, y));
        }

        if index > 0 {
            let (_, left_value) = &source[index - 1];
            let (right_point, right_value) = &source[index];
            let span = normalized_segment_span(&source[index - 1].0, right_point, duration);
            let authored = right_point
                .transitions
                .as_ref()
                .and_then(|transitions| transitions.r#in.as_ref())
                .and_then(|transition| transition.as_explicit_vec2())
                .unwrap_or(LEGACY_VIZIJ_DEFAULT_IN);
            let y = value_delta(left_value, right_value)
                .map(|delta| (authored.y - 1.0) * delta)
                .unwrap_or(authored.y - 1.0);
            ensure_transitions(&mut converted[index]).r#in =
                Some(AuthoredTransition::explicit((authored.x - 1.0) * span, y));
        }
    }

    Ok(converted)
}

fn normalized_segment_span(left: &SaPoint, right: &SaPoint, duration: f32) -> f32 {
    (((right.stamp - left.stamp) as f32) * duration).max(f32::EPSILON)
}

fn ensure_transitions(point: &mut Keypoint) -> &mut Transitions {
    point.transitions.get_or_insert_with(Transitions::default)
}

fn value_delta(left: &Value, right: &Value) -> Option<f32> {
    match (left, right) {
        (Value::Float(a), Value::Float(b)) => Some(b - a),
        _ => None,
    }
}

fn scale_studio_v1_transitions(transitions: PointTransitions, duration: f32) -> Transitions {
    Transitions {
        r#in: transitions
            .r#in
            .map(|transition| scale_studio_v1_transition(transition, duration)),
        r#out: transitions
            .r#out
            .map(|transition| scale_studio_v1_transition(transition, duration)),
        pairing: transitions.pairing,
    }
}

fn scale_studio_v1_transition(transition: RawTransition, duration: f32) -> AuthoredTransition {
    match transition {
        RawTransition::Explicit(v) => {
            AuthoredTransition::explicit(v.x as f32 * duration, v.y as f32)
        }
        RawTransition::Name(name) => AuthoredTransition::name(name),
    }
}

fn to_core_transitions(transitions: PointTransitions) -> Transitions {
    Transitions {
        r#in: transitions.r#in.map(to_core_transition),
        r#out: transitions.r#out.map(to_core_transition),
        pairing: transitions.pairing,
    }
}

fn to_core_transition(transition: RawTransition) -> AuthoredTransition {
    match transition {
        RawTransition::Explicit(v) => AuthoredTransition::explicit(v.x as f32, v.y as f32),
        RawTransition::Name(name) => AuthoredTransition::name(name),
    }
}

fn to_core_value(v: &RawValue) -> Result<Value, String> {
    match v {
        RawValue::Boolean(b) => Ok(Value::Bool(*b)),
        RawValue::Number(n) => Ok(Value::Float(*n as f32)),
        RawValue::String(s) => Ok(Value::Text(s.clone())),
        RawValue::Vector3 { x, y, z } => Ok(Value::Vec3([*x as f32, *y as f32, *z as f32])),
        RawValue::Vector2 { x, y } => Ok(Value::Vec2([*x as f32, *y as f32])),
        RawValue::Quat { x, y, z, w } => {
            Ok(Value::Quat([*x as f32, *y as f32, *z as f32, *w as f32]))
        }
        RawValue::Transform(TransformComponents {
            translation,
            rotation,
            scale,
        }) => Ok(Value::Transform {
            translation: [
                translation.x as f32,
                translation.y as f32,
                translation.z as f32,
            ],
            rotation: [
                rotation.x as f32,
                rotation.y as f32,
                rotation.z as f32,
                rotation.w as f32,
            ],
            scale: [scale.x as f32, scale.y as f32, scale.z as f32],
        }),
        // Euler (r,p,y) mapped to Vec3 [r,p,y]; adapters can remap axes if needed.
        RawValue::Euler { r, p, y } => Ok(Value::Vec3([*r as f32, *p as f32, *y as f32])),
        RawValue::Rgb { r, g, b } => Ok(Value::ColorRgba([*r as f32, *g as f32, *b as f32, 1.0])),
        RawValue::Hsl { h, s, l } => {
            let (r, g, b) = hsl_to_rgb(*h as f32, *s as f32, *l as f32);
            Ok(Value::ColorRgba([r, g, b, 1.0]))
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
    pub id: Option<String>,
    pub name: Option<String>,
    pub tracks: Vec<SaTrack>,
    pub groups: Option<serde_json::Value>,
    pub duration: Option<u64>,
    #[serde(rename = "defaultViewportExtent")]
    pub default_viewport_extent: Option<u64>,
    #[serde(rename = "formatVersion")]
    pub format_version: Option<u8>,
}

#[derive(Debug, Deserialize)]
struct SaTrack {
    pub id: String,
    pub name: Option<String>,
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
    pub stamp: f64,
    pub value: RawValue,
    pub transitions: Option<PointTransitions>,
}

#[derive(Debug, Copy, Clone, Deserialize)]
struct Vec2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Copy, Clone, Deserialize)]
struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Copy, Clone, Deserialize)]
struct Quat {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct TransformComponents {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

#[derive(Debug, Deserialize)]
struct PointTransitions {
    #[serde(default)]
    #[serde(rename = "in")]
    pub r#in: Option<RawTransition>,
    #[serde(default)]
    #[serde(rename = "out")]
    pub r#out: Option<RawTransition>,
    #[serde(default)]
    pub pairing: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawTransition {
    Explicit(Vec2),
    Name(String),
}

impl RawTransition {
    fn as_explicit_vec2(&self) -> Option<CoreVec2> {
        match self {
            RawTransition::Explicit(value) => Some(CoreVec2 {
                x: value.x as f32,
                y: value.y as f32,
            }),
            RawTransition::Name(_) => None,
        }
    }

    fn as_name_transition(&self) -> Option<AuthoredTransition> {
        match self {
            RawTransition::Explicit(_) => None,
            RawTransition::Name(name) => Some(AuthoredTransition::name(name.clone())),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawValue {
    Boolean(bool),
    Number(f64),
    String(String),
    // Put more specific shapes BEFORE less specific to avoid untagged matching pitfalls.
    Quat { x: f64, y: f64, z: f64, w: f64 },
    Transform(TransformComponents),
    Vector3 { x: f64, y: f64, z: f64 },
    Vector2 { x: f64, y: f64 },
    Euler { r: f64, p: f64, y: f64 },
    Rgb { r: f64, g: f64, b: f64 },
    Hsl { h: f64, s: f64, l: f64 },
}
