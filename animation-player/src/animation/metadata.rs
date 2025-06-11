use crate::AnimationTime;
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use js_sys::Date;

/// Animation metadata for tracking and management
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Creates new default metadata.
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        let now = (Date::now() / 1000.0) as u64;

        #[cfg(not(target_arch = "wasm32"))]
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            created_at: now,
            modified_at: now,
            author: None,
            description: None,
            tags: Vec::new(),
            version: "1.0.0".to_string(),
            duration: AnimationTime::zero(),
            frame_rate: 60.0,
        }
    }
}

impl Default for AnimationMetadata {
    fn default() -> Self {
        Self::new()
    }
}
