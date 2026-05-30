#![allow(dead_code)]
//! Canonical animation data model (StoredAnimation).
//! ValueKind/Value are defined in value.rs.

use serde::{Deserialize, Serialize};

use crate::ids::AnimId;
use vizij_api_core::Value;

/// Current Studio-compatible animation storage format.
pub const CURRENT_ANIMATION_FORMAT_VERSION: u8 = 2;

/// 2D vector used for transition control point deltas.
///
/// In Studio format v2, `x` is an anchor-relative time delta in milliseconds and `y` is an
/// anchor-relative value delta. Legacy Vizij assets used normalized cubic-bezier control points;
/// those are converted at the importer boundary.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

/// Any authored transition token Studio stores on one side of a keypoint.
///
/// Explicit handles are serialized as `{ x, y }`; standard easing families and directives are
/// serialized as strings.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AuthoredTransition {
    Explicit(Vec2),
    Name(String),
}

impl AuthoredTransition {
    pub fn explicit(x: f32, y: f32) -> Self {
        Self::Explicit(Vec2 { x, y })
    }

    pub fn name(value: impl Into<String>) -> Self {
        Self::Name(value.into())
    }
}

/// Per-keypoint transitions: control points for cubic-bezier timing.
/// Use `in` (arrival to this point) and `out` (departure from this point).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Transitions {
    #[serde(default)]
    #[serde(rename = "in")]
    pub r#in: Option<AuthoredTransition>,
    #[serde(default)]
    #[serde(rename = "out")]
    pub r#out: Option<AuthoredTransition>,
    #[serde(default)]
    pub pairing: Option<String>,
}

/// A single keypoint in clip-local time.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Keypoint {
    pub id: String,
    /// Studio v2 uses absolute milliseconds.
    pub stamp: f32,
    pub value: Value,
    #[serde(default)]
    pub transitions: Option<Transitions>,
}

/// Track settings (optional color).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrackSettings {
    pub color: Option<String>,
}

/// A track targeting a canonical output path with a series of keypoints.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub id: String,
    pub name: String,
    /// Canonical target path (e.g., "node/Transform.translation")
    #[serde(rename = "animatableId")]
    pub animatable_id: String,
    pub points: Vec<Keypoint>,
    #[serde(default)]
    pub settings: Option<TrackSettings>,
}

/// Canonical StoredAnimation format (standard, single supported schema).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnimationData {
    /// Optional internal id assigned when loaded into the engine.
    #[serde(skip)]
    pub id: Option<AnimId>,
    pub name: String,
    pub tracks: Vec<Track>,
    /// Arbitrary groupings (unused by core logic but preserved).
    #[serde(default)]
    pub groups: serde_json::Value,
    /// Duration in milliseconds.
    ///
    /// Studio v2 derives playback extent from track content, with `defaultViewportExtent` as a
    /// viewport hint. The core keeps an explicit duration for player/baking math, derived during
    /// import as `max(max_track_stamp, defaultViewportExtent, duration)`.
    #[serde(rename = "duration")]
    pub duration_ms: u32,
}

impl AnimationData {
    /// Validate basic invariants (monotonic finite stamps, non-zero duration).
    pub fn validate_basic(&self) -> Result<(), String> {
        if self.duration_ms == 0 {
            return Err("AnimationData.duration must be > 0 ms".into());
        }
        for track in &self.tracks {
            let mut last = -f32::INFINITY;
            for p in &track.points {
                if !p.stamp.is_finite() || p.stamp < 0.0 {
                    return Err(format!(
                        "Keypoint stamp must be finite and non-negative for '{}'",
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

    /// Convert clip-local seconds to canonical Studio v2 millisecond stamps.
    pub fn sample_stamp_for_seconds(&self, local_seconds: f32) -> f32 {
        if self.duration_ms == 0 || !local_seconds.is_finite() {
            return 0.0;
        }
        (local_seconds * 1000.0).clamp(0.0, self.duration_ms as f32)
    }

    /// Convert a finite-difference epsilon in the data's stamp domain into seconds.
    pub fn stamp_delta_to_seconds(&self, delta: f32) -> f32 {
        delta / 1000.0
    }
}
