use crate::AnimationTime;
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use js_sys::Date;

/// Animation metadata for tracking and management
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationMetadata {
    pub created_at: u64,
    pub modified_at: u64,
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

    /// Sets the creation timestamp.
    #[inline]
    pub fn with_created_at(mut self, timestamp: u64) -> Self {
        self.created_at = timestamp;
        self
    }

    /// Sets the modification timestamp.
    #[inline]
    pub fn with_modified_at(mut self, timestamp: u64) -> Self {
        self.modified_at = timestamp;
        self
    }

    /// Sets the author.
    #[inline]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Sets the description.
    #[inline]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Adds a tag.
    #[inline]
    pub fn add_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Sets the version.
    #[inline]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Sets the duration.
    #[inline]
    pub fn with_duration(mut self, duration: AnimationTime) -> Self {
        self.duration = duration;
        self
    }

    /// Sets the frame rate.
    #[inline]
    pub fn with_frame_rate(mut self, frame_rate: f64) -> Self {
        self.frame_rate = frame_rate;
        self
    }
}

impl Default for AnimationMetadata {
    fn default() -> Self {
        Self::new()
    }
}
