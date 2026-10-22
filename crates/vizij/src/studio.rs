//! The Studio-key profile: how a face takes Semio Studio's writes and
//! reports back to it.
//!
//! Studio addresses a device's animatables by id: `<id>`, the RobotData
//! animatable id of a number, and `<id>.<axis>` for each component of a
//! vector, euler or colour `<id>`, which Studio splits per axis on load into
//! a number of its own. For each number it writes, in one update, the
//! target — the value to show — and, while its timeline plays, the target
//! velocity; it reads the number's feedback back.
//!
//! A target is accepted in each form a client writes it in:
//!
//! - `<id>/target.position` and `<id>/target.velocity`, a component's
//!   `<id>/target.position.<axis>` and `<id>/target.velocity.<axis>`: the
//!   target as `arora_hal::set_targets` reads it (`k/target.c`), the same for
//!   every value type;
//! - `<studio id>.target_position` and `<studio id>.target_velocity`:
//!   Studio's current spelling — `<id>.target_position`, which Studio's
//!   production line writes for a whole number, and a component's
//!   `<id>.<axis>.target_position`, which the Studio-on-Arora line writes
//!   (the production line sends no per-axis target);
//! - `<studio id>.studio_value`: the target again, which Studio writes
//!   untyped beside `.target_position` for every number, and the device
//!   declares `f64` like the target. It is legacy, an input only so that
//!   Studio's update is accepted, and drives nothing: every animatable the
//!   face draws is a number or a number's component, for which Studio's
//!   update carries `.target_position` with the same value. It goes once
//!   Studio no longer sends it.
//!
//! Every animatable the profile maps is numeric, so its target is a
//! `position`; the profile takes no `<id>/target.value`.
//!
//! The device keeps a target's two forms in agreement (`TargetForms`): a
//! change that writes the target in either form writes it in both, so a
//! client speaking either form drives the face and the latest write wins.
//! A change that carries both forms of one target takes the `k/target.c`
//! form's value, as `arora_hal::set_targets` does.
//!
//! The view indexes a face's pose by bare animatable id — a compound feature
//! whole, under its `<id>` — so the profile is the mapping between the two:
//! a graph source, composed into the face's behavior like the ROS4HRI
//! mapping, generated from the face's [`FaceMeta`] bindings. Per animatable
//! (its value shape is its [`Rest`]):
//!
//! - a number reads `<id>/target.position` and writes `<id>`;
//! - a compound reads `<id>/target.position.<axis>` per component and
//!   writes the three joined under `<id>`;
//! - each target is echoed to its feedback: a number's `<id>.position`, a
//!   component's under both spellings of the axis, `<id>.position.<axis>`
//!   (the axis after the component, as `set_targets` measures it) and
//!   `<id>.<axis>.position` (the axis first, as Studio reads it today) —
//!   one output of the target, so the two always agree;
//! - the other keys — the target's other form, which agrees with it, the
//!   velocity in both forms and `.studio_value` — are read and go nowhere:
//!   they are inputs so that an update carrying them beside the target is
//!   accepted, since a device refuses a whole update that names a key it
//!   never opened.
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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use arora_hal::set_targets;
use arora_types::data::{
    DataError, DataStore, Key, KeyMeta, Slot, State, StateChange, Subscription,
};
use arora_types::value::Value;
use serde_json::{json, Value as Json};

use crate::view::meta::{Binding, FaceMeta, Rest};

/// Source id of the composed profile: its node ids take the `studio::`
/// prefix.
pub(crate) const STUDIO_SOURCE_ID: &str = "studio";

/// What a target sets that drives the animatable: its value.
pub(crate) const POSITION: &str = "position";
/// What a target sets that the face takes and leaves: its rate of change.
pub(crate) const VELOCITY: &str = "velocity";
/// The key Studio writes the target to again, untyped, under an
/// animatable's id: legacy, read and going nowhere.
pub(crate) const LEGACY_VALUE: &str = "studio_value";

