//! A node graph run on the behavior tree: stage inputs and set the clock,
//! evaluate, then read every node's ports and the writes of its `output`
//! nodes.
//!
//! [`TreeRuntime`] holds the graph's own store: the staged inputs, the clock
//! and the keys the graph writes. It [imports](crate::import) the graph,
//! builds the tree over that store, and lowers it onto a bridge running the
//! [node functions](crate::functions) and the
//! [graph format's](crate::vizij_functions) in-process. An evaluation is one
//! tick of the tree.
//!
//! What it evaluates is a choice ([`Evaluation`]): every node at every tick,
//! so every port can be read and every node's error surfaces; or only what
//! the graph's outputs read, which is what a device runs.
use std::collections::{HashMap, HashSet};

use arora_behavior::FunctionIndex;
use arora_behavior_tree::{lower_behavior_tree, BehaviorTree, LoweredTree};
use arora_simple_data_store::SimpleDataStore;
use arora_types::call::{Call, CallBridge, CallError, CallResult, Callable, CallableId};
use arora_types::data::{DataStore, Key, StateChange};
use arora_types::value::StructureField;
use uuid::Uuid;
use vizij_api_core::{Shape, TypedPath, Value, WriteBatch, WriteOp};

use crate::eval::shape_helpers::infer_shape;
use crate::eval::{NodeFunctions, PortValue, SpawnRequest};
use crate::functions::{is_stateful, state_param, DataflowNodes};
use crate::import::{import_with, node_arguments, ports_of, ImportOptions, ImportedGraph};
use crate::types::{GraphSpec, NodeId, NodeParams, NodeSpec, NodeType};
use crate::vizij_functions::VizijFunctions;

/// What an evaluation brings up to date.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Evaluation {
    /// Every node, at every tick, in topological order: each port can be
    /// read, and an error any node meets fails the tick.
    #[default]
    Every,
    /// What the graph's outputs read: its sinks, its keyed outputs and what
    /// they read. A node nothing reads does not run.
    Demanded,
}

/// The clock variables of the runtime's store, in seconds.
const TIME: &str = "graph/t";
const DT: &str = "graph/dt";

/// A node graph run on the behavior tree.
pub struct TreeRuntime {
    /// The graph's time, in seconds, as the next evaluation reads it.
    pub t: f32,
    /// The duration of the next evaluation's step, in seconds.
    pub dt: f32,
    /// Every observed node's ports after the last evaluation, by node id
    /// and port name.
    pub outputs: HashMap<NodeId, HashMap<String, PortValue>>,
    /// The writes the graph's `output` nodes made in the last evaluation, in
    /// evaluation order.
    pub writes: WriteBatch,
    /// The runs the graph's `spawn` nodes requested in the last evaluation.
    pub spawns: Vec<SpawnRequest>,
    /// How many times a graph was loaded: the tree is built at each.
    loads: u64,
    /// Whether an evaluation fills [`outputs`](Self::outputs).
    collect_outputs: bool,
    evaluation: Evaluation,
    store: SimpleDataStore,
    module: DataflowNodes,
    vizij: VizijFunctions,
    /// The store key of each path an input was staged on.
    keys: hashbrown::HashMap<TypedPath, Key>,
    /// Values staged for the next evaluation, with their declared shapes.
    staged: HashMap<TypedPath, (Value, Option<Shape>)>,
    /// The input keys the last evaluation staged: unset at the next unless
    /// staged again.
    live: HashMap<Key, Option<Shape>>,
    loaded: Option<Loaded>,
}

/// A graph imported and lowered.
struct Loaded {
    /// The spec's JSON text, which an evaluation of a spec compares with:
    /// written when first compared, and again after a node's parameters
    /// change in place.
    source: Option<String>,
    spec: GraphSpec,
    imported: ImportedGraph,
    lowered: LoweredTree,
    /// The graph's node indices in topological order.
    order: Vec<usize>,
    /// The indices of its `output` nodes, in that order.
    outputs: Vec<usize>,
    /// By node: the store key an `input` node reads, whose staged shape is
    /// its port's.
    input_keys: Vec<Option<Key>>,
    /// By node: how many calls it had made when its ports were last
    /// collected into [`TreeRuntime::outputs`]; a node whose call did not
    /// run since keeps its ports there.
    collected: std::cell::RefCell<Vec<Option<u64>>>,
}

