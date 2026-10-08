//! The Studio-key profile: how a face takes Semio Studio's writes and
//! reports back to it.
//!
//! Studio addresses a device's animatables by key. For an animatable `<id>`
//! it writes, in one update, `<id>.target_position` (the value to show),
//! `<id>.studio_value` (the same value, untyped) and, while its timeline
//! plays, `<id>.target_velocity`; it reads `<id>.position` back as the
//! device's feedback. `<id>` is the RobotData animatable id of a number, and
//! `<uuid>.<axis>` for each component of a vector, euler or colour, which
//! Studio splits per axis on load.
//!
//! The view indexes a face's pose by bare animatable id — a compound feature
//! whole, under its `<uuid>` — so the profile is the mapping between the two:
//! a graph source, composed into the face's behavior like the ROS4HRI
//! mapping, generated from the face's [`FaceMeta`] bindings. Per animatable
//! (its value shape is its [`Rest`]):
//!
//! - a number reads `<id>.target_position` and writes `<id>`;
//! - a compound reads `<uuid>.<axis>.target_position` per component and
//!   writes the three joined under `<uuid>`;
//! - each target is echoed to its `.position` key, Studio's feedback;
//! - `.studio_value` and `.target_velocity` are read and go nowhere: they are
//!   inputs only so that Studio's update, which carries them beside the
//!   target, is accepted — a device refuses a whole update that names a key
//!   it never opened.
//!
//! Every target rests at its animatable's RobotData default, so the face
//! holds its authored pose until Studio writes.
//!
//! Studio's update for an entity carries every animatable it knows of the
//! face, and a device refuses a whole update that names any key it never
//! opened, so the profile accepts Studio's keys for every RobotData
//! animatable. Two kinds are accepted and move nothing, and report no
//! feedback: an animatable the face's own graphs write (a rig driving it
//! from the standard controls) stays theirs, since two sources writing one
//! key would take turns; and an animatable bound to nothing the view draws
//! (a joint's value, a stroke) has nowhere to land.

use std::collections::{BTreeMap, HashSet};

use serde_json::{json, Value as Json};

use crate::view::meta::{Binding, FaceMeta, Rest};

/// Source id of the composed profile: its node ids take the `studio::`
/// prefix.
pub(crate) const STUDIO_SOURCE_ID: &str = "studio";

/// The key Studio writes an animatable's value to, under its id.
pub(crate) const TARGET_POSITION: &str = "target_position";
/// The key Studio reads an animatable's feedback from, under its id.
pub(crate) const POSITION: &str = "position";
/// The key Studio writes the same value to beside the target, untyped.
pub(crate) const STUDIO_VALUE: &str = "studio_value";
/// The key Studio writes an animatable's velocity to while its timeline plays.
pub(crate) const TARGET_VELOCITY: &str = "target_velocity";

/// The Studio-key profile of a face, as a composable graph source. It maps
/// every animatable `meta` binds but those whose key is in `written` — the
/// output paths of the face's own composed graphs — and accepts Studio's
/// keys for those and for the animatables bound to nothing the view draws
/// ([`FaceMeta::unbound`]) without mapping them.
pub(crate) fn studio_source(meta: &FaceMeta, written: &HashSet<String>) -> (String, Json) {
    let mut graph = Graph::default();
    // Sorted, so the source is the same graph on every load.
    let animatables: BTreeMap<String, &Binding> = meta
        .animatables
        .iter()
        .map(|(id, binding)| (id.to_string(), binding))
        .collect();
    let mut left = 0;
    for (id, binding) in &animatables {
        // The keys Studio writes the animatable under, each with its rest.
        let keys: Vec<(String, f32)> = match (binding.rest, binding.feature.axes()) {
            (Rest::Components(rests), Some(axes)) => axes
                .iter()
                .zip(rests)
                .map(|(axis, rest)| (format!("{id}.{axis}"), rest))
                .collect(),
            (Rest::Scalar(rest), _) | (Rest::Components([rest, ..]), None) => {
                vec![(id.clone(), rest)]
            }
        };
        if written.contains(id) {
            left += 1;
            for (key, rest) in &keys {
                graph.accepted(key, *rest);
            }
            continue;
        }
        let targets: Vec<String> = keys
            .iter()
            .map(|(key, rest)| graph.studio_key(key, *rest))
            .collect();
        if let [target] = targets.as_slice() {
            graph.output(id, target);
        } else {
            let joined = format!("join/{id}");
            graph.node(&joined, "join", json!({}));
            for (port, target) in targets.iter().enumerate() {
                graph.edge(target, &joined, &format!("operand_{port}"));
            }
            graph.output(id, &joined);
        }
    }
    for key in &meta.unbound {
        graph.accepted(key, 0.0);
    }
    if left > 0 || !meta.unbound.is_empty() {
        log::info!(
            "studio: Studio's writes to {left} animatables the face's own graphs drive, and \
             to {} keys of animatables bound to nothing the view draws, are accepted and \
             move nothing",
            meta.unbound.len()
        );
    }
    (
        STUDIO_SOURCE_ID.to_string(),
        json!({ "nodes": graph.nodes, "edges": graph.edges }),
    )
}

