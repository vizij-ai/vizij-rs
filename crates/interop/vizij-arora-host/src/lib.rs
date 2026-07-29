//! Portable Vizij host glue — the spec/data transforms that sit *above* the
//! device, shared by the native app (`vizij`) and the browser runtime
//! (`vizij-arora-web`, driven by `@vizij/runtime-react`):
//!
//! - [`compose_sources`] unions several graph sources into the one graph a
//!   device runs (the rig, the pose-driver, a playing program, …).
//! - [`Bundle`] reads the face's `VIZIJ_bundle`: its graphs, its motiongraph
//!   programs, the program to autoplay, and the neutral-pose config.
//! - [`ProgramSelect`] picks which program plays; [`Bundle::compose`] composes
//!   the base graphs plus that program.
//! - [`Bundle::neutral_stage_writes`] resolves the neutral inputs to the store
//!   writes that stage the face's resting pose.
//!
//! There is no renderer and no device lifecycle here — those stay in each host
//! (Bevy + a worker thread natively; three.js + a Web Worker in the browser).
//! This is only the logic both hosts would otherwise write twice, once in Rust
//! and once in TypeScript.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde_json::{json, Value as Json};
use vizij_api_core::json::normalize_graph_spec_value;

/// Which of a bundle's motiongraph programs the face boots playing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgramSelect {
    /// The bundle's own `activeMotionGraphId`.
    Auto,
    /// A specific program id.
    Id(String),
    /// No program — hold the rig's authored/neutral pose.
    None,
}

/// A face's `VIZIJ_bundle`, reduced to what a host drives the device with.
#[derive(Debug, Clone, Default)]
pub struct Bundle {
    /// Graph entries, `(kind, spec)` — `rig`, `pose-driver`, `motiongraph`, ….
    pub graphs: Vec<(String, Json)>,
    /// The motiongraph programs, `(id, spec)` — the graphs the face can play on
    /// top of its rig (e.g. Quori's "Speaks").
    pub programs: Vec<(String, Json)>,
    /// `metadata.activeMotionGraphId` (or the first `activeMotionGraphIds`).
    pub active_program_id: Option<String>,
    /// `poses.config.neutralInputs` — input name → neutral value.
    pub neutral_inputs: HashMap<String, f64>,
}

impl Bundle {
    /// Read the bundle from a glTF JSON document: the `VIZIJ_bundle` extension
    /// on a node, or (as the web loader also accepts) on the document root.
    /// `None` when the document carries no bundle.
    pub fn from_gltf_json(gltf: &Json) -> Option<Bundle> {
        let bundle = gltf
            .get("nodes")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .find_map(|node| node.get("extensions").and_then(|e| e.get("VIZIJ_bundle")))
            .or_else(|| gltf.get("extensions").and_then(|e| e.get("VIZIJ_bundle")))?;
        Some(Bundle::from_bundle_json(bundle))
    }

    /// Read the bundle from the `VIZIJ_bundle` object directly.
    pub fn from_bundle_json(bundle: &Json) -> Bundle {
        // The program the face boots playing: `activeMotionGraphId`, or the
        // first of `activeMotionGraphIds` (the web reads the same two).
        let metadata = bundle.get("metadata");
        let active_program_id = metadata
            .and_then(|m| m.get("activeMotionGraphId"))
            .and_then(Json::as_str)
            .or_else(|| {
                metadata
                    .and_then(|m| m.get("activeMotionGraphIds"))
                    .and_then(Json::as_array)
                    .and_then(|ids| ids.first())
                    .and_then(Json::as_str)
            })
            .map(str::to_string);

        let mut graphs = Vec::new();
        let mut programs = Vec::new();
        for entry in bundle
            .get("graphs")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            let kind = entry
                .get("kind")
                .and_then(Json::as_str)
                .unwrap_or("unknown")
                .to_string();
            let Some(spec) = entry.get("spec") else {
                continue;
            };
            // Motiongraphs are the playable programs, addressed by id; the base
            // graphs (rig, pose-driver) compose by kind.
            if kind == "motiongraph" {
                if let Some(id) = entry.get("id").and_then(Json::as_str) {
                    programs.push((id.to_string(), spec.clone()));
                }
            }
            graphs.push((kind, spec.clone()));
        }

        let mut neutral_inputs = HashMap::new();
        if let Some(neutral) = bundle
            .pointer("/poses/config/neutralInputs")
            .and_then(Json::as_object)
        {
            for (name, value) in neutral {
                if let Some(n) = value.as_f64() {
                    neutral_inputs.insert(name.clone(), n);
                }
            }
        }

