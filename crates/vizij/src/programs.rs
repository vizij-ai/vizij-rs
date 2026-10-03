//! A Vizij's programs, controlled while its device runs.
//!
//! A program is a graph a face plays over its rig: one of the bundle's
//! `motiongraph` graphs, or a graph a caller defines under an id of its own
//! (an authoring editor's live graph). Playing a program means its nodes are
//! part of the device's one running graph, under the `program::<id>::` prefix
//! the face's composition gives them ([`Bundle::compose`]); the node graph
//! stays the only interpreter, and every program runs in it beside the base
//! composition.
//!
//! [`Programs`] keeps each program's graph and state and turns a request into
//! the spec-level edit that brings the running graph there: playing grafts the
//! program's nodes and edges, pausing and stopping prune them, and replacing
//! the graph of a playing program removes the nodes it no longer has and
//! upserts the rest. An edit leaves every other node of the graph, and the
//! store, untouched; a node the replacement keeps keeps its runtime state.
//!
//! A paused program and a stopped one are both out of the graph, their
//! outputs holding their last values; they differ in what they report, and a
//! stop may also return the outputs to rest (see [`Programs::stop`]). Playing
//! again grafts the program afresh: its stateful nodes (springs, smoothing)
//! restart.
//!
//! [`Bundle::compose`]: vizij_arora_host::Bundle::compose

use std::collections::{HashMap, HashSet};

use arora_types::data::{Key, StateChange};
use arora_types::value::Value;
use serde_json::Value as Json;
use vizij_arora_behavior::graph_codec::GraphSpecDiff;
use vizij_graph_core::{GraphSpec, NodeType};

/// Where a program stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProgramState {
    /// In the running graph, writing its outputs every tick.
    Playing,
    /// Out of the graph, its outputs holding their last values, until played
    /// again.
    Paused,
    /// Out of the graph.
    Stopped,
}

impl ProgramState {
    /// The state's name on the JavaScript surface.
    pub(crate) fn name(self) -> &'static str {
        match self {
            ProgramState::Playing => "playing",
            ProgramState::Paused => "paused",
            ProgramState::Stopped => "stopped",
        }
    }
}

struct Program {
    /// The program's graph as the running graph holds it: node ids under
    /// [`source_prefix`].
    graph: GraphSpec,
    state: ProgramState,
    /// The program's node ids in the running graph; empty unless it plays.
    grafted: Vec<String>,
}

/// The programs of one device, by id.
pub(crate) struct Programs {
    programs: HashMap<String, Program>,
    /// The value each key rests at, for the keys that rest somewhere other
    /// than unset: the neutral pose the face staged at load.
    rest: HashMap<String, Value>,
}

/// The prefix of a program's node ids in the running graph — the source id
/// [`Bundle::compose`](vizij_arora_host::Bundle::compose) composes a program
/// under, followed by `::`.
fn source_prefix(id: &str) -> String {
    format!("program::{id}::")
}

/// `spec` as the running graph holds program `id`: normalized, node ids
/// prefixed the way the face's composition prefixes them. Errors when `spec`
/// is no graph spec, or when the graph does not encode as an edit — checked
/// here so that every later edit of the program encodes, and a request never
/// moves a program's state without its edit.
fn program_graph(id: &str, spec: &Json) -> Result<GraphSpec, String> {
    vizij_arora_behavior::parse_spec(&spec.to_string())
        .map_err(|e| format!("program {id:?}: {e}"))?;
    let composed = vizij_arora_host::compose_sources(&[(format!("program::{id}"), spec.clone())])
        .map_err(|e| format!("program {id:?}: {e:#}"))?;
    let graph: GraphSpec = serde_json::from_value(composed)
        .map_err(|e| format!("program {id:?}: invalid graph: {e}"))?;
    vizij_arora_behavior::encode_edit_call(&graft(&graph))
        .map_err(|e| format!("program {id:?}: {e}"))?;
    Ok(graph)
}

/// The edit that grafts `graph` into the running graph.
fn graft(graph: &GraphSpec) -> GraphSpecDiff {
    GraphSpecDiff {
        upsert_nodes: graph.nodes.clone(),
        upsert_edges: graph.edges.clone(),
        ..GraphSpecDiff::default()
    }
}

/// The edit that removes the nodes `grafted` (with their edges) from the
/// running graph.
fn prune(grafted: &[String]) -> GraphSpecDiff {
    GraphSpecDiff {
        remove_nodes: grafted.to_vec(),
        ..GraphSpecDiff::default()
    }
}

