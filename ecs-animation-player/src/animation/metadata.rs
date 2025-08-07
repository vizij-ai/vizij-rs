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

impl AnimationMetadata {
    #[cfg(target_arch = "wasm32")]
    fn now_secs() -> u64 {
        (Date::now() / 1000.0) as u64
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn now_secs() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Creates new metadata with default values and current timestamps.
    pub fn new() -> Self {
        let now = Self::now_secs();
        Self {
            created_at: now,
            modified_at: now,
            author: None,
            description: None,
            tags: Vec::new(),
            version: "1.0".to_string(),
            duration: AnimationTime::zero(),
            frame_rate: 0.0,
        }
    }

    /// Updates the `modified_at` timestamp to the current time.
    pub fn touch(&mut self) {
        self.modified_at = Self::now_secs();
    }
}