        Bundle {
            graphs,
            programs,
            active_program_id,
            neutral_inputs,
        }
    }

    /// The `(id, spec)` of the program `select` names, if any.
    pub fn program(&self, select: &ProgramSelect) -> Option<&(String, Json)> {
        let id = match select {
            ProgramSelect::None => return None,
            ProgramSelect::Auto => self.active_program_id.as_deref()?,
            ProgramSelect::Id(id) => id.as_str(),
        };
        self.programs.iter().find(|(pid, _)| pid == id)
    }

    /// Compose the device's behavior: the base graphs whose kind is in `wanted`,
    /// then the chosen program, then (when `with_animations`) the animation
    /// source — each **last** wins over the earlier ones on any store path they
    /// share (the web composes the same way, for the same last-writer-wins
    /// reason). So a playing program overrides the base rig, and a playing clip
    /// overrides both. Returns the composed graph spec.
    ///
    /// Pass `with_animations` when the device has the animation module loaded;
    /// [`animations_source`] dispatches to it, so composing it without the module
    /// would leave an `ExternalFunction` node with nothing to call.
    pub fn compose(
        &self,
        wanted: &[&str],
        select: &ProgramSelect,
        with_animations: bool,
    ) -> Result<Json> {
        let mut sources: Vec<(String, Json)> = self
            .graphs
            .iter()
            .filter(|(kind, _)| wanted.contains(&kind.as_str()))
            .map(|(kind, spec)| (kind.clone(), spec.clone()))
            .collect();
        if let Some((id, spec)) = self.program(select) {
            log::info!("autoplaying program {id}");
            sources.push((format!("program::{id}"), spec.clone()));
        }
        if with_animations {
            sources.push(animations_source());
        }
        compose_sources(&sources)
    }

    /// The store writes that stage the face's neutral pose: each `neutralInputs`
    /// entry resolved to its rig input node's store path, as `(path, value)`.
    /// Names that don't resolve to a rig input are skipped — the same tolerance
    /// as the web's `stagePoseNeutral`. Empty when there is no neutral config.
    pub fn neutral_stage_writes(&self) -> Vec<(String, f32)> {
        if self.neutral_inputs.is_empty() {
            return Vec::new();
        }
        let Some((_, rig)) = self.graphs.iter().find(|(kind, _)| kind == "rig") else {
            return Vec::new();
        };
        let map = collect_input_path_map(rig);
        self.neutral_inputs
            .iter()
            .filter_map(|(name, value)| map.get(name).map(|path| (path.clone(), *value as f32)))
            .collect()
    }
}

/// Source id of the animation source (see [`compose_sources`] for how source
/// ids namespace node ids).
pub const ANIMATIONS_SOURCE_ID: &str = "animations";

/// Store path the animation source writes the module's per-tick `[PlayerState]`
/// feedback to. A plain store key (not an `arora/` built-in), so it carries
/// over a bridge and across a device restart like any other value.
pub const ANIMATION_PLAYERS_PATH: &str = "vizij/animations/players";

// The animation module's declared ids (mirror `module.yaml` / the web host's
// `ANIMATION_MODULE_*`). The graph carries them as opaque handles: the
// `ExternalFunction` nodes name the module functions, the `output` node the
// `TrackOutput` fields it fans out by.
const FN_STEP: &str = "76697a69-6a00-0000-0f00-000000000004";
const FN_PLAYER_STATES: &str = "76697a69-6a00-0000-0f00-00000000000d";
const PARAM_DT_NS: &str = "76697a69-6a00-0000-0f04-000000000001";
const FIELD_OUTPUT_DEFAULT_KEY: &str = "76697a69-6a00-0000-0110-000000000002";
const FIELD_OUTPUT_VALUE: &str = "76697a69-6a00-0000-0110-000000000003";

/// The graph source that ticks the animation module **inside the device** (the
/// native port of the web host's `animationsGraphSource`): an `ExternalFunction`
/// node calls the module's `step` every tick, fed the runtime's built-in
/// `arora/dt` (nanoseconds), and a path-less `output` node fans the returned
/// `[TrackOutput]` batch onto the store keys each record names — its
/// `default_key`, the final rig paths decided at clip load. A second
/// `ExternalFunction` node writes `player_states()` to [`ANIMATION_PLAYERS_PATH`].
///
/// The source is inert until a clip plays: with no instances the module's `step`
/// returns nothing, so the `output` writes nothing and the rig/program pose
/// stands. Transport (load a clip, play/pause/seek/…) is driven through the
/// module's exported functions — over a bridge, or in-process — not from here.
pub fn animations_source() -> (String, Json) {
    let spec = json!({
        "nodes": [
            { "id": "dt", "type": "input", "params": { "path": "arora/dt" } },
            {
                "id": "step",
                "type": "externalfunction",
                "params": { "function": FN_STEP, "param_ids": [PARAM_DT_NS] },
            },
            {
                "id": "apply",
                "type": "output",
                "params": {
                    "key_field": FIELD_OUTPUT_DEFAULT_KEY,
                    "value_field": FIELD_OUTPUT_VALUE,
                },
            },
            {
                "id": "states",
                "type": "externalfunction",
                "params": { "function": FN_PLAYER_STATES, "param_ids": [] },
            },
            { "id": "states-out", "type": "output", "params": { "path": ANIMATION_PLAYERS_PATH } },
        ],
        "edges": [
            { "from": { "node_id": "dt" }, "to": { "node_id": "step", "input": "args_0" } },
            { "from": { "node_id": "step" }, "to": { "node_id": "apply", "input": "in" } },
            { "from": { "node_id": "states" }, "to": { "node_id": "states-out", "input": "in" } },
        ],
    });
    (ANIMATIONS_SOURCE_ID.to_string(), spec)
}

