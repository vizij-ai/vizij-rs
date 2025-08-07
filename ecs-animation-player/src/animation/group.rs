//! Defines the TrackGroup structure for organizing animation tracks.

use crate::animation::ids::TrackId;
use bevy::prelude::Reflect;
use bevy::prelude::ReflectDefault;
use serde::{Deserialize, Serialize};
/// A group of tracks, used for organizing related animation data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect, Default)]
#[reflect(Default, PartialEq)]
pub struct TrackGroup {
    /// Unique identifier for this group.
    pub id: String,
    /// Human-readable name for the group.
    pub name: String,
    /// A list of track IDs belonging to this group.
    pub tracks: Vec<TrackId>,
}

impl TrackGroup {
    /// Creates a new track group.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            tracks: Vec::new(),
        }
    }
}