fn node_ids(graph: &GraphSpec) -> Vec<String> {
    graph.nodes.iter().map(|node| node.id.clone()).collect()
}

impl Programs {
    /// The programs of a face: each of the bundle's `programs` (`(id, spec)`),
    /// `playing` the one its composition already holds, and `rest` the values
    /// the face staged at load. A program whose graph does not normalize is
    /// left out, with a warning: it could never play.
    pub(crate) fn new(
        programs: &[(String, Json)],
        playing: Option<&str>,
        rest: HashMap<String, Value>,
    ) -> Self {
        let mut this = Programs {
            programs: HashMap::new(),
            rest,
        };
        for (id, spec) in programs {
            match program_graph(id, spec) {
                Ok(graph) => {
                    let (state, grafted) = if playing == Some(id.as_str()) {
                        (ProgramState::Playing, node_ids(&graph))
                    } else {
                        (ProgramState::Stopped, Vec::new())
                    };
                    this.programs.insert(
                        id.clone(),
                        Program {
                            graph,
                            state,
                            grafted,
                        },
                    );
                }
                Err(e) => log::warn!("{e}: the program cannot play"),
            }
        }
        this
    }

    /// No programs, nothing staged: a device built from a bare graph.
    pub(crate) fn empty() -> Self {
        Programs::new(&[], None, HashMap::new())
    }

    /// Program `id`'s state; `None` for an id the device does not know.
    pub(crate) fn state(&self, id: &str) -> Option<ProgramState> {
        self.programs.get(id).map(|program| program.state)
    }

    fn get_mut(&mut self, id: &str) -> Result<&mut Program, String> {
        self.programs
            .get_mut(id)
            .ok_or_else(|| format!("no program {id:?}"))
    }

    /// Play program `id`: the edit grafting it, or `None` when it already
    /// plays.
    pub(crate) fn start(&mut self, id: &str) -> Result<Option<GraphSpecDiff>, String> {
        let program = self.get_mut(id)?;
        if program.state == ProgramState::Playing {
            return Ok(None);
        }
        program.state = ProgramState::Playing;
        program.grafted = node_ids(&program.graph);
        Ok(Some(graft(&program.graph)))
    }

    /// Pause program `id`: the edit pruning it, or `None` when it does not
    /// play. A stopped program stays stopped.
    pub(crate) fn pause(&mut self, id: &str) -> Result<Option<GraphSpecDiff>, String> {
        let program = self.get_mut(id)?;
        if program.state != ProgramState::Playing {
            return Ok(None);
        }
        program.state = ProgramState::Paused;
        Ok(Some(prune(&std::mem::take(&mut program.grafted))))
    }

    /// Stop program `id`: the edit pruning it (`None` when it does not play),
    /// and, when `reset`, the store change returning its outputs to rest —
    /// each key it writes back to the value the face staged for it at load,
    /// or cleared, so every input that reads the key falls back to its own
    /// default. The edit and the change land in the same step: the program
    /// is gone before its outputs are next written.
    pub(crate) fn stop(
        &mut self,
        id: &str,
        reset: bool,
    ) -> Result<(Option<GraphSpecDiff>, Option<StateChange>), String> {
        let program = self
            .programs
            .get_mut(id)
            .ok_or_else(|| format!("no program {id:?}"))?;
        let edit = (program.state == ProgramState::Playing)
            .then(|| prune(&std::mem::take(&mut program.grafted)));
        program.state = ProgramState::Stopped;
        if !reset {
            return Ok((edit, None));
        }
        let mut change = StateChange::new();
        for node in &program.graph.nodes {
            if !matches!(node.kind, NodeType::Output) {
                continue;
            }
            if let Some(path) = &node.params.path {
                let key = path.to_string();
                let rest = self.rest.get(&key).cloned();
                change.set.insert(Key::new(key), rest);
            }
        }
        Ok((edit, Some(change)))
    }

