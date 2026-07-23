//! [`ProcessingGraph`]: a Vizij node graph driven as an Arora
//! [`BehaviorInterpreter`](arora_behavior::BehaviorInterpreter) (VIZ-34).
//!
//! Each tick it reads its subscribed input paths from the shared store,
//! evaluates the graph for `dt`, and writes the graph's outputs back. Vizij
//! and Arora share one runtime value type ([`vizij_api_core::Value`] is
//! `arora_types::value::Value`), so values cross the store boundary directly.
//! The tick always reports [`BehaviorStatus::Running`] — a node graph runs
//! every frame, unlike a tree that runs to a terminal status. `dt` comes from
//! the runtime's built-in store key ([`arora_behavior::built_in::DT`], nanoseconds
//! since the previous step), published before each tick.
//!
//! Inject one into an Arora device with
//! `AroraBuilder::with_behavior_interpreter(Box::new(pg))`; it then
//! reads/writes the same blackboard the bridge and the HAL do. Swapping the
//! running graph does not rebuild the device: a [`spec_graph`] LOAD call
//! reaches [`ProcessingGraph::load`] through the engine's interpreter module.
//!
//! [`ProcessingGraph::load`]: arora_behavior::BehaviorInterpreter::load

pub mod spec_graph;

use std::collections::HashMap;

use arora_behavior::{
    built_in, BehaviorContext, BehaviorError, BehaviorInterpreter, BehaviorStatus, Graph,
};
use arora_types::call::{Call, CallBridge};
use arora_types::data::{DataStore, Key, StateChange};
use arora_types::value::{StructureField, Value};
use uuid::Uuid;
use vizij_api_core::TypedPath;
use vizij_graph_core::eval::{evaluate_all_with_functions, GraphRuntime, NodeFunctions};
use vizij_graph_core::types::{GraphSpec, NodeType};

/// Adapts an Arora [`CallBridge`] to graph-core's [`NodeFunctions`] host interface.
///
/// A graph `ExternalFunction` node carries an opaque function [`Uuid`] for the function it invokes.
/// The engine routes a [`Call`] by its `module_id` and refuses one naming no module, so this
/// adapter must know which module each function lives in; it holds a `function -> module` map
/// supplied at construction. The map is built from module-load summaries (arora-engine's
/// `LoadedModule { id, function_ids }`); this crate does not own that plumbing.
struct CallBridgeFunctions<'a> {
    bridge: &'a mut dyn CallBridge,
    /// function id -> module id, so a bare function handle can be dispatched to `arora_call`.
    function_modules: &'a HashMap<Uuid, Uuid>,
}

impl NodeFunctions for CallBridgeFunctions<'_> {
    fn call(&mut self, function: Uuid, args: &[(Uuid, Value)]) -> Result<Value, String> {
        let module_id = *self
            .function_modules
            .get(&function)
            .ok_or_else(|| format!("no module registered for external function {function}"))?;
        let args: Vec<StructureField> = args
            .iter()
            .map(|(id, value)| StructureField {
                id: *id,
                value: Box::new(value.clone()),
            })
            .collect();
        let result = self
            .bridge
            .arora_call(Call {
                module_id: Some(module_id),
                id: function,
                args,
            })
            .map_err(|e| format!("module call failed: {e}"))?;
        Ok(result.ret)
    }
}

/// A Vizij node graph as an Arora behavior interpreter.
pub struct ProcessingGraph {
    spec: GraphSpec,
    rt: GraphRuntime,
    /// Store paths staged into the graph before each evaluation.
    inputs: Vec<TypedPath>,
    /// function id -> module id, so `ExternalFunction` nodes can be dispatched through the
    /// [`CallBridge`]. See [`CallBridgeFunctions`] for why this map is needed and where it
    /// should come from.
    function_modules: HashMap<Uuid, Uuid>,
}

/// Normalize and deserialize a Vizij graph spec from JSON (any form the spec
/// normalizer accepts).
pub fn parse_spec(json: &str) -> Result<GraphSpec, String> {
    let mut spec: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("graph spec is not JSON: {e}"))?;
    vizij_api_core::json::normalize_graph_spec_value(&mut spec)
        .map_err(|e| format!("normalize graph spec failed: {e}"))?;
    serde_json::from_value(spec).map_err(|e| format!("invalid graph spec: {e}"))
}

/// The store paths the spec's `input` nodes read — what the graph subscribes
/// to on the device's store.
pub fn input_paths(spec: &GraphSpec) -> Vec<TypedPath> {
    spec.nodes
        .iter()
        .filter(|node| matches!(node.kind, NodeType::Input))
        .filter_map(|node| node.params.path.clone())
        .collect()
}

