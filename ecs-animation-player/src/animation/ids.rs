use crate::AnimationError;
use bevy::prelude::Reflect;
use bevy::prelude::ReflectDefault;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
/// Unique identifier for an animation track
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect, Default)]
#[reflect(Default, PartialEq, Hash)]
pub struct TrackId(Uuid);

impl TrackId {
    /// Generate a new track ID
    #[inline]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a track ID from a UUID string
    /// The string should be a valid UUID format like "d7a6b716-10b0-40bb-a894-8bc13a992737"
    #[inline]
    pub fn from_string(id: impl AsRef<str>) -> Result<Self, AnimationError> {
        Uuid::parse_str(id.as_ref())
            .map(Self)
            .map_err(|_| AnimationError::InvalidValue {
                reason: format!("Invalid track ID: {}", id.as_ref()),
            })
    }

    /// Get the underlying UUID
    #[inline]
    pub fn uuid(&self) -> Uuid {
        self.0
    }
}

impl From<Uuid> for TrackId {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl TryFrom<&str> for TrackId {
    type Error = AnimationError;

    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| AnimationError::InvalidValue {
                reason: format!("Invalid track ID: {}", s),
            })
    }
}

impl std::fmt::Display for TrackId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for an animation keypoint
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect, Default)]
#[reflect(Default, PartialEq, Hash)]
pub struct KeypointId(Uuid);

impl KeypointId {
    /// Generate a new keypoint ID
    #[inline]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a keypoint ID from a UUID string
    /// The string should be a valid UUID format like "d7a6b716-10b0-40bb-a894-8bc13a992737"
    #[inline]
    pub fn from_string(id: impl AsRef<str>) -> Result<Self, AnimationError> {
        Uuid::parse_str(id.as_ref())
            .map(Self)
            .map_err(|_| AnimationError::InvalidValue {
                reason: format!("Invalid keypoint ID: {}", id.as_ref()),
            })
    }

    /// Get the underlying UUID
    #[inline]
    pub fn uuid(&self) -> Uuid {
        self.0
    }
}

impl From<Uuid> for KeypointId {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl TryFrom<&str> for KeypointId {
    type Error = AnimationError;

    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| AnimationError::InvalidValue {
                reason: format!("Invalid keypoint ID: {}", s),
            })
    }
}

impl std::fmt::Display for KeypointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
