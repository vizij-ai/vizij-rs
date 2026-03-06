#![allow(dead_code)]
//! Engine-agnostic animation runtime used across Vizij hosts.
//!
//! The crate owns the canonical animation data model, playback engine, interpolation and
//! sampling helpers, output contracts, and baking utilities reused by Bevy adapters, wasm
//! bindings, and the orchestrator.

pub mod accumulate;
pub mod baking;
pub mod binding;
pub mod config;
pub mod data;
pub mod engine;
pub mod ids;
pub mod inputs;
pub mod interp;
pub mod outputs;
pub mod sampling;
pub mod scratch;
pub mod stored_animation;
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
