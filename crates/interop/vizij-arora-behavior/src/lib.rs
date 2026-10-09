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
//! Task runs graft into the same graph, each its own fragment under
//! `task/<run id>/`, beside the main behavior until it ends or is halted: a
//! skill's registered [`TaskFragment`], the generic module-call wrapper, or —
//! for [`run::RunBehavior::run_behavior`], the method the interpreter
//! implements itself — the graph the spawn carries. A run's nodes are the
//! graph's nodes under its id, so an EDIT there changes the run, and a LOAD
//! of the main behavior leaves the live runs in place.
//!
//! [`ProcessingGraph::load`]: arora_behavior::BehaviorInterpreter::load
//! [`ProcessingGraph::apply`]: arora_behavior::BehaviorInterpreter::apply

pub mod gaze;
pub mod graph_codec;
pub mod run;
pub mod speech;
pub mod viseme;

use std::collections::{HashMap, HashSet};

use arora_behavior::graph::{GraphDiff, LinkSource};
use arora_behavior::{
    built_in, interpreter_module, BehaviorContext, BehaviorError, BehaviorInterpreter,
    BehaviorStatus, Graph, RunPolicy, TaskHandle, TaskId,
};
use arora_types::call::{Call, CallBridge, CallResult};
use arora_types::data::{DataStore, Key, StateChange};
use arora_types::record::module::frozen;
use arora_types::value::{Structure, StructureField, Value};
use uuid::Uuid;
use vizij_api_core::TypedPath;
use vizij_graph_core::eval::{evaluate_all_with_functions, GraphRuntime, NodeFunctions};
pub use vizij_graph_core::task;
use vizij_graph_core::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeSpec, NodeType,
};

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

impl CallBridgeFunctions<'_> {
    fn dispatch(
        &mut self,
        module_id: Uuid,
        function: Uuid,
        args: &[(Uuid, Value)],
    ) -> Result<Value, String> {
        Ok(self.dispatch_full(module_id, function, args)?.ret)
    }

    /// The call's whole result: its return value and its mutable parameters.
    fn dispatch_full(
        &mut self,
        module_id: Uuid,
        function: Uuid,
        args: &[(Uuid, Value)],
    ) -> Result<CallResult, String> {
        let args: Vec<StructureField> = args
            .iter()
            .map(|(id, value)| StructureField {
                id: *id,
                value: Box::new(value.clone()),
            })
            .collect();
        self.bridge
            .arora_call(Call {
                module_id: Some(module_id),
                id: function,
                args,
            })
            .map_err(|e| format!("module call failed: {e}"))
    }
}

impl NodeFunctions for CallBridgeFunctions<'_> {
    fn call(&mut self, function: Uuid, args: &[(Uuid, Value)]) -> Result<Value, String> {
        let module_id = *self
            .function_modules
            .get(&function)
            .ok_or_else(|| format!("no module registered for external function {function}"))?;
        self.dispatch(module_id, function, args)
    }

    /// The call's return value with its mutable parameters after the call —
    /// what a `say` run's fragment reads the streamed viseme from.
    fn call_module_with_outputs(
        &mut self,
        module: Option<Uuid>,
        function: Uuid,
        args: &[(Uuid, Value)],
    ) -> Result<(Value, Vec<(Uuid, Value)>), String> {
        let module_id = match module {
            Some(module_id) => module_id,
            None => *self
                .function_modules
                .get(&function)
                .ok_or_else(|| format!("no module registered for external function {function}"))?,
        };
        let result = self.dispatch_full(module_id, function, args)?;
        let mutated = result
            .mutated
            .into_iter()
            .map(|field| (field.id, *field.value))
            .collect();
        Ok((result.ret, mutated))
    }

    /// A task-run call names its module itself, so it dispatches without a
    /// `function -> module` entry; one falls back to the map like any other
    /// external function.
    fn call_module(
        &mut self,
        module: Option<Uuid>,
        function: Uuid,
        args: &[(Uuid, Value)],
    ) -> Result<Value, String> {
        match module {
            Some(module_id) => self.dispatch(module_id, function, args),
            None => self.call(function, args),
        }
    }
}

/// The handle-side index of one live run: the function it implements and
/// the status key it reports on. The run itself is graph structure — its
/// nodes are the graph's nodes under its id ([`run_node_prefix`]), whatever
/// EDITs did to them since the spawn.
struct GraphRun {
    /// The function the run implements — what an exclusive spawn halts by.
    function: Uuid,
    status_key: Key,
}

/// The name of the graph's root: the `layers` that runs the main behavior,
/// then the runs.
const RUNNER: &str = "runner";

/// The prefix every node id of run `task` starts with: `task/<run id>`. It
/// also names the run's `flow`.
/// Run ids are uuids of one length, so no other run's ids share it.
fn run_node_prefix(task: TaskId) -> String {
    format!("task/{}", task.0)
}

/// A spawnable skill fragment: the implementation of a task-run function as
/// graph content. When one is registered for a function
/// ([`ProcessingGraph::set_task_fragment`]), SPAWN grafts a copy of it
/// instead of the generic module-call wrapper — the behavior is data,
/// introspectable and editable like the rest of the graph, not host code.
///
/// The fragment speaks a placeholder-path convention, rewritten to the run's
/// key prefix at graft time: an `output` on `task/status` (required) reports
/// the run's `Status`; `task/result` and `task/feedback` land on the handle's
/// result and feedback keys; every other `task/<name>` input is a parameter,
/// its spawn-time argument staged as the input's default and live-updatable
/// through the run's update key of the same name.
#[derive(Clone)]
pub struct TaskFragment {
    pub(crate) spec: GraphSpec,
    /// The function's parameters: id → the placeholder input name each feeds.
    parameters: HashMap<Uuid, String>,
    /// Whether a new run of the function takes over from the one before it:
    /// spawning halts every live run of the same function first, so the
    /// newest run is the only one writing (a viseme player's lips).
    exclusive: bool,
    /// The function the fragment implements, named and signed, when no
    /// module implements it: the interpreter describes it, and the device
    /// lists it under the interpreter module.
    description: Option<frozen::Export>,
}

impl TaskFragment {
    /// Parse a fragment from its asset JSON and the `parameter id → input
    /// name` map of the function it implements. Refused when the spec does
    /// not parse or declares no `task/status` output — a fragment that
    /// cannot report its lifecycle is not a task.
    pub fn parse(json: &str, parameters: HashMap<Uuid, String>) -> Result<Self, String> {
        let spec = parse_spec(json)?;
        if !writes_task_status(&spec) {
            return Err("a task fragment must declare an output on task/status".to_string());
        }
        Ok(Self {
            spec,
            parameters,
            exclusive: false,
            description: None,
        })
    }

    /// Describe the function the fragment implements, for a function no
    /// module implements: the interpreter lists it among its
    /// [`described_methods`](BehaviorInterpreter::described_methods), so a
    /// remote discovers it and spawns it through the interpreter module. A
    /// fragment wrapping a module's own function (the say skill around its
    /// provider) is left undescribed: the module describes it.
    pub fn described(mut self, description: frozen::Export) -> Self {
        self.description = Some(description);
        self
    }

    /// Make new runs of the function take over: spawning one halts every
    /// live run of the same function first (the halted runs end as
    /// preempted), so the newest run alone writes — for a player whose
    /// writes must not compete with an earlier call's.
    pub fn exclusive(mut self) -> Self {
        self.exclusive = true;
        self
    }

    /// [`parse`](Self::parse), with the face's rig prefix applied to the
    /// standard controls the fragment writes (`standard/vizij/…` outputs) —
    /// a fragment is authored against the bare standard, and a face
    /// namespaces its controls under its rig.
    pub fn parse_with_rig_prefix(
        json: &str,
        rig_prefix: &str,
        parameters: HashMap<Uuid, String>,
    ) -> Result<Self, String> {
        let mut spec: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("fragment json: {e}"))?;
        vizij_arora_host::standard::prefix_controls(&mut spec, rig_prefix);
        Self::parse(&spec.to_string(), parameters)
    }
}

