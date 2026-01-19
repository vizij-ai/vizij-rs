//! Controller implementations wired into the orchestrator scheduler.

/// Animation controller definitions and helpers.
pub mod animation;
/// Graph controller definitions and helpers.
pub mod graph;

pub use animation::AnimationControllerConfig;
pub use graph::{
    GraphControllerConfig, GraphMergeError, GraphMergeOptions, OutputConflictStrategy,
    Subscriptions,
};
