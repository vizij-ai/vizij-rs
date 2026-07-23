//! [`OrchestratorBehavior`]: a whole Vizij orchestrator driven as one Arora
//! [`BehaviorInterpreter`](arora_behavior::BehaviorInterpreter) (VIZ-38).
//!
//! The orchestrator coordinates several graph/animation controllers against its
//! own blackboard and **merges** their writes each frame — the load-bearing
//! piece (error / namespace / blend / add conflict strategies). Wrapping it
//! whole as a single interpreter preserves that merge exactly:
//! [`Orchestrator::step`] runs the controllers and merges internally, and this
//! adapter only moves values across the Arora store boundary — subscribed
//! inputs in, merged writes out, one shared `Value` type in both directions.
//! Decomposing the orchestrator into per-controller interpreters is a later
//! step; doing it this way first is the safe migration that cannot lose the
//! merge semantics.

use arora_behavior::{BehaviorContext, BehaviorError, BehaviorInterpreter, BehaviorStatus};
use arora_types::data::{DataStore, Key, StateChange};
use vizij_api_core::TypedPath;
use vizij_orchestrator::Orchestrator;

use crate::built_in_dt_seconds;

/// A Vizij orchestrator as an Arora behavior interpreter.
pub struct OrchestratorBehavior {
    orchestrator: Orchestrator,
    /// Store paths staged into the orchestrator's blackboard before each step.
    inputs: Vec<TypedPath>,
}

impl OrchestratorBehavior {
    /// Wrap an [`Orchestrator`] plus the store paths it consumes as inputs.
    ///
    /// Pass an empty `inputs` for a self-contained orchestrator (its controllers
    /// drive everything); list the store paths it should read otherwise.
    pub fn new(orchestrator: Orchestrator, inputs: Vec<TypedPath>) -> Self {
        Self {
            orchestrator,
            inputs,
        }
    }

    /// Tick the orchestrator against `store` for `dt`: stage subscribed inputs,
    /// step (controllers run and their writes are merged), then publish the
    /// merged writes. The inherent method behind the [`BehaviorInterpreter`]
    /// impl — handy for driving an orchestrator directly and for tests.
    pub fn tick_store(&mut self, store: &dyn DataStore, dt: f32) -> Result<(), BehaviorError> {
        let delta = if dt.is_finite() { dt.max(0.0) } else { 0.0 };

        // Stage subscribed inputs from the store into the blackboard. The
        // store and the blackboard speak the same `Value`, so each read goes
        // in as-is.
        for tp in &self.inputs {
            let key = Key::new(tp.to_string());
            if let Some(value) = store
                .read(std::slice::from_ref(&key))
                .into_iter()
                .next()
                .flatten()
            {
                self.orchestrator
                    .set_input(&tp.to_string(), value, None)
                    .map_err(|e| BehaviorError {
                        message: e.to_string(),
                    })?;
            }
        }

        // Step: controllers run on the schedule and their writes are merged
        // inside the orchestrator (conflict strategies preserved).
        let frame = self.orchestrator.step(delta).map_err(|e| BehaviorError {
            message: e.to_string(),
        })?;

        // Publish the merged writes to the Arora store.
        let mut change = StateChange::new();
        for op in frame.merged_writes.into_vec() {
            change
                .set
                .insert(Key::new(op.path.to_string()), Some(op.value));
        }
        store.write(change).map_err(|e| BehaviorError {
            message: e.to_string(),
        })?;
        Ok(())
    }
}

impl BehaviorInterpreter for OrchestratorBehavior {
    fn tick(&mut self, ctx: &mut BehaviorContext) -> Result<BehaviorStatus, BehaviorError> {
        let dt = built_in_dt_seconds(ctx.store);
        self.tick_store(ctx.store, dt)?;
        // An orchestrator runs every frame.
        Ok(BehaviorStatus::Running)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_simple_data_store::SimpleDataStore;
    use arora_types::value::Value;
    use vizij_api_core::value::float;
    use vizij_graph_core::types::{
        EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeSpec, NodeType,
    };
    use vizij_orchestrator::{GraphControllerConfig, Schedule, Subscriptions};

    /// A one-graph controller: a constant wired to an output at `path`.
    fn constant_graph(path: &str, value: f32) -> GraphControllerConfig {
        let spec = GraphSpec {
            nodes: vec![
                NodeSpec {
                    id: "k".into(),
                    kind: NodeType::Constant,
                    params: NodeParams {
                        value: Some(float(value)),
                        ..Default::default()
                    },
                    output_shapes: Default::default(),
                    input_defaults: Default::default(),
                },
                NodeSpec {
                    id: "o".into(),
                    kind: NodeType::Output,
                    params: NodeParams {
                        path: Some(TypedPath::parse(path).expect("typed path")),
                        ..Default::default()
                    },
                    output_shapes: Default::default(),
                    input_defaults: Default::default(),
                },
            ],
            edges: vec![EdgeSpec {
                from: EdgeOutputEndpoint {
                    node_id: "k".into(),
                    output: "out".into(),
                },
                to: EdgeInputEndpoint {
                    node_id: "o".into(),
                    input: "in".into(),
                },
                selector: None,
            }],
            ..Default::default()
        }
        .with_cache();

        GraphControllerConfig {
            id: "g".into(),
            spec,
            subs: Subscriptions::default(),
        }
    }

    fn read(store: &SimpleDataStore, path: &str) -> Option<Value> {
        store.read(&[Key::from(path)]).into_iter().next().flatten()
    }

    #[test]
    fn orchestrator_publishes_merged_writes_to_the_store() {
        let store = SimpleDataStore::new();
        let orchestrator =
            Orchestrator::new(Schedule::SinglePass).with_graph(constant_graph("out/value", 2.0));
        let mut behavior = OrchestratorBehavior::new(orchestrator, vec![]);

        // One tick steps the orchestrator; its merged write lands in the store.
        behavior.tick_store(&store, 0.016).expect("tick");
        assert_eq!(read(&store, "out/value"), Some(float(2.0)));
    }
}