impl Default for TreeRuntime {
    fn default() -> Self {
        Self::new(Evaluation::Every)
    }
}

impl TreeRuntime {
    /// A runtime holding no graph yet, evaluating as `evaluation` says.
    pub fn new(evaluation: Evaluation) -> Self {
        Self {
            t: 0.0,
            dt: 0.0,
            outputs: HashMap::new(),
            writes: WriteBatch::default(),
            spawns: Vec::new(),
            loads: 0,
            collect_outputs: true,
            evaluation,
            store: SimpleDataStore::new(),
            module: DataflowNodes::new(),
            vizij: VizijFunctions::default(),
            keys: hashbrown::HashMap::new(),
            staged: HashMap::new(),
            live: HashMap::new(),
            loaded: None,
        }
    }

    /// Route each external function of `function_modules` to its module.
    pub fn set_function_modules(&mut self, function_modules: HashMap<Uuid, Uuid>) {
        self.module.set_function_modules(function_modules);
    }

    /// Stage `value` on `path` for the next evaluation, which alone sees it.
    pub fn set_input(&mut self, path: TypedPath, value: Value, declared: Option<Shape>) {
        self.staged.insert(path, (value, declared));
    }

    /// Whether an evaluation fills [`outputs`](Self::outputs) with what
    /// every node computed (the default), or only makes the writes: a host
    /// that only applies the writes saves a copy of every port.
    pub fn set_collect_outputs(&mut self, collect: bool) {
        self.collect_outputs = collect;
    }

    /// How many times a graph was loaded, the tree built at each.
    pub fn loads(&self) -> u64 {
        self.loads
    }

    /// The number of nodes the module keeps something for.
    #[cfg(test)]
    pub(crate) fn module_sites(&self) -> usize {
        self.module.sites()
    }

    /// Drop the value staged on `path` for the next evaluation.
    pub fn clear_input(&mut self, path: &TypedPath) {
        self.staged.remove(path);
    }

    /// The store the graph reads its inputs from and writes its outputs to.
    pub fn store(&self) -> &SimpleDataStore {
        &self.store
    }

    /// The tree the graph runs as, once loaded.
    pub fn tree(&self) -> Option<&BehaviorTree> {
        self.loaded.as_ref().map(|loaded| loaded.lowered.tree())
    }

    /// The graph as the tree holds it, once loaded.
    pub fn imported(&self) -> Option<&ImportedGraph> {
        self.loaded.as_ref().map(|loaded| &loaded.imported)
    }

    /// The state the stateful node `id` of the graph holds, as the value of
    /// its `state` argument: what its next call starts from.
    pub fn state_value(&self, id: &str) -> Option<Value> {
        let loaded = self.loaded.as_ref()?;
        let node = loaded.imported.node(id)?;
        loaded
            .lowered
            .tree()
            .argument_value(node.tree_id, state_param())
            .filter(|state| *state != Value::Unit)
    }

    /// The runtime state of every stateful node of the graph, decoded, by
    /// node id.
    #[cfg(test)]
    pub(crate) fn node_states(&self) -> HashMap<String, crate::eval::context::NodeRuntimeState> {
        let Some(loaded) = &self.loaded else {
            return HashMap::new();
        };
        loaded
            .spec
            .nodes
            .iter()
            .filter_map(|node| {
                let value = self.state_value(&node.id);
                let state =
                    self.module
                        .state(&node.kind.kind()?, &node.params.params(), value.as_ref())?;
                Some((node.id.clone(), state))
            })
            .collect()
    }

    /// Run `spec` from now on. Every node of `spec` the previous graph also
    /// had keeps its state. A call to another module is ticked at every
    /// tick, as no function is described ([`load_with_functions`](Self::load_with_functions)).
    pub fn load(&mut self, spec: &GraphSpec) -> Result<(), String> {
        self.load_with(spec, &FunctionIndex::new(), &mut NoModules)
    }

