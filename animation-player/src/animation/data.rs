use crate::animation::group::TrackGroup; 
use crate::animation::ids::{KeypointId, TrackId};
use crate::animation::metadata::AnimationMetadata;
use crate::animation::track::AnimationTrack;
use crate::animation::transition::AnimationTransition;
use crate::AnimationTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete animation data containing multiple tracks
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationData {
    /// Unique identifier for this animation
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Metadata for the animation
    pub metadata: AnimationMetadata,
    /// All tracks in this animation
    pub tracks: HashMap<TrackId, AnimationTrack>,
    /// Groups of tracks for organizational purposes
    #[serde(default)]
    pub groups: HashMap<String, TrackGroup>,
    /// Transitions between keypoints that define interpolation behavior
    #[serde(default)]
    pub transitions: HashMap<String, AnimationTransition>,
}

impl AnimationData {
    /// Create a new animation
    #[inline]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            metadata: AnimationMetadata::new(),
            tracks: HashMap::new(),
            groups: HashMap::new(),
            transitions: HashMap::new(),
        }
    }

    /// Add a track to this animation
    pub fn add_track(&mut self, track: AnimationTrack) {
        // Update animation duration if necessary
        if let Some(track_range) = track.time_range() {
            if track_range.end > self.metadata.duration {
                self.metadata.duration = track_range.end;
            }
        }

        self.tracks.insert(track.id, track);
    }

    /// Remove a track by ID
    pub fn remove_track(&mut self, id: TrackId) -> Option<AnimationTrack> {
        let removed = self.tracks.remove(&id);

        // Recalculate duration
        self.recalculate_duration();

        removed
    }

    /// Get a track by ID
    #[inline]
    pub fn get_track(&self, id: TrackId) -> Option<&AnimationTrack> {
        self.tracks.get(&id)
    }

    /// Get a mutable reference to a track by ID
    #[inline]
    pub fn get_track_mut(&mut self, id: TrackId) -> Option<&mut AnimationTrack> {
        self.tracks.get_mut(&id)
    }

    /// Add a track group to this animation
    pub fn add_group(&mut self, group: TrackGroup) {
        self.groups.insert(group.id.clone(), group);
    }

    /// Remove a track group by ID
    pub fn remove_group(&mut self, id: &str) -> Option<TrackGroup> {
        self.groups.remove(id)
    }

    /// Get a track group by ID
    #[inline]
    pub fn get_group(&self, id: &str) -> Option<&TrackGroup> {
        self.groups.get(id)
    }

    /// Add a transition between keypoints
    pub fn add_transition(&mut self, transition: AnimationTransition) {
        self.transitions.insert(transition.id.clone(), transition);
    }

    /// Get a transition for a keypoint pair
    #[inline]
    pub fn get_transition_for_keypoints(
        &self,
        prev_id: KeypointId,
        next_id: KeypointId,
    ) -> Option<&AnimationTransition> {
        self.transitions
            .values()
            .find(|t| t.keypoints[0] == prev_id && t.keypoints[1] == next_id)
    }

    pub fn get_track_transition_for_time(
        &self,
        time: AnimationTime,
        track_id: &TrackId,
    ) -> Option<&AnimationTransition> {
        let (prev, next) = self.tracks[track_id].surrounding_keypoints(time)?;
        match (prev, next) {
            (Some(prev_kp), Some(next_kp)) => {
                self.get_transition_for_keypoints(prev_kp.id, next_kp.id)
            }
            (Some(_), None) => None,
            (None, Some(_)) => None,
            (None, None) => None,
        }
    }

    /// Get all tracks as a vector
    #[inline]
    pub fn tracks_vec(&self) -> Vec<&AnimationTrack> {
        self.tracks.values().collect()
    }

    /// Get the duration of this animation
    #[inline]
    pub fn duration(&self) -> AnimationTime {
        self.metadata.duration
    }

    /// Recalculate the animation duration based on tracks
    pub fn recalculate_duration(&mut self) {
        let max_time = self
            .tracks
            .values()
            .filter_map(|track| track.time_range())
            .map(|range| range.end)
            .max()
            .unwrap_or(AnimationTime::zero());

        self.metadata.duration = max_time;
    }
}
