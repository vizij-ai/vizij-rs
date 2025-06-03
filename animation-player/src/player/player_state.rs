use crate::animation::instance::PlaybackMode;
use crate::player::playback_state::PlaybackState;
use crate::AnimationTime;

/// Player state managed by the engine
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayerState {
    pub playback_state: PlaybackState,
    pub speed: f64, // 1.0, 1.4, -2.0, etc.
    pub mode: PlaybackMode,
    pub start_time: AnimationTime,
    pub end_time: Option<AnimationTime>,
    pub last_update_time: AnimationTime, // For delta calculation
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Stopped,
            speed: 1.0,
            mode: PlaybackMode::Loop,
            start_time: AnimationTime::zero(),
            end_time: None,
            last_update_time: AnimationTime::zero(),
        }
    }
}
