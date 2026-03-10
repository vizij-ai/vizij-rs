#![allow(dead_code)]
//! Engine-agnostic animation runtime used across Vizij hosts.
//!
//! The crate owns the canonical animation data model, playback engine, interpolation and
//! sampling helpers, output contracts, and baking utilities reused by Bevy adapters, wasm
//! bindings, and the orchestrator.

/// Accumulation helpers used while blending per-track samples.
pub mod accumulate;
/// Baking/export helpers for sampling animations into frame sequences.
pub mod baking;
/// Binding traits and tables for resolving canonical animation paths into host handles.
pub mod binding;
/// Engine configuration types.
pub mod config;
/// Canonical animation clip data model.
pub mod data;
/// Playback engine, handles, and inspection helpers.
pub mod engine;
/// Strongly typed ids for animations, players, and instances.
pub mod ids;
/// Per-tick command and update inputs.
pub mod inputs;
/// Interpolation registry and helpers.
pub mod interp;
/// Per-tick change/event outputs.
pub mod outputs;
/// Track sampling helpers shared by the engine and baking APIs.
pub mod sampling;
/// Scratch buffers reused across frames.
pub mod scratch;
/// Parser for the stored-animation JSON format used by fixtures and wrappers.
pub mod stored_animation;
/// Animation-specific value helpers layered on top of `vizij-api-core`.
pub mod value;

// Re-exports for consumers (adapters)
/// Baking helpers and exported baked-data contracts.
pub use baking::{
    bake_animation_data, bake_animation_data_with_derivatives, export_baked_json,
    export_baked_with_derivatives_json, BakedAnimationData, BakedDerivativeAnimationData,
    BakedDerivativeTrack, BakingConfig,
};
/// Binding traits and table types used by host adapters.
pub use binding::{BindingSet, BindingTable, ChannelKey, TargetHandle, TargetResolver};
/// Engine configuration.
pub use config::Config;
/// Canonical animation clip data types.
pub use data::{AnimationData, Keypoint, Track, Transitions, Vec2};
/// Playback engine and its inspection/configuration helpers.
pub use engine::{Engine, InstanceCfg, Player, PrebindReport};
/// Strongly typed ids for the animation runtime.
pub use ids::{AnimId, InstId, PlayerId};
/// Per-tick command/update inputs.
pub use inputs::{Inputs, InstanceUpdate, LoopMode, PlayerCommand};
/// Interpolation registry.
pub use interp::InterpRegistry;
/// Per-tick output payloads and events.
pub use outputs::{Change, ChangeWithDerivative, CoreEvent, Outputs, OutputsWithDerivatives};
/// Direct sampling helpers for standalone tooling.
pub use sampling::{sample_track, sample_track_with_derivative};
/// Scratch allocator used internally by the engine.
pub use scratch::Scratch;
/// Stored-animation parser entrypoint.
pub use stored_animation::parse_stored_animation_json;
/// Shared normalized value types.
pub use vizij_api_core::{Value, ValueKind};