/// One number Studio addresses: an animatable's id, and the axis of a
/// component of a vector, euler or colour, which Studio addresses as a
/// number of its own, `<id>.<axis>`.
#[derive(Clone, Copy)]
pub(crate) struct Number<'a> {
    id: &'a str,
    axis: Option<&'a str>,
}

impl<'a> Number<'a> {
    /// The number Studio addresses as `studio_id`: `<id>`, or `<id>.<axis>`.
    pub(crate) fn of(studio_id: &'a str) -> Self {
        match studio_id.split_once('.') {
            Some((id, axis)) => Self {
                id,
                axis: Some(axis),
            },
            None => Self {
                id: studio_id,
                axis: None,
            },
        }
    }

    /// Studio's id for it: `<id>`, or `<id>.<axis>`.
    fn studio_id(&self) -> String {
        match self.axis {
            Some(axis) => format!("{}.{axis}", self.id),
            None => self.id.to_string(),
        }
    }

    /// Its target setting `what` in the `k/target.c` form:
    /// `<id>/target.<what>`, a component's `<id>/target.<what>.<axis>`.
    pub(crate) fn target(&self, what: &str) -> String {
        match self.axis {
            Some(axis) => format!("{}/target.{what}.{axis}", self.id),
            None => format!("{}/target.{what}", self.id),
        }
    }

    /// Its target setting `what` as Studio writes it today:
    /// `<studio id>.target_<what>`.
    pub(crate) fn studio_target(&self, what: &str) -> String {
        format!("{}.target_{what}", self.studio_id())
    }

    /// Studio's feedback key for it: `<studio id>.position`, the target it
    /// last took — for a component, the axis first, `<id>.<axis>.position`.
    pub(crate) fn feedback(&self) -> String {
        format!("{}.{POSITION}", self.studio_id())
    }

    /// The keys its feedback is published under: [`Self::feedback`], and
    /// for a component the axis after the component too,
    /// `<id>.position.<axis>`, the measured key its `k/target.c` form
    /// targets.
    pub(crate) fn feedbacks(&self) -> Vec<String> {
        let mut keys = vec![self.feedback()];
        if let Some(axis) = self.axis {
            keys.push(format!("{}.{POSITION}.{axis}", self.id));
        }
        keys
    }

    /// The legacy key Studio writes the target to again:
    /// `<studio id>.studio_value`.
    pub(crate) fn legacy(&self) -> String {
        format!("{}.{LEGACY_VALUE}", self.studio_id())
    }
}

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
        // The numbers Studio addresses the animatable as, each with its rest.
        let numbers: Vec<(Number, f32)> = match (binding.rest, binding.feature.axes()) {
            (Rest::Components(rests), Some(axes)) => axes
                .iter()
                .zip(rests)
                .map(|(axis, rest)| {
                    (
                        Number {
                            id,
                            axis: Some(*axis),
                        },
                        rest,
                    )
                })
                .collect(),
            (Rest::Scalar(rest), _) | (Rest::Components([rest, ..]), None) => {
                vec![(Number { id, axis: None }, rest)]
            }
        };
        if written.contains(id) {
            left += 1;
            for (number, rest) in &numbers {
                graph.accepted(number, *rest);
            }
            continue;
        }
        let targets: Vec<String> = numbers
            .iter()
            .map(|(number, rest)| graph.studio_key(number, *rest))
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
        graph.accepted(&Number::of(key), 0.0);
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

    /// An input of a key Studio writes: typed `f64`, as Studio sends its
    /// numbers, so the store takes Studio's writes.
    fn input(&mut self, path: &str, rest: f32) -> String {
        let id = format!("in/{path}");
        self.node(
            &id,
            "input",
            json!({ "path": path, "value": { "f64": f64::from(rest) } }),
        );
        id
    }

    fn output(&mut self, path: &str, from: &str) {
        let id = format!("out/{path}");
        self.node(&id, "output", json!({ "path": path }));
        self.edge(from, &id, "in");
    }

    /// The keys Studio writes and reads for `number`: its target, echoed to
    /// its feedback, and the keys it reads and leaves. Returns the target's
    /// node.
    fn studio_key(&mut self, number: &Number, rest: f32) -> String {
        let position = self.input(&number.target(POSITION), rest);
        for feedback in number.feedbacks() {
            self.output(&feedback, &position);
        }
        self.input(&number.studio_target(POSITION), rest);
        self.left(number, rest);
        position
    }

    /// The keys Studio writes for `number`, read and going nowhere: Studio's
    /// update naming them is accepted, and no feedback claims they moved
    /// anything.
    fn accepted(&mut self, number: &Number, rest: f32) {
        self.input(&number.target(POSITION), rest);
        self.input(&number.studio_target(POSITION), rest);
        self.left(number, rest);
    }

    /// The keys of `number` the profile reads and leaves whatever it does
    /// with the target: the velocity in both forms, and the legacy value.
    fn left(&mut self, number: &Number, rest: f32) {
        self.input(&number.target(VELOCITY), 0.0);
        self.input(&number.studio_target(VELOCITY), 0.0);
        self.input(&number.legacy(), rest);
    }
}