/// A Vizij node graph as an Arora behavior interpreter.
pub struct ProcessingGraph {
    /// The retained shared-model graph — the editable source of truth. Edits
    /// ([`load`](BehaviorInterpreter::load), [`apply`](BehaviorInterpreter::apply))
    /// mutate this; the evaluator's [`spec`](Self::spec) is re-lowered from it
    /// when [`dirty`](Self::dirty).
    graph: Graph,
    /// The main behavior's root: the runner's first child, before the runs.
    main: Option<Uuid>,
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
    /// Live task runs by the identity their [`TaskHandle`] carries. Each run is
    /// a grafted fragment in [`graph`](Self::graph); this index holds its
    /// pruning coordinates and status key.
    runs: HashMap<TaskId, GraphRun>,
    /// The live runs in the order they were spawned — the order the runner
    /// holds them in after the main behavior, so a run writes after the
    /// behavior and a later run after an earlier one.
    run_order: Vec<TaskId>,
    /// Registered skill fragments by function id: what SPAWN grafts for these
    /// functions instead of the generic module-call wrapper.
    fragments: HashMap<Uuid, TaskFragment>,
    /// Halts requested since the last tick; applied by the next tick, which
    /// owns the store.
    pending_halts: Vec<TaskId>,
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
        let graph = graph_codec::encode(&spec)?;
        let mut this = Self {
            main: graph.root,
            graph,
            spec: GraphSpec::default(),
            dirty: true,
            rt: GraphRuntime::default(),
            inputs: Vec::new(),
            function_modules: HashMap::new(),
            runs: HashMap::new(),
            run_order: Vec::new(),
            fragments: HashMap::new(),
            pending_halts: Vec::new(),
        };
        this.install_runner();
        Ok(this)
    }

    /// Make the runner the graph's root: a `layers` holding the main
    /// behavior, then each live run's `flow` in spawn order. Re-installed
    /// whenever a run comes or goes.
    fn install_runner(&mut self) {
        let children = self
            .main
            .into_iter()
            .chain(
                self.run_order
                    .iter()
                    .map(|task| graph_codec::composite_id(&run_node_prefix(*task))),
            )
            .collect();
        self.graph.root = Some(graph_codec::insert_composite(
            &mut self.graph,
            graph_codec::layers_function(),
            RUNNER,
            children,
        ));
    }

    /// Register the skill fragment SPAWN grafts for `function` — asset
    /// content implementing the method, in place of the generic module-call
    /// wrapper.
    pub fn set_task_fragment(&mut self, function: Uuid, fragment: TaskFragment) {
        self.fragments.insert(function, fragment);
    }

    /// The retained shared-model graph — the editable source of truth,
    /// including any live task-run fragments. What a LOAD replaces, an EDIT
    /// edits, and an introspector reads.
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Graft run `task` of `function` — its fragment `diff` — into the
    /// retained graph and index it by its status key. Nothing changes on an
    /// error.
    fn graft(
        &mut self,
        task: TaskId,
        function: Uuid,
        status_key: Key,
        diff: graph_codec::GraphSpecDiff,
    ) -> Result<(), BehaviorError> {
        let graph_diff = graph_codec::spec_diff_to_graph_diff(&diff)
            .map_err(|message| BehaviorError { message })?;
        self.graph.apply(graph_diff).map_err(|e| BehaviorError {
            message: format!("graft run: {e}"),
        })?;
        let children = diff
            .upsert_nodes
            .iter()
            .map(|node| graph_codec::node_id_uuid(&node.id))
            .collect();
        graph_codec::insert_composite(
            &mut self.graph,
            graph_codec::flow_function(),
            &run_node_prefix(task),
            children,
        );
        self.runs.insert(
            task,
            GraphRun {
                function,
                status_key,
            },
        );
        self.run_order.push(task);
        self.install_runner();
        if !self.dirty {
            self.lower_grafted_run(diff);
        }
        Ok(())
    }

    /// Lower a grafted run in place rather than the whole graph again. The
    /// run is a component of its own — its edges join its nodes only, and it
    /// meets the rest of the graph through store paths — so its nodes go
    /// after everything lowered so far and its plan after theirs, which is
    /// where a full lowering puts them too: the run's `flow` is the runner's
    /// last child. Whatever is not a separate component is left to a full
    /// lowering.
    fn lower_grafted_run(&mut self, diff: graph_codec::GraphSpecDiff) {
        if !diff.remove_nodes.is_empty() || !diff.remove_edges.is_empty() {
            self.dirty = true;
            return;
        }
        let first = self.spec.nodes.len();
        self.spec.nodes.extend(diff.upsert_nodes);
        self.spec.edges.extend(diff.upsert_edges);
        self.relowered();
        match self.rt.plan.append_component(&self.spec, first) {
            Ok(()) => self.inputs = input_paths(&self.spec),
            Err(_) => self.dirty = true,
        }
    }

    /// Take a pruned run's nodes out of the lowered graph and its plan in
    /// place — nothing else read them, being a component of its own. Left to
    /// a full lowering when they were not.
    fn lower_pruned_run(&mut self, task: TaskId) {
        let prefix = run_node_prefix(task);
        let removed: Vec<usize> = self
            .spec
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.id.starts_with(&prefix))
            .map(|(idx, _)| idx)
            .collect();
        if removed.is_empty() {
            return;
        }
        let removed_ids: HashSet<String> = removed
            .iter()
            .map(|&idx| self.spec.nodes[idx].id.clone())
            .collect();
        self.spec
            .nodes
            .retain(|node| !removed_ids.contains(&node.id));
        self.spec.edges.retain(|edge| {
            !removed_ids.contains(&edge.from.node_id) && !removed_ids.contains(&edge.to.node_id)
        });
        self.relowered();
        match self.rt.plan.remove_nodes(&self.spec, &removed) {
            Ok(()) => self.inputs = input_paths(&self.spec),
            Err(_) => self.dirty = true,
        }
    }

    /// A lowered spec changed in place: a new version for the plan to serve.
    /// Its fingerprint is not recomputed — the plan is keyed by version —
    /// and is cleared rather than left stale.
    fn relowered(&mut self) {
        self.spec.version += 1;
        self.spec.fingerprint = 0;
    }

    /// The ids of the retained graph's nodes that belong to run `task` —
    /// those under its [`run_node_prefix`] — as uuids.
    fn run_nodes(&self, task: TaskId) -> Vec<Uuid> {
        let prefix = run_node_prefix(task);
        self.graph
            .nodes
            .keys()
            .filter(|id| {
                self.graph
                    .variables
                    .get(id)
                    .is_some_and(|name| name.starts_with(&prefix))
            })
            .copied()
            .collect()
    }

    /// Remove run `task`'s nodes from the retained graph; takes effect at the
    /// next lowering. The run's status key keeps its last value — pruning
    /// removes structure, not state.
    fn prune_run(&mut self, task: TaskId) -> Result<(), BehaviorError> {
        let diff = GraphDiff {
            remove_nodes: self.run_nodes(task),
            ..GraphDiff::default()
        };
        self.graph.apply(diff).map_err(|e| BehaviorError {
            message: format!("prune run: {e}"),
        })?;
        self.run_order.retain(|live| *live != task);
        self.install_runner();
        if !self.dirty {
            self.lower_pruned_run(task);
        }
        Ok(())
    }

    /// Place each node an edit added that no composite holds: a run's node
    /// at the end of its run's `flow`, any other at the end of the main
    /// behavior, so an edit to the behavior still runs before the runs.
    fn place(&mut self, added: &[Uuid]) {
        let held: HashSet<Uuid> = self
            .graph
            .nodes
            .values()
            .filter(|node| graph_codec::is_composite(node))
            .flat_map(|node| node.children.iter().flatten().copied())
            .collect();
        for id in added {
            if held.contains(id)
                || self
                    .graph
                    .nodes
                    .get(id)
                    .is_none_or(graph_codec::is_composite)
            {
                continue;
            }
            let name = self.graph.variables.get(id);
            let parent = self
                .run_order
                .iter()
                .map(|task| run_node_prefix(*task))
                .find(|prefix| name.is_some_and(|name| name.starts_with(prefix.as_str())))
                .map(|prefix| graph_codec::composite_id(&prefix))
                .or(self.main);
            if let Some(parent) = parent
                .and_then(|parent| self.graph.nodes.get_mut(&parent))
                .filter(|parent| graph_codec::is_composite(parent))
            {
                parent.children.get_or_insert_with(Vec::new).push(*id);
            }
        }
    }

    /// Apply the halts requested since the last tick: write `Status::Failure`
    /// to each halted run's status key and prune its fragment. A halt naming
    /// an unknown or finished run was already served — nothing to do.
    fn process_halts(&mut self, store: &dyn DataStore) -> Result<(), BehaviorError> {
        for task in std::mem::take(&mut self.pending_halts) {
            let Some(run) = self.runs.remove(&task) else {
                continue;
            };
            let mut change = StateChange::new();
            change
                .set
                .insert(run.status_key.clone(), Some(task::failure()));
            store.write(change).map_err(|e| BehaviorError {
                message: e.to_string(),
            })?;
            self.prune_run(task)?;
        }
        Ok(())
    }

    /// Prune every run whose status key holds a terminal status. The latched
    /// task-run node would never fire again anyway; sweeping returns the graph
    /// to runs-that-are-live structure.
    fn sweep_terminal_runs(&mut self, store: &dyn DataStore) -> Result<(), BehaviorError> {
        let ended: Vec<TaskId> = self
            .runs
            .iter()
            .filter(|(_, run)| {
                store
                    .read(std::slice::from_ref(&run.status_key))
                    .into_iter()
                    .next()
                    .flatten()
                    .is_some_and(|status| task::is_terminal(&status))
            })
            .map(|(task, _)| *task)
            .collect();
        for task in ended {
            if self.runs.remove(&task).is_some() {
                self.prune_run(task)?;
            }
        }
        Ok(())
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
        // Halts requested since the last tick: write each halted run's
        // terminal status and prune its fragment — this phase owns the store.
        self.process_halts(store)?;

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

        // A run whose status just went terminal is done; its fragment leaves
        // the graph.
        self.sweep_terminal_runs(store)?;
        Ok(())
    }
}

