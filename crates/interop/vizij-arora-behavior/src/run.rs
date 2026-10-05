//! `run_behavior`: a task run whose behavior is the graph its spawn carries.
//!
//! The [`ProcessingGraph`](crate::ProcessingGraph) implements one generic
//! task method itself, [`RunBehavior::run_behavior`]`(name, behavior)`, and
//! describes it, so it is listed under the interpreter module like the
//! skills. Spawning it runs `behavior` beside the device's main behavior: the
//! LOAD call makes a graph *the* behavior, a `run_behavior` run adds one
//! more, and HALT ends it. Starting a behavior is the interpreter module's
//! SPAWN, stopping it is HALT, and its state is its run's status key, which
//! every client of the device reads.
//!
//! - `behavior` is the Vizij graph as the interpreter module's LOAD carries
//!   one: its [`graph_codec`] form, converted to a `Value` by
//!   [`arora_types::value_serde`] — a dynamic record, which is how the
//!   signature types it. [`call`] builds the spawn's call from a spec.
//! - The graph grafts as the run's fragment: its node ids under
//!   `task/<run id>/`, its `task/…` paths on the run's keys, as a skill
//!   fragment's are. The run adds two outputs of its own: its status,
//!   `Running` until it is halted (unless the graph writes `task/status`
//!   itself, and so decides when it ends), and its `name` on
//!   [`name_key`] — what any client reads to tell which behavior the run runs
//!   ([`runs`]).
//! - A halt prunes the fragment and leaves the store as it is: the run's
//!   outputs hold their last values. Spawning again is a new run, its nodes
//!   starting from fresh state; a graph that acts on its start does so
//!   itself. Returning the keys a run wrote to rest is the client's step.
//! - [`edit`] is the EDIT that changes a running behavior in place: the
//!   nodes the new graph keeps keep their state.
//!
//! A run's nodes are the graph's nodes under its id: an EDIT that adds or
//! removes nodes there changes the run, and a LOAD of the main behavior
//! leaves it running.
//!
//! [`graph_codec`]: crate::graph_codec

use std::collections::{HashMap, HashSet};

use arora_behavior::{interpreter_module, Graph, Status, TaskHandle, TaskId};
use arora_types::call::Call;
use arora_types::data::{DataStore, Key};
use arora_types::value::{StructureField, Value};
use arora_types::value_serde;
use uuid::Uuid;
use vizij_graph_core::types::GraphSpec;

use crate::graph_codec::{self, GraphSpecDiff};

/// The generic task run: `run_behavior` runs `behavior` beside the device's
/// main behavior, under `name`, until halted.
#[arora_module::contract(name = "run_behavior")]
pub trait RunBehavior {
    #[export(id = "d5343617-a887-47da-82ea-9fcbe858f5c6")]
    fn run_behavior(
        &mut self,
        #[param(id = "86ead3c9-a77e-495b-96f4-8522756aba68")] name: String,
        #[param(id = "98fba9c7-9840-4064-b368-9a1d2b0d2dff")] behavior: Value,
    ) -> Status;
}

use run_behavior::ids::run_behavior as ids;

/// The function id of `run_behavior`.
pub const FUNCTION: Uuid = ids::FUNCTION;

/// `run_behavior` as its contract describes it: what the interpreter lists
/// among its described methods.
pub fn description() -> arora_types::record::module::frozen::Export {
    run_behavior::descriptions()
        .remove(&FUNCTION)
        .expect("the run_behavior contract declares run_behavior")
}

/// The key prefix of run `task`'s keys: `arora/tasks/<interpreter
/// module>/<run_behavior>/<run id>`.
pub fn prefix(task: TaskId) -> String {
    format!(
        "arora/tasks/{}/{}/{}",
        interpreter_module::ID,
        FUNCTION,
        task.0
    )
}

/// The key run `task` writes its name to, beside its status.
pub fn name_key(task: TaskId) -> Key {
    Key::from(format!("{}/name", prefix(task)))
}

/// The handle of run `task`, as its spawn returns it: the status, feedback
/// and result keys under [`prefix`], no update keys, and the HALT call.
pub fn handle(task: TaskId) -> TaskHandle {
    let prefix = prefix(task);
    TaskHandle {
        id: task,
        stop: interpreter_module::encode_halt(task),
        status: Key::from(format!("{prefix}/status")),
        feedback: vec![Key::from(format!("{prefix}/feedback"))],
        result: vec![Key::from(format!("{prefix}/result"))],
        update: Vec::new(),
    }
}

