//! Animation Player Core
//!
//! A high-performance animation engine designed for real-time streaming and interpolation.
//! Supports both WebAssembly and native environments with extensible interpolation functions.

// Use wee_alloc as the global allocator for smaller WASM size
#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
use wee_alloc::WeeAlloc;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
#[doc(hidden)]
static ALLOC: WeeAlloc = WeeAlloc::INIT;

pub mod animation;
pub mod config;
pub mod error;
pub mod event;
pub mod interpolation;
pub mod loaders;
pub mod player;
pub mod time;
pub mod value;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

// Re-export common types for convenience
pub use animation::{
    AnimationBaking, AnimationData, AnimationKeypoint, AnimationTrack, BakedAnimationData,
    BakedDataStatistics, BakingConfig, KeypointId, TrackId,
};
pub use config::{AnimationEngineConfig, PerformanceThresholds};
pub use error::AnimationError;
pub use event::{AnimationEvent, EventType};
pub use interpolation::{
    InterpolationCacheKey, InterpolationContext, InterpolationMetrics, InterpolationRegistry,
    Interpolator,
};
pub use player::{AnimationEngine, AnimationPlayer, PlaybackMetrics, PlaybackState};
pub use time::{AnimationTime, TimeRange};
pub use value::Value;

/// Animation player result type
pub type Result<T> = core::result::Result<T, AnimationError>;
