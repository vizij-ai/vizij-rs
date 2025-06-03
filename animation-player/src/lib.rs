//! Animation Player Core
//!
//! A high-performance animation engine designed for real-time streaming and interpolation.
//! Supports both WebAssembly and native environments with extensible interpolation functions.

#![cfg_attr(not(feature = "std"), no_std)]

// Use wee_alloc as the global allocator for smaller WASM size
#[cfg(all(feature = "wasm", feature = "wee_alloc"))]
#[doc(hidden)]
use wee_alloc::WeeAlloc;

#[cfg(all(feature = "wasm", feature = "wee_alloc"))]
#[global_allocator]
#[doc(hidden)]
static ALLOC: WeeAlloc = WeeAlloc::INIT;

pub mod animation;
pub mod baking;
pub mod config;
pub mod error;
pub mod event;
pub mod interpolation;
pub mod loaders;
pub mod player;
pub mod time;
pub mod value;

#[cfg(feature = "wasm")]
pub mod wasm;

// Re-export common types for convenience
pub use animation::{
    AnimationData, AnimationKeypoint, AnimationTrack, BakedAnimationData, BakedDataStatistics,
    BakingConfig, KeypointId, TrackId,
};
pub use config::{AnimationConfig, PerformanceThresholds};
pub use error::AnimationError;
pub use event::{AnimationEvent, EventType};
pub use interpolation::{
    InterpolationCacheKey, InterpolationContext, InterpolationFunction, InterpolationMetrics,
    InterpolationRegistry,
};
pub use player::{AnimationEngine, AnimationPlayer, PlaybackMetrics, PlaybackState};
pub use time::{AnimationTime, TimeRange};
pub use value::Value;

/// Animation player result type
pub type Result<T> = core::result::Result<T, AnimationError>;
