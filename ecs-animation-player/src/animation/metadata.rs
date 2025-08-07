use crate::AnimationTime;
use bevy::prelude::Reflect;
use bevy::prelude::ReflectDefault;
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use js_sys::Date;

/// Animation metadata for tracking and management
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect, Default)]
#[reflect(Default, PartialEq)]
pub struct AnimationMetadata {
    pub created_at: u64,  // Timestamp in seconds since UNIX epoch
    pub modified_at: u64, // Timestamp in seconds since UNIX epoch
    pub author: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub version: String,
    pub duration: AnimationTime,
    pub frame_rate: f64,
}
