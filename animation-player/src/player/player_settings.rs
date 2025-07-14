use crate::animation::instance::PlaybackMode;
use crate::AnimationTime;

/// Returns the default player name.
fn default_player_name() -> String {
    "unnamed".to_string()
}

/// Configurable settings for a player
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlayerSettings {
    /// Player name
    #[serde(default = "default_player_name")]
    pub name: String,
    /// Playback speed multiplier
    pub speed: f64,
    /// Playback mode
    pub mode: PlaybackMode,
    /// Number of loops to play before stopping (None for infinite)
    pub loop_until_target: Option<u32>,
    /// Time offset for starting the animation relative to others
    pub offset: AnimationTime,
    /// Time within the player to start playback
    pub start_time: AnimationTime,
    /// Optional end time for playback
    pub end_time: Option<AnimationTime>,
    /// Attached animation instance IDs
    #[serde(default)]
    pub instance_ids: Vec<String>,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            name: default_player_name(),
            speed: 1.0,
            mode: PlaybackMode::Loop,
            loop_until_target: None,
            offset: AnimationTime::zero(),
            start_time: AnimationTime::zero(),
            end_time: None,
            instance_ids: Vec::new(),
        }
    }
}
