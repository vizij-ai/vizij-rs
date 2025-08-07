use crate::animation::ids::KeypointId;
use crate::{AnimationTime, Value};
use bevy::prelude::Reflect;
use bevy::prelude::ReflectDefault;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
/// A keypoint in an animation track
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect, Default)]
#[reflect(Default, PartialEq)]
pub struct AnimationKeypoint {
    /// Unique identifier for this keypoint
    pub id: KeypointId,
    /// Time at which this keypoint occurs
    pub time: AnimationTime,
    /// Value at this keypoint
    pub value: Value,
    /// Optional metadata
    pub metadata: HashMap<String, String>,
}

impl AnimationKeypoint {
    /// Create a new keypoint
    #[inline]
    pub fn new(time: impl Into<AnimationTime>, value: Value) -> Self {
        Self {
            id: KeypointId::new(),
            time: time.into(),
            value,
            metadata: HashMap::new(),
        }
    }

    /// Create a new keypoint with a specific ID.
    #[inline]
    pub fn new_with_id(
        id: impl AsRef<str>,
        time: impl Into<AnimationTime>,
        value: Value,
    ) -> Result<Self, crate::AnimationError> {
        Ok(Self {
            id: KeypointId::from_string(id)?,
            time: time.into(),
            value,
            metadata: HashMap::new(),
        })
    }

    /// Add metadata
    #[inline]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get metadata
    #[inline]
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}