impl ProcessingGraph {
    /// Wrap a graph spec plus the store paths it consumes as inputs.
    pub fn from_spec(spec: GraphSpec, inputs: Vec<TypedPath>) -> Self {
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
        let dt = built_in_dt_seconds(ctx.store);
        self.tick_store(ctx.store, &mut *ctx.call_bridge, dt)?;
        // A node graph is continuous: tick it again next step.
        Ok(BehaviorStatus::Running)
    }

    /// Replace the running Vizij graph in place — the interpreter module's
    /// LOAD entry point, reached through the engine like any module call, so a
    /// recompose never rebuilds the device (VIZ-57).
    ///
    /// `graph` must be a [`spec_graph`] carrier: the shared model's one-node
    /// form whose literal input holds the Vizij spec JSON (see that module for
    /// why the spec rides the shared model opaquely). The spec is parsed and
    /// installed in place while the graph runtime is kept **warm**: nodes that
    /// survive the swap keep their integration state (springs/dampers/URDF
    /// chains) and the graph clock stays continuous, so a program starting or
    /// stopping no longer restarts every stateful node. The store and the
    /// `function -> module` map are untouched — the store belongs to the
    /// device, and the loaded-module set is fixed at device build.
    fn load(&mut self, graph: Graph) -> Result<(), BehaviorError> {
        let json = spec_graph::decode(&graph).map_err(|message| BehaviorError { message })?;
        let mut spec = parse_spec(&json).map_err(|message| BehaviorError { message })?;
        self.inputs = input_paths(&spec);
        // Keep `self.rt` warm across the swap: `evaluate_all` garbage-collects
        // state for nodes that disappear and grows storage for new ones, so no
        // manual reset is needed and surviving nodes keep their state.
        //
        // Carry the version forward before re-caching. A freshly parsed spec
        // restarts at version 0 (→ 1 after `with_cache`); left as-is that would
        // collide with the previously installed spec's version and, since the
        // eval path takes the version-keyed `PlanCache` fast path, serve the
        // *old* plan for the new graph. Bumping from the current version keeps
        // it strictly increasing so the plan always rebuilds for the new
        // topology.
        spec.version = self.spec.version;
        self.spec = spec.with_cache();
        Ok(())
    }
}

