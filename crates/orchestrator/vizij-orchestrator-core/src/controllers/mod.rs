//! Controller types used by the orchestrator runtime.
//!
//! Graph controllers wrap `vizij-graph-core` specs and subscriptions, while animation
//! controllers translate blackboard state into `vizij-animation-core` inputs.

pub mod animation;
pub mod graph;

pub use animation::AnimationControllerConfig;
pub use graph::{
    GraphControllerConfig, GraphMergeError, GraphMergeOptions, OutputConflictStrategy,
    Subscriptions,
};