    /// [`load`](Self::load) `spec`, placing each call to another module by
    /// the description `functions` gives of its function
    /// ([`placement`](crate::functions::placement)): such as a device's
    /// function index.
    pub fn load_with_functions(
        &mut self,
        spec: &GraphSpec,
        functions: &FunctionIndex,
    ) -> Result<(), String> {
        self.load_with(spec, functions, &mut NoModules)
    }

    fn load_with(
        &mut self,
        spec: &GraphSpec,
        functions: &FunctionIndex,
        inner: &mut dyn CallBridge,
    ) -> Result<(), String> {
        let options = ImportOptions {
            time: TIME.to_string(),
            dt: DT.to_string(),
        };
        let imported = import_with(spec, &options, functions)?;
        let store = &self.store;
        let resolver = |name: &str| Some(store.slot(&Key::from(name)));
        let tree = imported.build(&resolver)?;
        let observed: Vec<Uuid> = match self.evaluation {
            Evaluation::Every => imported
                .graph_nodes
                .iter()
                .flat_map(|node| std::iter::once(node.tree_id).chain(node.check))
                .collect(),
            Evaluation::Demanded => Vec::new(),
        };
        tree.observe_detached(
            observed
                .into_iter()
                .chain(imported.keyed_outputs.iter().map(|output| output.tree_id)),
        );
        let order = crate::topo::topo_order(&spec.nodes, &spec.edges)?;
        let index: HashMap<&str, usize> = spec
            .nodes
            .iter()
            .enumerate()
            .map(|(at, node)| (node.id.as_str(), at))
            .collect();
        let order: Vec<usize> = order.iter().map(|id| index[id.as_str()]).collect();
        let outputs = order
            .iter()
            .copied()
            .filter(|at| spec.nodes[*at].kind == NodeType::Output)
            .collect();
        let mut vizij = self.vizij.bridge(inner);
        let mut bridge = self.module.bridge(&mut vizij);
        if let Some(previous) = &self.loaded {
            carry_state(&tree, previous, &imported);
        }
        let lowered = lower_behavior_tree(tree, function_index(), &mut bridge, false)
            .map_err(|e| e.to_string())?;
        if let Some(previous) = self.loaded.take() {
            previous.lowered.unregister(&mut bridge);
        }
        self.loads += 1;
        self.module.retain_sites(|node| index.contains_key(node));
        self.vizij.retain(|node| index.contains_key(node));
        let input_keys = spec
            .nodes
            .iter()
            .map(|node| {
                let path = node.params.path.as_ref()?;
                (node.kind == NodeType::Input).then(|| Key::new(path.to_string()))
            })
            .collect();
        self.outputs.clear();
        self.loaded = Some(Loaded {
            source: None,
            spec: spec.clone(),
            imported,
            lowered,
            order,
            outputs,
            input_keys,
            collected: std::cell::RefCell::new(vec![None; spec.nodes.len()]),
        });
        Ok(())
    }