/// Union several Vizij graph specs into the one graph a device runs.
///
/// The device runs a single graph as its behavior, so separate sources become a
/// union of nodes and edges. Node ids are prefixed `{source_id}::` so sources
/// can't collide; `params.path` is deliberately **not** prefixed — path identity
/// on the device's shared store is the cross-source contract (a pose written to
/// a store path is read by a rig input with the same path next tick). Each spec
/// is normalized first (legacy input-connection forms become edges) so id
/// rewriting sees the canonical `nodes`/`edges` shape.
///
/// Output-path collisions across sources (two `output` nodes writing one path)
/// are warned and resolved last-writer-wins by source order — the same tolerance
/// as the web's `composeGraphSpecs`; current bundles don't collide.
pub fn compose_sources(sources: &[(String, Json)]) -> Result<Json> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut output_owner: HashMap<String, String> = HashMap::new();
    for (source_id, spec) in sources {
        let mut normalized = spec.clone();
        normalize_graph_spec_value(&mut normalized)
            .map_err(|e| anyhow!("source {source_id:?} does not normalize: {e:?}"))?;

        for node in nodes_of(&normalized) {
            let Some(path) = output_path(node) else {
                continue;
            };
            if let Some(owner) = output_owner.get(&path) {
                if owner != source_id {
                    log::warn!(
                        "output path {path:?} is written by both {owner:?} and {source_id:?}; \
                         last writer wins ({source_id:?})"
                    );
                }
            }
            output_owner.insert(path, source_id.clone());
        }

        let prefix = format!("{source_id}::");
        for node in nodes_of(&normalized) {
            let mut node = node.clone();
            if let Some(id) = node.get("id").and_then(Json::as_str) {
                node["id"] = json!(format!("{prefix}{id}"));
            }
            nodes.push(node);
        }
        for edge in edges_of(&normalized) {
            let mut edge = edge.clone();
            for end in ["from", "to"] {
                if let Some(id) = edge
                    .get(end)
                    .and_then(|e| e.get("node_id"))
                    .and_then(Json::as_str)
                {
                    edge[end]["node_id"] = json!(format!("{prefix}{id}"));
                }
            }
            edges.push(edge);
        }
    }
    Ok(json!({ "nodes": nodes, "edges": edges }))
}

fn nodes_of(spec: &Json) -> impl Iterator<Item = &Json> {
    spec.get("nodes")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
}

fn edges_of(spec: &Json) -> impl Iterator<Item = &Json> {
    spec.get("edges")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
}

/// The trimmed `params.path` of an `output` node, if this node is one.
fn output_path(node: &Json) -> Option<String> {
    let is_output = node
        .get("type")
        .and_then(Json::as_str)
        .is_some_and(|t| t.eq_ignore_ascii_case("output"));
    if !is_output {
        return None;
    }
    node.get("params")
        .and_then(|p| p.get("path"))
        .and_then(Json::as_str)
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
}

