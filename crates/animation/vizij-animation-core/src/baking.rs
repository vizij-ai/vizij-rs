#![allow(dead_code)]
//! Baking API: produce baked samples for an AnimationData clip over a time window.

use serde::{Deserialize, Serialize};

use crate::data::AnimationData;
use crate::ids::AnimId;
use crate::sampling::sample_track;
use crate::value::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakingConfig {
    /// Target frame rate (Hz) for baked samples.
    pub frame_rate: f32,
    /// Start time (seconds) in clip space.
    pub start_time: f32,
    /// End time (seconds) in clip space; if None, uses animation duration (seconds).
    pub end_time: Option<f32>,
}

impl Default for BakingConfig {
    fn default() -> Self {
        Self {
            frame_rate: 60.0,
            start_time: 0.0,
            end_time: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakedTrack {
    /// Canonical target path (animatable id)
    pub target_path: String,
    /// Sampled values at each frame.
    pub values: Vec<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakedAnimationData {
    pub anim: AnimId,
    pub frame_rate: f32,
    pub start_time: f32,
    pub end_time: f32,
    pub tracks: Vec<BakedTrack>,
}

/// Bake a single AnimationData using the provided config.
pub fn bake_animation_data(
    anim_id: AnimId,
    data: &AnimationData,
    cfg: &BakingConfig,
) -> BakedAnimationData {
    let sr = cfg.frame_rate.max(1.0);
    let start = cfg.start_time.max(0.0);
    // Convert canonical duration (ms) to seconds for baking time domain
    let duration_s = data.duration_ms as f32 / 1000.0;
    let end = cfg.end_time.unwrap_or(duration_s).max(start);
    let span = end - start;
    let frames_f = (span * sr).ceil();
    let frame_count = frames_f as usize + 1; // inclusive of end

    let mut tracks = Vec::with_capacity(data.tracks.len());
    for track in &data.tracks {
        let mut values = Vec::with_capacity(frame_count);
        for f in 0..frame_count {
            let t = start + (f as f32) / sr; // seconds in clip space
            let u = if duration_s > 0.0 {
                (t / duration_s).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let v = sample_track(track, u);
            values.push(v);
        }
        tracks.push(BakedTrack {
            target_path: track.animatable_id.clone(),
            values,
        });
    }

    BakedAnimationData {
        anim: anim_id,
        frame_rate: sr,
        start_time: start,
        end_time: end,
        tracks,
    }
}

/// Export baked data as serde_json::Value (stable schema for FFI/serialization).
pub fn export_baked_json(baked: &BakedAnimationData) -> serde_json::Value {
    serde_json::to_value(baked).unwrap_or(serde_json::Value::Null)
}