/// The current step's `dt` in seconds, read from the runtime-maintained
/// built-in key ([`built_in::DT`], integer nanoseconds). `0.0` when the key is
/// absent or not the `U64` the runtime publishes.
pub(crate) fn built_in_dt_seconds(store: &dyn DataStore) -> f32 {
    match store
        .read(&[Key::from(built_in::DT)])
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
    use vizij_api_core::value::{float, vec3};

    /// A bridge the passthrough graphs never invoke (they contain no ExternalFunction nodes).
    #[derive(Default)]
    struct NoopBridge;

    impl CallBridge for NoopBridge {
        fn arora_call(&mut self, _call: Call) -> Result<CallResult, CallError> {
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

    /// The in-place load path: a spec-carrier graph swaps the running Vizij
    /// graph without touching the store or the device around it.
    #[test]
    fn load_swaps_the_graph_in_place() {
        let store = SimpleDataStore::new();
        let mut graph = ProcessingGraph::from_spec(
            passthrough("sensor/x", "actuator/y"),
            vec![TypedPath::parse("sensor/x").unwrap()],
        );
        let mut bridge = NoopBridge;

        store
            .write(StateChange::set("sensor/x", float(0.5)))
            .unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(read(&store, "actuator/y"), Some(float(0.5)));

        // Load a different passthrough; the next tick runs the new spec.
        let json = serde_json::json!({
            "nodes": [
                { "id": "in",  "type": "input",  "params": { "path": "sensor/b" } },
                { "id": "out", "type": "output", "params": { "path": "actuator/b" } }
            ],
            "edges": [
                { "from": { "node_id": "in" }, "to": { "node_id": "out", "input": "in" } }
            ]
        })
        .to_string();
        graph
            .load(spec_graph::encode(&json))
            .expect("the carrier graph loads");

        store
            .write(StateChange::set("sensor/b", float(0.25)))
            .unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(read(&store, "actuator/b"), Some(float(0.25)));
        // The old spec no longer runs…
        store
            .write(StateChange::set("sensor/x", float(0.9)))
            .unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(read(&store, "actuator/y"), Some(float(0.5)));
        // …and the store around the swap was never reset.
        assert_eq!(read(&store, "sensor/x"), Some(float(0.9)));
    }

    /// A non-carrier graph is refused: Vizij edition goes through the
    /// spec-carrier load, not the shared model's structural form.
    #[test]
    fn load_rejects_a_non_carrier_graph() {
        let mut graph = ProcessingGraph::from_spec(passthrough("a", "b"), vec![]);
        assert!(graph.load(Graph::empty()).is_err());
    }

    /// A load keeps the graph runtime warm: the graph clock (surfaced by a
    /// `Time` node, which reads `rt.t`) stays continuous across a recompose
    /// instead of restarting at zero. Guards the runtime-continuity behavior
    /// and, with `load_swaps_the_graph_in_place`, the version-carry that keeps
    /// the plan cache from serving the old plan for the new graph.
    fn clock_graph_json(output: &str) -> serde_json::Value {
        json!({
            "nodes": [
                { "id": "clock", "type": "time" },
                { "id": "out", "type": "output", "params": { "path": output } }
            ],
            "edges": [
                { "from": { "node_id": "clock" }, "to": { "node_id": "out", "input": "in" } }
            ]
        })
    }

    #[test]
    fn load_keeps_the_graph_runtime_warm() {
        let mut initial = clock_graph_json("clock/a");
        vizij_api_core::json::normalize_graph_spec_value(&mut initial).expect("normalize");
        let initial: GraphSpec = serde_json::from_value(initial).expect("graph spec");

        let store = SimpleDataStore::new();
        let mut graph = ProcessingGraph::from_spec(initial, vec![]);
        let mut bridge = NoopBridge;

        // Accumulate three frames of graph time (rt.t ~= 0.3).
        for _ in 0..3 {
            graph.tick_store(&store, &mut bridge, 0.1).expect("tick");
        }
        match read(&store, "clock/a") {
            Some(Value::F32(t)) => {
                assert!((t - 0.3).abs() < 1e-3, "clock/a = {t}, expected ~0.3")
            }
            other => panic!("expected F32, got {other:?}"),
        }

        // Recompose to a different clock graph. The runtime stays warm, so the
        // clock keeps counting from ~0.3 rather than restarting at 0 — a reset
        // runtime would show ~0.1 here.
        graph
            .load(spec_graph::encode(&clock_graph_json("clock/b").to_string()))
            .expect("carrier graph loads");
        graph.tick_store(&store, &mut bridge, 0.1).expect("tick");
        match read(&store, "clock/b") {
            Some(Value::F32(t)) => {
                assert!(t > 0.35, "clock/b = {t}, expected warm continuation ~0.4")
            }
            other => panic!("expected F32, got {other:?}"),
        }
    }

    #[test]
    fn built_in_dt_reads_the_runtime_clock() {
        let store = SimpleDataStore::new();
        assert_eq!(built_in_dt_seconds(&store), 0.0);
        store
            .write(StateChange::set(built_in::DT, Value::U64(16_000_000)))
            .unwrap();
        assert!((built_in_dt_seconds(&store) - 0.016).abs() < 1e-6);
    }

    /// A path-less `output` applies a keyed record batch — the shape a module
    /// call's "what changed" arrives in — onto the store keys the records
    /// name, through the tick's single StateChange flush.
    #[test]
    fn pathless_output_applies_a_keyed_batch_to_the_store() {
        const KEY_FIELD: &str = "76697a69-0000-0000-0000-00000000aaaa";
        const VALUE_FIELD: &str = "76697a69-0000-0000-0000-00000000bbbb";
        const RECORD_TYPE: &str = "76697a69-0000-0000-0000-00000000cccc";

        let record = |key: &str, v: f32| {
            json!({ "fields": [
                { "id": KEY_FIELD, "value": { "str": key } },
                { "id": VALUE_FIELD, "value": { "f32": v } },
            ]})
        };
        let mut spec = json!({
            "nodes": [
                { "id": "src", "type": "constant", "params": { "value": {
                    "structs": { "id": RECORD_TYPE, "elements": [
                        record("anim/x", 0.25),
                        record("anim/y", 0.5),
                        // A repeated key: batch order is preserved into the
                        // write set, and the StateChange flush (a map) keeps
                        // the last entry. Explicit combination of concurrent
                        // publishers is VIZ-76's ground.
                        record("anim/x", 0.75),
                    ]}
                }}},
                { "id": "sink", "type": "output", "params": {
                    "key_field": KEY_FIELD, "value_field": VALUE_FIELD
                }}
            ],
            "edges": [
                { "from": { "node_id": "src" }, "to": { "node_id": "sink", "input": "in" } }
            ]
        });
        vizij_api_core::json::normalize_graph_spec_value(&mut spec).expect("normalize");
        let spec: GraphSpec = serde_json::from_value(spec).expect("graph spec");

        let store = SimpleDataStore::new();
        let mut graph = ProcessingGraph::from_spec(spec, vec![]);
        let mut bridge = NoopBridge;
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");

        assert_eq!(read(&store, "anim/x"), Some(float(0.75)));
        assert_eq!(read(&store, "anim/y"), Some(float(0.5)));
    }
}