/// A node of a run's own, outside any fragment: an id under the run's
/// [`run_node_prefix`] no fragment node id takes (those are under
/// `task/<run id>/`).
fn run_own_node(task: TaskId, name: &str) -> String {
    format!("{}:{name}", run_node_prefix(task))
}

/// An output writing `path` from a constant `value`: two nodes of run
/// `task`'s own, named after `name`.
fn constant_output(
    task: TaskId,
    name: &str,
    path: &str,
    value: Value,
) -> Result<(Vec<NodeSpec>, EdgeSpec), BehaviorError> {
    let path = TypedPath::parse(path).map_err(|e| BehaviorError {
        message: format!("'{path}' does not parse as a path: {e}"),
    })?;
    let constant = run_own_node(task, &format!("{name}:value"));
    let output = run_own_node(task, name);
    Ok((
        vec![
            NodeSpec {
                id: constant.clone(),
                kind: NodeType::Constant,
                params: NodeParams {
                    value: Some(value),
                    ..NodeParams::default()
                },
                output_shapes: Default::default(),
                input_defaults: Default::default(),
            },
            NodeSpec {
                id: output.clone(),
                kind: NodeType::Output,
                params: NodeParams {
                    path: Some(path),
                    ..NodeParams::default()
                },
                output_shapes: Default::default(),
                input_defaults: Default::default(),
            },
        ],
        EdgeSpec {
            from: EdgeOutputEndpoint {
                node_id: constant,
                output: "out".to_string(),
            },
            to: EdgeInputEndpoint {
                node_id: output,
                input: "in".to_string(),
            },
            selector: None,
        },
    ))
}

/// The generic run graft: an input on the run's update key (defaulting to
/// the spawn-time argument bundle) feeding a [`NodeType::TaskRun`] node that
/// calls the module function each tick, over an [`NodeType::Output`] on the
/// run's status key. Returns the diff and the run's update keys.
fn wrapper_graft(
    task: TaskId,
    prefix: &str,
    call: &Call,
) -> Result<(graph_codec::GraphSpecDiff, Vec<Key>), BehaviorError> {
    let status_path = TypedPath::parse(&format!("{prefix}/status")).map_err(|e| BehaviorError {
        message: format!("the status key does not parse as a path: {e}"),
    })?;
    let update_path = TypedPath::parse(&format!("{prefix}/update")).map_err(|e| BehaviorError {
        message: format!("the update key does not parse as a path: {e}"),
    })?;

    let run_node = format!("task/{}/run", task.0);
    let status_node = format!("task/{}/status", task.0);
    let args_node = format!("task/{}/args", task.0);
    // The spawn-time argument bundle: a structure of the call's args, the
    // default the args input serves until a live update lands on the run's
    // update key.
    let args_bundle = Value::Structure(Structure {
        id: call.id,
        fields: call.args.clone(),
    });
    let diff = graph_codec::GraphSpecDiff {
        upsert_nodes: vec![
            // The run's args source: an input on the update key, defaulting
            // to the spawn-time bundle. A caller (the ROS bridge on a live
            // goal) writes new args to the update key; the next tick stages
            // them here and the run picks them up — no graph edit per update.
            NodeSpec {
                id: args_node.clone(),
                kind: NodeType::Input,
                params: NodeParams {
                    path: Some(update_path),
                    value: Some(args_bundle),
                    ..NodeParams::default()
                },
                output_shapes: Default::default(),
                input_defaults: Default::default(),
            },
            NodeSpec {
                id: run_node.clone(),
                kind: NodeType::TaskRun,
                params: NodeParams {
                    module: call.module_id,
                    function: Some(call.id),
                    ..NodeParams::default()
                },
                output_shapes: Default::default(),
                input_defaults: Default::default(),
            },
            NodeSpec {
                id: status_node.clone(),
                kind: NodeType::Output,
                params: NodeParams {
                    path: Some(status_path),
                    ..NodeParams::default()
                },
                output_shapes: Default::default(),
                input_defaults: Default::default(),
            },
        ],
        upsert_edges: vec![
            EdgeSpec {
                from: EdgeOutputEndpoint {
                    node_id: args_node,
                    output: "out".to_string(),
                },
                to: EdgeInputEndpoint {
                    node_id: run_node.clone(),
                    input: "args".to_string(),
                },
                selector: None,
            },
            EdgeSpec {
                from: EdgeOutputEndpoint {
                    node_id: run_node,
                    output: "out".to_string(),
                },
                to: EdgeInputEndpoint {
                    node_id: status_node,
                    input: "in".to_string(),
                },
                selector: None,
            },
        ],
        ..graph_codec::GraphSpecDiff::default()
    };
    Ok((diff, vec![Key::from(format!("{prefix}/update"))]))
}

/// `spec`'s nodes and edges as run `task` holds them: node ids namespaced
/// under `task/<run id>/`, placeholder `task/…` paths rewritten to the run's
/// key prefix, each parameter input's default replaced by the spawn-time
/// argument it names (`arguments`, by placeholder name), and a `task/update`
/// input's default by the whole argument `bundle` when there is one (how a
/// fragment hosts the run's own module call).
pub(crate) fn graft_spec(
    spec: &GraphSpec,
    task: TaskId,
    prefix: &str,
    arguments: &HashMap<&str, &Value>,
    bundle: Option<&Value>,
) -> Result<(Vec<NodeSpec>, Vec<EdgeSpec>), BehaviorError> {
    let grafted_id = |id: &str| format!("task/{}/{}", task.0, id);
    let mut nodes = Vec::with_capacity(spec.nodes.len());
    for node in &spec.nodes {
        let mut node = node.clone();
        if let Some(path) = &node.params.path {
            let path = path.to_string();
            if let Some(placeholder) = path.strip_prefix("task/") {
                let run_path = format!("{prefix}/{placeholder}");
                node.params.path =
                    Some(TypedPath::parse(&run_path).map_err(|e| BehaviorError {
                        message: format!("'{run_path}' does not parse as a path: {e}"),
                    })?);
                if matches!(node.kind, NodeType::Input) {
                    if placeholder == "update" {
                        if let Some(bundle) = bundle {
                            node.params.value = Some(bundle.clone());
                        }
                    } else if let Some(value) = arguments.get(placeholder) {
                        node.params.value = Some((*value).clone());
                    }
                }
            }
        }
        node.id = grafted_id(&node.id);
        nodes.push(node);
    }
    let edges = spec
        .edges
        .iter()
        .map(|edge| {
            let mut edge = edge.clone();
            edge.from.node_id = grafted_id(&edge.from.node_id);
            edge.to.node_id = grafted_id(&edge.to.node_id);
            edge
        })
        .collect();
    Ok((nodes, edges))
}

/// Whether `spec` reports its own lifecycle: an output on `task/status`.
fn writes_task_status(spec: &GraphSpec) -> bool {
    spec.nodes.iter().any(|node| {
        matches!(node.kind, NodeType::Output)
            && node
                .params
                .path
                .as_ref()
                .is_some_and(|path| path.to_string() == "task/status")
    })
}

/// The skill graft: a copy of the registered [`TaskFragment`] ([`graft_spec`],
/// a `task/update` input defaulting to the whole argument bundle). Returns
/// the diff and the run's update keys: one per parameter, plus the bundle's.
fn fragment_graft(
    fragment: &TaskFragment,
    task: TaskId,
    prefix: &str,
    call: &Call,
) -> Result<(graph_codec::GraphSpecDiff, Vec<Key>), BehaviorError> {
    // The spawn-time arguments by the placeholder input name each feeds.
    let mut arguments: HashMap<&str, &Value> = HashMap::new();
    for argument in &call.args {
        if let Some(name) = fragment.parameters.get(&argument.id) {
            arguments.insert(name.as_str(), argument.value.as_ref());
        }
    }
    let args_bundle = Value::Structure(Structure {
        id: call.id,
        fields: call.args.clone(),
    });
    let (upsert_nodes, upsert_edges) =
        graft_spec(&fragment.spec, task, prefix, &arguments, Some(&args_bundle))?;

    let mut names: Vec<&String> = fragment.parameters.values().collect();
    names.sort();
    let update = names
        .into_iter()
        .map(|name| Key::from(format!("{prefix}/{name}")))
        .chain(std::iter::once(Key::from(format!("{prefix}/update"))))
        .collect();
    Ok((
        graph_codec::GraphSpecDiff {
            upsert_nodes,
            upsert_edges,
            ..graph_codec::GraphSpecDiff::default()
        },
        update,
    ))
}

