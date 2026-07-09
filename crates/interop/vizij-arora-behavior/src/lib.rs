//! [`ProcessingGraph`]: a Vizij node graph driven as an Arora
//! [`BehaviorInterpreter`](arora_behavior::BehaviorInterpreter) (VIZ-34).
//!
//! Each tick it reads its subscribed input paths from the shared store,
//! evaluates the graph for `dt`, and writes the graph's outputs back. Vizij
//! and Arora share one runtime value type ([`vizij_api_core::Value`] is
//! `arora_types::value::Value`), so values cross the store boundary directly.
//! The tick always reports [`BehaviorStatus::Running`] — a node graph runs
//! every frame, unlike a tree that runs to a terminal status. `dt` comes from
//! the runtime's golden store key ([`arora_behavior::golden::DT`], nanoseconds
//! since the previous step), published before each tick.
//!
//! Queue one into an Arora runtime with
//! `Runtime::queue_behavior(Box::new(pg))`; it then reads/writes the same
//! blackboard the behavior tree and the bridge do.
//!
//! [`orchestrator::OrchestratorBehavior`] wraps a whole Vizij orchestrator
//! (many controllers + their merge) as one interpreter (VIZ-38).

pub mod orchestrator;

use arora_behavior::{golden, BehaviorContext, BehaviorError, BehaviorInterpreter, BehaviorStatus};
use arora_types::data::{DataStore, Key, StateChange};
use arora_types::value::Value;
use vizij_api_core::TypedPath;
use vizij_graph_core::eval::{evaluate_all, GraphRuntime};
use vizij_graph_core::types::GraphSpec;

/// A Vizij node graph as an Arora behavior interpreter.
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
    /// write outputs. This is the inherent method behind the
    /// [`BehaviorInterpreter`] impl — handy for driving a graph directly and
    /// for tests.
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
                self.rt.set_input(tp.clone(), value, None);
            }
        }

        evaluate_all(&mut self.rt, &self.spec).map_err(|message| BehaviorError { message })?;

        // Write the graph's outputs back to the store.
        let writes = std::mem::take(&mut self.rt.writes);
        let mut change = StateChange::new();
        for op in writes.into_vec() {
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

impl BehaviorInterpreter for ProcessingGraph {
    fn tick(&mut self, ctx: &mut BehaviorContext) -> Result<BehaviorStatus, BehaviorError> {
        let dt = golden_dt_seconds(ctx.store);
        self.tick_store(ctx.store, dt)?;
        // A node graph is continuous: tick it again next step.
        Ok(BehaviorStatus::Running)
    }
}

/// The current step's `dt` in seconds, read from the runtime-maintained
/// golden key ([`golden::DT`], integer nanoseconds). `0.0` when the key is
/// absent or not the `U64` the runtime publishes.
pub(crate) fn golden_dt_seconds(store: &dyn DataStore) -> f32 {
    match store
        .read(&[Key::from(golden::DT)])
        .into_iter()
        .next()
        .flatten()
    {
        Some(Value::U64(nanos)) => (nanos as f64 / 1e9) as f32,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_simple_data_store::SimpleDataStore;
    use serde_json::json;
    use vizij_api_core::value::{float, vec3};

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

    fn read(store: &SimpleDataStore, path: &str) -> Option<Value> {
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
            .write(StateChange::set("sensor/x", float(0.75)))
            .unwrap();
        graph.tick_store(&store, 0.016).expect("tick");
        assert_eq!(read(&store, "actuator/y"), Some(float(0.75)));

        // A Vizij composite (`Value::Structure`) flows through unchanged too.
        let pos = vec3([1.0, 2.0, 3.0]);
        store
            .write(StateChange::set("sensor/x", pos.clone()))
            .unwrap();
        graph.tick_store(&store, 0.016).expect("tick");
        assert_eq!(read(&store, "actuator/y"), Some(pos));
    }

    #[test]
    fn golden_dt_reads_the_runtime_clock() {
        let store = SimpleDataStore::new();
        assert_eq!(golden_dt_seconds(&store), 0.0);
        store
            .write(StateChange::set(golden::DT, Value::U64(16_000_000)))
            .unwrap();
        assert!((golden_dt_seconds(&store) - 0.016).abs() < 1e-6);
    }
}
