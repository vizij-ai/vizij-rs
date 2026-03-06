//! Deterministic orchestration layer for Vizij graphs, animations, and blackboard state.
//!
//! The orchestrator coordinates graph and animation controllers against a shared blackboard,
//! runs them according to a configurable schedule, and returns merged writes plus conflict
//! diagnostics for each frame.

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

    /// Export the specified graph's `GraphSpec` as a `serde_json::Value`.
    pub fn export_graph_json(&self, id: &str) -> Result<serde_json::Value> {
        let controller = self
            .graphs
            .get(id)
            .ok_or_else(|| anyhow!("graph '{id}' is not registered"))?;
        serde_json::to_value(&controller.spec).map_err(|err| err.into())
    }

    /// Export the specified graph's `GraphSpec` as a pretty formatted JSON string.
    pub fn export_graph_json_pretty(&self, id: &str) -> Result<String> {
        let controller = self
            .graphs
            .get(id)
            .ok_or_else(|| anyhow!("graph '{id}' is not registered"))?;
        serde_json::to_string_pretty(&controller.spec).map_err(|err| err.into())
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
