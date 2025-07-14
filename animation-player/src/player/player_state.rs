use crate::player::playback_state::PlaybackState;
use crate::AnimationTime;

/// Runtime properties tracked by the engine for each player
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlayerState {
    /// Current playback state
    pub playback_state: PlaybackState,
    /// Last time the player was updated
    pub last_update_time: AnimationTime,
    /// The current number of loops completed
    pub current_loop_count: u32,
    /// Whether playback is currently moving forward (for PingPong mode)
    pub is_playing_forward: bool,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Stopped,
            last_update_time: AnimationTime::zero(),
            current_loop_count: 0,
            is_playing_forward: true,
        }
    }
}