/// The device's store, keeping the two forms of each target the profile
/// reads in agreement — the `k/target.c` form and Studio's
/// `<studio id>.target_<what>`: a change that writes a target in either form
/// writes it in both, the value [`set_targets`] reads for it when the change
/// carries both, which is the `k/target.c` form's. A change that unsets a
/// target in either form, and sets it in neither, unsets both.
///
/// The profile reads one form, so a client writing either drives the face,
/// and the store never holds two values for one target, whichever form a
/// reader asks for. Every other key passes as the change states it.
pub(crate) struct TargetForms {
    store: Box<dyn DataStore>,
    targets: Arc<Targets>,
}

/// Each target with both of its forms among a composed spec's inputs.
#[derive(Default)]
struct Targets {
    /// Each target's forms.
    forms: Vec<Vec<Key>>,
    /// Each form → its target's index in `forms`.
    index: HashMap<Key, usize>,
}

/// The key of the measured value `key` targets, as [`set_targets`] reads
/// it; `None` for a key that is no target in its grammar.
fn measured(key: &Key) -> Option<Key> {
    let change = StateChange {
        set: HashMap::from([(key.clone(), None)]),
        unset: HashSet::new(),
    };
    set_targets(&change).pop().map(|target| target.measured)
}

impl TargetForms {
    /// `store`, keeping in agreement each target the profile composed into
    /// `spec` reads. `store` alone when `spec` carries no profile.
    ///
    /// A target is the inputs [`set_targets`] reads as targeting one
    /// measured key — a number's `<id>/target.<what>` and
    /// `<id>.target_<what>` — with, for a component's
    /// `<id>/target.<what>.<axis>`, Studio's `<id>.<axis>.target_<what>`,
    /// which is no target in that grammar.
    pub(crate) fn over(store: Box<dyn DataStore>, spec: &Json) -> Box<dyn DataStore> {
        let inputs = studio_inputs(spec);
        let mut by_measured: HashMap<Key, Vec<Key>> = HashMap::new();
        for path in &inputs {
            let key = Key::from(path.as_str());
            if let Some(measured) = measured(&key) {
                by_measured.entry(measured).or_default().push(key);
            }
        }
        let mut targets = Targets::default();
        for mut forms in by_measured.into_values() {
            // A component's `<id>/target.<what>.<axis>` names its number,
            // which Studio's form spells `<id>.<axis>.target_<what>`.
            let studio = forms.iter().find_map(|form| {
                let (id, target) = form.path.rsplit_once("/target.")?;
                let (what, axis) = target.split_once('.')?;
                let number = Number {
                    id,
                    axis: Some(axis),
                };
                Some(number.studio_target(what))
            });
            if let Some(studio) = studio.filter(|studio| inputs.contains(studio)) {
                forms.push(Key::from(studio));
            }
            if forms.len() < 2 {
                continue;
            }
            for form in &forms {
                targets.index.insert(form.clone(), targets.forms.len());
            }
            targets.forms.push(forms);
        }
        if targets.forms.is_empty() {
            return store;
        }
        Box::new(Self {
            store,
            targets: Arc::new(targets),
        })
    }