/// Map a graph's input nodes to their store paths, keyed the way the bundle's
/// `neutralInputs` names them — the input node id with its `input_` prefix
/// dropped, plus the `direct_`/`pose_control_` aliases (a port of the web host's
/// `collectInputPathMap`). First writer wins per key, except a `direct_` alias
/// overrides.
pub fn collect_input_path_map(spec: &Json) -> HashMap<String, String> {
    fn add(map: &mut HashMap<String, String>, key: &str, path: &str, force: bool) {
        if key.is_empty() || (!force && map.contains_key(key)) {
            return;
        }
        map.insert(key.to_string(), path.to_string());
    }
    let mut map = HashMap::new();
    for node in nodes_of(spec) {
        let is_input = node
            .get("type")
            .and_then(Json::as_str)
            .is_some_and(|t| t.eq_ignore_ascii_case("input"));
        if !is_input {
            continue;
        }
        let Some(path) = node
            .get("params")
            .and_then(|p| p.get("path"))
            .and_then(Json::as_str)
            .map(str::trim)
            .filter(|p| !p.is_empty())
        else {
            continue;
        };
        let id = node.get("id").and_then(Json::as_str).unwrap_or("");
        let key = id
            .strip_prefix("input_")
            .unwrap_or(if id.is_empty() { path } else { id });
        add(&mut map, key, path, false);
        if let Some(rest) = key.strip_prefix("direct_") {
            add(&mut map, rest, path, true);
        }
        if let Some(rest) = key.strip_prefix("pose_control_") {
            add(&mut map, rest, path, false);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(nodes: Json, edges: Json) -> Json {
        json!({ "nodes": nodes, "edges": edges })
    }

    #[test]
    fn compose_prefixes_node_ids_but_shares_paths() {
        let a = graph(
            json!([{ "id": "n", "type": "output", "params": { "path": "rig/x" } }]),
            json!([]),
        );
        let b = graph(
            json!([{ "id": "n", "type": "input", "params": { "path": "rig/x" } }]),
            json!([]),
        );
        let composed = compose_sources(&[("a".into(), a), ("b".into(), b)]).unwrap();
        let ids: Vec<&str> = composed["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect();
        // Ids are namespaced per source; the shared store path is untouched.
        assert_eq!(ids, vec!["a::n", "b::n"]);
        assert_eq!(composed["nodes"][0]["params"]["path"], "rig/x");
        assert_eq!(composed["nodes"][1]["params"]["path"], "rig/x");
    }

    fn bundle_json() -> Json {
        json!({
            "metadata": { "activeMotionGraphId": "prog.speaks" },
            "graphs": [
                { "id": "the_rig", "kind": "rig", "spec": graph(
                    json!([{ "id": "input_gaze_x", "type": "input", "params": { "path": "rig/gaze/x" } }]),
                    json!([]),
                ) },
                { "id": "prog.speaks", "kind": "motiongraph", "spec": graph(
                    json!([{ "id": "o", "type": "output", "params": { "path": "rig/gaze/x" } }]),
                    json!([]),
                ) },
                { "id": "prog.live", "kind": "motiongraph", "spec": graph(json!([]), json!([])) }
            ],
            "poses": { "config": { "neutralInputs": { "gaze_x": 0.25, "missing": 1.0 } } }
        })
    }

    #[test]
    fn bundle_reads_programs_and_active_id() {
        let b = Bundle::from_bundle_json(&bundle_json());
        assert_eq!(b.active_program_id.as_deref(), Some("prog.speaks"));
        assert_eq!(b.programs.len(), 2);
        assert_eq!(b.program(&ProgramSelect::Auto).unwrap().0, "prog.speaks");
        assert_eq!(
            b.program(&ProgramSelect::Id("prog.live".into())).unwrap().0,
            "prog.live"
        );
        assert!(b.program(&ProgramSelect::None).is_none());
        assert!(b.program(&ProgramSelect::Id("nope".into())).is_none());
    }

    #[test]
    fn compose_appends_the_active_program_last() {
        let b = Bundle::from_bundle_json(&bundle_json());
        let composed = b
            .compose(&["rig", "pose-driver"], &ProgramSelect::Auto, false)
            .unwrap();
        let ids: Vec<&str> = composed["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect();
        // Rig source first, program source last (last writer wins on rig/gaze/x).
        assert_eq!(ids, vec!["rig::input_gaze_x", "program::prog.speaks::o"]);

        // No autoplay → base graphs only.
        let base = b.compose(&["rig"], &ProgramSelect::None, false).unwrap();
        assert_eq!(base["nodes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn compose_appends_the_animation_source_last() {
        let b = Bundle::from_bundle_json(&bundle_json());
        let composed = b.compose(&["rig"], &ProgramSelect::None, true).unwrap();
        let ids: Vec<String> = composed["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap().to_string())
            .collect();
        // The rig source, then the animation source's nodes, namespaced by its
        // source id — its `step`/`player_states` dispatch drives the module.
        assert!(ids.contains(&"rig::input_gaze_x".to_string()));
        assert!(ids.contains(&"animations::step".to_string()));
        assert!(ids.contains(&"animations::apply".to_string()));
        assert!(ids.contains(&"animations::states-out".to_string()));
    }

    #[test]
    fn neutral_writes_resolve_through_the_rig_input_map() {
        let b = Bundle::from_bundle_json(&bundle_json());
        let writes = b.neutral_stage_writes();
        // "gaze_x" resolves via input_gaze_x → rig/gaze/x; "missing" is dropped.
        assert_eq!(writes, vec![("rig/gaze/x".to_string(), 0.25_f32)]);
    }
}
