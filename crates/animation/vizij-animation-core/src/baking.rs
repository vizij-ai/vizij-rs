#![allow(dead_code)]
//! Baking API: produce baked samples for an AnimationData clip over a time window.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::data::AnimationData;
use crate::engine::{clip_seconds, normalized_time};
use crate::ids::AnimId;
use crate::sampling::{
    sample_track, sample_track_with_derivative_epsilon, DEFAULT_DERIVATIVE_EPSILON,
};
use vizij_api_core::Value;

/// The most samples one bake takes, over all its tracks; a bake with
/// derivatives counts each derivative sample as well.
///
/// A bake reserves its samples before sampling, so one asking for more is
/// refused with [`BakeError::TooManySamples`] whatever memory is available:
/// no frame rate or window a caller passes makes it reserve more than this.
/// At the bound, the samples of scalar and fixed-size kinds (vectors,
/// quaternions, colors, transforms) take tens to a few hundred megabytes; a
/// sample that owns data sized by its keyframes (a numeric array, a text)
/// allocates it on top, infallibly.
pub const MAX_BAKE_SAMPLES: usize = 1 << 20;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakingConfig {
    /// Target frame rate (Hz) for baked samples.
    ///
    /// Non-finite or non-positive values fall back to `60.0`, then clamp to at least `1.0`.
    pub frame_rate: f64,
    /// Start time (seconds) in clip space.
    ///
    /// Clamps into `[0, duration]`; NaN is `0.0`.
    pub start_time: f64,
    /// End time (seconds) in clip space; if `None`, uses the animation duration in seconds.
    ///
    /// Non-finite values fall back to the clip duration, then clamp into `[start_time, duration]`.
    pub end_time: Option<f64>,
    /// Optional override for the finite-difference epsilon used when estimating derivatives.
    ///
    /// Non-finite or non-positive values fall back to the default epsilon.
    pub derivative_epsilon: Option<f32>,
}

impl Default for BakingConfig {
    fn default() -> Self {
        Self {
            frame_rate: 60.0,
            start_time: 0.0,
            end_time: None,
            derivative_epsilon: None,
        }
    }
}

/// Why a bake produced no samples.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum BakeError {
    /// No animation is loaded under the id.
    NotLoaded(AnimId),
    /// The window at the frame rate takes `samples` samples over all tracks,
    /// more than [`MAX_BAKE_SAMPLES`].
    TooManySamples {
        /// The samples the bake would take.
        samples: f64,
    },
    /// The samples could not be allocated.
    OutOfMemory,
}

impl fmt::Display for BakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BakeError::NotLoaded(anim) => write!(f, "no animation is loaded under id {}", anim.0),
            BakeError::TooManySamples { samples } => write!(
                f,
                "a bake of {samples} samples: at most {MAX_BAKE_SAMPLES} are taken"
            ),
            BakeError::OutOfMemory => f.write_str("the baked samples could not be allocated"),
        }
    }
}

impl std::error::Error for BakeError {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakedTrack {
    /// Canonical target path (animatable id)
    pub target_path: String,
    /// Sampled values at each frame.
    pub values: Vec<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakedDerivativeTrack {
    /// Canonical target path (animatable id).
    pub target_path: String,
    /// Sampled derivatives at each frame. `None` marks an unavailable derivative sample.
    pub values: Vec<Option<Value>>,
}

/// Baked animation values for one clip over a fixed sample window.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakedAnimationData {
    /// Source animation id.
    pub anim: AnimId,
    /// Effective frame rate used during baking.
    pub frame_rate: f64,
    /// Clip-space start time in seconds.
    pub start_time: f64,
    /// Clip-space end time in seconds.
    pub end_time: f64,
    /// Per-track sampled values.
    pub tracks: Vec<BakedTrack>,
}

/// Baked animation derivatives for one clip over a fixed sample window.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakedDerivativeAnimationData {
    /// Source animation id.
    pub anim: AnimId,
    /// Effective frame rate used during baking.
    pub frame_rate: f64,
    /// Clip-space start time in seconds.
    pub start_time: f64,
    /// Clip-space end time in seconds.
    pub end_time: f64,
    /// Per-track sampled derivatives.
    pub tracks: Vec<BakedDerivativeTrack>,
}

/// A bake's effective rate and window, and the frames it samples.
struct Window {
    rate: f64,
    start: f64,
    end: f64,
    duration: f64,
    frames: usize,
}