    /// Give node `id` of the loaded graph the parameters `params` in place:
    /// the tree keeps running, every node keeping its state, and the node
    /// runs with its new parameters at the next evaluation. This is the
    /// cheap edit an editor makes when a slider moves; a
    /// [`load`](Self::load) of the edited spec imports the whole graph.
    ///
    /// Returns `false`, changing nothing, for an edit only a load can make,
    /// because it changes what the tree is: the node's output ports, the
    /// store path an `input` or `output` node binds, or which parameters the
    /// node sets. Also `false` when no graph is loaded or the loaded graph
    /// has no node `id`.
    pub fn set_params(&mut self, id: &str, params: &NodeParams) -> Result<bool, String> {
        let Some(loaded) = &mut self.loaded else {
            return Ok(false);
        };
        let Some(at) = loaded.spec.nodes.iter().position(|node| node.id == id) else {
            return Ok(false);
        };
        let Some(graph_node) = loaded.imported.node(id) else {
            return Ok(false);
        };
        let node = &loaded.spec.nodes[at];
        let binding =
            |params: &NodeParams| (params.path.clone(), params.key_field, params.value_field);
        if matches!(node.kind, NodeType::Input | NodeType::Output)
            && binding(&node.params) != binding(params)
        {
            return Ok(false);
        }
        let edited = NodeSpec {
            params: params.clone(),
            ..node.clone()
        };
        let referenced: hashbrown::HashSet<String> = loaded
            .spec
            .edges
            .iter()
            .filter(|edge| edge.from.node_id == id)
            .map(|edge| edge.from.output.clone())
            .collect();
        let edited_params = params.params();
        if ports_of(&edited, &edited_params, &referenced) != graph_node.ports {
            return Ok(false);
        }
        let before = node_arguments(node, &node.params.params())?;
        let after = node_arguments(&edited, &edited_params)?;
        if before.len() != after.len()
            || before
                .iter()
                .zip(&after)
                .any(|((old, _), (new, _))| old != new)
        {
            return Ok(false);
        }
        let tree = loaded.lowered.tree();
        for ((_, old), (parameter, value)) in before.iter().zip(after) {
            if *old != value {
                tree.set_argument_value(graph_node.tree_id, parameter, Some(value));
            }
        }
        loaded.spec.nodes[at].params = edited.params;
        loaded.source = None;
        Ok(true)
    }

    /// Evaluate `spec`, loading it first when it is not the graph loaded.
    pub fn evaluate_spec(&mut self, spec: &GraphSpec) -> Result<(), String> {
        self.evaluate_spec_with(spec, &mut NoModules)
    }

    /// [`evaluate_spec`](Self::evaluate_spec), the calls the graph makes to
    /// other modules going to `inner`.
    pub fn evaluate_spec_with(
        &mut self,
        spec: &GraphSpec,
        inner: &mut dyn CallBridge,
    ) -> Result<(), String> {
        let source = serde_json::to_string(spec).map_err(|e| e.to_string())?;
        let same = match &mut self.loaded {
            Some(loaded) => {
                let loaded_source = match &mut loaded.source {
                    Some(text) => text,
                    unwritten => unwritten
                        .insert(serde_json::to_string(&loaded.spec).map_err(|e| e.to_string())?),
                };
                *loaded_source == source
            }
            None => false,
        };
        if !same {
            self.load_with(spec, &FunctionIndex::new(), inner)?;
        }
        self.evaluate_with(inner)
    }

    /// Evaluate the loaded graph once.
    pub fn evaluate(&mut self) -> Result<(), String> {
        self.evaluate_with(&mut NoModules)
    }