/// The [`run_behavior`](run::RunBehavior::run_behavior) graft: the graph the
/// call carries ([`graft_spec`]), plus the run's own two outputs — its name
/// on the run's `name` key, and its status, `Running` until it is halted,
/// unless the graph writes `task/status` itself. No update keys.
fn behavior_graft(
    task: TaskId,
    prefix: &str,
    call: &Call,
) -> Result<graph_codec::GraphSpecDiff, BehaviorError> {
    let (name, spec) = run::arguments(call).map_err(|message| BehaviorError { message })?;
    let (mut upsert_nodes, mut upsert_edges) =
        graft_spec(&spec, task, prefix, &HashMap::new(), None)?;
    let (nodes, edge) =
        constant_output(task, "name", &format!("{prefix}/name"), Value::String(name))?;
    upsert_nodes.extend(nodes);
    upsert_edges.push(edge);
    if !writes_task_status(&spec) {
        let (nodes, edge) =
            constant_output(task, "status", &format!("{prefix}/status"), task::running())?;
        upsert_nodes.extend(nodes);
        upsert_edges.push(edge);
    }
    Ok(graph_codec::GraphSpecDiff {
        upsert_nodes,
        upsert_edges,
        ..graph_codec::GraphSpecDiff::default()
    })
}

impl BehaviorInterpreter for ProcessingGraph {
    /// [`run_behavior`](run::RunBehavior::run_behavior), and the functions
    /// the registered fragments implement, where a fragment is
    /// [`described`](TaskFragment::described).
    fn described_methods(&self) -> HashMap<Uuid, frozen::Export> {
        self.fragments
            .iter()
            .filter_map(|(function, fragment)| Some((*function, fragment.description.clone()?)))
            .chain(std::iter::once((run::FUNCTION, run::description())))
            .collect()
    }

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
    /// continuous, so a recompose does not restart every stateful node. The
    /// store and the `function -> module` map are untouched — the store
    /// belongs to the device, and the loaded-module set is fixed at device
    /// build.
    ///
    /// The live task runs are not the main behavior, so a load leaves them
    /// running: their nodes carry over into the new graph with the links
    /// between them and the names that decode them, and keep their state.
    /// A link from a run node to a node the new graph lacks is dropped.
    fn load(&mut self, mut graph: Graph) -> Result<(), BehaviorError> {
        let carried: HashSet<Uuid> = self
            .runs
            .keys()
            .flat_map(|task| self.run_nodes(*task))
            .filter(|id| !graph.nodes.contains_key(id))
            .collect();
        let within = |source: &LinkSource| {
            let mut source = source;
            loop {
                match source {
                    LinkSource::Literal(_) => return true,
                    LinkSource::Port(port) => return carried.contains(&port.node),
                    LinkSource::Select { source: inner, .. } => source = inner,
                    // A Vizij graph has no variable links (`graph_codec` decodes
                    // none), so none is carried.
                    LinkSource::Variable(_) => return false,
                }
            }
        };
        let links: Vec<_> = self
            .graph
            .links
            .iter()
            .filter(|link| carried.contains(&link.target.node) && within(&link.source))
            .cloned()
            .collect();
        // The names the carried nodes and links decode by: node ids, kinds,
        // slots.
        let mut named: HashSet<Uuid> = HashSet::new();
        for id in &carried {
            let node = &self.graph.nodes[id];
            named.insert(node.id);
            named.insert(node.function);
            named.extend(node.inputs.iter().chain(&node.outputs).map(|io| io.id));
        }
        for link in &links {
            named.insert(link.target.port);
        }
        for id in named {
            if let Some(name) = self.graph.variables.get(&id) {
                graph.variables.entry(id).or_insert_with(|| name.clone());
            }
        }
        for id in &carried {
            graph.nodes.insert(*id, self.graph.nodes[id].clone());
        }
        graph.links.extend(links);
        let runner = graph_codec::composite_id(RUNNER);
        self.main = match graph.root {
            // A graph read back from this interpreter: its main behavior is
            // its runner's first child.
            Some(root) if root == runner => graph
                .nodes
                .get(&root)
                .and_then(|node| node.children.as_ref()?.first().copied()),
            Some(root) => Some(root),
            // A graph with no structure: its nodes, by id, are the main
            // behavior.
            None => {
                let mut children: Vec<Uuid> = graph
                    .nodes
                    .keys()
                    .filter(|id| !carried.contains(id))
                    .copied()
                    .collect();
                children.sort_by_key(|id| graph.variables.get(id).cloned());
                Some(graph_codec::insert_composite(
                    &mut graph,
                    graph_codec::flow_function(),
                    "main",
                    children,
                ))
            }
        };
        self.graph = graph;
        self.install_runner();
        self.dirty = true;
        Ok(())
    }

    /// Edit the running Vizij graph — the interpreter module's EDIT entry point,
    /// reached through the engine like LOAD. Applies the [`GraphDiff`] to the
    /// retained graph (add/remove nodes and links) and re-lowers at the next
    /// tick. Unedited nodes keep their id, so their runtime state survives the
    /// edit — an add/remove of one node does not restart the rest. The store and
    /// the `function -> module` map are untouched.
    ///
    /// An added node keeps the place its parent's `children` give it; one no
    /// composite holds is [`place`](Self::place)d. A new root is the main
    /// behavior's: the runner stays the root, with the runs after it.
    fn apply(&mut self, diff: GraphDiff) -> Result<(), BehaviorError> {
        let added: Vec<Uuid> = diff.add_nodes.iter().map(|node| node.id).collect();
        let root = diff.set_root;
        self.graph.apply(diff).map_err(|e| BehaviorError {
            message: format!("graph diff: {e}"),
        })?;
        if let Some(root) = root.filter(|root| *root != graph_codec::composite_id(RUNNER)) {
            self.main = Some(root);
        }
        self.place(&added);
        self.install_runner();
        self.dirty = true;
        Ok(())
    }

    /// Spawn `call` as a concurrent task run — the interpreter module's SPAWN
    /// entry point. The run grafts into the running graph: for
    /// [`run_behavior`](run::RunBehavior::run_behavior), the graph the call
    /// carries, with the run's name and status outputs; a registered
    /// [`TaskFragment`] verbatim (its placeholder paths rewritten to the
    /// run's key prefix, its parameter inputs defaulted from the spawn-time
    /// arguments); or the generic two-node wrapper — a [`NodeType::TaskRun`]
    /// node carrying the whole call over an [`NodeType::Output`] on the run's
    /// status key. The graph's ordinary evaluation then advances the run once
    /// per tick and the Output convention publishes its `Status` — the run is
    /// structure, introspectable through [`graph`](Self::graph) like
    /// everything else, and pruned when it ends.
    fn spawn(&mut self, call: Call, policy: RunPolicy) -> Result<TaskHandle, BehaviorError> {
        // v1 runs every task concurrently — the graph's ordinary semantics
        // (overlapping actuation writes are last-write-wins). Richer
        // `RunPolicy` arbitration lands as visible graph structure later; the
        // policy is accepted and treated as `Concurrent` until then.
        let _ = policy;
        let task = TaskId(Uuid::new_v4());
        if call.id == run::FUNCTION {
            let diff = behavior_graft(task, &run::prefix(task), &call)?;
            let handle = run::handle(task);
            self.graft(task, call.id, handle.status.clone(), diff)?;
            return Ok(handle);
        }
        let module = call
            .module_id
            .map(|m| m.to_string())
            .unwrap_or_else(|| "none".to_string());
        let prefix = format!("arora/tasks/{module}/{}/{}", call.id, task.0);
        let status_key = Key::from(format!("{prefix}/status"));

        let (diff, update) = match self.fragments.get(&call.id) {
            Some(fragment) => {
                // An exclusive function's live runs yield to the new one:
                // halted on the next tick, which prunes them with their
                // terminal status written — the run that takes over is then
                // the only writer.
                if fragment.exclusive {
                    let live: Vec<TaskId> = self
                        .runs
                        .iter()
                        .filter(|(_, run)| run.function == call.id)
                        .map(|(id, _)| *id)
                        .collect();
                    self.pending_halts.extend(live);
                }
                fragment_graft(fragment, task, &prefix, &call)?
            }
            None => wrapper_graft(task, &prefix, &call)?,
        };
        self.graft(task, call.id, status_key.clone(), diff)?;

        Ok(TaskHandle {
            id: task,
            stop: interpreter_module::encode_halt(task),
            status: status_key,
            feedback: vec![Key::from(format!("{prefix}/feedback"))],
            result: vec![Key::from(format!("{prefix}/result"))],
            update,
        })
    }