/// The paths the outputs of `spec` write — what a face's own graphs drive.
pub(crate) fn written_paths(spec: &Json) -> HashSet<String> {
    nodes_of(spec, "output")
        .filter_map(|node| node.pointer("/params/path")?.as_str())
        .map(str::to_string)
        .collect()
}

/// The keys the profile reads in a composed `spec`: Studio's, numbers in
/// the units of the feature they drive (a translation in scene units, a
/// rotation in radians) rather than the face's `[0, 1]` inputs.
pub(crate) fn studio_inputs(spec: &Json) -> HashSet<String> {
    let prefix = format!("{STUDIO_SOURCE_ID}::");
    nodes_of(spec, "input")
        .filter(|node| {
            node.get("id")
                .and_then(Json::as_str)
                .is_some_and(|id| id.starts_with(&prefix))
        })
        .filter_map(|node| node.pointer("/params/path")?.as_str())
        .map(str::to_string)
        .collect()
}

fn nodes_of<'a>(spec: &'a Json, ty: &'a str) -> impl Iterator<Item = &'a Json> {
    spec.get("nodes")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter(move |node| {
            node.get("type")
                .and_then(Json::as_str)
                .is_some_and(|t| t.eq_ignore_ascii_case(ty))
        })
}

/// The profile's graph, node by node. Node ids name the key a node reads
/// or writes: `in/<key>` and `out/<key>`.
#[derive(Default)]
struct Graph {
    nodes: Vec<Json>,
    edges: Vec<Json>,
}

impl Graph {
    fn node(&mut self, id: &str, ty: &str, params: Json) {
        self.nodes
            .push(json!({ "id": id, "type": ty, "params": params }));
    }

    fn edge(&mut self, from: &str, to: &str, input: &str) {
        self.edges.push(json!({
            "from": { "node_id": from },
            "to": { "node_id": to, "input": input },
        }));
    }

    fn input(&mut self, path: &str, rest: f32) -> String {
        let id = format!("in/{path}");
        self.node(&id, "input", json!({ "path": path, "value": rest }));
        id
    }

    fn output(&mut self, path: &str, from: &str) {
        let id = format!("out/{path}");
        self.node(&id, "output", json!({ "path": path }));
        self.edge(from, &id, "in");
    }

    /// The keys Studio writes and reads for the animatable (or component)
    /// `key`: its target, echoed to its feedback, and the two it ignores.
    /// Returns the target's node.
    fn studio_key(&mut self, key: &str, rest: f32) -> String {
        let target = self.input(&format!("{key}.{TARGET_POSITION}"), rest);
        self.output(&format!("{key}.{POSITION}"), &target);
        self.input(&format!("{key}.{STUDIO_VALUE}"), rest);
        self.input(&format!("{key}.{TARGET_VELOCITY}"), 0.0);
        target
    }

