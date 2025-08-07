pub mod animation;
pub mod config;
pub mod ecs;
pub mod error;
pub mod event;
pub mod interpolation;
pub mod loaders;
pub mod time;
pub mod value;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

// Re-export common types for convenience
pub use animation::{
    AnimationBaking, AnimationData, AnimationInstanceSettings, AnimationKeypoint, AnimationTrack,
    AnimationTransition, BakedAnimationData, BakedDataStatistics, BakingConfig, KeypointId,
    PlaybackMode, TrackId,
};
pub use config::{AnimationEngineConfig, PerformanceThresholds};
pub use ecs::components::AnimationInstance;
pub use error::AnimationError;
pub use event::{AnimationEvent, EventType};
pub use interpolation::{
    InterpolationCacheKey, InterpolationContext, InterpolationRegistry, Interpolator,
};
pub mod player;
pub use player::playback_state::PlaybackState;
pub use time::{AnimationTime, TimeRange};
pub use value::*;

/// Animation player result type
pub type Result<T> = core::result::Result<T, AnimationError>;