    /// Halt a run — the interpreter module's HALT entry point. Applied on the
    /// next tick, which owns the store: the run's terminal status is written
    /// and its fragment pruned. Idempotent — halting an unknown or finished
    /// run is a clean no-op.
    fn halt(&mut self, task: TaskId) -> Result<(), BehaviorError> {
        self.pending_halts.push(task);
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

    /// The Vizij nodes of the interpreter's graph, its composites aside.
    fn vizij_nodes(graph: &ProcessingGraph) -> usize {
        graph
            .graph()
            .nodes
            .values()
            .filter(|node| !graph_codec::is_composite(node))
            .count()
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

    /// A bridge scripting the spawned function: serves each call from a queue
    /// of return values (an empty queue keeps serving `Running`) and records
    /// every call.
    struct RunBridge {
        responses: std::collections::VecDeque<Value>,
        calls: Vec<Call>,
    }

    impl RunBridge {
        fn new(responses: Vec<Value>) -> Self {
            Self {
                responses: responses.into(),
                calls: Vec::new(),
            }
        }
    }

    impl CallBridge for RunBridge {
        fn arora_call(&mut self, call: Call) -> Result<CallResult, CallError> {
            self.calls.push(call);
            let ret = self.responses.pop_front().unwrap_or_else(task::running);
            Ok(CallResult {
                ret,
                mutated: Vec::new(),
            })
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

    fn look_at() -> Call {
        Call {
            module_id: Some(Uuid::from_u128(0x6761)),
            id: Uuid::from_u128(0x6c61),
            args: vec![StructureField {
                id: Uuid::from_u128(0x7861),
                value: Box::new(float(0.5)),
            }],
        }
    }

    fn read_key(store: &SimpleDataStore, key: &Key) -> Option<Value> {
        store
            .read(std::slice::from_ref(key))
            .into_iter()
            .next()
            .flatten()
    }

    /// A fragment that keeps running and writes `value` to `shared/x`.
    fn writer_fragment(value: f32) -> TaskFragment {
        let spec = json!({
            "nodes": [
                { "id": "v", "type": "constant", "params": { "value": float(value) } },
                { "id": "out", "type": "output", "params": { "path": "shared/x" } },
                { "id": "s", "type": "constant", "params": { "value": task::running() } },
                { "id": "status", "type": "output", "params": { "path": "task/status" } },
            ],
            "edges": [
                { "from": { "node_id": "v" }, "to": { "node_id": "out", "input": "in" } },
                { "from": { "node_id": "s" }, "to": { "node_id": "status", "input": "in" } },
            ],
        });
        TaskFragment::parse(&spec.to_string(), HashMap::new()).expect("the fragment parses")
    }

    const EARLY: Uuid = Uuid::from_u128(0xea71);
    const LATE: Uuid = Uuid::from_u128(0x1a7e);

    /// The main behavior writes 1.0 to `shared/x`; the `EARLY` and `LATE`
    /// runs write 2.0 and 3.0 to it.
    fn shared_writers() -> ProcessingGraph {
        let behavior = parse_spec(
            &json!({
                "nodes": [
                    { "id": "v", "type": "constant", "params": { "value": float(1.0) } },
                    { "id": "out", "type": "output", "params": { "path": "shared/x" } },
                ],
                "edges": [
                    { "from": { "node_id": "v" }, "to": { "node_id": "out", "input": "in" } },
                ],
            })
            .to_string(),
        )
        .expect("the behavior parses");
        let mut graph = ProcessingGraph::from_spec(behavior).expect("from_spec");
        graph.set_task_fragment(EARLY, writer_fragment(2.0));
        graph.set_task_fragment(LATE, writer_fragment(3.0));
        graph
    }

    fn run_of(function: Uuid) -> Call {
        Call {
            module_id: None,
            id: function,
            args: Vec::new(),
        }
    }

    /// Where a run and the main behavior write one path, the run wins, and a
    /// later run wins over an earlier one — whether the runs were lowered in
    /// place or the whole graph was lowered again.
    #[test]
    fn a_later_run_writes_after_the_behavior_and_earlier_runs() {
        let store = SimpleDataStore::new();
        let mut bridge = RunBridge::new(Vec::new());
        let mut graph = shared_writers();
        let tick = |graph: &mut ProcessingGraph, bridge: &mut RunBridge| {
            graph.tick_store(&store, bridge, 0.016).expect("tick");
            read(&store, "shared/x")
        };

        assert_eq!(tick(&mut graph, &mut bridge), Some(float(1.0)));
        let early = graph.spawn(run_of(EARLY), RunPolicy::Concurrent).unwrap();
        assert_eq!(tick(&mut graph, &mut bridge), Some(float(2.0)));
        graph.spawn(run_of(LATE), RunPolicy::Concurrent).unwrap();
        assert_eq!(tick(&mut graph, &mut bridge), Some(float(3.0)));

        // A full lowering keeps the precedence.
        graph.dirty = true;
        assert_eq!(tick(&mut graph, &mut bridge), Some(float(3.0)));
        // With the early run gone, the late one still writes last.
        graph.halt(early.id).unwrap();
        assert_eq!(tick(&mut graph, &mut bridge), Some(float(3.0)));
    }

    /// The order a plan follows and what it binds, in comparable form.
    fn lowered(graph: &ProcessingGraph) -> Vec<String> {
        let mut out: Vec<String> = graph
            .spec
            .nodes
            .iter()
            .map(|n| format!("node {}", n.id))
            .collect();
        let order: Vec<&str> = graph
            .rt
            .plan
            .order
            .iter()
            .map(|&idx| graph.spec.nodes[idx].id.as_str())
            .collect();
        out.push(format!("order {order:?}"));
        let mut edges: Vec<String> = graph
            .spec
            .edges
            .iter()
            .map(|e| {
                format!(
                    "edge {}.{} -> {}.{}",
                    e.from.node_id, e.from.output, e.to.node_id, e.to.input
                )
            })
            .collect();
        edges.sort();
        out.extend(edges);
        let mut inputs: Vec<String> = graph.inputs.iter().map(|p| format!("input {p}")).collect();
        inputs.sort();
        out.extend(inputs);
        out
    }

    /// Lowering runs in place as they come and go gives exactly what
    /// lowering the whole graph again gives.
    #[test]
    fn lowering_runs_in_place_matches_a_full_lowering() {
        let store = SimpleDataStore::new();
        let mut bridge = RunBridge::new(Vec::new());
        let mut graph = shared_writers();
        graph.tick_store(&store, &mut bridge, 0.016).unwrap();

        let first = graph.spawn(run_of(EARLY), RunPolicy::Concurrent).unwrap();
        graph.spawn(run_of(LATE), RunPolicy::Concurrent).unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).unwrap();
        graph.halt(first.id).unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).unwrap();
        graph.spawn(run_of(EARLY), RunPolicy::Concurrent).unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).unwrap();
        // The runs were lowered in place: the fingerprint is the in-place
        // path's mark, a full lowering recomputing it.
        assert_eq!(graph.spec.fingerprint, 0);
        let in_place = lowered(&graph);

        graph.dirty = true;
        graph.lower().unwrap();
        graph.rt.plan.ensure_versioned(&graph.spec).unwrap();
        assert_eq!(lowered(&graph), in_place);
    }

    /// A behavior writing 4.0 then 5.0 to `shared/x`, listed in that order
    /// under ids that sort the other way.
    fn later_listed_writer() -> GraphSpec {
        parse_spec(
            &json!({
                "nodes": [
                    { "id": "z_first", "type": "constant", "params": { "value": float(4.0) } },
                    { "id": "z_first_out", "type": "output", "params": { "path": "shared/x" } },
                    { "id": "a_second", "type": "constant", "params": { "value": float(5.0) } },
                    { "id": "a_second_out", "type": "output", "params": { "path": "shared/x" } },
                ],
                "edges": [
                    { "from": { "node_id": "z_first" }, "to": { "node_id": "z_first_out", "input": "in" } },
                    { "from": { "node_id": "a_second" }, "to": { "node_id": "a_second_out", "input": "in" } },
                ],
            })
            .to_string(),
        )
        .expect("the behavior parses")
    }

    /// Of two writers of one path, the later-listed wins, whatever their ids:
    /// the order is the graph's, carried through its encoding and a LOAD.
    #[test]
    fn the_later_listed_writer_of_a_path_wins() {
        let store = SimpleDataStore::new();
        let mut graph = ProcessingGraph::from_spec(later_listed_writer()).expect("from_spec");
        graph.tick_store(&store, &mut NoopBridge, 0.016).unwrap();
        assert_eq!(read(&store, "shared/x"), Some(float(5.0)));

        let mut other = shared_writers();
        other
            .load(graph_codec::encode(&later_listed_writer()).expect("encode"))
            .expect("load");
        other.tick_store(&store, &mut NoopBridge, 0.016).unwrap();
        assert_eq!(read(&store, "shared/x"), Some(float(5.0)));
    }

    /// A node an edit adds to the main behavior runs before the runs, so a
    /// live run still writes after it.
    #[test]
    fn an_edit_to_the_behavior_still_runs_before_the_runs() {
        let store = SimpleDataStore::new();
        let mut bridge = RunBridge::new(Vec::new());
        let mut graph = shared_writers();
        graph.spawn(run_of(EARLY), RunPolicy::Concurrent).unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).unwrap();
        assert_eq!(read(&store, "shared/x"), Some(float(2.0)));

        let added = later_listed_writer();
        let diff = graph_codec::GraphSpecDiff {
            upsert_nodes: added.nodes,
            upsert_edges: added.edges,
            ..graph_codec::GraphSpecDiff::default()
        };
        graph
            .apply(graph_codec::spec_diff_to_graph_diff(&diff).expect("translate"))
            .expect("apply");
        graph.tick_store(&store, &mut bridge, 0.016).unwrap();
        assert_eq!(read(&store, "shared/x"), Some(float(2.0)));
    }

