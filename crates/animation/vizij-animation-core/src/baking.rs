#![allow(dead_code)]
//! Baking API: produce baked samples for an `AnimationData` clip over a time window.

use serde::{Deserialize, Serialize};

use crate::data::AnimationData;
use crate::ids::AnimId;
use crate::sampling::{sample_track_with_derivative_epsilon, DEFAULT_DERIVATIVE_EPSILON};
use vizij_api_core::Value;

/// Configuration for baking sampled animation data.
///
/// # Examples
/// ```rust
/// use vizij_animation_core::BakingConfig;
///
/// let cfg = BakingConfig {
///     frame_rate: 30.0,
///     start_time: 0.0,
///     end_time: Some(1.0),
///     derivative_epsilon: None,
/// };
/// assert_eq!(cfg.frame_rate, 30.0);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakingConfig {
    /// Target frame rate (Hz) for baked samples.
    pub frame_rate: f32,
    /// Start time (seconds) in clip space.
    pub start_time: f32,
    /// End time (seconds) in clip space; if None, uses animation duration (seconds).
    pub end_time: Option<f32>,
    /// Optional override for the finite-difference epsilon used when estimating derivatives.
    pub derivative_epsilon: Option<f32>,
}

impl Default for BakingConfig {
    /// Creates a new `Default`.
    fn default() -> Self {
        Self {
            frame_rate: 60.0,
            start_time: 0.0,
            end_time: None,
            derivative_epsilon: None,
        }
    }
}

/// Baked values for a single track.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakedTrack {
    /// Canonical target path (animatable id).
    pub target_path: String,
    /// Sampled values at each frame.
    pub values: Vec<Value>,
}

/// Baked derivatives for a single track.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakedDerivativeTrack {
    /// Canonical target path (animatable id).
    pub target_path: String,
    /// Sampled derivative values at each frame (None for non-numeric tracks).
    pub values: Vec<Option<Value>>,
}

/// Baked animation values across all tracks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakedAnimationData {
    /// Animation identifier for the source clip.
    pub anim: AnimId,
    /// Sampling rate used for baking (Hz).
    pub frame_rate: f32,
    /// Start time in seconds.
    pub start_time: f32,
    /// End time in seconds.
    pub end_time: f32,
    /// Baked per-track samples.
    pub tracks: Vec<BakedTrack>,
}

/// Baked animation derivatives across all tracks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakedDerivativeAnimationData {
    /// Animation identifier for the source clip.
    pub anim: AnimId,
    /// Sampling rate used for baking (Hz).
    pub frame_rate: f32,
    /// Start time in seconds.
    pub start_time: f32,
    /// End time in seconds.
    pub end_time: f32,
    /// Baked per-track derivative samples.
    pub tracks: Vec<BakedDerivativeTrack>,
}

/// Bake a single `AnimationData` using the provided config.
///
/// Sampling is clamped to `[start_time, min(end_time, duration)]` in seconds.
///
/// # Examples
/// ```rust
/// use vizij_animation_core::{bake_animation_data, AnimId, AnimationData, BakingConfig};
///
/// let data = AnimationData {
///     id: None,
///     name: "clip".into(),
///     tracks: Vec::new(),
///     groups: serde_json::Value::Null,
///     duration_ms: 1000,
/// };
/// let baked = bake_animation_data(AnimId(1), &data, &BakingConfig::default());
/// assert_eq!(baked.tracks.len(), 0);
/// ```
pub fn bake_animation_data(
    anim_id: AnimId,
    data: &AnimationData,
    cfg: &BakingConfig,
) -> BakedAnimationData {
    bake_animation_data_with_derivatives(anim_id, data, cfg).0
}

/// Bake animation values and derivatives simultaneously.
///
/// Sampling is clamped to `[start_time, min(end_time, duration)]` in seconds.
///
/// # Examples
/// ```rust
/// use vizij_animation_core::{bake_animation_data_with_derivatives, AnimId, AnimationData, BakingConfig};
///
/// let data = AnimationData {
///     id: None,
///     name: "clip".into(),
///     tracks: Vec::new(),
///     groups: serde_json::Value::Null,
///     duration_ms: 1000,
/// };
/// let (values, derivatives) =
///     bake_animation_data_with_derivatives(AnimId(1), &data, &BakingConfig::default());
/// assert_eq!(values.tracks.len(), derivatives.tracks.len());
/// ```
pub fn bake_animation_data_with_derivatives(
    anim_id: AnimId,
    data: &AnimationData,
    cfg: &BakingConfig,
) -> (BakedAnimationData, BakedDerivativeAnimationData) {
    let sr = if cfg.frame_rate.is_finite() && cfg.frame_rate > 0.0 {
        cfg.frame_rate
    } else {
        60.0
    };
    let sr = sr.max(1.0);
    let start = cfg.start_time.max(0.0);
    // Convert canonical duration (ms) to seconds for baking time domain
    let duration_s = data.duration_ms as f32 / 1000.0;
    let mut end = cfg.end_time.unwrap_or(duration_s);
    if !end.is_finite() {
        end = duration_s;
    }
    let end = end.clamp(start, duration_s);
    let span = end - start;
    let frames_f = (span * sr).ceil();
    let frame_count = frames_f as usize + 1; // inclusive of end

    let derivative_epsilon = cfg
        .derivative_epsilon
        .filter(|eps| eps.is_finite() && *eps > 0.0)
        .unwrap_or(DEFAULT_DERIVATIVE_EPSILON);

    let mut tracks = Vec::with_capacity(data.tracks.len());
    let mut derivative_tracks = Vec::with_capacity(data.tracks.len());
    for track in &data.tracks {
        let mut values = Vec::with_capacity(frame_count);
        let mut derivatives = Vec::with_capacity(frame_count);
        for f in 0..frame_count {
            let t = start + (f as f32) / sr; // seconds in clip space
            let u = if duration_s > 0.0 {
                (t / duration_s).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (v, deriv) =
                sample_track_with_derivative_epsilon(track, u, duration_s, derivative_epsilon);
            values.push(v);
            derivatives.push(deriv);
        }
        tracks.push(BakedTrack {
            target_path: track.animatable_id.clone(),
            values,
        });
        derivative_tracks.push(BakedDerivativeTrack {
            target_path: track.animatable_id.clone(),
            values: derivatives,
        });
    }

    (
        BakedAnimationData {
            anim: anim_id,
            frame_rate: sr,
            start_time: start,
            end_time: end,
            tracks,
        },
        BakedDerivativeAnimationData {
            anim: anim_id,
            frame_rate: sr,
            start_time: start,
            end_time: end,
            tracks: derivative_tracks,
        },
    )
}

/// Export baked data as `serde_json::Value` (stable schema for FFI/serialization).
///
/// This is a convenience wrapper around `serde_json::to_value` that returns
/// `Value::Null` on serialization failure.
pub fn export_baked_json(baked: &BakedAnimationData) -> serde_json::Value {
    serde_json::to_value(baked).unwrap_or(serde_json::Value::Null)
}

/// Export baked values and derivatives as `serde_json::Value`.
///
/// The returned object has `{ values, derivatives }` keys for tooling parity.
pub fn export_baked_with_derivatives_json(
    baked: &BakedAnimationData,
    derivatives: &BakedDerivativeAnimationData,
) -> serde_json::Value {
    serde_json::json!({
        "values": baked,
        "derivatives": derivatives,
    })
}
