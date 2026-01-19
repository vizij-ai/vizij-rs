#![allow(dead_code)]
//! Canonical animation data model for StoredAnimation assets.
//!
//! `Value` and `ValueKind` live in `vizij_api_core` and are re-exported by
//! `crate::value` for convenience.

use serde::{Deserialize, Serialize};

use crate::ids::AnimId;
use vizij_api_core::Value;

/// 2D vector used for transition control points (normalized 0..1 domain).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Vec2 {
    /// Normalized x coordinate (0..1).
    pub x: f32,
    /// Normalized y coordinate (0..1).
    pub y: f32,
}

/// Per-keypoint transitions: control points for cubic-bezier timing.
/// Use `in` (arrival to this point) and `out` (departure from this point).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Transitions {
    #[serde(default)]
    #[serde(rename = "in")]
    /// Optional incoming (arrival) control point.
    pub r#in: Option<Vec2>,
    #[serde(default)]
    #[serde(rename = "out")]
    /// Optional outgoing (departure) control point.
    pub r#out: Option<Vec2>,
}

/// A single keypoint in normalized time [0..1].
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Keypoint {
    /// Stable keypoint identifier from the authoring tool.
    pub id: String,
    /// Normalized time in [0,1] within the clip duration.
    pub stamp: f32,
    /// Canonical sampled value at this stamp.
    pub value: Value,
    #[serde(default)]
    /// Optional per-key transitions for cubic-bezier timing.
    pub transitions: Option<Transitions>,
}

/// Track settings (optional color).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrackSettings {
    /// Optional UI hint for track color.
    pub color: Option<String>,
}

/// A track targeting a canonical output path with a series of keypoints.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Track {
    /// Stable track identifier from the authoring tool.
    pub id: String,
    /// Display name (may be duplicated across tracks).
    pub name: String,
    /// Canonical target path (e.g., "node/Transform.translation")
    #[serde(rename = "animatableId")]
    pub animatable_id: String,
    /// Ordered keypoints for this track.
    pub points: Vec<Keypoint>,
    #[serde(default)]
    /// Optional per-track settings (UI hints).
    pub settings: Option<TrackSettings>,
}

/// Canonical StoredAnimation format (standard, single supported schema).
///
/// This is the normalized in-memory form used by the engine. Use
/// [`parse_stored_animation_json`](crate::stored_animation::parse_stored_animation_json)
/// to convert fixture JSON into this struct.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnimationData {
    /// Optional internal id assigned when loaded into the engine.
    #[serde(skip)]
    pub id: Option<AnimId>,
    /// Display name of the animation clip.
    pub name: String,
    /// Track list targeting canonical output paths.
    pub tracks: Vec<Track>,
    /// Arbitrary groupings (unused by core logic but preserved).
    #[serde(default)]
    pub groups: serde_json::Value,
    /// Duration in milliseconds (authoritative for mapping normalized stamps to seconds).
    #[serde(rename = "duration")]
    pub duration_ms: u32,
}

impl AnimationData {
    /// Validate basic invariants (monotonic stamps in `[0,1]`, non-zero duration).
    ///
    /// # Errors
    /// Returns an error string when any track contains non-finite or out-of-range stamps,
    /// when stamps are not non-decreasing, or when duration is zero.
    pub fn validate_basic(&self) -> Result<(), String> {
        if self.duration_ms == 0 {
            return Err("AnimationData.duration must be > 0 ms".into());
        }
        for track in &self.tracks {
            let mut last = -f32::INFINITY;
            for p in &track.points {
                if !p.stamp.is_finite() || p.stamp < 0.0 || p.stamp > 1.0 {
                    return Err(format!(
                        "Keypoint stamp must be in [0,1] and finite for '{}'",
                        track.animatable_id
                    ));
                }
                if p.stamp < last {
                    return Err(format!(
                        "Keypoint stamps must be non-decreasing for '{}'",
                        track.animatable_id
                    ));
                }
                last = p.stamp;
            }
        }
        Ok(())
    }
}