    /// The keys Studio writes for the animatable (or component) `key`, read
    /// and going nowhere: Studio's update naming them is accepted, and no
    /// feedback claims they moved anything.
    fn accepted(&mut self, key: &str, rest: f32) {
        self.input(&format!("{key}.{TARGET_POSITION}"), rest);
        self.input(&format!("{key}.{STUDIO_VALUE}"), rest);
        self.input(&format!("{key}.{TARGET_VELOCITY}"), 0.0);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use uuid::Uuid;

    use super::*;
    use crate::view::meta::{Binding, FeatureKind};

    /// A Studio-exported face's bindings: `translation.x` on an animatable
    /// of its own, resting at 0.5, and `color.r/g/b` on the components of
    /// the compound animatable `color`, resting at (0.1, 0.2, 0.3).
    fn studio_face() -> (FaceMeta, Uuid, Uuid) {
        let (x, color) = (Uuid::new_v4(), Uuid::new_v4());
        let binding = |feature, axis, rest| Binding {
            node_name: "GltfNode0".into(),
            feature,
            axis,
            rest,
        };
        let meta = FaceMeta {
            animatables: HashMap::from([
                (
                    x,
                    binding(FeatureKind::Translation, Some(0), Rest::Scalar(0.5)),
                ),
                (
                    color,
                    binding(FeatureKind::Color, None, Rest::Components([0.1, 0.2, 0.3])),
                ),
            ]),
            ..FaceMeta::default()
        };
        (meta, x, color)
    }

    fn paths(spec: &Json, ty: &str) -> Vec<String> {
        let mut paths: Vec<String> = nodes_of(spec, ty)
            .filter_map(|node| node.pointer("/params/path")?.as_str())
            .map(str::to_string)
            .collect();
        paths.sort();
        paths
    }

    /// A number takes its target, a compound one target per component; each
    /// target is echoed to its feedback, and the two keys Studio sends
    /// beside it are read and go nowhere.
    #[test]
    fn the_profile_reads_studio_s_keys_and_writes_the_view_s() {
        let (meta, x, color) = studio_face();
        let (id, spec) = studio_source(&meta, &HashSet::new());
        assert_eq!(id, STUDIO_SOURCE_ID);
        let keys = |key: String| {
            [TARGET_POSITION, STUDIO_VALUE, TARGET_VELOCITY].map(|k| format!("{key}.{k}"))
        };
        let mut inputs: Vec<String> = keys(x.to_string()).to_vec();
        for axis in ["r", "g", "b"] {
            inputs.extend(keys(format!("{color}.{axis}")));
        }
        inputs.sort();
        assert_eq!(paths(&spec, "input"), inputs);
        let mut outputs = vec![x.to_string(), format!("{x}.{POSITION}"), color.to_string()];
        outputs.extend(["r", "g", "b"].map(|axis| format!("{color}.{axis}.{POSITION}")));
        outputs.sort();
        assert_eq!(paths(&spec, "output"), outputs);
    }

    /// An animatable the face's own graphs write stays theirs, and one bound
    /// to nothing the view draws has nowhere to land: Studio's keys for both
    /// are accepted, and the profile writes nothing of theirs.
    #[test]
    fn studio_s_keys_of_a_rig_driven_or_unbound_animatable_go_nowhere() {
        let (mut meta, x, color) = studio_face();
        let joint = Uuid::new_v4().to_string();
        meta.unbound = vec![joint.clone()];
        let (_, spec) = studio_source(&meta, &HashSet::from([color.to_string()]));
        let inputs = paths(&spec, "input");
        for key in [format!("{color}.g"), joint] {
            for k in [TARGET_POSITION, STUDIO_VALUE, TARGET_VELOCITY] {
                assert!(
                    inputs.contains(&format!("{key}.{k}")),
                    "{key}.{k} is accepted"
                );
            }
        }
        let outputs = paths(&spec, "output");
        assert!(
            outputs.iter().all(|path| path.starts_with(&x.to_string())),
            "{outputs:?}"
        );
    }
}