/// `spec` as `run_behavior`'s `behavior` argument: the graph the interpreter
/// module's LOAD carries, as a `Value`. A client calling `run_behavior` by
/// name (Arora's by-name invoke) passes this. Errors when the spec does not
/// encode.
pub fn behavior(spec: &GraphSpec) -> Result<Value, String> {
    let graph = graph_codec::encode(spec)?;
    value_serde::to_value(&graph).map_err(|e| format!("the graph: {e}"))
}

/// The call that runs `behavior` under `name` — what the interpreter
/// module's SPAWN takes (`interpreter_module::encode_spawn`). Errors when the
/// spec does not encode.
pub fn call(name: &str, behavior: &GraphSpec) -> Result<Call, String> {
    let behavior = self::behavior(behavior)?;
    Ok(Call {
        module_id: Some(interpreter_module::ID),
        id: FUNCTION,
        args: vec![
            StructureField {
                id: ids::NAME,
                value: Box::new(Value::String(name.to_string())),
            },
            StructureField {
                id: ids::BEHAVIOR,
                value: Box::new(behavior),
            },
        ],
    })
}

/// A spawned `run_behavior` call's name and graph. A missing name reads
/// empty; a missing or malformed behavior is an error.
pub(crate) fn arguments(call: &Call) -> Result<(String, GraphSpec), String> {
    let argument = |id: Uuid| {
        call.args
            .iter()
            .find(|field| field.id == id)
            .map(|field| field.value.as_ref())
    };
    let name = match argument(ids::NAME) {
        None => String::new(),
        Some(Value::String(name)) => name.clone(),
        Some(other) => return Err(format!("run_behavior: the name is no string: {other}")),
    };
    let behavior = argument(ids::BEHAVIOR).ok_or("run_behavior: the call carries no behavior")?;
    let graph: Graph = value_serde::from_value(behavior.clone())
        .map_err(|e| format!("run_behavior: the behavior is no graph: {e}"))?;
    let spec = graph_codec::decode(&graph).map_err(|e| format!("run_behavior: {e}"))?;
    Ok((name, spec))
}

/// The `run_behavior` runs `store` holds keys of, by the name each wrote,
/// each with its handle — what a client that did not spawn a run reads to
/// find it, and halts it by. A run's keys outlive it: an ended run is
/// listed too, its status key terminal. Sorted by name, then id.
pub fn runs(store: &dyn DataStore) -> Vec<(TaskHandle, String)> {
    let root = format!("arora/tasks/{}/{}/", interpreter_module::ID, FUNCTION);
    let mut runs: Vec<(TaskHandle, String)> = store
        .snapshot()
        .storage
        .into_iter()
        .filter_map(|(key, value)| {
            let id = key.path.strip_prefix(&root)?.strip_suffix("/name")?;
            let task = TaskId(Uuid::parse_str(id).ok()?);
            match value {
                Some(Value::String(name)) => Some((handle(task), name)),
                _ => None,
            }
        })
        .collect();
    runs.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.id.0.cmp(&b.0.id.0)));
    runs
}

/// The EDIT that changes run `task`'s behavior from `from` to `to` in place:
/// the nodes of `from` that `to` drops are removed, and `to`'s nodes and
/// edges upserted under the run's ids. A node `to` keeps keeps its runtime
/// state. `from` is the behavior the run runs: what it was spawned with, or
/// the `to` of the edit before.
pub fn edit(task: TaskId, from: &GraphSpec, to: &GraphSpec) -> Result<GraphSpecDiff, String> {
    let prefix = prefix(task);
    let no_arguments = HashMap::new();
    let (from, _) =
        crate::graft_spec(from, task, &prefix, &no_arguments, None).map_err(|e| e.message)?;
    let (upsert_nodes, upsert_edges) =
        crate::graft_spec(to, task, &prefix, &no_arguments, None).map_err(|e| e.message)?;
    let kept: HashSet<&str> = upsert_nodes.iter().map(|node| node.id.as_str()).collect();
    let remove_nodes = from
        .into_iter()
        .map(|node| node.id)
        .filter(|id| !kept.contains(id.as_str()))
        .collect();
    Ok(GraphSpecDiff {
        upsert_nodes,
        remove_nodes,
        upsert_edges,
        remove_edges: Vec::new(),
    })
}
