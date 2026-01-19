#![allow(dead_code)]
//! Vizij animation core runtime.
//!
//! This crate provides the engine-agnostic animation data model plus the runtime
//! `Engine` that loads animations, manages players/instances, and emits sampled
//! outputs each tick. Use it directly in native hosts or via the Bevy and WASM
//! adapters.

/// Accumulators and helpers for merging sampled outputs.
pub mod accumulate;
/// Baking utilities for compressing animation data.
pub mod baking;
/// Binding tables that map animation channels to runtime targets.
pub mod binding;
/// Runtime configuration types for the animation engine.
pub mod config;
/// Animation data structures (tracks, keypoints, transitions).
pub mod data;
/// Core engine runtime, players, and instances.
pub mod engine;
/// Strongly-typed identifiers for animation assets.
pub mod ids;
/// Input payloads and command enums for updating players.
pub mod inputs;
/// Interpolation registry and math helpers.
pub mod interp;
/// Output containers and change events produced per tick.
pub mod outputs;
/// Sampling helpers for track evaluation.
pub mod sampling;
/// Reusable scratch buffers for runtime evaluation.
pub mod scratch;
/// JSON-backed stored animation parsing helpers.
pub mod stored_animation;
/// Value wrappers and convenience conversions.
pub mod value;

// Re-exports for consumers (adapters)
pub use baking::{
    bake_animation_data, bake_animation_data_with_derivatives, export_baked_json,
    export_baked_with_derivatives_json, BakedAnimationData, BakedDerivativeAnimationData,
    BakedDerivativeTrack, BakingConfig,
};
pub use binding::{BindingSet, BindingTable, ChannelKey, TargetHandle, TargetResolver};
pub use config::Config;
pub use data::{AnimationData, Keypoint, Track, Transitions, Vec2};
pub use engine::{Engine, InstanceCfg, Player, PrebindReport};
pub use ids::{AnimId, InstId, PlayerId};
pub use inputs::{Inputs, InstanceUpdate, LoopMode, PlayerCommand};
pub use interp::InterpRegistry;
pub use outputs::{Change, ChangeWithDerivative, CoreEvent, Outputs, OutputsWithDerivatives};
pub use sampling::{sample_track, sample_track_with_derivative};
pub use scratch::Scratch;
pub use stored_animation::parse_stored_animation_json;
pub use vizij_api_core::{Value, ValueKind};
