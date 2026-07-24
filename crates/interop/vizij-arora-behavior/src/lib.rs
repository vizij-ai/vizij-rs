//! [`ProcessingGraph`]: a Vizij node graph driven as an Arora
//! [`BehaviorInterpreter`] (VIZ-34).
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
//! reads/writes the same blackboard the bridge and the HAL do. The running
//! graph is the shared model's [`graph_codec`] form: it is swapped whole with a
//! LOAD call ([`ProcessingGraph::load`]) or edited node-by-node with an EDIT
//! call carrying a [`GraphDiff`] ([`ProcessingGraph::apply`]), both reaching the
//! interpreter through the engine's interpreter module, so neither rebuilds the
//! device.
//!
//! [`ProcessingGraph::load`]: arora_behavior::BehaviorInterpreter::load
//! [`ProcessingGraph::apply`]: arora_behavior::BehaviorInterpreter::apply

pub mod graph_codec;

use std::collections::HashMap;

use arora_behavior::graph::GraphDiff;
use arora_behavior::{
    built_in, interpreter_module, BehaviorContext, BehaviorError, BehaviorInterpreter,
    BehaviorStatus, Graph,
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
    /// The retained shared-model graph — the editable source of truth. Edits
    /// ([`load`](BehaviorInterpreter::load), [`apply`](BehaviorInterpreter::apply))
    /// mutate this; the evaluator's [`spec`](Self::spec) is re-lowered from it
    /// when [`dirty`](Self::dirty).
    graph: Graph,
    /// The lowered Vizij spec the evaluator runs — [`graph_codec::decode`] of
    /// [`graph`](Self::graph), rebuilt on the next tick after an edit.
    spec: GraphSpec,
    /// Whether [`graph`](Self::graph) changed since [`spec`](Self::spec) was
    /// last lowered.
    dirty: bool,
    rt: GraphRuntime,
    /// Store paths staged into the graph before each evaluation. Derived from
    /// the lowered spec's `input` nodes each time the graph is re-lowered.
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

/// Normalize and deserialize a [`graph_codec::GraphSpecDiff`] from JSON. The
/// upserted nodes and edges are run through the same spec normalizer as
/// [`parse_spec`] (they may use vizij shorthand value forms).
pub fn parse_spec_diff(json: &str) -> Result<graph_codec::GraphSpecDiff, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("graph edit is not JSON: {e}"))?;
    if let Some(object) = value.as_object_mut() {
        let empty = || serde_json::Value::Array(Vec::new());
        let mut spec = serde_json::json!({
            "nodes": object.get("upsert_nodes").cloned().unwrap_or_else(empty),
            "edges": object.get("upsert_edges").cloned().unwrap_or_else(empty),
        });
        vizij_api_core::json::normalize_graph_spec_value(&mut spec)
            .map_err(|e| format!("normalize graph edit failed: {e}"))?;
        object.insert("upsert_nodes".to_string(), spec["nodes"].take());
        object.insert("upsert_edges".to_string(), spec["edges"].take());
    }
    serde_json::from_value(value).map_err(|e| format!("invalid graph edit: {e}"))
}

/// Build the interpreter-module LOAD [`Call`] that installs `spec` as the
/// running behavior (its [`graph_codec`] form). An embedder dispatches this
/// (through an `arora::Caller` or `Arora::call`) to swap the Vizij graph in
/// place — reaching [`ProcessingGraph::load`](BehaviorInterpreter::load).
pub fn encode_load_call(spec: &GraphSpec) -> Result<Call, String> {
    Ok(interpreter_module::encode_load(&graph_codec::encode(spec)?))
}