    /// A LOAD replaces the main behavior under the live runs, which still
    /// write after it — a graph with no structure included, its nodes then
    /// the main behavior by id.
    #[test]
    fn a_loaded_behavior_runs_before_the_live_runs() {
        let store = SimpleDataStore::new();
        let mut bridge = RunBridge::new(Vec::new());
        let mut graph = shared_writers();
        graph.spawn(run_of(EARLY), RunPolicy::Concurrent).unwrap();

        graph
            .load(graph_codec::encode(&later_listed_writer()).expect("encode"))
            .expect("load");
        graph.tick_store(&store, &mut bridge, 0.016).unwrap();
        assert_eq!(read(&store, "shared/x"), Some(float(2.0)));

        let mut bare = graph_codec::encode(&later_listed_writer()).expect("encode");
        let root = bare.root.take().expect("a root");
        bare.nodes.remove(&root);
        graph.load(bare).expect("load");
        graph.tick_store(&store, &mut bridge, 0.016).unwrap();
        assert_eq!(read(&store, "shared/x"), Some(float(2.0)));

        // The interpreter's own graph loads back as it was.
        let own = graph.graph().clone();
        graph.load(own).expect("load");
        graph.tick_store(&store, &mut bridge, 0.016).unwrap();
        assert_eq!(read(&store, "shared/x"), Some(float(2.0)));
    }

    /// SPAWN grafts a run as graph structure: the fragment is visible through
    /// `graph()` before it ever ticks, ordinary evaluation advances it once per
    /// tick, and its `Status` lands on the handle's status key.
    #[test]
    fn spawn_grafts_a_run_that_reports_on_its_status_key() {
        let store = SimpleDataStore::new();
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");
        store
            .write(StateChange::set("sensor/x", float(0.75)))
            .unwrap();
        let nodes_before = vizij_nodes(&graph);

        let handle = graph
            .spawn(look_at(), RunPolicy::Concurrent)
            .expect("spawn");
        // The fragment is three nodes: args input, taskrun leaf, status output.
        assert_eq!(vizij_nodes(&graph), nodes_before + 3);
        assert!(handle.status.path.starts_with("arora/tasks/"));

        let mut bridge = RunBridge::new(Vec::new());
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(read_key(&store, &handle.status), Some(task::running()));

        // The spawned call reached its module intact.
        assert_eq!(bridge.calls.len(), 1);
        assert_eq!(bridge.calls[0].module_id, look_at().module_id);
        assert_eq!(bridge.calls[0].id, look_at().id);
        assert_eq!(bridge.calls[0].args, look_at().args);

        // A run outlives the tick that started it.
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(bridge.calls.len(), 2);
    }

    /// A run returning a terminal status is swept out of the graph and its
    /// function is never invoked again; the status key keeps the terminal
    /// value.
    #[test]
    fn a_terminal_run_is_swept_and_never_invoked_again() {
        let store = SimpleDataStore::new();
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");
        store
            .write(StateChange::set("sensor/x", float(0.75)))
            .unwrap();
        let nodes_before = vizij_nodes(&graph);

        let handle = graph
            .spawn(look_at(), RunPolicy::Concurrent)
            .expect("spawn");
        let mut bridge = RunBridge::new(vec![task::success()]);
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");

        assert_eq!(read_key(&store, &handle.status), Some(task::success()));
        assert_eq!(vizij_nodes(&graph), nodes_before);

        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(bridge.calls.len(), 1);
        assert_eq!(read_key(&store, &handle.status), Some(task::success()));
    }

    /// HALT writes `Failure` and prunes on the next tick — which owns the
    /// store — and is idempotent at every stage.
    #[test]
    fn halt_fails_the_run_and_prunes_its_fragment() {
        let store = SimpleDataStore::new();
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");
        store
            .write(StateChange::set("sensor/x", float(0.75)))
            .unwrap();
        let nodes_before = vizij_nodes(&graph);

        let handle = graph
            .spawn(look_at(), RunPolicy::Concurrent)
            .expect("spawn");
        let mut bridge = RunBridge::new(Vec::new());
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(read_key(&store, &handle.status), Some(task::running()));

        graph.halt(handle.id).expect("halt");
        graph.halt(handle.id).expect("halting twice is a no-op");
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");

        assert_eq!(read_key(&store, &handle.status), Some(task::failure()));
        assert_eq!(vizij_nodes(&graph), nodes_before);
        assert_eq!(bridge.calls.len(), 1);

        graph
            .halt(handle.id)
            .expect("halting a finished run is a no-op");
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(read_key(&store, &handle.status), Some(task::failure()));
    }

    /// Runs coexist: the main graph keeps flowing and concurrent runs demux on
    /// their own status keys.
    #[test]
    fn runs_coexist_with_the_main_graph_and_each_other() {
        let store = SimpleDataStore::new();
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");
        store
            .write(StateChange::set("sensor/x", float(0.75)))
            .unwrap();

        let first = graph
            .spawn(look_at(), RunPolicy::Concurrent)
            .expect("spawn");
        let second = graph
            .spawn(look_at(), RunPolicy::Concurrent)
            .expect("spawn");
        assert_ne!(first.status.path, second.status.path);

        let mut bridge = RunBridge::new(Vec::new());
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");

        assert_eq!(read(&store, "actuator/y"), Some(float(0.75)));
        assert_eq!(read_key(&store, &first.status), Some(task::running()));
        assert_eq!(read_key(&store, &second.status), Some(task::running()));
        assert_eq!(bridge.calls.len(), 2);
    }

    /// A live goal update reaches the running call: writing new args to the
    /// run's update key makes the next tick invoke the call with them, with no
    /// graph edit per update — the run's args ride an `input` on the update key
    /// (ARORA-84).
    #[test]
    fn a_live_update_reaches_the_running_call() {
        let store = SimpleDataStore::new();
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");
        store
            .write(StateChange::set("sensor/x", float(0.75)))
            .unwrap();
        let arg_id = Uuid::from_u128(0x7861);

        let handle = graph
            .spawn(look_at(), RunPolicy::Concurrent)
            .expect("spawn");
        let mut bridge = RunBridge::new(Vec::new());

        // First tick: the run invokes with the spawn-time args (0.5).
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(
            bridge.calls.last().expect("a call").args[0].value.as_ref(),
            &float(0.5),
        );

        // The caller pushes a moving target: new args on the update key.
        let update = Value::Structure(Structure {
            id: look_at().id,
            fields: vec![StructureField {
                id: arg_id,
                value: Box::new(float(0.9)),
            }],
        });
        let mut change = StateChange::new();
        change.set.insert(handle.update[0].clone(), Some(update));
        store.write(change).unwrap();

        // Next tick: the run invokes with the updated args (0.9) — no re-spawn.
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(
            bridge.calls.last().expect("a call").args[0].value.as_ref(),
            &float(0.9),
        );
    }

