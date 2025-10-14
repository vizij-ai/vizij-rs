//! vizij-orchestrator
//!
//! Minimal scaffold of the orchestrator crate. See implementation_plan.md for full plan.
//!
//! This file provides a tiny, safe-to-compile public surface for early integration and
//! iterative development. Most types here are thin wrappers / placeholders that will be
//! expanded in subsequent steps (blackboard, controllers, scheduler, diagnostics).

pub mod blackboard;
pub mod controllers;
pub mod diagnostics;
pub mod fixtures;
pub mod scheduler;

use anyhow::Result;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use vizij_api_core::WriteBatch;

pub use crate::blackboard::{Blackboard, BlackboardEntry, ConflictLog};
pub use crate::controllers::{
    AnimationControllerConfig, GraphControllerConfig, GraphMergeError, GraphMergeOptions,
    OutputConflictStrategy, Subscriptions,
};
pub use crate::scheduler::Schedule;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorFrame {
    pub epoch: u64,
    pub dt: f32,
    /// Merged writes produced during the frame (append order is deterministic: pass order then controller order).
    pub merged_writes: WriteBatch,
    pub conflicts: Vec<ConflictLog>,
    /// Per-pass timings in milliseconds. Currently populated with synthetic values derived
    /// from the configured `dt` may change if scheduler wires in real wall-clock measurements.
    pub timings_ms: HashMap<String, f32>,
    pub events: Vec<serde_json::Value>,
}

#[derive(Debug)]
pub struct Orchestrator {
    pub blackboard: Blackboard,
    pub epoch: u64,
    pub schedule: Schedule,
    /// Registered graph controllers keyed by id.
    pub graphs: IndexMap<String, crate::controllers::graph::GraphController>,
    /// Registered animation controllers keyed by id.
    pub anims: IndexMap<String, crate::controllers::animation::AnimationController>,
}

impl Orchestrator {
    /// Create a new Orchestrator with an initial schedule.
    pub fn new(schedule: Schedule) -> Self {
        Self {
            blackboard: Blackboard::new(),
            epoch: 0,
            schedule,
            graphs: IndexMap::new(),
            anims: IndexMap::new(),
        }
    }

    /// Register a graph controller.
    pub fn with_graph(mut self, cfg: GraphControllerConfig) -> Self {
        let g = crate::controllers::graph::GraphController::new(cfg);
        self.graphs.insert(g.id.clone(), g);
        self
    }

    /// Merge multiple graph configs into a single graph controller and register it.
    pub fn with_merged_graph(
        self,
        id: impl Into<String>,
        graphs: Vec<GraphControllerConfig>,
    ) -> Result<Self, crate::controllers::graph::GraphMergeError> {
        self.with_merged_graph_with_options(id, graphs, GraphMergeOptions::default())
    }

    /// Merge multiple graph configs with explicit conflict options.
    pub fn with_merged_graph_with_options(
        mut self,
        id: impl Into<String>,
        graphs: Vec<GraphControllerConfig>,
        options: GraphMergeOptions,
    ) -> Result<Self, crate::controllers::graph::GraphMergeError> {
        let merged_cfg = GraphControllerConfig::merged_with_options(id, graphs, options)?;
        let graph_id = merged_cfg.id.clone();
        let controller = crate::controllers::graph::GraphController::new(merged_cfg);
        self.graphs.insert(graph_id, controller);
        Ok(self)
    }

    /// Register an animation controller.
    pub fn with_animation(mut self, cfg: AnimationControllerConfig) -> Self {
        let a = crate::controllers::animation::AnimationController::new(cfg);
        self.anims.insert(a.id.clone(), a);
        self
    }

    /// Set a blackboard input value at a given typed path.
    /// This is a convenience for tests and host integrations.
    pub fn set_input(
        &mut self,
        path: &str,
        value: serde_json::Value,
        shape: Option<serde_json::Value>,
    ) -> Result<()> {
        self.blackboard
            .set(path.into(), value, shape, self.epoch, "host".into())?;
        Ok(())
    }

    /// Advance the orchestrator by dt seconds and return an OrchestratorFrame.
    pub fn step(&mut self, dt: f32) -> Result<OrchestratorFrame> {
        // advance epoch first to mark this frame
        self.epoch = self.epoch.wrapping_add(1);

        // Dispatch to scheduler
        let frame = match self.schedule {
            Schedule::SinglePass => crate::scheduler::run_single_pass(self, dt)?,
            Schedule::TwoPass => crate::scheduler::run_two_pass(self, dt)?,
            // RateDecoupled (future): fall back to single-pass for now
            _ => crate::scheduler::run_single_pass(self, dt)?,
        };

        Ok(frame)
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new(Schedule::SinglePass)
    }
}