/// Build the interpreter-module EDIT [`Call`] that applies `diff` to the running
/// behavior (as a [`graph_codec`] [`GraphDiff`]). An embedder dispatches this to
/// edit the Vizij graph in place — reaching
/// [`ProcessingGraph::apply`](BehaviorInterpreter::apply).
pub fn encode_edit_call(diff: &graph_codec::GraphSpecDiff) -> Result<Call, String> {
    Ok(interpreter_module::encode_edit(
        &graph_codec::spec_diff_to_graph_diff(diff)?,
    ))
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
    /// Build from a Vizij graph spec: encode it to the shared model's
    /// [`graph_codec`] form (the retained, editable source of truth). Errors only
    /// if the spec cannot be structurally encoded (it is total over valid specs).
    /// The spec is lowered — and the input paths derived — at the first tick.
    pub fn from_spec(spec: GraphSpec) -> Result<Self, String> {
        Ok(Self {
            graph: graph_codec::encode(&spec)?,
            spec: GraphSpec::default(),
            dirty: true,
            rt: GraphRuntime::default(),
            inputs: Vec::new(),
            function_modules: HashMap::new(),
        })
    }

    /// Re-lower the evaluator's spec from the retained graph and refresh the
    /// input paths, keeping the runtime warm and the plan-cache version
    /// monotonic. Applied at the next tick after an edit, so a lowering problem
    /// surfaces there (the store-carrying phase), per the [`BehaviorInterpreter`]
    /// contract.
    fn lower(&mut self) -> Result<(), BehaviorError> {
        let mut spec =
            graph_codec::decode(&self.graph).map_err(|message| BehaviorError { message })?;
        self.inputs = input_paths(&spec);
        // Carry the version forward before re-caching. A freshly decoded spec
        // restarts at version 0 (→ 1 after `with_cache`); bumping from the
        // current version keeps it strictly increasing, so the version-keyed
        // `PlanCache` always rebuilds the plan for the new topology rather than
        // serving the previous graph's plan.
        spec.version = self.spec.version;
        self.spec = spec.with_cache();
        self.dirty = false;
        Ok(())
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
        // An edit landed since the last lowering: rebuild the spec from the
        // retained graph against this tick, so the edit (and any lowering
        // problem it introduced) takes effect here.
        if self.dirty {
            self.lower()?;
        }

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

    /// Replace the running Vizij graph in place — the interpreter module's LOAD
    /// entry point, reached through the engine like any module call, so a
    /// recompose never rebuilds the device (VIZ-57).
    ///
    /// `graph` is the shared model's [`graph_codec`] form of the new Vizij graph.
    /// It becomes the retained graph and lowers at the next tick, while the graph
    /// runtime is kept **warm**: nodes that survive the swap keep their
    /// integration state (springs/dampers/URDF chains) and the graph clock stays
    /// continuous, so a program starting or stopping no longer restarts every
    /// stateful node. The store and the `function -> module` map are untouched —
    /// the store belongs to the device, and the loaded-module set is fixed at
    /// device build.
    fn load(&mut self, graph: Graph) -> Result<(), BehaviorError> {
        self.graph = graph;
        self.dirty = true;
        Ok(())
    }

    /// Edit the running Vizij graph — the interpreter module's EDIT entry point,
    /// reached through the engine like LOAD. Applies the [`GraphDiff`] to the
    /// retained graph (add/remove nodes and links) and re-lowers at the next
    /// tick. Unedited nodes keep their id, so their runtime state survives the
    /// edit — an add/remove of one node does not restart the rest. The store and
    /// the `function -> module` map are untouched.
    fn apply(&mut self, diff: GraphDiff) -> Result<(), BehaviorError> {
        self.graph.apply(diff).map_err(|e| BehaviorError {
            message: format!("graph diff: {e}"),
        })?;
        self.dirty = true;
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
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");

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
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");
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
        let spec = parse_spec(&json).expect("parse spec");
        graph
            .load(graph_codec::encode(&spec).expect("encode"))
            .expect("the structural graph loads");

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

    /// An `apply(GraphDiff)` edits the running graph in place: adding a node and
    /// rewiring the sink to it changes what the next tick writes, without a
    /// whole-graph reload. This is the EDIT path (VIZ-79) — Vizij edition now
    /// goes through the shared model's structural form, not a spec carrier.
    #[test]
    fn apply_edits_the_running_graph() {
        let store = SimpleDataStore::new();
        // in(sensor/x) -> out(actuator/y): the sink mirrors the sensor.
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");
        let mut bridge = NoopBridge;

        store
            .write(StateChange::set("sensor/x", float(0.1)))
            .unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(read(&store, "actuator/y"), Some(float(0.1)));

        // Insert a constant `k = 0.5` and rewire the sink's input to it. The sink
        // (`out`) is upserted, so its incident edge is included per the diff
        // contract; the old `in -> out` edge is replaced by `k -> out`.
        let diff = graph_codec::GraphSpecDiff {
            upsert_nodes: serde_json::from_value(json!([
                { "id": "k",   "type": "constant", "params": { "value": { "f32": 0.5 } } },
                { "id": "out", "type": "output",   "params": { "path": "actuator/y" } }
            ]))
            .unwrap(),
            upsert_edges: serde_json::from_value(json!([
                { "from": { "node_id": "k", "output": "out" }, "to": { "node_id": "out", "input": "in" } }
            ]))
            .unwrap(),
            ..Default::default()
        };
        graph
            .apply(graph_codec::spec_diff_to_graph_diff(&diff).expect("translate"))
            .expect("apply");

        // The sink now writes the constant, not the sensor.
        store
            .write(StateChange::set("sensor/x", float(0.9)))
            .unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(read(&store, "actuator/y"), Some(float(0.5)));
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
        let mut graph = ProcessingGraph::from_spec(initial).expect("from_spec");
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
        let next = parse_spec(&clock_graph_json("clock/b").to_string()).expect("parse spec");
        graph
            .load(graph_codec::encode(&next).expect("encode"))
            .expect("structural graph loads");
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
        let mut graph = ProcessingGraph::from_spec(spec).expect("from_spec");
        let mut bridge = NoopBridge;
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");

        assert_eq!(read(&store, "anim/x"), Some(float(0.75)));
        assert_eq!(read(&store, "anim/y"), Some(float(0.5)));
    }
}
