use anyhow::{anyhow, Result};

use vizij_api_core::{TypedPath, WriteBatch};
use vizij_graph_core::eval::{evaluate_all, GraphRuntime};
use vizij_graph_core::types::GraphSpec;

use crate::blackboard::Blackboard;

/// Subscriptions specify which blackboard paths a graph consumes/produces.
/// Only subscribed input paths will be staged into the GraphRuntime to reduce
/// unnecessary work and keep evaluation deterministic.
#[derive(Debug, Clone)]
pub struct Subscriptions {
    pub inputs: Vec<TypedPath>,
    pub outputs: Vec<TypedPath>,
    pub mirror_writes: bool,
}

impl Default for Subscriptions {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            mirror_writes: true,
        }
    }
}

/// Lightweight config for registering a graph with the orchestrator.
#[derive(Debug, Clone)]
pub struct GraphControllerConfig {
    pub id: String,
    pub spec: GraphSpec,
    /// Optional subscriptions to restrict staging/publishing.
    pub subs: Subscriptions,
}

/// Controller owning a persistent GraphRuntime for evaluations.
#[derive(Debug)]
pub struct GraphController {
    pub id: String,
    pub spec: GraphSpec,
    pub rt: GraphRuntime,
    pub subs: Subscriptions,
}

impl GraphController {
    pub fn new(cfg: GraphControllerConfig) -> Self {
        Self {
            id: cfg.id,
            spec: cfg.spec,
            rt: GraphRuntime::default(),
            subs: cfg.subs,
        }
    }

    /// Evaluate the graph given the current blackboard state and epoch.
    ///
    /// Behavior:
    ///  - Advance the GraphRuntime epoch so newly staged inputs become visible.
    ///  - Stage subscribed Blackboard inputs into the runtime (only inputs listed in Subscriptions).
    ///  - Call evaluate_all(runtime, &spec)
    ///  - Collect runtime.writes and return as WriteBatch.
    pub fn evaluate(&mut self, bb: &mut Blackboard, _epoch: u64, _dt: f32) -> Result<WriteBatch> {
        // Stage only subscribed blackboard entries into the graph runtime.
        for tp in &self.subs.inputs {
            if let Some(entry) = bb.get(&tp.to_string()) {
                let path = tp.clone();
                let value = entry.value.clone();
                let shape = entry.shape.clone();
                self.rt.set_input(path, value, shape);
            }
        }

        // Preserve any pre-populated writes (e.g., injected by tests or external tooling)
        let mut combined = WriteBatch::new();
        combined.append(std::mem::take(&mut self.rt.writes));

        // Call into graph evaluation
        evaluate_all(&mut self.rt, &self.spec).map_err(|e| anyhow!("evaluate_all error: {}", e))?;

        // Collect new writes produced during evaluation and append to combined batch.
        let new_writes: WriteBatch = std::mem::take(&mut self.rt.writes);
        combined.append(new_writes);

        Ok(combined)
    }
}
