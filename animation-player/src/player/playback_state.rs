use serde::{Deserialize, Serialize};

/// Playback state of an animation player
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaybackState {
    /// Player is stopped
    Stopped,
    /// Player is playing
    Playing,
    /// Player is paused
    Paused,
    /// Player has reached the end
    Ended,
    /// Player encountered an error
    Error,
}

impl PlaybackState {
    /// Get the name of this playback state
    #[inline]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Ended => "ended",
            Self::Error => "error",
        }
    }

    /// Check if the player is actively playing
    #[inline]
    pub fn is_playing(&self) -> bool {
        matches!(self, Self::Playing)
    }

    /// Check if the player can be resumed
    #[inline]
    pub fn can_resume(&self) -> bool {
        matches!(self, Self::Paused | Self::Stopped | Self::Ended)
    }

    /// Check if the player can be paused
    #[inline]
    pub fn can_pause(&self) -> bool {
        matches!(self, Self::Playing)
    }

    /// Check if the player can be stopped
    #[inline]
    pub fn can_stop(&self) -> bool {
        !matches!(self, Self::Stopped | Self::Error)
    }
}

impl From<&str> for PlaybackState {
    fn from(s: &str) -> Self {
        match s {
            "stopped" => Self::Stopped,
            "playing" => Self::Playing,
            "paused" => Self::Paused,
            "ended" => Self::Ended,
            "error" => Self::Error,
            _ => Self::Stopped,
        }
    }
}
