//! Portable Vizij host glue — the spec/data transforms that sit *above* the
//! device, shared by the native app (`vizij`) and the browser runtime
//! (`vizij-arora-web`, driven by `@vizij/runtime-react`):
//!
//! - [`compose_sources`] unions several graph sources into the one graph a
//!   device runs (the rig, the pose-driver, a playing program, …).
//! - [`Bundle`] reads the face's `VIZIJ_bundle`: its graphs, its motiongraph
//!   programs, the profiles it declares, the program to autoplay, the
//!   neutral-pose config, and what an app builds its controls from — the
//!   poses and their groups, the rig's inputs, the animations
//!   ([`contents`]) and the bundle's open-ended metadata.
//! - [`ProgramSelect`] picks which program plays; [`Bundle::compose`] composes
//!   the base graphs plus that program.
//! - [`Bundle::neutral_stage_writes`] resolves the neutral inputs to the store
//!   writes that stage the face's resting pose.
//!
//! There is no renderer and no device lifecycle here — those stay in each host
//! (Bevy + a worker thread natively; three.js + a Web Worker in the browser).
//! This is only the logic both hosts would otherwise write twice, once in Rust
//! and once in TypeScript.

pub mod captions;
pub mod contents;
#[cfg(feature = "publish-diagnostics")]
pub mod diagnostics;
#[cfg(feature = "publish-frames")]
pub mod frames;
mod graph_builder;
pub mod mappings;
pub mod profile;
pub mod ros4hri;
pub mod skills;
pub mod standard;

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use arora_shape::json::normalize_graph_spec_value;
use serde_json::{json, Value as Json};

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
    /// The `id` each of [`graphs`](Bundle::graphs)' entries carries, by
    /// position: `graph_ids[i]` is `graphs[i]`'s, `None` for an entry
    /// without one.
    pub graph_ids: Vec<Option<String>>,
    /// Standard mappings embedded in the face, `(mapping id, spec)` — the
    /// [`STANDARD_MAPPING_KIND`](mappings::STANDARD_MAPPING_KIND) graph
    /// entries (ids `standard::<mapping>`, prefix stripped). An embedded copy
    /// is the author's pinned override of the shipped mapping:
    /// [`compose`](Bundle::compose) loads it and suppresses the built-in
    /// mapping of the same id.
    pub standard_mappings: Vec<(String, Json)>,
    /// Skill fragments embedded in the face, `(skill id, spec)` — the `skill`
    /// graph entries (ids `skill::<function>`, prefix stripped). An embedded
    /// copy is the author's pinned override of the shipped behavior: the
    /// device registers it as the function's task fragment instead of the
    /// built-in. Never composed — a fragment grafts per run.
    pub skills: Vec<(String, Json)>,
    /// The motiongraph programs, `(id, spec)` — the graphs the face can play on
    /// top of its rig (e.g. Quori's "Speaks").
    pub programs: Vec<(String, Json)>,
    /// Program id → the `label` its graph entry carries, for the programs
    /// that have one.
    pub program_labels: HashMap<String, String>,
    /// The profiles this face declares it implements — the interfaces its
    /// graphs are authored against, carried in the bundle's top-level
    /// `profiles` array. A profile is an interface, not a graph, so it sits
    /// beside `graphs` rather than inside it. Declaring them is what lets a
    /// coverage check know which interface to hold the face to.
    pub profiles: Vec<profile::Profile>,
    /// `metadata.activeMotionGraphId` (or the first `activeMotionGraphIds`).
    pub active_program_id: Option<String>,
    /// `poses.config.neutralInputs` — input name → neutral value.
    pub neutral_inputs: HashMap<String, f64>,
    /// `metadata.faceId` — the rig's namespace (its input paths live under
    /// `rig/<faceId>/`).
    pub face_id: Option<String>,
    /// `poses.config.poses`.
    pub poses: Vec<contents::Pose>,
    /// `poses.config.poseGroups`.
    pub pose_groups: Vec<contents::PoseGroup>,
    /// The inputs the first `rig` graph declares (its spec's
    /// `metadata.vizij.inputs`).
    pub rig_inputs: Vec<contents::RigInput>,
    /// The authored `animations`.
    pub animations: Vec<contents::Animation>,
    /// `metadata` as authored — open-ended: the face id, the speech
    /// configuration (`speechConfig`), the active motion graph, exporter
    /// details. `None` when the bundle has none.
    pub metadata: Option<Json>,
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
        let mut graph_ids = Vec::new();
        let mut programs = Vec::new();
        let mut program_labels = HashMap::new();
        let mut standard_mappings = Vec::new();
        let mut skills = Vec::new();
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
            // graphs (rig, pose-driver) compose by kind. Embedded standard
            // mappings are held apart, by mapping id — they compose always and
            // suppress their built-in (see `compose`), never selected by kind.
            if kind == "motiongraph" {
                if let Some(id) = entry.get("id").and_then(Json::as_str) {
                    programs.push((id.to_string(), spec.clone()));
                    if let Some(label) = entry.get("label").and_then(Json::as_str) {
                        program_labels.insert(id.to_string(), label.to_string());
                    }
                }
            }
            if kind == mappings::STANDARD_MAPPING_KIND {
                let id = entry.get("id").and_then(Json::as_str).unwrap_or_default();
                let mapping_id = id.strip_prefix("standard::").unwrap_or(id);
                standard_mappings.push((mapping_id.to_string(), spec.clone()));
                continue;
            }
            // Embedded skill fragments are held apart by skill id: they never
            // compose (a fragment grafts per run) — the device registers them
            // as task fragments, overriding the built-in of the same id.
            if kind == skills::SKILL_KIND {
                let id = entry.get("id").and_then(Json::as_str).unwrap_or_default();
                let skill_id = id.strip_prefix("skill::").unwrap_or(id);
                skills.push((skill_id.to_string(), spec.clone()));
                continue;
            }
            graphs.push((kind, spec.clone()));
            graph_ids.push(entry.get("id").and_then(Json::as_str).map(str::to_string));
        }

        // Profiles are declared data, not graphs — a malformed entry is
        // warned and skipped rather than failing the whole bundle, so an older
        // reader meeting a newer profile shape still loads the face.
        let profiles: Vec<profile::Profile> = bundle
            .get("profiles")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entry| match serde_json::from_value(entry.clone()) {
                Ok(profile) => Some(profile),
                Err(e) => {
                    let id = entry.get("id").and_then(Json::as_str).unwrap_or("?");
                    log::warn!("skipping the bundle's malformed profile {id:?}: {e}");
                    None
                }
            })
            .collect();

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

        let face_id = metadata
            .and_then(|m| m.get("faceId"))
            .and_then(Json::as_str)
            .map(str::to_string);

        let pose_groups = contents::pose_groups(bundle);
        let poses = contents::poses(bundle, &pose_groups);
        let rig_inputs = graphs
            .iter()
            .find(|(kind, _)| kind == "rig")
            .map(|(_, spec)| contents::rig_inputs(spec))
            .unwrap_or_default();

        Bundle {
            graphs,
            graph_ids,
            standard_mappings,
            skills,
            programs,
            program_labels,
            profiles,
            active_program_id,
            neutral_inputs,
            face_id,
            poses,
            pose_groups,
            rig_inputs,
            animations: contents::animations(bundle),
            metadata: metadata.cloned(),
        }
    }

    /// The prefix of this face's rig input paths — `rig/<faceId>/`, or empty
    /// when the bundle declares no face id. Standard mappings prepend it to
    /// the control paths they write, and a face-scoped profile to every path
    /// it declares.
    pub fn rig_prefix(&self) -> String {
        self.face_id
            .as_deref()
            .map(|id| format!("rig/{id}/"))
            .unwrap_or_default()
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
    /// then the standard mappings, then (when `with_animations`) the animation
    /// source, then the chosen program — each **last** wins over the earlier
    /// ones on any store path they share, the order the composed spec lists
    /// them being the order it evaluates in. So a mapping overrides the base
    /// rig's resting writes, a playing animation overrides the mapping, and a
    /// playing program overrides everything, as a program or skill a device
    /// runs as a task run does. Returns the composed graph spec.
    ///
    /// Mappings come from two places: the face's own embedded copies
    /// ([`standard_mappings`](Bundle::standard_mappings)), always composed,
    /// and the built-in `mappings` the host passes (e.g.
    /// [`ros4hri::ros4hri_source`]) — skipped for any id the face embeds,
    /// because an embedded copy is the author's pinned override of the shipped
    /// mapping.
    ///
    /// Pass `with_animations` when the device has the animation module loaded;
    /// [`animations_source`] dispatches to it, so composing it without the module
    /// would leave an `ExternalFunction` node with nothing to call.
    pub fn compose(
        &self,
        wanted: &[&str],
        select: &ProgramSelect,
        with_animations: bool,
        mappings: &[(String, Json)],
    ) -> Result<Json> {
        compose_sources(&self.sources(wanted, select, with_animations, mappings))
    }

    /// The graph sources [`compose`](Bundle::compose) unions, `(source id,
    /// spec)` in composition order: the base graphs of the `wanted` kinds,
    /// the face's embedded mappings, the built-in `mappings` it does not
    /// embed, the animation source, and the `select`ed program. What
    /// [`contended_outputs`] reads.
    ///
    /// A mapping reads the face's settings ([`standard::FACE_SETTINGS`]) at
    /// the face's rest: every input of a mapping on a setting rests at the
    /// default a base graph authors for it, when that is of the setting's
    /// type, else at the `vizij-face` profile's rest — never at the
    /// mapping's own default. A key has one rest, and a setting's is the
    /// face's: the mapping is shared by every face, the setting says what
    /// this one is like.
    pub fn sources(
        &self,
        wanted: &[&str],
        select: &ProgramSelect,
        with_animations: bool,
        mappings: &[(String, Json)],
    ) -> Vec<(String, Json)> {
        let mut sources: Vec<(String, Json)> = self
            .graphs
            .iter()
            .filter(|(kind, _)| wanted.contains(&kind.as_str()))
            .map(|(kind, spec)| (kind.clone(), spec.clone()))
            .collect();
        let settings = self.setting_rests(&sources);
        // The face's own embedded mappings compose unconditionally, and each
        // suppresses the built-in mapping of the same id: an embedded copy is
        // the author's pinned override of the shipped mapping.
        for (id, spec) in &self.standard_mappings {
            sources.push((mappings::embedded_graph_id(id), resting_at(spec, &settings)));
        }
        for (id, spec) in mappings {
            if self.standard_mappings.iter().any(|(e, _)| e == id) {
                log::info!("embedded standard mapping {id} overrides the built-in");
                continue;
            }
            sources.push((id.clone(), resting_at(spec, &settings)));
        }
        if with_animations {
            sources.push(animations_source());
        }
        if let Some((id, spec)) = self.program(select) {
            log::info!("autoplaying program {id}");
            sources.push((format!("program::{id}"), spec.clone()));
        }
        sources
    }

    /// The rest each of the face's settings takes, by store path under the
    /// face's prefix: the default the first input node of `base` reading it
    /// authors, read as composition normalizes it, when it is of the type the
    /// `vizij-face` profile states for the setting — else the profile's rest.
    /// The same rule the device states the setting's rest by, so the mapping
    /// and the store agree on it before a reset and after.
    fn setting_rests(&self, base: &[(String, Json)]) -> HashMap<String, Json> {
        let prefix = self.rig_prefix();
        let base: Vec<Json> = base
            .iter()
            .filter_map(|(_, spec)| {
                let mut spec = spec.clone();
                normalize_graph_spec_value(&mut spec).ok()?;
                Some(spec)
            })
            .collect();
        let face = profile::vizij_face_profile();
        standard::FACE_SETTINGS
            .iter()
            .filter_map(|setting| {
                let key = face.keys.iter().find(|key| key.path == *setting)?;
                let ty = key.value_type.as_ref()?;
                let path = format!("{prefix}{setting}");
                let authored = base
                    .iter()
                    .flat_map(nodes_of)
                    .filter(|node| is_input(node))
                    .find(|node| input_path(node) == Some(path.as_str()))
                    .and_then(|node| node.pointer("/params/value"))
                    .filter(|value| !value.is_null())
                    .filter(|value| {
                        arora_shape::json::parse_value((*value).clone())
                            .is_ok_and(|value| value.conforms_to(ty))
                    })
                    .cloned();
                let rest = match authored {
                    Some(rest) => rest,
                    None => serde_json::to_value(key.default_value.as_ref()?).ok()?,
                };
                Some((path, rest))
            })
            .collect()
    }

    /// Every store path the bundle's graphs read (their `input` nodes) — its
    /// base graphs, programs, embedded mappings and skills — with the rig
    /// prefix stripped: the face's input surface in the standard's terms,
    /// what [`profile::coverage`] grades.
    pub fn input_paths(&self) -> Vec<String> {
        let prefix = self.rig_prefix();
        let mut paths: Vec<String> = self
            .graphs
            .iter()
            .chain(&self.standard_mappings)
            .chain(&self.skills)
            .flat_map(|(_, spec)| nodes_of(spec))
            .filter(|node| {
                node.get("type")
                    .and_then(Json::as_str)
                    .is_some_and(|t| t.eq_ignore_ascii_case("input"))
            })
            .filter_map(|node| node.pointer("/params/path").and_then(Json::as_str))
            .map(|path| path.strip_prefix(&prefix).unwrap_or(path).to_string())
            .collect();
        paths.sort();
        paths.dedup();
        paths
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

    /// How this face's animation channels name store keys: through its first
    /// `rig` graph's input nodes, under its [`rig_prefix`](Bundle::rig_prefix).
    pub fn channel_keys(&self) -> ChannelKeys {
        let rig = self.graphs.iter().find(|(kind, _)| kind == "rig");
        ChannelKeys {
            prefix: self.rig_prefix(),
            inputs: rig
                .map(|(_, spec)| {
                    nodes_of(spec)
                        .filter(|node| {
                            node.get("type")
                                .and_then(Json::as_str)
                                .is_some_and(|t| t.eq_ignore_ascii_case("input"))
                        })
                        .filter_map(|node| node.pointer("/params/path").and_then(Json::as_str))
                        .map(|path| path.trim().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            by_name: rig
                .map(|(_, spec)| collect_input_path_map(spec))
                .unwrap_or_default(),
        }
    }
}

/// How an animation track's `channel` names the store key it drives on one
/// face, resolved through the face's rig — [`Bundle::channel_keys`].
///
/// A channel is rig-relative (`gaze/left_right`, `poses/<id>.weight`), so its
/// key is the rig input at `<rig prefix><channel>`. A channel that is no such
/// input resolves, in order: to itself when it already is a rig input path;
/// to the input the rig names it by (its node id, as
/// [`collect_input_path_map`] reads them); else to `<rig prefix><channel>`
/// regardless, a key some other graph may read. A channel already under
/// `rig/` is never prefixed again.
#[derive(Debug, Clone, Default)]
pub struct ChannelKeys {
    prefix: String,
    inputs: std::collections::HashSet<String>,
    by_name: HashMap<String, String>,
}

impl ChannelKeys {
    /// The store key `channel` drives.
    pub fn key(&self, channel: &str) -> String {
        let channel = channel.trim().trim_start_matches('/');
        if self.inputs.contains(channel) {
            return channel.to_string();
        }
        let prefixed = format!("{}{channel}", self.prefix);
        if self.inputs.contains(&prefixed) {
            return prefixed;
        }
        if let Some(path) = self.by_name.get(channel) {
            return path.clone();
        }
        if channel.starts_with("rig/") {
            channel.to_string()
        } else {
            prefixed
        }
    }
}

/// Source id of the animation source (see [`compose_sources`] for how source
/// ids namespace node ids).
pub const ANIMATIONS_SOURCE_ID: &str = "animations";

/// Store path the animation source writes the module's per-tick `[PlayerState]`
/// feedback to. A plain store key (not an `arora/` built-in), so it carries
/// over a bridge and across a device restart like any other value.
pub const ANIMATION_PLAYERS_PATH: &str = "vizij/animations/players";

// The animation module's declared ids (its Rust declaration, `animation` in
// arora-animation-module, which this crate does not depend on; the `vizij`
// crate tests that they match). The graph carries them as opaque
// handles: the `ExternalFunction` nodes name the module functions, the
// `output` node the `TrackOutput` fields it fans out by.
const FN_STEP: &str = "76697a69-6a00-0000-0f00-000000000004";
const FN_PLAYER_STATES: &str = "76697a69-6a00-0000-0f00-00000000000d";
const PARAM_DT_NS: &str = "76697a69-6a00-0000-0f04-000000000001";
const PARAM_TIME_NS: &str = "76697a69-6a00-0000-0f04-000000000002";
const FIELD_OUTPUT_DEFAULT_KEY: &str = "76697a69-6a00-0000-0110-000000000002";
const FIELD_OUTPUT_VALUE: &str = "76697a69-6a00-0000-0110-000000000003";

/// The graph source that ticks the animation module **inside the device**: an
/// `ExternalFunction` node calls the module's `step` every tick, fed the
/// runtime's built-in `arora/dt` and `arora/time` (nanoseconds), so the
/// module's players stand where the device's clock puts them — `play_at`
/// counts on it, and a step at a zero `dt` moves none of them — and a
/// path-less `output` node fans the returned `[TrackOutput]` batch onto the
/// store keys each record names, its `default_key`: the final rig paths
/// decided when an animation loads, each written in the type the store
/// declares for it. A second `ExternalFunction` node writes `player_states()`
/// to [`ANIMATION_PLAYERS_PATH`].
///
/// The source is inert until an animation plays: with no instance of weight
/// the module's `step` returns nothing — an animation loads silent, its
/// instance at weight 0 — so the `output` writes nothing and the
/// rig/program pose stands. Loading and transport (load/unload, play/pause/
/// seek/…, an instance's weight) are driven through the module's declared
/// functions — over a bridge, or in-process — not from here.
pub fn animations_source() -> (String, Json) {
    use arora_behavior::built_in;
    let spec = json!({
        "nodes": [
            { "id": "dt", "type": "input", "params": { "path": built_in::DT } },
            { "id": "time", "type": "input", "params": { "path": built_in::TIME } },
            {
                "id": "step",
                "type": "externalfunction",
                "params": { "function": FN_STEP, "param_ids": [PARAM_DT_NS, PARAM_TIME_NS] },
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
            { "from": { "node_id": "time" }, "to": { "node_id": "step", "input": "args_1" } },
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
/// Output-path collisions across sources (two `output` nodes writing one path,
/// [`contended_outputs`]) are warned and resolved last-writer-wins by source
/// order — the same tolerance as the web's `composeGraphSpecs`.
pub fn compose_sources(sources: &[(String, Json)]) -> Result<Json> {
    for contention in contended_outputs(sources) {
        log::warn!("{contention}");
    }
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for (source_id, spec) in sources {
        let mut normalized = spec.clone();
        normalize_graph_spec_value(&mut normalized)
            .map_err(|e| anyhow!("source {source_id:?} does not normalize: {e:?}"))?;

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

/// A store path written by more than one source of a composition: only the
/// last source's write lands, so the others' motion on it never shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contention {
    pub path: String,
    /// The sources writing it, in composition order: the last one wins.
    pub sources: Vec<String>,
}

impl std::fmt::Display for Contention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let winner = self.sources.last().map(String::as_str).unwrap_or_default();
        write!(
            f,
            "output path {:?} is written by {}; last writer wins ({winner:?})",
            self.path,
            self.sources
                .iter()
                .map(|source| format!("{source:?}"))
                .collect::<Vec<_>>()
                .join(" and ")
        )
    }
}

/// The paths more than one of `sources` writes (its `output` nodes), by path,
/// each with its writers in composition order — what [`compose_sources`]
/// resolves last-writer-wins.
pub fn contended_outputs(sources: &[(String, Json)]) -> Vec<Contention> {
    let mut writers: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for (source_id, spec) in sources {
        for path in nodes_of(spec).filter_map(output_path) {
            let sources = writers.entry(path).or_default();
            if !sources.contains(source_id) {
                sources.push(source_id.clone());
            }
        }
    }
    writers
        .into_iter()
        .filter(|(_, sources)| sources.len() > 1)
        .map(|(path, sources)| Contention { path, sources })
        .collect()
}

/// `spec` with each input node on a path of `rests` resting at its value.
fn resting_at(spec: &Json, rests: &HashMap<String, Json>) -> Json {
    let mut spec = spec.clone();
    if rests.is_empty() {
        return spec;
    }
    for node in spec
        .get_mut("nodes")
        .and_then(Json::as_array_mut)
        .into_iter()
        .flatten()
    {
        if !is_input(node) {
            continue;
        }
        let Some(rest) = input_path(node).and_then(|path| rests.get(path)) else {
            continue;
        };
        let rest = rest.clone();
        if let Some(params) = node.get_mut("params").and_then(Json::as_object_mut) {
            params.insert("value".to_string(), rest);
        }
    }
    spec
}

/// Whether `node` is an `input` node.
fn is_input(node: &Json) -> bool {
    node.get("type")
        .and_then(Json::as_str)
        .is_some_and(|t| t.eq_ignore_ascii_case("input"))
}

/// The trimmed `params.path` an input node reads.
fn input_path(node: &Json) -> Option<&str> {
    node.pointer("/params/path")
        .and_then(Json::as_str)
        .map(str::trim)
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

    /// A path two sources write is contended, its writers in composition
    /// order; a path one source writes twice, or one source writes and
    /// another reads, is not.
    #[test]
    fn contended_outputs_name_each_path_and_its_writers() {
        let output = |id: &str, path: &str| json!({ "id": id, "type": "output", "params": { "path": path } });
        let a = graph(
            json!([
                output("x", "rig/x"),
                output("y", "rig/y"),
                output("y2", "rig/y")
            ]),
            json!([]),
        );
        let b = graph(
            json!([
                output("x", "rig/x"),
                { "id": "y", "type": "input", "params": { "path": "rig/y" } }
            ]),
            json!([]),
        );
        let c = graph(json!([output("x", "rig/x")]), json!([]));
        let contended = contended_outputs(&[("a".into(), a), ("b".into(), b), ("c".into(), c)]);
        assert_eq!(
            contended,
            [Contention {
                path: "rig/x".into(),
                sources: vec!["a".into(), "b".into(), "c".into()],
            }]
        );
        assert_eq!(
            contended[0].to_string(),
            r#"output path "rig/x" is written by "a" and "b" and "c"; last writer wins ("c")"#
        );
    }

    /// A face's input surface is every path its graphs read, embedded
    /// mappings and skills included, with its rig prefix stripped.
    #[test]
    fn a_bundle_s_input_paths_are_its_graphs_reads_unprefixed() {
        let input = |path: &str| json!({ "id": path, "type": "input", "params": { "path": path } });
        let bundle = Bundle::from_bundle_json(&json!({
            "metadata": { "faceId": "f" },
            "graphs": [
                { "kind": "rig", "spec": graph(json!([input("rig/f/standard/vizij/blink")]), json!([])) },
                { "kind": "standard-profile", "id": "standard::ros4hri",
                  "spec": graph(json!([input("standard/ros4hri/viseme")]), json!([])) },
                { "kind": "skill", "id": "skill::look_at",
                  "spec": graph(json!([input("rig/f/standard/vizij/blink")]), json!([])) },
            ],
        }));
        assert_eq!(
            bundle.input_paths(),
            ["standard/ros4hri/viseme", "standard/vizij/blink"]
        );
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

    /// An animation channel resolves through the face's rig: the rig input
    /// under the rig prefix, an input path as is, an input by its node name,
    /// else the prefixed path.
    #[test]
    fn a_channel_resolves_to_the_rig_input_it_names() {
        let mut bundle = bundle_json();
        bundle["metadata"]["faceId"] = json!("quori");
        bundle["graphs"][0]["spec"] = graph(
            json!([
                { "id": "input_direct_gaze_left_right", "type": "input",
                  "params": { "path": "rig/quori/gaze/left_right" } },
                { "id": "input_mouth_smile", "type": "input",
                  "params": { "path": "rig/quori/mouth/smile_amount" } },
            ]),
            json!([]),
        );
        let keys = Bundle::from_bundle_json(&bundle).channel_keys();
        assert_eq!(keys.key("gaze/left_right"), "rig/quori/gaze/left_right");
        assert_eq!(keys.key("/gaze/left_right"), "rig/quori/gaze/left_right");
        assert_eq!(
            keys.key("rig/quori/gaze/left_right"),
            "rig/quori/gaze/left_right"
        );
        assert_eq!(keys.key("mouth_smile"), "rig/quori/mouth/smile_amount");
        assert_eq!(keys.key("gaze_left_right"), "rig/quori/gaze/left_right");
        assert_eq!(keys.key("poses/p.weight"), "rig/quori/poses/p.weight");
        assert_eq!(keys.key("rig/other/x"), "rig/other/x");
        // No rig, no prefix: a channel is its own key.
        assert_eq!(Bundle::default().channel_keys().key("gaze/x"), "gaze/x");
    }

    /// A glTF document's channels resolve through the rig of the bundle it
    /// carries, on a node or on its root alike: a rig input's relative path
    /// to that input, an input's id to its path, a channel naming no input
    /// under the rig prefix.
    #[test]
    fn a_documents_channels_resolve_through_its_bundles_rig() {
        let bundle = json!({
            "metadata": { "faceId": "f" },
            "graphs": [{ "kind": "rig", "spec": graph(
                json!([
                    { "id": "input_direct_gaze_left_right", "type": "input",
                      "params": { "path": "rig/f/gaze/left_right" } },
                    { "id": "input_mouth_smile", "type": "input",
                      "params": { "path": "rig/f/mouth/smile_amount" } },
                ]),
                json!([]),
            ) }],
        });
        for gltf in [
            json!({ "nodes": [{}, { "extensions": { "VIZIJ_bundle": bundle } }] }),
            json!({ "nodes": [{}], "extensions": { "VIZIJ_bundle": bundle } }),
        ] {
            let keys = Bundle::from_gltf_json(&gltf)
                .expect("the document carries a bundle")
                .channel_keys();
            let resolved: Vec<String> = ["gaze/left_right", "mouth_smile", "brow/raise"]
                .into_iter()
                .map(|channel| keys.key(channel))
                .collect();
            assert_eq!(
                resolved,
                [
                    "rig/f/gaze/left_right",
                    "rig/f/mouth/smile_amount",
                    "rig/f/brow/raise"
                ]
            );
        }
        assert!(Bundle::from_gltf_json(&json!({ "nodes": [{}] })).is_none());
    }

    /// A channel drives the one key it names, and no key it only resembles:
    /// under another face's rig prefix it keeps that prefix; a channel that
    /// ends rig input paths names none of them; an input's id names that
    /// input, not the pose driver's input beside it (`pose/control/<id>`),
    /// whose path the id ends; and an input the rig reads only through its
    /// override — `override/<id>/value`, which moves the input only while
    /// `override/<id>/enabled` is 1 — is named neither by its path nor by its
    /// id, so either keys under the rig prefix, where the rig reads nothing.
    #[test]
    fn a_channel_drives_one_key_and_none_it_only_resembles() {
        let input =
            |id: &str, path: &str| json!({ "id": id, "type": "input", "params": { "path": path } });
        let keys = Bundle::from_bundle_json(&json!({
            "metadata": { "faceId": "quori" },
            "graphs": [{ "kind": "rig", "spec": graph(
                json!([
                    input("input_direct_gaze_left_right", "rig/quori/gaze/left_right"),
                    input("input_direct_lids_blink", "rig/quori/lids/blink"),
                    input("input_direct_standard_vizij_blink", "rig/quori/standard/vizij/blink"),
                    input("input_direct_standard_vizij_left_eye_pos_x",
                          "rig/quori/standard/vizij/left_eye/pos/x"),
                    input("input_direct_standard_vizij_right_eye_pos_x",
                          "rig/quori/standard/vizij/right_eye/pos/x"),
                    input("input_propsrig_l_eye_scale_x", "rig/quori/propsrig/l_eye/scale/x"),
                    input("input_pose_control_propsrig_l_eye_scale_x",
                          "rig/quori/pose/control/propsrig_l_eye_scale_x"),
                    input("input_override_enabled_propsrig_l_eye_scale_z",
                          "rig/quori/override/propsrig_l_eye_scale_z/enabled"),
                    input("input_override_value_propsrig_l_eye_scale_z",
                          "rig/quori/override/propsrig_l_eye_scale_z/value"),
                ]),
                json!([]),
            ) }],
        }))
        .channel_keys();

        // Another face's prefix is kept: the track drives that face's key,
        // not this face's input of the same relative path.
        assert_eq!(
            keys.key("rig/hugo/gaze/left_right"),
            "rig/hugo/gaze/left_right"
        );
        // The end of several input paths is none of them.
        assert_eq!(keys.key("blink"), "rig/quori/blink");
        assert_eq!(keys.key("pos/x"), "rig/quori/pos/x");
        // An input's id names that input alone.
        assert_eq!(keys.key("lids_blink"), "rig/quori/lids/blink");
        assert_eq!(
            keys.key("propsrig_l_eye_scale_x"),
            "rig/quori/propsrig/l_eye/scale/x"
        );
        // An input the rig reads only through its override.
        assert_eq!(
            keys.key("propsrig/l_eye/scale/z"),
            "rig/quori/propsrig/l_eye/scale/z"
        );
        assert_eq!(
            keys.key("propsrig_l_eye_scale_z"),
            "rig/quori/propsrig_l_eye_scale_z"
        );
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
    fn bundle_reads_each_graphs_id_by_position() {
        let b = Bundle::from_bundle_json(&bundle_json());
        let listed: Vec<(&str, Option<&str>)> = b
            .graphs
            .iter()
            .zip(&b.graph_ids)
            .map(|((kind, _), id)| (kind.as_str(), id.as_deref()))
            .collect();
        assert_eq!(
            listed,
            [
                ("rig", Some("the_rig")),
                ("motiongraph", Some("prog.speaks")),
                ("motiongraph", Some("prog.live")),
            ]
        );
        let unnamed = Bundle::from_bundle_json(&json!({
            "graphs": [{ "kind": "rig", "spec": graph(json!([]), json!([])) }]
        }));
        assert_eq!(unnamed.graph_ids, [None]);
    }

    /// What an app reads beside the graphs: program labels, the pose
    /// config, the rig's declared inputs, the animations, and the metadata as
    /// authored.
    #[test]
    fn bundle_reads_what_an_app_builds_its_controls_from() {
        let mut bundle = bundle_json();
        bundle["graphs"][1]["label"] = json!("Speaks");
        bundle["graphs"][0]["spec"]["metadata"] = json!({ "vizij": { "inputs": [
            { "id": "gaze_x", "path": "/gaze/x", "range": { "min": -1, "max": 1 } },
        ] } });
        bundle["metadata"]["speechConfig"] = json!({ "voice": "Ruth", "visemeGroupId": "v" });
        bundle["poses"]["config"]["poseGroups"] =
            json!([{ "id": "v", "name": "Visemes", "path": "visemes" }]);
        bundle["poses"]["config"]["poses"] =
            json!([{ "id": "pose_a", "name": "A", "group": "visemes" }]);
        bundle["animations"] =
            json!([{ "id": "wave", "clip": { "name": "Wave", "duration": 5, "tracks": [] } }]);
        let b = Bundle::from_bundle_json(&bundle);

        assert_eq!(b.program_labels.len(), 1);
        assert_eq!(b.program_labels["prog.speaks"], "Speaks");
        assert_eq!(b.rig_inputs.len(), 1);
        assert_eq!(b.rig_inputs[0].path, "gaze/x");
        assert_eq!(b.pose_groups[0].id, "v");
        assert_eq!(b.poses[0].group_ids, ["v"]);
        assert_eq!(b.animations[0].id, "wave");
        let metadata = b.metadata.unwrap();
        assert_eq!(metadata["speechConfig"]["voice"], "Ruth");
        assert_eq!(metadata["activeMotionGraphId"], "prog.speaks");

        // A bundle without them reads empty.
        let bare = Bundle::from_bundle_json(&json!({}));
        assert!(bare.poses.is_empty() && bare.rig_inputs.is_empty() && bare.animations.is_empty());
        assert!(bare.metadata.is_none());
    }

    #[test]
    fn compose_appends_the_active_program_last() {
        let b = Bundle::from_bundle_json(&bundle_json());
        let composed = b
            .compose(&["rig", "pose-driver"], &ProgramSelect::Auto, false, &[])
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
        let base = b
            .compose(&["rig"], &ProgramSelect::None, false, &[])
            .unwrap();
        assert_eq!(base["nodes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn compose_orders_base_mappings_animations_then_program() {
        let b = Bundle::from_bundle_json(&bundle_json());
        let composed = b
            .compose(
                &["rig"],
                &ProgramSelect::Auto,
                true,
                &[ros4hri::ros4hri_source("rig/f/")],
            )
            .unwrap();
        let ids: Vec<&str> = composed["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect();
        // Base rig first, then the mapping, the animations, and the program
        // last: a later source out-writes an earlier one on a shared path.
        let rig = ids
            .iter()
            .position(|id| *id == "rig::input_gaze_x")
            .unwrap();
        let mapping = ids
            .iter()
            .position(|id| *id == "ros4hri::in/expression/name")
            .unwrap();
        let program = ids
            .iter()
            .position(|id| *id == "program::prog.speaks::o")
            .unwrap();
        let animations = ids
            .iter()
            .position(|id| id.starts_with("animations::"))
            .unwrap();
        assert!(rig < mapping && mapping < animations && animations < program);

        // Without mappings the composition is untouched — the opt-in default.
        let bare = b
            .compose(&["rig"], &ProgramSelect::Auto, false, &[])
            .unwrap();
        assert!(!bare["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["id"].as_str().unwrap().starts_with("ros4hri::")));
    }

    /// `bundle_json()` with an embedded copy of the `ros4hri` standard mapping:
    /// one input node under the graph id VIZ-92 pins (`standard::ros4hri`).
    fn bundle_json_with_embedded_ros4hri() -> Json {
        let mut bundle = bundle_json();
        bundle["graphs"].as_array_mut().unwrap().push(json!({
            "id": "standard::ros4hri",
            "kind": mappings::STANDARD_MAPPING_KIND,
            "spec": graph(
                json!([{ "id": "mine", "type": "input",
                         "params": { "path": "standard/ros4hri/expression/valence" } }]),
                json!([]),
            ),
        }));
        bundle
    }

    #[test]
    fn bundle_reads_embedded_standard_mappings() {
        let b = Bundle::from_bundle_json(&bundle_json_with_embedded_ros4hri());
        // The entry lands under its bare mapping id (`standard::` stripped)…
        assert_eq!(b.standard_mappings.len(), 1);
        assert_eq!(b.standard_mappings[0].0, "ros4hri");
        // …and is held apart from the kind-selectable base graphs.
        assert!(b
            .graphs
            .iter()
            .all(|(kind, _)| kind != mappings::STANDARD_MAPPING_KIND));
    }

    #[test]
    fn embedded_mapping_composes_and_suppresses_its_built_in() {
        let b = Bundle::from_bundle_json(&bundle_json_with_embedded_ros4hri());
        let other_builtin = (
            "gaze_only".to_string(),
            graph(
                json!([{ "id": "g", "type": "input",
                         "params": { "path": "standard/gaze_only/target" } }]),
                json!([]),
            ),
        );
        let composed = b
            .compose(
                &["rig"],
                &ProgramSelect::None,
                false,
                &[ros4hri::ros4hri_source("rig/f/"), other_builtin],
            )
            .unwrap();
        let ids: Vec<&str> = composed["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect();
        // The embedded copy composes under its stable graph id…
        assert!(ids.contains(&"standard::ros4hri::mine"));
        // …the built-in of the same id is not composed at all…
        assert!(ids.iter().all(|id| !id.starts_with("ros4hri::")));
        // …and built-ins the face does not embed are untouched (the
        // suppression is per mapping id, not a global kill switch).
        assert!(ids.contains(&"gaze_only::g"));
    }

    /// A mapping reads the face's settings at the face's rest. The built-in
    /// ROS4HRI mapping and a mapping the face embeds both rest the gaze range
    /// the face's adaptation authors — in the legacy `kind` form too, as
    /// composition normalizes it — and the eye spacing, which the face reads
    /// with no default, at the profile's rest. A face authoring nothing, or a
    /// value of another type, rests every mapping at the profile's rest,
    /// whatever default a mapping carries. An input the face reads that is no
    /// setting keeps the mapping's own rest.
    #[test]
    fn a_mapping_reads_the_face_s_settings_at_the_face_s_rest() {
        use arora_types::value::Value;
        let mut bundle = bundle_json();
        bundle["metadata"]["faceId"] = json!("f");
        let gaze_range = format!("rig/f/{}", standard::GAZE_RANGE);
        let eye_spacing = format!("rig/f/{}", standard::GAZE_EYE_SPACING);
        let speech = format!("rig/f/{}", standard::SPEECH);
        // A mapping the face embeds, of another id, reading the gaze range at
        // a default of its own.
        bundle["graphs"].as_array_mut().unwrap().push(json!({
            "id": "standard::gaze_only",
            "kind": mappings::STANDARD_MAPPING_KIND,
            "spec": graph(
                json!([{ "id": "range", "type": "input",
                         "params": { "path": gaze_range, "value": 0.3 } }]),
                json!([]),
            ),
        }));
        let with_adaptation = |nodes: Json| {
            let mut bundle = bundle.clone();
            bundle["graphs"].as_array_mut().unwrap().push(json!({
                "kind": "standard-adaptation",
                "spec": graph(nodes, json!([])),
            }));
            bundle
        };
        // The rest the input on `path` of the composed source `source` takes.
        let rest = |bundle: &Json, source: &str, path: &str| {
            let composed = Bundle::from_bundle_json(bundle)
                .compose(
                    &["rig", "standard-adaptation"],
                    &ProgramSelect::None,
                    false,
                    &[ros4hri::ros4hri_source("rig/f/")],
                )
                .unwrap();
            let value = composed["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| {
                    n["id"]
                        .as_str()
                        .unwrap()
                        .starts_with(&format!("{source}::"))
                        && n["params"]["path"] == path
                })
                .unwrap_or_else(|| panic!("{source} reads {path}"))["params"]["value"]
                .clone();
            arora_shape::json::parse_value(value).expect("a rest value")
        };

        for face in [
            bundle.clone(),
            with_adaptation(json!([
                { "id": "range", "type": "input", "params": { "path": gaze_range, "value": "wide" } },
            ])),
        ] {
            assert_eq!(rest(&face, "ros4hri", &gaze_range), Value::F32(0.78));
            assert_eq!(rest(&face, "ros4hri", &eye_spacing), Value::F32(0.06));
            assert_eq!(
                rest(&face, "standard::gaze_only", &gaze_range),
                Value::F32(0.78)
            );
        }

        let authored = with_adaptation(json!([
            { "id": "range", "kind": "input", "params": { "path": gaze_range, "value": 0.5 } },
            { "id": "spacing", "type": "input", "params": { "path": eye_spacing } },
            { "id": "said", "type": "input", "params": { "path": speech, "value": "hello" } },
        ]));
        assert_eq!(rest(&authored, "ros4hri", &gaze_range), Value::F32(0.5));
        assert_eq!(
            rest(&authored, "standard::gaze_only", &gaze_range),
            Value::F32(0.5)
        );
        assert_eq!(rest(&authored, "ros4hri", &eye_spacing), Value::F32(0.06));
        assert_eq!(
            rest(&authored, "ros4hri", &speech),
            Value::String(String::new())
        );
    }

    /// A declared profile is read back typed; a malformed entry is dropped
    /// without taking the well-formed ones — or the face — with it.
    #[test]
    fn bundle_reads_declared_profiles_and_skips_malformed_ones() {
        let mut bundle = bundle_json();
        bundle["profiles"] = json!([
            serde_json::to_value(profile::ros4hri_profile()).unwrap(),
            { "id": "broken", "keys": "not a list" },
        ]);
        let b = Bundle::from_bundle_json(&bundle);
        assert_eq!(b.profiles.len(), 1);
        assert_eq!(b.profiles[0].id, "ros4hri");
        assert_eq!(b.profiles[0].scope, profile::Scope::Device);
    }

    #[test]
    fn compose_appends_the_animation_source_last() {
        let b = Bundle::from_bundle_json(&bundle_json());
        let composed = b
            .compose(&["rig"], &ProgramSelect::None, true, &[])
            .unwrap();
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