    /// `change`, each target it writes in either form written in both.
    fn agreed(&self, mut change: StateChange) -> StateChange {
        let index = &self.targets.index;
        // The value each target takes: the one `set_targets` reads, else the
        // one Studio's form sets, which that grammar does not read.
        let mut taken: HashMap<usize, Option<Value>> = set_targets(&change)
            .into_iter()
            .filter_map(|target| Some((*index.get(target.key)?, target.value.clone())))
            .collect();
        for (key, value) in &change.set {
            if let Some(&target) = index.get(key) {
                taken.entry(target).or_insert_with(|| value.clone());
            }
        }
        let unset: HashSet<usize> = change
            .unset
            .iter()
            .filter_map(|key| index.get(key).copied())
            .filter(|target| !taken.contains_key(target))
            .collect();
        for (target, value) in taken {
            for form in &self.targets.forms[target] {
                change.unset.remove(form);
                change.set.insert(form.clone(), value.clone());
            }
        }
        for target in unset {
            change
                .unset
                .extend(self.targets.forms[target].iter().cloned());
        }
        change
    }
}

impl DataStore for TargetForms {
    fn read(&self, keys: &[Key]) -> Vec<Option<Value>> {
        self.store.read(keys)
    }

    fn write(&self, changes: StateChange) -> Result<(), DataError> {
        self.store.write(self.agreed(changes))
    }

    fn snapshot(&self) -> State {
        self.store.snapshot()
    }

    /// A target form's slot writes through [`DataStore::write`], so it
    /// writes both forms; any other key's is the store's own.
    fn slot(&self, key: &Key) -> Box<dyn Slot> {
        if self.targets.index.contains_key(key) {
            Box::new(FormSlot {
                store: self.clone_box(),
                key: key.clone(),
            })
        } else {
            self.store.slot(key)
        }
    }

    fn subscribe(&self) -> Subscription {
        self.store.subscribe()
    }

    fn subscribe_prefix(&self, prefix: &str) -> Subscription {
        self.store.subscribe_prefix(prefix)
    }

    fn meta(&self, keys: &[Key]) -> Vec<Option<KeyMeta>> {
        self.store.meta(keys)
    }

    fn all_meta(&self) -> HashMap<Key, KeyMeta> {
        self.store.all_meta()
    }

    fn set_meta(&self, meta: HashMap<Key, KeyMeta>) -> Result<(), DataError> {
        self.store.set_meta(meta)
    }

    fn set_prefix_meta(&self, meta: HashMap<String, KeyMeta>) -> Result<(), DataError> {
        self.store.set_prefix_meta(meta)
    }

    fn clone_box(&self) -> Box<dyn DataStore> {
        Box::new(Self {
            store: self.store.clone_box(),
            targets: Arc::clone(&self.targets),
        })
    }
}

/// A target form's cell, written through [`TargetForms`].
struct FormSlot {
    store: Box<dyn DataStore>,
    key: Key,
}

impl Slot for FormSlot {
    fn get(&self) -> Option<Value> {
        self.store
            .read(std::slice::from_ref(&self.key))
            .pop()
            .flatten()
    }

