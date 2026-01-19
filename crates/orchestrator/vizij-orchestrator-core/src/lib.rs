//! Deterministic orchestration of graphs and animations.
//!
//! `vizij-orchestrator-core` coordinates graph controllers and animation engines against a
//! shared blackboard. It stages inputs, runs controllers in configurable passes, merges
//! writes deterministically, and reports conflicts for diagnostics.
//!
//! This crate is the Rust host counterpart to `vizij-orchestrator-wasm`.

pub mod blackboard;
pub mod controllers;
pub mod diagnostics;
pub mod fixtures;
pub mod scheduler;

use anyhow::{anyhow, Result};
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

/// Output from a single `Orchestrator::step`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorFrame {
    /// Epoch counter for the frame that produced this output.
    pub epoch: u64,
    /// Delta time passed to `step`, in seconds.
    pub dt: f32,
    /// Merged writes produced during the frame (append order is deterministic: pass order then controller order).
    pub merged_writes: WriteBatch,
    /// Conflict logs emitted while applying controller write batches.
    pub conflicts: Vec<ConflictLog>,
    /// Per-pass timings in milliseconds. Currently populated with synthetic values derived
    /// from the configured `dt`; may change if scheduler wires in real wall-clock measurements.
    pub timings_ms: HashMap<String, f32>,
    /// Serialized engine events emitted by animation controllers.
    pub events: Vec<serde_json::Value>,
}

/// Owns controllers, blackboard state, and the active schedule.
#[derive(Debug)]
pub struct Orchestrator {
    /// Shared blackboard storing the latest values per path.
    pub blackboard: Blackboard,
    /// Current epoch counter (increments each `step`).
    pub epoch: u64,
    /// Active schedule for controller execution.
    pub schedule: Schedule,
    /// Registered graph controllers keyed by id.
    pub graphs: IndexMap<String, crate::controllers::graph::GraphController>,
    /// Registered animation controllers keyed by id.
    pub anims: IndexMap<String, crate::controllers::animation::AnimationController>,
}

impl Orchestrator {
    /// Create a new orchestrator with an initial schedule.
    pub fn new(schedule: Schedule) -> Self {
        Self {
            blackboard: Blackboard::new(),
            epoch: 0,
            schedule,
            graphs: IndexMap::new(),
            anims: IndexMap::new(),
        }
    }

    /// Register a graph controller and return the updated orchestrator.
    ///
    /// The graph is owned by the orchestrator and will be evaluated in schedule order.
    pub fn with_graph(mut self, cfg: GraphControllerConfig) -> Self {
        let g = crate::controllers::graph::GraphController::new(cfg);
        self.graphs.insert(g.id.clone(), g);
        self
    }

    /// Merge multiple graph configs into a single graph controller and register it.
    ///
    /// This uses [`GraphMergeOptions::default`], which rejects conflicting outputs.
    /// See [`with_merged_graph_with_options`] to customize merge strategy.
    pub fn with_merged_graph(
        self,
        id: impl Into<String>,
        graphs: Vec<GraphControllerConfig>,
    ) -> Result<Self, crate::controllers::graph::GraphMergeError> {
        self.with_merged_graph_with_options(id, graphs, GraphMergeOptions::default())
    }

    /// Merge multiple graph configs with explicit conflict options.
    ///
    /// Returns [`GraphMergeError`] if the merge cannot be completed.
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

    /// Export a graph's `GraphSpec` as a JSON value.
    ///
    /// # Errors
    /// Returns an error when the graph id is not registered or the spec cannot be
    /// serialized to JSON.
    pub fn export_graph_json(&self, id: &str) -> Result<serde_json::Value> {
        let controller = self
            .graphs
            .get(id)
            .ok_or_else(|| anyhow!("graph '{id}' is not registered"))?;
        serde_json::to_value(&controller.spec).map_err(|err| err.into())
    }