    /// The skill path: a registered fragment — the real look_at asset from
    /// `vizij-arora-host` — implements the spawned run as graph content, no
    /// module call anywhere. Tracking writes the gaze surface, steers on a
    /// live parameter update, and runs until halted; a glance settles to
    /// Success; an unsupported policy fails with the `std_skills` ENOTSUP
    /// errno on the result key.
    #[test]
    fn a_registered_fragment_implements_the_spawned_run() {
        use crate::gaze::{self, look_at::ids::look_at as ids};

        let store = SimpleDataStore::new();
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");
        graph.set_task_fragment(ids::FUNCTION, gaze::look_at_fragment());
        let mut bridge = NoopBridge;
        store
            .write(StateChange::set("sensor/x", float(0.75)))
            .unwrap();

        let look_at = |policy: &str, target: [f32; 3], frame: &str| Call {
            module_id: Some(arora_behavior::interpreter_module::ID),
            id: ids::FUNCTION,
            args: vec![
                StructureField {
                    id: ids::POLICY,
                    value: Box::new(Value::String(policy.to_string())),
                },
                StructureField {
                    id: ids::TARGET,
                    value: Box::new(Value::ArrayF32(target.to_vec())),
                },
                StructureField {
                    id: ids::FRAME,
                    value: Box::new(Value::String(frame.to_string())),
                },
            ],
        };

        // Tracking: the goal lands on the gaze surface and the run stays
        // Running — the halt is the exit. The handle's update keys are per
        // parameter.
        let handle = graph
            .spawn(
                look_at("", [1.0, 2.0, 3.0], "sellion_link"),
                RunPolicy::Concurrent,
            )
            .expect("spawn");
        let target_key = handle
            .update
            .iter()
            .find(|key| key.path.ends_with("/target"))
            .expect("a target update key")
            .clone();
        graph.tick_store(&store, &mut bridge, 0.05).expect("tick");
        assert_eq!(
            read(&store, "standard/ros4hri/gaze/target"),
            Some(Value::ArrayF32(vec![1.0, 2.0, 3.0])),
        );
        assert_eq!(
            read(&store, "standard/ros4hri/gaze/frame"),
            Some(Value::String("sellion_link".to_string())),
        );
        assert_eq!(read_key(&store, &handle.status), Some(task::running()));

        // A live update on the parameter key steers the running fragment.
        let mut change = StateChange::new();
        change
            .set
            .insert(target_key, Some(Value::ArrayF32(vec![4.0, 5.0, 6.0])));
        store.write(change).unwrap();
        graph.tick_store(&store, &mut bridge, 0.05).expect("tick");
        assert_eq!(
            read(&store, "standard/ros4hri/gaze/target"),
            Some(Value::ArrayF32(vec![4.0, 5.0, 6.0])),
        );

        // The halt ends the run as Failure (the cancel semantics) and prunes
        // the whole grafted fragment.
        graph.halt(handle.id).expect("halt");
        graph.tick_store(&store, &mut bridge, 0.05).expect("tick");
        assert_eq!(read_key(&store, &handle.status), Some(task::failure()));

        // A glance holds its fixation, then succeeds on its own.
        let handle = graph
            .spawn(
                look_at("glance", [0.5, 0.0, 0.5], ""),
                RunPolicy::Concurrent,
            )
            .expect("spawn");
        for _ in 0..6 {
            graph.tick_store(&store, &mut bridge, 0.2).expect("tick");
        }
        assert_eq!(read_key(&store, &handle.status), Some(task::success()));

        // An unsupported policy fails with the ENOTSUP errno on the result
        // key — the value the ROS action plane answers verbatim.
        let handle = graph
            .spawn(
                look_at("social", [0.0, 0.0, 0.0], ""),
                RunPolicy::Concurrent,
            )
            .expect("spawn");
        graph.tick_store(&store, &mut bridge, 0.05).expect("tick");
        assert_eq!(read_key(&store, &handle.status), Some(task::failure()));
        assert_eq!(read_key(&store, &handle.result[0]), Some(Value::U8(134)));
    }

    /// The continuous gaze policies: `idle` and `random` ignore the goal
    /// target, wander the gaze around a point 10 m straight ahead in the
    /// face's own frame, and run until halted.
    #[test]
    fn idle_and_random_gaze_wander_until_halted() {
        use crate::gaze::{self, look_at::ids::look_at as ids};

        let store = SimpleDataStore::new();
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");
        graph.set_task_fragment(ids::FUNCTION, gaze::look_at_fragment());
        let mut bridge = NoopBridge;
        store
            .write(StateChange::set("sensor/x", float(0.75)))
            .unwrap();

        for policy in ["idle", "random"] {
            let handle = graph
                .spawn(
                    Call {
                        module_id: Some(arora_behavior::interpreter_module::ID),
                        id: ids::FUNCTION,
                        args: vec![
                            StructureField {
                                id: ids::POLICY,
                                value: Box::new(Value::String(policy.to_string())),
                            },
                            StructureField {
                                id: ids::TARGET,
                                value: Box::new(Value::ArrayF32(vec![1.0, 2.0, 3.0])),
                            },
                            StructureField {
                                id: ids::FRAME,
                                value: Box::new(Value::String("sellion_link".to_string())),
                            },
                        ],
                    },
                    RunPolicy::Concurrent,
                )
                .expect("spawn");
            let mut targets = Vec::new();
            for _ in 0..20 {
                graph.tick_store(&store, &mut bridge, 0.25).expect("tick");
                assert_eq!(
                    read_key(&store, &handle.status),
                    Some(task::running()),
                    "{policy} keeps running",
                );
                match read(&store, "standard/ros4hri/gaze/target") {
                    Some(Value::ArrayF32(target)) => targets.push(target),
                    other => panic!("{policy}: gaze target {other:?}"),
                }
            }
            assert_eq!(
                read(&store, "standard/ros4hri/gaze/frame"),
                Some(Value::String(String::new())),
                "{policy} gazes in the face's frame",
            );
            for target in &targets {
                let [x, y, z] = target.as_slice() else {
                    panic!("{policy}: gaze target {target:?}");
                };
                assert_eq!(*x, 10.0, "{policy} looks 10 m ahead");
                assert!(y.abs() <= 2.5 && z.abs() <= 1.5, "{policy}: {target:?}");
            }
            assert!(
                targets.windows(2).any(|pair| pair[0] != pair[1]),
                "{policy} moves the gaze: {targets:?}",
            );

            graph.halt(handle.id).expect("halt");
            graph.tick_store(&store, &mut bridge, 0.05).expect("tick");
            assert_eq!(read_key(&store, &handle.status), Some(task::failure()));
        }
    }

    /// A behavior that damps the `in/target` input (from 0) onto `out`: its
    /// damp node's state is what a run carries across a LOAD or an EDIT.
    fn damped(out: &str) -> GraphSpec {
        parse_spec(
            &json!({
                "nodes": [
                    { "id": "target", "type": "input", "params": { "path": "in/target", "value": 0.0 } },
                    { "id": "damp", "type": "damp", "params": { "half_life": 0.5 } },
                    { "id": "out", "type": "output", "params": { "path": out } }
                ],
                "edges": [
                    { "from": { "node_id": "target" }, "to": { "node_id": "damp", "input": "in" } },
                    { "from": { "node_id": "damp" }, "to": { "node_id": "out", "input": "in" } }
                ]
            })
            .to_string(),
        )
        .expect("the damped graph parses")
    }

    fn f32_at(store: &SimpleDataStore, path: &str) -> f32 {
        match read(store, path) {
            Some(Value::F32(value)) => value,
            other => panic!("{path} = {other:?}"),
        }
    }

    /// Spawn `behavior` as a run_behavior run under `name`.
    fn spawn_behavior(graph: &mut ProcessingGraph, name: &str, behavior: &GraphSpec) -> TaskHandle {
        let call = run::call(name, behavior).expect("the behavior encodes");
        graph
            .spawn(call, RunPolicy::Concurrent)
            .expect("run_behavior spawns")
    }

    /// A damped run, started at rest and moving toward `in/target = 1`
    /// for `ticks` ticks of 0.1 s. Returns the main graph's node count, the
    /// handle and the value it last wrote to `out`.
    fn moving_run(
        graph: &mut ProcessingGraph,
        store: &SimpleDataStore,
        ticks: usize,
    ) -> (usize, TaskHandle, f32) {
        store
            .write(StateChange::set("sensor/x", float(0.75)))
            .unwrap();
        let nodes_before = vizij_nodes(graph);
        let handle = spawn_behavior(graph, "damped", &damped("out"));
        let mut bridge = NoopBridge;
        graph.tick_store(store, &mut bridge, 0.1).expect("tick");
        assert_eq!(f32_at(store, "out"), 0.0, "the run starts at rest");
        store
            .write(StateChange::set("in/target", float(1.0)))
            .unwrap();
        for _ in 0..ticks {
            graph.tick_store(store, &mut bridge, 0.1).expect("tick");
        }
        let moving = f32_at(store, "out");
        assert!(0.1 < moving && moving < 0.9, "on its way: {moving}");
        (nodes_before, handle, moving)
    }

