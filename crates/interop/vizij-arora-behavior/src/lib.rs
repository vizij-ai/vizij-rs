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

use std::collections::HashMap;

use arora_behavior::{golden, BehaviorContext, BehaviorError, BehaviorInterpreter, BehaviorStatus};
use arora_types::call::{Call, CallBridge};
use arora_types::data::{DataStore, Key, StateChange};
use arora_types::value::{StructureField, Value};
use uuid::Uuid;
use vizij_api_core::TypedPath;
use vizij_graph_core::eval::{evaluate_all_with_functions, GraphRuntime, NodeFunctions};
use vizij_graph_core::types::GraphSpec;

/// A Vizij node graph spec over the Arora [`Value`] — the concrete value this
/// behavior binds `vizij-graph-core`'s value-generic spec to. Thin wrappers
/// (e.g. `vizij-arora-web`) name this rather than re-picking the value type.
pub type BehaviorGraphSpec = GraphSpec<Value>;

/// Adapts an Arora [`CallBridge`] to graph-core's [`NodeFunctions`] host interface.
///
/// A graph `ExternalFunction` node carries only an opaque string id for the function it invokes.
/// This adapter treats that id as an Arora function UUID (its string form). Arora's
/// [`CallBridge::arora_call`] dispatches by *module* id — and the engine looks the module up
/// directly, ignoring `Call::module_id` (see `arora-engine`'s `Engine::arora_call`). So this
/// adapter must know which module each function lives in; it holds a `function -> module` map
/// supplied at construction. The map is built from module-load summaries (arora-engine's
/// `LoadedModule { id, function_ids }`); this crate does not own that plumbing.
struct CallBridgeFunctions<'a> {
    bridge: &'a mut dyn CallBridge,
    /// function id -> module id, so a bare function handle can be dispatched to `arora_call`.
    function_modules: &'a HashMap<Uuid, Uuid>,
}

impl<'a> NodeFunctions<Value> for CallBridgeFunctions<'a> {
    fn call(&mut self, function: &str, args: &[(Uuid, Value)]) -> Result<Value, String> {
        let function_id = Uuid::parse_str(function)
            .map_err(|_| format!("external function id '{function}' is not a valid UUID"))?;
        let module_id = *self
            .function_modules
            .get(&function_id)
            .ok_or_else(|| format!("no module registered for external function {function_id}"))?;
        let args: Vec<StructureField> = args
            .iter()
            .map(|(id, value)| StructureField {
                id: *id,
                value: Box::new(value.clone()),
            })
            .collect();
        let result = self
            .bridge
            .arora_call(
                &module_id,
                Call {
                    module_id: Some(module_id),
                    id: function_id,
                    args,
                },
            )
            .map_err(|e| format!("module call failed: {e}"))?;
        Ok(result.ret)
    }
}

/// A Vizij node graph as an Arora behavior interpreter.
pub struct ProcessingGraph {
    spec: GraphSpec<Value>,
    rt: GraphRuntime<Value>,
    /// Store paths staged into the graph before each evaluation.
    inputs: Vec<TypedPath>,
    /// function id -> module id, so `ExternalFunction` nodes can be dispatched through the
    /// [`CallBridge`]. See [`CallBridgeFunctions`] for why this map is needed and where it
    /// should come from.
    function_modules: HashMap<Uuid, Uuid>,
}

impl ProcessingGraph {
    /// Wrap a graph spec plus the store paths it consumes as inputs.
    pub fn from_spec(spec: GraphSpec<Value>, inputs: Vec<TypedPath>) -> Self {
        Self {
            spec: spec.with_cache(),
            rt: GraphRuntime::default(),
            inputs,
            function_modules: HashMap::new(),
        }
    }

    /// Set the `function id -> module id` map used to dispatch `ExternalFunction` nodes.
    ///
    /// Until this is populated, an `ExternalFunction` node errors with "no module registered".
    pub fn set_function_modules(&mut self, function_modules: HashMap<Uuid, Uuid>) {
        self.function_modules = function_modules;
    }

    /// Tick the graph against `store` for `dt`: read subscribed inputs, evaluate,
    /// write outputs. This is the inherent method behind the
    /// [`BehaviorInterpreter`] impl — handy for driving a graph directly and
    /// for tests.
    ///
    /// `call_bridge` is the Arora host call interface; `ExternalFunction` nodes dispatch through
    /// it, resolving each function to its module via the `function id -> module id` map.
    pub fn tick_store(
        &mut self,
        store: &dyn DataStore,
        call_bridge: &mut dyn CallBridge,
        dt: f32,
    ) -> Result<(), BehaviorError> {
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

        let mut functions = CallBridgeFunctions {
            bridge: call_bridge,
            function_modules: &self.function_modules,
        };
        evaluate_all_with_functions(&mut self.rt, &self.spec, &mut functions)
            .map_err(|message| BehaviorError { message })?;

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
        self.tick_store(ctx.store, &mut *ctx.call_bridge, dt)?;
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
    use arora_types::call::{Call, CallError, CallResult, Callable, CallableId};
    use serde_json::json;
    use std::rc::Rc;
    use uuid::Uuid;
    use vizij_api_core::value::{float, vec3};

    /// A bridge the passthrough graphs never invoke (they contain no ExternalFunction nodes).
    #[derive(Default)]
    struct NoopBridge;

    impl CallBridge for NoopBridge {
        fn arora_call(&mut self, _module: &Uuid, _call: Call) -> Result<CallResult, CallError> {
            unimplemented!("passthrough graphs make no external function calls")
        }
        fn arora_register_callable(&mut self, _callable: Rc<dyn Callable>) -> CallableId {
            unimplemented!()
        }
        fn arora_unregister_callable(&mut self, _callable_id: &CallableId) {
            unimplemented!()
        }
        fn arora_call_indirect(&mut self, _callable_id: &CallableId) -> Result<Value, CallError> {
            unimplemented!()
        }
    }

    fn passthrough(input: &str, output: &str) -> GraphSpec<Value> {
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

        let mut bridge = NoopBridge;

        // A scalar flows store -> graph -> store.
        store
            .write(StateChange::set("sensor/x", float(0.75)))
            .unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(read(&store, "actuator/y"), Some(float(0.75)));

        // A Vizij composite (`Value::Structure`) flows through unchanged too.
        let pos = vec3([1.0, 2.0, 3.0]);
        store
            .write(StateChange::set("sensor/x", pos.clone()))
            .unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
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