impl Window {
    /// `cfg` applied to `data`, refused when the bake would take more than
    /// [`MAX_BAKE_SAMPLES`] samples, `per_frame` per track and frame.
    fn new(data: &AnimationData, cfg: &BakingConfig, per_frame: usize) -> Result<Self, BakeError> {
        let rate = if cfg.frame_rate.is_finite() && cfg.frame_rate > 0.0 {
            cfg.frame_rate
        } else {
            60.0
        };
        let rate = rate.max(1.0);
        // Canonical duration (ms) in seconds, the baking time domain.
        let duration = clip_seconds(data.duration_ms);
        let start = cfg.start_time.max(0.0).min(duration);
        let end = cfg
            .end_time
            .filter(|end| end.is_finite())
            .unwrap_or(duration)
            .clamp(start, duration);
        // Frames up to the first at or past the end, by the times the frames
        // are sampled at: the span times the rate estimates the last one,
        // a hair off when the span is an inexact decimal (0.07 s at 100 Hz
        // is 7.000000000000001 frames), and the frame times settle it.
        let at = |frame: f64| start + frame / rate;
        let mut last = ((end - start) * rate).ceil();
        if last >= 1.0 && at(last - 1.0) >= end {
            last -= 1.0;
        } else if at(last) < end {
            last += 1.0;
        }
        let frames = if data.tracks.is_empty() {
            0.0
        } else {
            last + 1.0
        };
        let samples = frames * (data.tracks.len() * per_frame) as f64;
        if samples.is_nan() || samples > MAX_BAKE_SAMPLES as f64 {
            return Err(BakeError::TooManySamples { samples });
        }
        Ok(Self {
            rate,
            start,
            end,
            duration,
            frames: frames as usize,
        })
    }

    /// The normalized clip time of frame `f`, sampled as a player's step at
    /// that clip time is.
    fn u(&self, f: usize) -> f32 {
        normalized_time(self.start + f as f64 / self.rate, self.duration)
    }
}

/// An empty vector with room for `len` elements, or [`BakeError::OutOfMemory`].
fn with_capacity<T>(len: usize) -> Result<Vec<T>, BakeError> {
    let mut v = Vec::new();
    v.try_reserve_exact(len)
        .map_err(|_| BakeError::OutOfMemory)?;
    Ok(v)
}

/// Bake a single [`AnimationData`] using the provided config: each track
/// sampled at every frame of the window.
///
/// Invalid or non-finite config values are clamped/fallback-adjusted to safe
/// defaults; a bake of more than [`MAX_BAKE_SAMPLES`] samples is refused.
pub fn bake_animation_data(
    anim_id: AnimId,
    data: &AnimationData,
    cfg: &BakingConfig,
) -> Result<BakedAnimationData, BakeError> {
    let window = Window::new(data, cfg, 1)?;
    let mut tracks = with_capacity(data.tracks.len())?;
    for track in &data.tracks {
        let mut values = with_capacity(window.frames)?;
        for f in 0..window.frames {
            // Encode the POD samples into wire-form Values for the baked artifact.
            values.push(sample_track(track, window.u(f)).into());
        }
        tracks.push(BakedTrack {
            target_path: track.animatable_id.clone(),
            values,
        });
    }
    Ok(BakedAnimationData {
        anim: anim_id,
        frame_rate: window.rate,
        start_time: window.start,
        end_time: window.end,
        tracks,
    })
}

/// Bake animation values and derivatives simultaneously: the values
/// [`bake_animation_data`] gives, and each frame's derivative.
///
/// The returned time window is expressed in clip seconds even though `AnimationData` stores
/// durations in milliseconds. A bake of more than [`MAX_BAKE_SAMPLES`]
/// samples, values and derivatives together, is refused.
pub fn bake_animation_data_with_derivatives(
    anim_id: AnimId,
    data: &AnimationData,
    cfg: &BakingConfig,
) -> Result<(BakedAnimationData, BakedDerivativeAnimationData), BakeError> {
    let window = Window::new(data, cfg, 2)?;
    let derivative_epsilon = cfg
        .derivative_epsilon
        .filter(|eps| eps.is_finite() && *eps > 0.0)
        .unwrap_or(DEFAULT_DERIVATIVE_EPSILON);

    let mut tracks = with_capacity(data.tracks.len())?;
    let mut derivative_tracks = with_capacity(data.tracks.len())?;
    for track in &data.tracks {
        let mut values = with_capacity(window.frames)?;
        let mut derivatives = with_capacity(window.frames)?;
        for f in 0..window.frames {
            let (v, deriv) = sample_track_with_derivative_epsilon(
                track,
                window.u(f),
                window.duration as f32,
                derivative_epsilon,
            );
            values.push(v.into());
            derivatives.push(deriv.map(Value::from));
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

    Ok((
        BakedAnimationData {
            anim: anim_id,
            frame_rate: window.rate,
            start_time: window.start,
            end_time: window.end,
            tracks,
        },
        BakedDerivativeAnimationData {
            anim: anim_id,
            frame_rate: window.rate,
            start_time: window.start,
            end_time: window.end,
            tracks: derivative_tracks,
        },
    ))
}

/// Export baked data as `serde_json::Value` using the stable baked-data schema.
pub fn export_baked_json(baked: &BakedAnimationData) -> serde_json::Value {
    serde_json::to_value(baked).unwrap_or(serde_json::Value::Null)
}

/// Export baked values and derivatives as a JSON object with `values` and `derivatives` fields.
pub fn export_baked_with_derivatives_json(
    baked: &BakedAnimationData,
    derivatives: &BakedDerivativeAnimationData,
) -> serde_json::Value {
    serde_json::json!({
        "values": baked,
        "derivatives": derivatives,
    })
}
