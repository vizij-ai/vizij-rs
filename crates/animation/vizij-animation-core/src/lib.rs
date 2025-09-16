#![allow(dead_code)]
//! Vizij Animation Core (engine-agnostic)
//!
//! Step 1: scaffolding of core types and Engine skeleton per IMPLEMENTATION_PLAN.md.
//! This crate defines data models, IDs, inputs/outputs contracts, binding types,
//! scratch buffers, an interpolation registry placeholder, baking stubs, and an
//! Engine skeleton (no sampling/blending yet).

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
pub use baking::{BakedAnimationData, BakingConfig};
pub use binding::{BindingSet, BindingTable, ChannelKey, TargetHandle, TargetResolver};
pub use config::Config;
pub use data::{AnimationData, Keypoint, Track, Transitions, Vec2};
pub use engine::{Engine, InstanceCfg, Player};
pub use ids::{AnimId, InstId, PlayerId};
pub use inputs::{Inputs, InstanceUpdate, LoopMode, PlayerCommand};
pub use interp::InterpRegistry;
pub use outputs::{Change, CoreEvent, Outputs};
pub use sampling::sample_track;
pub use scratch::Scratch;
pub use stored_animation::parse_stored_animation_json;
pub use vizij_api_core::{Value, ValueKind};
