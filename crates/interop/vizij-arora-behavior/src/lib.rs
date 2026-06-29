//! [`ProcessingGraph`]: a Vizij node graph driven as an Arora
//! [`Behavior`](arora_behavior::Behavior) (VIZ-34).
//!
//! Each tick it reads its subscribed input paths from the shared store (Arora
//! values → Vizij via [`vizij_arora`]), evaluates the graph for `dt`, and writes
//! the graph's outputs back (Vizij → Arora). It always reports
//! [`BehaviorStatus::Running`] — a node graph runs every frame, unlike a tree
//! that runs to a terminal status.
//!
//! Queue one into an Arora runtime with `Runtime::queue_behavior(Box::new(pg))`;
//! it then reads/writes the same blackboard the behavior tree and the bridge do.
//!
//! [`orchestrator::OrchestratorBehavior`] wraps a whole Vizij orchestrator (many
//! controllers + their merge) as one `Behavior` (VIZ-38).

pub mod orchestrator;

use arora_behavior::{Behavior, BehaviorContext, BehaviorError, BehaviorStatus};
use arora_types::data::{DataStore, Key, StateChange};
use vizij_api_core::TypedPath;
use vizij_graph_core::eval::{evaluate_all, GraphRuntime};
use vizij_graph_core::types::GraphSpec;

/// A Vizij node graph as an Arora behavior.
pub struct ProcessingGraph {
    spec: GraphSpec,
    rt: GraphRuntime,
    /// Store paths staged into the graph before each evaluation.
    inputs: Vec<TypedPath>,
}

impl ProcessingGraph {
    /// Wrap a graph spec plus the store paths it consumes as inputs.
    pub fn from_spec(spec: GraphSpec, inputs: Vec<TypedPath>) -> Self {
        Self {
            spec: spec.with_cache(),
            rt: GraphRuntime::default(),
            inputs,
        }
    }

    /// Tick the graph against `store` for `dt`: read subscribed inputs, evaluate,
    /// write outputs. This is the inherent method behind the [`Behavior`] impl —
    /// handy for driving a graph directly and for tests.
    pub fn tick_store(&mut self, store: &dyn DataStore, dt: f32) -> Result<(), BehaviorError> {
        let delta = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.rt.dt = delta;
        self.rt.t += delta;

        // Read subscribed inputs from the store and stage them into the graph.
        for tp in &self.inputs {
            let key = Key::new(tp.to_string());
            if let Some(value) = store
                .read(std::slice::from_ref(&key))
                .into_iter()
                .next()
                .flatten()
            {
                let vizij = vizij_arora::from_arora(&value).map_err(conv)?;
                self.rt.set_input(tp.clone(), vizij, None);
            }
        }

        evaluate_all(&mut self.rt, &self.spec).map_err(|message| BehaviorError { message })?;

        // Write the graph's outputs back to the store.
        let writes = std::mem::take(&mut self.rt.writes);
        let mut change = StateChange::new();
        for op in writes.iter() {
            let value = vizij_arora::to_arora(&op.value).map_err(conv)?;
            change
                .set
                .insert(Key::new(op.path.to_string()), Some(value));
        }
        store.write(change).map_err(|e| BehaviorError {
            message: e.to_string(),
        })?;
        Ok(())
    }
}

impl Behavior for ProcessingGraph {
    fn tick(&mut self, ctx: &mut BehaviorContext) -> Result<BehaviorStatus, BehaviorError> {
        self.tick_store(ctx.store, ctx.dt)?;
        // A node graph is continuous: tick it again next step.
        Ok(BehaviorStatus::Running)
    }
}

fn conv(e: vizij_arora::ConversionError) -> BehaviorError {
    BehaviorError {
        message: e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_simple_data_store::SimpleDataStore;
    use arora_types::value::Value as AValue;
    use serde_json::json;
    use vizij_api_core::Value as VValue;

    fn passthrough(input: &str, output: &str) -> GraphSpec {
        let mut spec = json!({
            "nodes": [
                { "id": "in",  "type": "input",  "params": { "path": input } },
                { "id": "out", "type": "output", "params": { "path": output } }
            ],
            "edges": [
                { "from": { "node_id": "in" }, "to": { "node_id": "out", "input": "in" } }
            ]
        });
        vizij_api_core::json::normalize_graph_spec_value(&mut spec).expect("normalize");
        serde_json::from_value(spec).expect("graph spec")
    }

    fn read(store: &SimpleDataStore, path: &str) -> Option<AValue> {
        store.read(&[Key::from(path)]).into_iter().next().flatten()
    }

    #[test]
    fn graph_reads_and_writes_the_arora_store() {
        let store = SimpleDataStore::new();
        let mut graph = ProcessingGraph::from_spec(
            passthrough("sensor/x", "actuator/y"),
            vec![TypedPath::parse("sensor/x").unwrap()],
        );

        // A scalar flows store -> graph -> store.
        store
            .write(StateChange::set(
                "sensor/x",
                vizij_arora::to_arora(&VValue::Float(0.75)).unwrap(),
            ))
            .unwrap();
        graph.tick_store(&store, 0.016).expect("tick");
        assert_eq!(read(&store, "actuator/y"), Some(AValue::F32(0.75)));

        // A Vizij composite flows through as a Value::Structure too.
        let vec3 = vizij_arora::to_arora(&VValue::Vec3([1.0, 2.0, 3.0])).unwrap();
        store
            .write(StateChange::set("sensor/x", vec3.clone()))
            .unwrap();
        graph.tick_store(&store, 0.016).expect("tick");
        assert_eq!(read(&store, "actuator/y"), Some(vec3));
    }
}