    fn set(&self, value: Option<Value>) -> Result<(), DataError> {
        let mut change = StateChange::new();
        change.set.insert(self.key.clone(), value);
        self.store.write(change)
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
            node_names: vec!["GltfNode0".into()],
            feature,
            axis,
            rest,
        };
        let meta = FaceMeta {
            animatables: HashMap::from([
                (
                    x.to_string(),
                    binding(FeatureKind::Translation, Some(0), Rest::Scalar(0.5)),
                ),
                (
                    color.to_string(),
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

    /// The keys the profile reads for the animatable (or component) `key`:
    /// the target in both forms, the velocity in both, and the legacy value.
    fn read_keys(key: &str) -> [String; 5] {
        [
            Number::of(key).target(POSITION),
            Number::of(key).studio_target(POSITION),
            Number::of(key).target(VELOCITY),
            Number::of(key).studio_target(VELOCITY),
            Number::of(key).legacy(),
        ]
    }

    /// A number takes its target, a compound one target per component; each
    /// target is echoed to its feedback, a component's under both spellings
    /// of its axis, and the other keys Studio sends beside it are read and go
    /// nowhere.
    #[test]
    fn the_profile_reads_studio_s_keys_and_writes_the_view_s() {
        let (meta, x, color) = studio_face();
        let (id, spec) = studio_source(&meta, &HashSet::new());
        assert_eq!(id, STUDIO_SOURCE_ID);
        let mut inputs: Vec<String> = read_keys(&x.to_string()).to_vec();
        for axis in ["r", "g", "b"] {
            inputs.extend(read_keys(&format!("{color}.{axis}")));
        }
        inputs.sort();
        assert_eq!(paths(&spec, "input"), inputs);
        let mut outputs = vec![
            x.to_string(),
            Number::of(&x.to_string()).feedback(),
            color.to_string(),
        ];
        outputs.extend(["r", "g", "b"].map(|axis| format!("{color}.{axis}.position")));
        outputs.extend(["r", "g", "b"].map(|axis| format!("{color}.position.{axis}")));
        outputs.sort();
        assert_eq!(paths(&spec, "output"), outputs);
        // The animatable and its feedback follow the `/target.position` form.
        let edges = spec["edges"].as_array().expect("edges");
        let target_node = format!("in/{}", Number::of(&x.to_string()).target(POSITION));
        for out in [x.to_string(), Number::of(&x.to_string()).feedback()] {
            assert!(
                edges
                    .iter()
                    .any(|edge| edge["from"]["node_id"] == target_node
                        && edge["to"]["node_id"] == format!("out/{out}")),
                "{out} follows {target_node}"
            );
        }
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
            for path in read_keys(&key) {
                assert!(inputs.contains(&path), "{path} is accepted");
            }
        }
        let outputs = paths(&spec, "output");
        assert!(
            outputs.iter().all(|path| path.starts_with(&x.to_string())),
            "{outputs:?}"
        );
    }

    use arora_shape::value::{as_float, float};
    use vizij_arora_store::BlackboardStore;

    /// A Studio-exported face's store, keeping its targets' forms in
    /// agreement, and the number animatable's id.
    fn agreeing_store() -> (Box<dyn DataStore>, String) {
        let (meta, x, _) = studio_face();
        let spec = vizij_arora_host::compose_sources(&[studio_source(&meta, &HashSet::new())])
            .expect("compose the profile");
        (
            TargetForms::over(Box::new(BlackboardStore::new()), &spec),
            x.to_string(),
        )
    }

    fn write(store: &dyn DataStore, values: &[(&str, Option<f64>)]) {
        let mut change = StateChange::new();
        for (key, value) in values {
            match value {
                Some(value) => {
                    change
                        .set
                        .insert(Key::from(*key), Some(float(*value as f32)));
                }
                None => {
                    change.unset.insert(Key::from(*key));
                }
            }
        }
        store.write(change).expect("write");
    }

    fn read(store: &dyn DataStore, key: &str) -> Option<Value> {
        store.read(&[Key::from(key)]).pop().flatten()
    }

    /// A target written in either form lands in both, the latest write
    /// winning whichever form it took.
    #[test]
    fn a_target_written_in_either_form_lands_in_both() {
        let (store, x) = agreeing_store();
        let (slash, underscore) = (
            Number::of(&x).target(POSITION),
            Number::of(&x).studio_target(POSITION),
        );
        for (form, value) in [(&underscore, 0.25), (&slash, 0.5), (&underscore, 0.75)] {
            write(&*store, &[(form, Some(value))]);
            for read_form in [&slash, &underscore] {
                assert_eq!(
                    read(&*store, read_form),
                    Some(float(value as f32)),
                    "{read_form} after {form} = {value}"
                );
            }
        }
        let (slash, underscore) = (
            Number::of(&x).target(VELOCITY),
            Number::of(&x).studio_target(VELOCITY),
        );
        write(&*store, &[(&underscore, Some(2.0))]);
        assert_eq!(read(&*store, &slash), Some(float(2.0)), "the velocity too");
    }

    /// A component's target, `<id>/target.position.<axis>` or Studio's
    /// `<id>.<axis>.target_position`, lands in both, the first when a change
    /// carries both.
    #[test]
    fn a_component_s_target_written_in_either_form_lands_in_both() {
        let (meta, _, color) = studio_face();
        let spec = vizij_arora_host::compose_sources(&[studio_source(&meta, &HashSet::new())])
            .expect("compose the profile");
        let store = TargetForms::over(Box::new(BlackboardStore::new()), &spec);
        let green = format!("{color}.g");
        let number = Number::of(&green);
        let (slash, studio) = (number.target(POSITION), number.studio_target(POSITION));
        assert_eq!(slash, format!("{color}/target.position.g"));
        assert_eq!(studio, format!("{color}.g.target_position"));
        write(&*store, &[(&studio, Some(0.25))]);
        assert_eq!(read(&*store, &slash), Some(float(0.25)));
        write(&*store, &[(&slash, Some(0.5))]);
        assert_eq!(read(&*store, &studio), Some(float(0.5)));
        write(&*store, &[(&studio, Some(0.125)), (&slash, Some(0.75))]);
        assert_eq!(read(&*store, &slash), Some(float(0.75)));
        assert_eq!(read(&*store, &studio), Some(float(0.75)));
    }

    /// A change carrying both forms of one target takes the
    /// `k/target.c` form's value.
    #[test]
    fn a_change_carrying_both_forms_takes_the_slash_form() {
        let (store, x) = agreeing_store();
        let (slash, underscore) = (
            Number::of(&x).target(POSITION),
            Number::of(&x).studio_target(POSITION),
        );
        write(&*store, &[(&underscore, Some(0.25)), (&slash, Some(0.5))]);
        assert_eq!(read(&*store, &slash), Some(float(0.5)));
        assert_eq!(read(&*store, &underscore), Some(float(0.5)));
        write(&*store, &[(&slash, Some(0.75)), (&underscore, Some(0.25))]);
        assert_eq!(read(&*store, &slash), Some(float(0.75)));
        assert_eq!(read(&*store, &underscore), Some(float(0.75)));
        // A set beats an unset of the other form.
        write(&*store, &[(&underscore, None), (&slash, Some(0.1))]);
        assert_eq!(read(&*store, &underscore), Some(float(0.1)));
    }

    /// An unset of a target in either form, setting it in neither, unsets
    /// both, and a slot onto either form writes both.
    #[test]
    fn an_unset_or_a_slot_write_keeps_the_forms_in_agreement() {
        let (store, x) = agreeing_store();
        let (slash, underscore) = (
            Number::of(&x).target(POSITION),
            Number::of(&x).studio_target(POSITION),
        );
        write(&*store, &[(&slash, Some(0.5))]);
        write(&*store, &[(&underscore, None)]);
        assert_eq!(read(&*store, &slash), None);
        assert_eq!(read(&*store, &underscore), None);

        store
            .slot(&Key::from(underscore.as_str()))
            .set(Some(float(0.4)))
            .expect("set");
        assert_eq!(read(&*store, &slash), Some(float(0.4)));
        assert_eq!(
            store.clone_box().slot(&Key::from(slash.as_str())).get(),
            Some(float(0.4)),
            "a sibling handle keeps the forms too"
        );
    }

    /// The legacy value is a key of its own: writing it sets no target.
    #[test]
    fn the_legacy_value_sets_no_target() {
        let (store, x) = agreeing_store();
        let legacy = Number::of(&x).legacy();
        write(&*store, &[(&legacy, Some(0.3))]);
        assert_eq!(read(&*store, &legacy), Some(float(0.3)));
        assert_eq!(read(&*store, &Number::of(&x).target(POSITION)), None);
        assert_eq!(read(&*store, &Number::of(&x).studio_target(POSITION)), None);
    }

    /// The device's rest module writes through the device's store: a
    /// `reset_keys` naming one form of a target rests both, and the face
    /// follows.
    #[test]
    fn reset_keys_naming_one_form_rests_both() {
        use crate::modules::rest::{rest::ids::reset_keys, MODULE_ID};
        use arora_types::call::Call;
        use arora_types::value::StructureField;

        let (meta, x, _) = studio_face();
        let spec = vizij_arora_host::compose_sources(&[studio_source(&meta, &HashSet::new())])
            .expect("compose the profile")
            .to_string();
        let store = BlackboardStore::new();
        crate::face::declare_keys(
            &store,
            &spec,
            &[],
            &[],
            &crate::face::InputRanges::default(),
        );
        let mut arora =
            crate::face::builder_for(&spec, vizij_arora_hal::RigHal::new(), store, &[], None)
                .expect("build the device over the profile")
                .build()
                .expect("build arora");
        let x = x.to_string();
        let (slash, underscore) = (
            Number::of(&x).target(POSITION),
            Number::of(&x).studio_target(POSITION),
        );
        write(arora.store(), &[(&slash, Some(0.25))]);
        arora
            .call(Call {
                module_id: Some(MODULE_ID),
                id: reset_keys::FUNCTION,
                args: vec![StructureField {
                    id: reset_keys::KEYS,
                    value: Box::new(Value::ArrayString(vec![underscore.clone()])),
                }],
            })
            .expect("reset_keys");
        // The profile's rest for `x` is its RobotData default, 0.5.
        for form in [&underscore, &slash] {
            let rested = read(arora.store(), form);
            assert_eq!(rested.as_ref().and_then(as_float), Some(0.5), "{form}");
        }
    }

    /// A component of an animatable bound to nothing the view draws pairs
    /// its forms whatever its id spells: a `/` in the id, a `.` in the axis.
    #[test]
    fn an_unbound_component_pairs_its_forms_whatever_its_id() {
        let (mut meta, _, _) = studio_face();
        meta.unbound = vec!["face/plate.r".to_string(), "u.x.x".to_string()];
        let spec = vizij_arora_host::compose_sources(&[studio_source(&meta, &HashSet::new())])
            .expect("compose the profile");
        let store = TargetForms::over(Box::new(BlackboardStore::new()), &spec);
        for key in &meta.unbound {
            let number = Number::of(key);
            write(&*store, &[(&number.studio_target(POSITION), Some(0.5))]);
            assert_eq!(
                read(&*store, &number.target(POSITION)),
                Some(float(0.5)),
                "{key}"
            );
        }
    }

    /// Without the profile in the spec, the store is the store: a key named
    /// like a target is a key like any other.
    #[test]
    fn a_spec_without_the_profile_keeps_no_forms() {
        let spec = json!({ "nodes": [
            { "id": "i", "type": "input", "params": { "path": "a/target.position", "value": 0.0 } },
            { "id": "j", "type": "input", "params": { "path": "a.target_position", "value": 0.0 } },
        ] });
        let store = TargetForms::over(Box::new(BlackboardStore::new()), &spec);
        write(&*store, &[("a.target_position", Some(0.5))]);
        assert_eq!(read(&*store, "a/target.position"), None);
    }
}