    /// Evaluate the loaded graph once, the calls it makes to other modules
    /// going to `inner`.
    pub fn evaluate_with(&mut self, inner: &mut dyn CallBridge) -> Result<(), String> {
        self.writes = WriteBatch::default();
        self.spawns.clear();
        let Some(loaded) = &self.loaded else {
            return Err("no graph is loaded".to_string());
        };

        // The clock, and the inputs staged for this evaluation alone.
        let mut change = StateChange::new();
        change.set.insert(Key::from(TIME), Some(Value::F32(self.t)));
        change.set.insert(Key::from(DT), Some(Value::F32(self.dt)));
        let staged = std::mem::take(&mut self.staged);
        let mut live = HashMap::new();
        for (path, (value, declared)) in staged {
            let key = match self.keys.get(&path) {
                Some(key) => key.clone(),
                None => {
                    let key = Key::new(path.to_string());
                    self.keys.insert(path, key.clone());
                    key
                }
            };
            change.set.insert(key.clone(), Some(value));
            live.insert(key, declared);
        }
        for key in self.live.keys() {
            change.set.entry(key.clone()).or_insert(None);
        }
        self.live = live;
        self.store.write(change).map_err(|e| e.to_string())?;

        let mut vizij = self.vizij.bridge(inner);
        let mut bridge = self.module.bridge(&mut vizij);
        loaded
            .lowered
            .tick(&mut bridge)
            .map_err(|e| e.to_string())?;
        self.spawns = self.module.take_spawns();

        // What every observed node computed, and what the outputs wrote.
        let tree = loaded.lowered.tree();
        let mut keyed = StateChange::new();
        let visited = if self.collect_outputs {
            &loaded.order
        } else {
            &loaded.outputs
        };
        let mut collected = loaded.collected.borrow_mut();
        for at in visited {
            let (node, graph_node) = (&loaded.spec.nodes[*at], &loaded.imported.graph_nodes[*at]);
            // A detached node whose call did not run since its ports were
            // collected keeps them; an input's shape is the one staged with
            // it, so its ports are collected again.
            let calls = (!graph_node.attached)
                .then(|| tree.detached_calls(graph_node.tree_id))
                .flatten();
            let kept = self.collect_outputs
                && calls.is_some()
                && collected[*at] == calls
                && node.kind != NodeType::Input
                && self.outputs.contains_key(&node.id);
            if kept && node.kind != NodeType::Output {
                continue;
            }
            let Some(returned) = loaded.imported.returned(tree, graph_node) else {
                continue;
            };
            let live = &self.live;
            let port_of = |port: &str, value: Value| {
                let shape = node
                    .output_shapes
                    .get(port)
                    .cloned()
                    .or_else(|| live.get(loaded.input_keys[*at].as_ref()?).cloned()?)
                    .unwrap_or_else(|| infer_shape(&value));
                PortValue::with_shape(value, shape)
            };
            // A node of `out` alone returns its port's value, taken as it is.
            let mut ports = HashMap::new();
            let mut out = None;
            if graph_node.returns_out() {
                if returned != Value::Unit {
                    out = Some(port_of("out", returned));
                }
            } else {
                for port in &graph_node.ports {
                    if let Some(value) = graph_node.port_value(&returned, port) {
                        ports.insert(port.clone(), port_of(port, value));
                    }
                }
            }
            if node.kind == NodeType::Output {
                let port = out.as_ref().or_else(|| ports.get("out"));
                if let (Some(path), Some(port)) = (&node.params.path, port) {
                    self.writes.push(WriteOp::new_with_shape(
                        path.clone(),
                        port.value.clone(),
                        Some(port.shape.clone()),
                    ));
                } else if let (Some(key_field), Some(value_field), Some(port)) =
                    (node.params.key_field, node.params.value_field, port)
                {
                    for op in
                        crate::eval::vizij::keyed_batch_writes(&port.value, key_field, value_field)
                            .map_err(|message| format!("node '{}': {message}", node.id))?
                    {
                        keyed
                            .set
                            .insert(Key::new(op.path.to_string()), Some(op.value.clone()));
                        self.writes.push(op);
                    }
                }
            }
            if self.collect_outputs {
                if let Some(out) = out {
                    ports.insert("out".to_string(), out);
                }
                collected[*at] = calls;
                match self.outputs.get_mut(&node.id) {
                    Some(kept) => *kept = ports,
                    None => {
                        self.outputs.insert(node.id.clone(), ports);
                    }
                }
            }
        }
        drop(collected);
        if !keyed.set.is_empty() {
            self.store.write(keyed).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// Carry into `tree`, built from `imported`, the state of `previous`: every
/// node `imported` shares unchanged with it keeps the cells its arguments
/// hold, and a changed stateful node keeps its state: a node whose
/// parameters change runs on from where it was.
fn carry_state(tree: &BehaviorTree, previous: &Loaded, imported: &ImportedGraph) {
    let held: HashMap<Uuid, &arora_behavior_tree::schema::Node> = previous
        .imported
        .nodes
        .iter()
        .map(|node| (node.id, node))
        .collect();
    let fresh: HashSet<Uuid> = imported
        .nodes
        .iter()
        .filter(|node| held.get(&node.id) != Some(node))
        .map(|node| node.id)
        .collect();
    let before = previous.lowered.tree();
    tree.keep_state(before, &fresh);
    for node in &imported.graph_nodes {
        let stateful = node.kind.kind().is_some_and(|kind| is_stateful(&kind));
        if fresh.contains(&node.tree_id) && stateful {
            let state = before.argument_value(node.tree_id, state_param());
            if state.is_some() {
                tree.set_argument_value(node.tree_id, state_param(), state);
            }
        }
    }
}

/// Evaluate `spec` on `rt`, loading it first when it is not the graph loaded.
pub fn evaluate_all(rt: &mut TreeRuntime, spec: &GraphSpec) -> Result<(), String> {
    rt.evaluate_spec(spec)
}

/// Evaluate `spec` on `rt`, its external functions and task runs calling
/// `functions`.
pub fn evaluate_all_with_functions(
    rt: &mut TreeRuntime,
    spec: &GraphSpec,
    functions: &mut dyn NodeFunctions,
) -> Result<(), String> {
    rt.module.set_default_module(Some(FUNCTIONS_MODULE));
    let result = rt.evaluate_spec_with(spec, &mut FunctionsBridge { functions });
    rt.module.set_default_module(None);
    result
}

/// Evaluate the graph `rt` has loaded, its calls to other modules going to
/// `functions`.
pub fn evaluate_with_functions(
    rt: &mut TreeRuntime,
    functions: &mut dyn NodeFunctions,
) -> Result<(), String> {
    rt.module.set_default_module(Some(FUNCTIONS_MODULE));
    let result = rt.evaluate_with(&mut FunctionsBridge { functions });
    rt.module.set_default_module(None);
    result
}

/// The module a [`FunctionsBridge`] answers for.
const FUNCTIONS_MODULE: Uuid = Uuid::from_u128(0x6e6f_6465_2d67_7261_7068_0000_0000_00ff);

/// A bridge calling a [`NodeFunctions`] host, its out-parameters the call's
/// mutated parameters.
pub struct FunctionsBridge<'a> {
    /// The host the calls go to.
    pub functions: &'a mut dyn NodeFunctions,
}

impl CallBridge for FunctionsBridge<'_> {
    fn arora_call(&mut self, call: Call) -> Result<CallResult, CallError> {
        let args: Vec<(Uuid, Value)> = call
            .args
            .into_iter()
            .map(|field| (field.id, *field.value))
            .collect();
        let module = call.module_id.filter(|module| *module != FUNCTIONS_MODULE);
        let (ret, mutated) = self
            .functions
            .call_module_with_outputs(module, call.id, &args)
            .map_err(|message| CallError::Generic { message })?;
        Ok(CallResult {
            ret,
            mutated: mutated
                .into_iter()
                .map(|(id, value)| StructureField {
                    id,
                    value: Box::new(value),
                })
                .collect(),
        })
    }

    fn arora_register_callable(&mut self, _: std::rc::Rc<dyn Callable>) -> CallableId {
        unreachable!("the node-graph bridge registers the tree's callables")
    }

    fn arora_unregister_callable(&mut self, _: &CallableId) {}

    fn arora_call_indirect(&mut self, callable_id: &CallableId) -> Result<Value, CallError> {
        Err(CallError::Internal {
            message: format!("no callable {}", callable_id.id),
        })
    }
}

/// The functions a graph's tree calls in-process: the node vocabulary's
/// and the graph format's.
fn function_index() -> &'static FunctionIndex {
    static INDEX: std::sync::OnceLock<FunctionIndex> = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index = crate::functions::function_index().clone();
        index.extend(
            crate::vizij_functions::function_index()
                .iter()
                .map(|(id, function)| (*id, function.clone())),
        );
        index
    })
}

/// A bridge to no module: what a graph runs over when it calls none.
struct NoModules;

impl CallBridge for NoModules {
    fn arora_call(&mut self, call: Call) -> Result<CallResult, CallError> {
        Err(CallError::FunctionNotFound { id: call.id })
    }

    fn arora_register_callable(&mut self, _: std::rc::Rc<dyn Callable>) -> CallableId {
        unreachable!("the node-graph bridge registers the tree's callables")
    }

    fn arora_unregister_callable(&mut self, _: &CallableId) {}

    fn arora_call_indirect(&mut self, callable_id: &CallableId) -> Result<Value, CallError> {
        Err(CallError::Internal {
            message: format!("no callable {}", callable_id.id),
        })
    }
}
