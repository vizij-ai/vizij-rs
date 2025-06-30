use crate::animation::instance::PlaybackMode;
use crate::player::playback_state::PlaybackState;
use crate::AnimationTime;

/// Player state managed by the engine
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayerState {
    pub playback_state: PlaybackState,
    pub speed: f64, // Represents timescale
    pub mode: PlaybackMode,
    pub offset: AnimationTime, // Time offset for starting the animation (relative to other animations)
    pub start_time: AnimationTime, // Time within the current animation to start playback at
    pub end_time: Option<AnimationTime>,
    pub last_update_time: AnimationTime, // For delta calculation
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Stopped,
            speed: 1.0,
            mode: PlaybackMode::Loop,
            offset: AnimationTime::zero(), // Initialize offset
            start_time: AnimationTime::zero(),
            end_time: None,
            last_update_time: AnimationTime::zero(),
        }
    }
}