    /// The interpreter describes run_behavior itself, so a device lists it
    /// under the interpreter module with its contract's signature.
    #[test]
    fn the_interpreter_describes_run_behavior() {
        let graph = ProcessingGraph::from_spec(passthrough("a", "b")).expect("from_spec");
        let described = graph.described_methods();
        assert_eq!(described.get(&run::FUNCTION), Some(&run::description()));
        assert_eq!(run::description().name, "run_behavior");
    }

    /// run_behavior runs the graph its call carries beside the main graph:
    /// the run writes its outputs, `Running` on its status key and its name
    /// on its name key. Halted, it fails and its nodes leave the graph,
    /// while the store keeps what it wrote.
    #[test]
    fn run_behavior_runs_the_graph_it_carries_until_halted() {
        let store = SimpleDataStore::new();
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");
        store
            .write(StateChange::set("sensor/x", float(0.75)))
            .unwrap();
        let nodes_before = vizij_nodes(&graph);

        let handle = spawn_behavior(
            &mut graph,
            "constant",
            &passthrough("program/in", "program/out"),
        );
        assert_eq!(handle, run::handle(handle.id));
        let mut bridge = NoopBridge;
        store
            .write(StateChange::set("program/in", float(0.5)))
            .unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(read(&store, "program/out"), Some(float(0.5)));
        assert_eq!(read(&store, "actuator/y"), Some(float(0.75)));
        assert_eq!(read_key(&store, &handle.status), Some(task::running()));
        assert_eq!(
            read_key(&store, &run::name_key(handle.id)),
            Some(Value::String("constant".to_string()))
        );
        // The graph and the run's own name and status outputs.
        assert_eq!(vizij_nodes(&graph), nodes_before + 2 + 4);

        // Any client finds the run by its name.
        let runs = run::runs(&store);
        assert_eq!(runs, vec![(handle.clone(), "constant".to_string())]);

        graph.halt(handle.id).expect("halt");
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(read_key(&store, &handle.status), Some(task::failure()));
        assert_eq!(vizij_nodes(&graph), nodes_before);
        store
            .write(StateChange::set("program/in", float(0.25)))
            .unwrap();
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(
            read(&store, "program/out"),
            Some(float(0.5)),
            "a halted run's outputs hold"
        );
    }

    /// A behavior that writes `task/status` decides when its run ends: no
    /// status of the run's own competes with it.
    #[test]
    fn a_behavior_writing_its_status_ends_its_run() {
        let store = SimpleDataStore::new();
        let mut graph = ProcessingGraph::from_spec(passthrough("a", "b")).expect("from_spec");
        let behavior = parse_spec(
            &json!({
                "nodes": [
                    { "id": "done", "type": "constant", "params": { "value": serde_json::to_value(task::success()).unwrap() } },
                    { "id": "status", "type": "output", "params": { "path": "task/status" } }
                ],
                "edges": [
                    { "from": { "node_id": "done" }, "to": { "node_id": "status", "input": "in" } }
                ]
            })
            .to_string(),
        )
        .expect("parse");
        store.write(StateChange::set("a", float(0.0))).unwrap();
        let nodes_before = vizij_nodes(&graph);
        let handle = spawn_behavior(&mut graph, "once", &behavior);
        let mut bridge = NoopBridge;
        graph.tick_store(&store, &mut bridge, 0.016).expect("tick");
        assert_eq!(read_key(&store, &handle.status), Some(task::success()));
        assert_eq!(vizij_nodes(&graph), nodes_before, "swept");
    }

    /// A LOAD replaces the main behavior and leaves the live runs running:
    /// a run's nodes carry over with their state, and its halt still prunes
    /// them.
    #[test]
    fn runs_survive_a_load() {
        let store = SimpleDataStore::new();
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");
        let (_, handle, moving) = moving_run(&mut graph, &store, 3);
        let wrapper = graph
            .spawn(look_at(), RunPolicy::Concurrent)
            .expect("spawn a wrapper run");

        let main = passthrough("sensor/b", "actuator/b");
        graph
            .load(graph_codec::encode(&main).expect("encode"))
            .expect("load");
        let nodes_after_load = vizij_nodes(&graph);
        let mut bridge = RunBridge::new(Vec::new());
        store
            .write(StateChange::set("sensor/b", float(0.25)))
            .unwrap();
        graph.tick_store(&store, &mut bridge, 0.1).expect("tick");
        assert_eq!(
            read(&store, "actuator/b"),
            Some(float(0.25)),
            "the new main graph runs"
        );
        let next = f32_at(&store, "out");
        assert!(
            moving < next && next < 0.9,
            "the run's damp kept its state: {moving} → {next}"
        );
        assert_eq!(read_key(&store, &handle.status), Some(task::running()));
        assert_eq!(read_key(&store, &wrapper.status), Some(task::running()));
        assert_eq!(
            bridge.calls.len(),
            1,
            "the wrapper run still calls its function"
        );

        graph.halt(handle.id).expect("halt");
        graph.halt(wrapper.id).expect("halt");
        graph.tick_store(&store, &mut bridge, 0.1).expect("tick");
        assert_eq!(
            vizij_nodes(&graph),
            nodes_after_load - 3 - 4 - 3,
            "the halts pruned both runs"
        );
        assert_eq!(vizij_nodes(&graph), 2, "the main graph remains");
    }

    /// run::edit changes a running behavior in place: the nodes the new
    /// graph keeps keep their state, the ones it adds join the run, and the
    /// halt prunes the run as edited.
    #[test]
    fn an_edit_inside_a_run_keeps_its_state_and_its_halt_prunes_the_edit() {
        let store = SimpleDataStore::new();
        let mut graph =
            ProcessingGraph::from_spec(passthrough("sensor/x", "actuator/y")).expect("from_spec");
        let (nodes_before, handle, moving) = moving_run(&mut graph, &store, 3);

        // The same damp, now also feeding a second output; the first output
        // node is replaced by another id.
        let edited = parse_spec(
            &json!({
                "nodes": [
                    { "id": "target", "type": "input", "params": { "path": "in/target", "value": 0.0 } },
                    { "id": "damp", "type": "damp", "params": { "half_life": 0.5 } },
                    { "id": "out-renamed", "type": "output", "params": { "path": "out" } },
                    { "id": "copy", "type": "output", "params": { "path": "out/copy" } }
                ],
                "edges": [
                    { "from": { "node_id": "target" }, "to": { "node_id": "damp", "input": "in" } },
                    { "from": { "node_id": "damp" }, "to": { "node_id": "out-renamed", "input": "in" } },
                    { "from": { "node_id": "damp" }, "to": { "node_id": "copy", "input": "in" } }
                ]
            })
            .to_string(),
        )
        .expect("parse");
        let diff = run::edit(handle.id, &damped("out"), &edited).expect("the edit");
        assert_eq!(diff.remove_nodes, vec![format!("task/{}/out", handle.id.0)]);
        graph
            .apply(graph_codec::spec_diff_to_graph_diff(&diff).expect("translate"))
            .expect("apply");

        let mut bridge = NoopBridge;
        graph.tick_store(&store, &mut bridge, 0.1).expect("tick");
        let next = f32_at(&store, "out");
        assert!(
            moving < next && next < 0.9,
            "the kept damp kept its state: {moving} → {next}"
        );
        assert_eq!(f32_at(&store, "out/copy"), next, "the added node runs");
        assert_eq!(vizij_nodes(&graph), nodes_before + 4 + 4);

        graph.halt(handle.id).expect("halt");
        graph.tick_store(&store, &mut bridge, 0.1).expect("tick");
        assert_eq!(
            vizij_nodes(&graph),
            nodes_before,
            "the halt pruned the run as edited"
        );
    }

    /// A malformed run_behavior call is refused at the spawn, the graph
    /// untouched.
    #[test]
    fn a_run_behavior_call_without_a_graph_is_refused() {
        let mut graph = ProcessingGraph::from_spec(passthrough("a", "b")).expect("from_spec");
        let nodes_before = vizij_nodes(&graph);
        let mut call = run::call("bad", &passthrough("c", "d")).expect("encode");
        call.args
            .retain(|field| field.id == run::run_behavior::ids::run_behavior::NAME);
        assert!(graph.spawn(call, RunPolicy::Concurrent).is_err());
        assert_eq!(vizij_nodes(&graph), nodes_before);
    }
}