    /// Give program `id` the graph `spec`, defining the program when the
    /// device does not know `id`. A playing program changes in place: the
    /// edit removes the nodes the new graph no longer has and upserts the
    /// new graph's nodes and edges, so a node it keeps keeps its runtime
    /// state. `None` when the program does not play. On an error nothing
    /// changes.
    pub(crate) fn set(&mut self, id: &str, spec: &Json) -> Result<Option<GraphSpecDiff>, String> {
        let graph = program_graph(id, spec)?;
        let Some(program) = self.programs.get_mut(id) else {
            self.programs.insert(
                id.to_string(),
                Program {
                    graph,
                    state: ProgramState::Stopped,
                    grafted: Vec::new(),
                },
            );
            return Ok(None);
        };
        let edit = (program.state == ProgramState::Playing).then(|| {
            let kept: HashSet<&str> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
            GraphSpecDiff {
                remove_nodes: program
                    .grafted
                    .iter()
                    .filter(|id| !kept.contains(id.as_str()))
                    .cloned()
                    .collect(),
                ..graft(&graph)
            }
        });
        if edit.is_some() {
            program.grafted = node_ids(&graph);
        }
        program.graph = graph;
        Ok(edit)
    }

    /// The running graph was replaced whole by `spec`: a program plays when
    /// `spec` holds nodes under its prefix, and is otherwise out of the graph
    /// — stopped, or still paused when it was.
    pub(crate) fn loaded(&mut self, spec: &GraphSpec) {
        for (id, program) in &mut self.programs {
            let prefix = source_prefix(id);
            program.grafted = spec
                .nodes
                .iter()
                .filter(|node| node.id.starts_with(&prefix))
                .map(|node| node.id.clone())
                .collect();
            program.state = match (program.grafted.is_empty(), program.state) {
                (false, _) => ProgramState::Playing,
                (true, ProgramState::Paused) => ProgramState::Paused,
                (true, _) => ProgramState::Stopped,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use vizij_api_core::value::float;
    use vizij_arora_host::{Bundle, ProgramSelect};

    /// A program writing `path` from a constant.
    fn writes(path: &str, value: f32) -> Json {
        json!({
            "nodes": [
                { "id": "c", "type": "constant", "params": { "value": value } },
                { "id": "out", "type": "output", "params": { "path": path } }
            ],
            "edges": [
                { "from": { "node_id": "c" }, "to": { "node_id": "out", "input": "in" } }
            ]
        })
    }

    fn ids(nodes: &[vizij_graph_core::NodeSpec]) -> Vec<&str> {
        let mut ids: Vec<&str> = nodes.iter().map(|node| node.id.as_str()).collect();
        ids.sort();
        ids
    }

    #[test]
    fn the_composed_program_plays_under_the_composition_ids() {
        let bundle = Bundle::from_bundle_json(&json!({
            "metadata": { "activeMotionGraphId": "speaks" },
            "graphs": [
                { "kind": "rig", "spec": writes("rig/x", 0.0) },
                { "kind": "motiongraph", "id": "speaks", "spec": writes("a", 1.0) },
                { "kind": "motiongraph", "id": "live", "spec": writes("b", 1.0) }
            ]
        }));
        let composed: GraphSpec = serde_json::from_value(
            bundle
                .compose(&["rig"], &ProgramSelect::Auto, false, &[])
                .unwrap(),
        )
        .unwrap();
        let mut programs = Programs::new(&bundle.programs, Some("speaks"), HashMap::new());
        assert_eq!(programs.state("speaks"), Some(ProgramState::Playing));
        assert_eq!(programs.state("live"), Some(ProgramState::Stopped));
        assert_eq!(programs.state("nope"), None);
        assert_eq!(ProgramState::Playing.name(), "playing");
        assert_eq!(ProgramState::Stopped.name(), "stopped");

        // What stopping prunes is what the composition holds.
        let (edit, reset) = programs.stop("speaks", false).unwrap();
        let mut pruned = edit.unwrap().remove_nodes;
        pruned.sort();
        let mut composed_ids: Vec<String> = composed
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .filter(|id| id.starts_with("program::speaks::"))
            .collect();
        composed_ids.sort();
        assert_eq!(pruned, composed_ids);
        assert!(reset.is_none());

        // And reloading the composition reads it playing again.
        programs.loaded(&composed);
        assert_eq!(programs.state("speaks"), Some(ProgramState::Playing));
        assert_eq!(programs.state("live"), Some(ProgramState::Stopped));
    }

    #[test]
    fn start_grafts_and_pause_prunes() {
        let mut programs =
            Programs::new(&[("live".into(), writes("b", 1.0))], None, HashMap::new());
        let edit = programs.start("live").unwrap().unwrap();
        assert_eq!(
            ids(&edit.upsert_nodes),
            ["program::live::c", "program::live::out"]
        );
        assert_eq!(edit.upsert_edges.len(), 1);
        assert!(programs.start("live").unwrap().is_none(), "already playing");

        let edit = programs.pause("live").unwrap().unwrap();
        assert_eq!(edit.remove_nodes.len(), 2);
        assert_eq!(programs.state("live"), Some(ProgramState::Paused));
        assert_eq!(ProgramState::Paused.name(), "paused");
        assert!(programs.pause("live").unwrap().is_none(), "already out");

        programs.stop("live", false).unwrap();
        assert!(programs.pause("live").unwrap().is_none());
        assert_eq!(programs.state("live"), Some(ProgramState::Stopped));
        assert!(programs.start("nope").is_err());
    }

    #[test]
    fn stop_with_reset_returns_the_outputs_to_rest() {
        let rest = HashMap::from([("staged".to_string(), float(0.25))]);
        let program = json!({
            "nodes": [
                { "id": "c", "type": "constant", "params": { "value": 1.0 } },
                { "id": "o1", "type": "output", "params": { "path": "staged" } },
                { "id": "o2", "type": "output", "params": { "path": "unstaged" } }
            ],
            "edges": [
                { "from": { "node_id": "c" }, "to": { "node_id": "o1", "input": "in" } },
                { "from": { "node_id": "c" }, "to": { "node_id": "o2", "input": "in" } }
            ]
        });
        let mut programs = Programs::new(&[("p".into(), program)], Some("p"), rest);
        let (edit, reset) = programs.stop("p", true).unwrap();
        assert!(edit.is_some());
        let reset = reset.unwrap().set;
        assert_eq!(reset.get(&Key::new("staged")), Some(&Some(float(0.25))));
        assert_eq!(reset.get(&Key::new("unstaged")), Some(&None));
        assert_eq!(reset.len(), 2);

        // A stopped program resets again on request, with no edit.
        let (edit, reset) = programs.stop("p", true).unwrap();
        assert!(edit.is_none());
        assert_eq!(reset.unwrap().set.len(), 2);
    }

    #[test]
    fn set_defines_a_program_and_replaces_a_playing_one_in_place() {
        let mut programs = Programs::empty();
        assert!(programs.set("editor", &writes("a", 1.0)).unwrap().is_none());
        assert_eq!(programs.state("editor"), Some(ProgramState::Stopped));
        programs.start("editor").unwrap();

        // The new graph keeps `out`, drops `c`, adds `k`.
        let replacement = json!({
            "nodes": [
                { "id": "k", "type": "constant", "params": { "value": 2.0 } },
                { "id": "out", "type": "output", "params": { "path": "a" } }
            ],
            "edges": [
                { "from": { "node_id": "k" }, "to": { "node_id": "out", "input": "in" } }
            ]
        });
        let edit = programs.set("editor", &replacement).unwrap().unwrap();
        assert_eq!(edit.remove_nodes, ["program::editor::c"]);
        assert_eq!(
            ids(&edit.upsert_nodes),
            ["program::editor::k", "program::editor::out"]
        );
        assert_eq!(edit.upsert_edges.len(), 1);

        // Stopping prunes what the replacement grafted.
        let (edit, _) = programs.stop("editor", false).unwrap();
        let mut pruned = edit.unwrap().remove_nodes;
        pruned.sort();
        assert_eq!(pruned, ["program::editor::k", "program::editor::out"]);

        // A stopped program's graph changes with no edit; a bad graph changes
        // nothing.
        assert!(programs.set("editor", &writes("a", 3.0)).unwrap().is_none());
        assert!(programs.set("editor", &json!({ "nodes": 3 })).is_err());
        assert_eq!(programs.state("editor"), Some(ProgramState::Stopped));
    }

    #[test]
    fn a_loaded_graph_without_a_program_leaves_it_out() {
        let mut programs = Programs::new(
            &[
                ("a".into(), writes("a", 1.0)),
                ("b".into(), writes("b", 1.0)),
            ],
            Some("a"),
            HashMap::new(),
        );
        programs.start("b").unwrap();
        programs.pause("b").unwrap();
        programs.loaded(&GraphSpec::default());
        assert_eq!(programs.state("a"), Some(ProgramState::Stopped));
        assert_eq!(programs.state("b"), Some(ProgramState::Paused));
        assert!(
            programs.stop("a", false).unwrap().0.is_none(),
            "nothing grafted"
        );
    }
}