    /// Export a graph's `GraphSpec` as pretty formatted JSON.
    ///
    /// # Errors
    /// Returns an error when the graph id is not registered or serialization fails.
    pub fn export_graph_json_pretty(&self, id: &str) -> Result<String> {
        let controller = self
            .graphs
            .get(id)
            .ok_or_else(|| anyhow!("graph '{id}' is not registered"))?;
        serde_json::to_string_pretty(&controller.spec).map_err(|err| err.into())
    }

    /// Register an animation controller and return the updated orchestrator.
    pub fn with_animation(mut self, cfg: AnimationControllerConfig) -> Self {
        let a = crate::controllers::animation::AnimationController::new(cfg);
        self.anims.insert(a.id.clone(), a);
        self
    }

    /// Set a blackboard input value at a typed path.
    ///
    /// This is a convenience for tests and host integrations that operate with JSON
    /// payloads instead of `vizij-api-core` values.
    ///
    /// # Errors
    /// Returns an error when the path is invalid or the JSON payload cannot be parsed
    /// into a `vizij-api-core` value or shape.
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

    /// Advance the orchestrator by `dt` seconds and return an [`OrchestratorFrame`].
    ///
    /// `step` increments the internal epoch before evaluating controllers, so the
    /// returned frame always reflects the new epoch value.
    ///
    /// # Errors
    /// Returns an error if any controller fails to evaluate or if the schedule runner
    /// encounters an unexpected failure.
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

#[cfg(test)]
mod tests {
    use super::*;
    use vizij_graph_core::types::{
        EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeSpec, NodeType,
    };

    fn sample_graph(id: &str, value: f32) -> GraphControllerConfig {
        let spec = GraphSpec {
            nodes: vec![
                NodeSpec {
                    id: format!("{id}::constant"),
                    kind: NodeType::Constant,
                    params: NodeParams {
                        value: Some(vizij_api_core::Value::Float(value)),
                        ..Default::default()
                    },
                    output_shapes: Default::default(),
                    input_defaults: Default::default(),
                },
                NodeSpec {
                    id: format!("{id}::output"),
                    kind: NodeType::Output,
                    params: NodeParams {
                        path: Some(
                            vizij_api_core::TypedPath::parse("sample/value").expect("typed path"),
                        ),
                        ..Default::default()
                    },
                    output_shapes: Default::default(),
                    input_defaults: Default::default(),
                },
            ],
            edges: vec![EdgeSpec {
                from: EdgeOutputEndpoint {
                    node_id: format!("{id}::constant"),
                    output: "out".to_string(),
                },
                to: EdgeInputEndpoint {
                    node_id: format!("{id}::output"),
                    input: "in".to_string(),
                },
                selector: None,
            }],
            ..Default::default()
        }
        .with_cache();

        GraphControllerConfig {
            id: id.to_string(),
            spec,
            subs: Subscriptions::default(),
        }
    }

    #[test]
    fn export_graph_produces_json_value() {
        let cfg = sample_graph("graph:sample", 1.5);
        let orch = Orchestrator::new(Schedule::SinglePass).with_graph(cfg);
        let json = orch.export_graph_json("graph:sample").expect("export ok");
        assert!(json["nodes"].is_array(), "nodes array present");
        assert_eq!(
            json["nodes"][0]["type"].as_str(),
            Some("constant"),
            "first node is constant"
        );
    }

    #[test]
    fn export_graph_pretty_string_round_trips() {
        let cfg = sample_graph("graph:pretty", 2.0);
        let orch = Orchestrator::new(Schedule::SinglePass).with_graph(cfg);
        let json_str = orch
            .export_graph_json_pretty("graph:pretty")
            .expect("export string ok");
        let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("parse ok");
        assert_eq!(
            parsed["nodes"][1]["type"].as_str(),
            Some("output"),
            "output node retained"
        );
    }

    #[test]
    fn export_graph_missing_id_errors() {
        let orch = Orchestrator::new(Schedule::SinglePass);
        let err = orch
            .export_graph_json("missing")
            .expect_err("missing graph should error");
        let msg = format!("{err}");
        assert!(msg.contains("missing"), "error message mentions id");
    }
}
