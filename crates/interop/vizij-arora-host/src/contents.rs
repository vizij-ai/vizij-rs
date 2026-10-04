//! The parts of a face's `VIZIJ_bundle` an app reads to build its controls:
//! the poses and the groups they sort into, the rig's inputs with their
//! ranges, and the authored animations. [`Bundle`](crate::Bundle) holds
//! them typed, parsed from the bundle once.
//!
//! Each type serializes in camelCase, the names `@vizij/runtime`'s
//! `describe()` returns. Entries missing what identifies them (a pose or a
//! group without an `id`, a rig input without a `path`, an animation without
//! an id, a track without a `channel`, a keyframe without a finite `time` and
//! `value`) are skipped; every other field is optional in the bundle and
//! `None` when absent.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value as Json;

/// One pose: a named set of rig input values, driven through its weight at
/// `<rig prefix>poses/<id>.weight`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pose {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    /// The ids of the groups the pose belongs to, in the bundle's order: its
    /// `groupIds`, then its `groupId`, then the group whose `path` (or id)
    /// its `group` names. No duplicates.
    pub group_ids: Vec<String>,
    /// Rig input id → the value the pose sets it to at full weight.
    pub values: BTreeMap<String, f64>,
}

/// A group of poses — e.g. visemes or emotions.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoseGroup {
    pub id: String,
    pub name: Option<String>,
    /// The group's path segment (`visemes`, `emotions`), which a pose's
    /// `group` may name instead of the id.
    pub path: Option<String>,
}

/// One input of the face's rig, as the rig graph's `metadata.vizij.inputs`
/// declares it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RigInput {
    pub id: Option<String>,
    /// The input's path relative to the face's rig prefix, without a leading
    /// slash: the store key is `<rig prefix><path>`.
    pub path: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub default_value: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

/// An authored animation: an entry of the bundle's `animations`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Animation {
    pub id: String,
    pub name: Option<String>,
    /// In seconds: the animation's `duration`, or its last keyframe's time
    /// when it declares none.
    pub duration: f64,
    pub tracks: Vec<AnimationTrack>,
}

/// One animated channel of an [`Animation`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationTrack {
    /// The rig-relative path the track drives (`gaze/left_right`,
    /// `poses/<id>.weight`).
    pub channel: String,
    /// `linear`, `step` or `cubic` as authored; a keyframe's own
    /// interpolation overrides it.
    pub interpolation: Option<String>,
    /// In time order.
    pub keyframes: Vec<Keyframe>,
}

/// A point of an [`AnimationTrack`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Keyframe {
    /// In seconds from the animation's start.
    pub time: f64,
    pub value: f64,
    pub interpolation: Option<String>,
}

fn string(json: &Json, key: &str) -> Option<String> {
    json.get(key).and_then(Json::as_str).map(str::to_string)
}

fn number(json: &Json, key: &str) -> Option<f64> {
    json.get(key)
        .and_then(Json::as_f64)
        .filter(|n| n.is_finite())
}

fn array(json: Option<&Json>) -> impl Iterator<Item = &Json> {
    json.and_then(Json::as_array).into_iter().flatten()
}

fn trim_slashes(path: &str) -> &str {
    path.trim().trim_matches('/')
}

/// `poses.config.poseGroups`.
pub(crate) fn pose_groups(bundle: &Json) -> Vec<PoseGroup> {
    array(bundle.pointer("/poses/config/poseGroups"))
        .filter_map(|group| {
            Some(PoseGroup {
                id: string(group, "id")?,
                name: string(group, "name"),
                path: string(group, "path"),
            })
        })
        .collect()
}

/// `poses.config.poses`, each with its group membership resolved against
/// `groups`.
pub(crate) fn poses(bundle: &Json, groups: &[PoseGroup]) -> Vec<Pose> {
    array(bundle.pointer("/poses/config/poses"))
        .filter_map(|pose| {
            let mut group_ids: Vec<String> = Vec::new();
            let mut add = |id: &str| {
                if !id.is_empty() && !group_ids.iter().any(|g| g == id) {
                    group_ids.push(id.to_string());
                }
            };
            array(pose.get("groupIds"))
                .filter_map(Json::as_str)
                .for_each(&mut add);
            if let Some(id) = pose.get("groupId").and_then(Json::as_str) {
                add(id);
            }
            if let Some(named) = pose.get("group").and_then(Json::as_str) {
                let named = trim_slashes(named);
                if let Some(group) = groups
                    .iter()
                    .find(|g| g.id == named || g.path.as_deref().map(trim_slashes) == Some(named))
                {
                    add(&group.id);
                }
            }
            let values = pose
                .get("values")
                .and_then(Json::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(input, value)| Some((input.clone(), value.as_f64()?)))
                .collect();
            Some(Pose {
                id: string(pose, "id")?,
                name: string(pose, "name"),
                description: string(pose, "description"),
                group_ids,
                values,
            })
        })
        .collect()
}

/// The rig graph's `metadata.vizij.inputs`.
pub(crate) fn rig_inputs(rig_spec: &Json) -> Vec<RigInput> {
    array(rig_spec.pointer("/metadata/vizij/inputs"))
        .filter_map(|input| {
            let path = trim_slashes(input.get("path")?.as_str()?);
            if path.is_empty() {
                return None;
            }
            let range = input.get("range");
            Some(RigInput {
                id: string(input, "id"),
                path: path.to_string(),
                label: string(input, "label"),
                group: string(input, "group"),
                default_value: number(input, "defaultValue"),
                min: range.and_then(|r| number(r, "min")),
                max: range.and_then(|r| number(r, "max")),
            })
        })
        .collect()
}

/// The bundle's `animations`: `[{ id, clip: { id, name, duration, tracks } }]`.
pub(crate) fn animations(bundle: &Json) -> Vec<Animation> {
    array(bundle.get("animations"))
        .filter(|entry| entry.get("clip").is_some())
        .filter_map(animation)
        .collect()
}

/// One animation, read as the bundle reads it: an `animations` entry
/// (`{ id, clip: { id, name, duration, tracks } }`, the entry's `id` first),
/// or an animation on its own in the shape [`Animation`] serializes to —
/// what a host hands back to replace an animation live. `None` without an id.
pub fn animation(json: &Json) -> Option<Animation> {
    let body = json.get("clip").unwrap_or(json);
    let id = string(json, "id").or_else(|| string(body, "id"))?;
    let tracks: Vec<AnimationTrack> = array(body.get("tracks"))
        .filter_map(|track| {
            let mut keyframes: Vec<Keyframe> = array(track.get("keyframes"))
                .filter_map(|keyframe| {
                    Some(Keyframe {
                        time: number(keyframe, "time")?,
                        value: number(keyframe, "value")?,
                        interpolation: string(keyframe, "interpolation"),
                    })
                })
                .collect();
            keyframes.sort_by(|a, b| a.time.total_cmp(&b.time));
            Some(AnimationTrack {
                channel: string(track, "channel")?,
                interpolation: string(track, "interpolation"),
                keyframes,
            })
        })
        .collect();
    let duration = number(body, "duration").unwrap_or_else(|| {
        tracks
            .iter()
            .filter_map(|t| t.keyframes.last())
            .map(|k| k.time)
            .fold(0.0, f64::max)
    });
    Some(Animation {
        id,
        name: string(body, "name"),
        duration,
        tracks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_pose_joins_the_groups_its_ids_and_its_group_path_name() {
        let bundle = json!({ "poses": { "config": {
            "poseGroups": [
                { "id": "default", "name": "Visemes", "path": "visemes" },
                { "id": "emotionsv2", "name": "Emotions", "path": "/emotions/" },
                { "name": "no id" },
            ],
            "poses": [
                { "id": "pose_a", "name": "A", "description": "Open jaw", "groupIds": ["default"],
                  "groupId": "default", "group": "visemes", "values": { "jaw": 0.5, "bad": "x" } },
                { "id": "pose_happy", "group": "emotions" },
                { "id": "pose_loose", "group": "nowhere" },
                { "name": "no id" },
            ],
        } } });
        let groups = pose_groups(&bundle);
        assert_eq!(
            groups.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(),
            ["default", "emotionsv2"]
        );
        let poses = poses(&bundle, &groups);
        assert_eq!(poses.len(), 3);
        assert_eq!(poses[0].group_ids, ["default"]);
        assert_eq!(poses[0].name.as_deref(), Some("A"));
        assert_eq!(poses[0].description.as_deref(), Some("Open jaw"));
        assert_eq!(poses[1].description, None);
        assert_eq!(poses[0].values, BTreeMap::from([("jaw".to_string(), 0.5)]));
        assert_eq!(poses[1].group_ids, ["emotionsv2"]);
        assert!(poses[2].group_ids.is_empty());
    }

    #[test]
    fn a_rig_input_path_is_relative_to_the_rig_prefix() {
        let rig = json!({ "metadata": { "vizij": { "inputs": [
            { "id": "gaze_x", "path": "/gaze/left_right", "label": "Gaze",
              "group": "gaze", "defaultValue": 0, "range": { "min": -1, "max": 1 } },
            { "id": "bare", "path": "lids" },
            { "id": "no_path" },
        ] } } });
        let inputs = rig_inputs(&rig);
        assert_eq!(
            inputs,
            [
                RigInput {
                    id: Some("gaze_x".into()),
                    path: "gaze/left_right".into(),
                    label: Some("Gaze".into()),
                    group: Some("gaze".into()),
                    default_value: Some(0.0),
                    min: Some(-1.0),
                    max: Some(1.0),
                },
                RigInput {
                    id: Some("bare".into()),
                    path: "lids".into(),
                    label: None,
                    group: None,
                    default_value: None,
                    min: None,
                    max: None,
                },
            ]
        );
    }

    #[test]
    fn an_animation_without_a_duration_lasts_to_its_last_keyframe() {
        let bundle = json!({ "animations": [
            { "id": "wave", "clip": { "id": "inner", "name": "Wave", "tracks": [
                { "channel": "gaze/x", "interpolation": "cubic", "keyframes": [
                    { "time": 2.5, "value": 1 },
                    { "time": 0, "value": 0, "interpolation": "step" },
                    { "time": "late", "value": 3 },
                ] },
                { "keyframes": [] },
            ] } },
            { "clip": { "id": "timed", "duration": 5, "tracks": [] } },
            { "id": "empty" },
        ] });
        let animations = animations(&bundle);
        assert_eq!(animations.len(), 2);
        assert_eq!(animations[0].id, "wave");
        assert_eq!(animations[0].duration, 2.5);
        let tracks = &animations[0].tracks;
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].channel, "gaze/x");
        assert_eq!(tracks[0].interpolation.as_deref(), Some("cubic"));
        assert_eq!(
            tracks[0].keyframes,
            [
                Keyframe {
                    time: 0.0,
                    value: 0.0,
                    interpolation: Some("step".into()),
                },
                Keyframe {
                    time: 2.5,
                    value: 1.0,
                    interpolation: None,
                },
            ]
        );
        assert_eq!(animations[1].id, "timed");
        assert_eq!(animations[1].duration, 5.0);
    }

    /// An animation as [`Animation`] serializes reads back as itself: what a
    /// host hands back to replace an animation live is what `describe()` gave
    /// it.
    #[test]
    fn an_animation_on_its_own_reads_back_as_itself() {
        let bundle = json!({ "animations": [
            { "id": "wave", "clip": { "name": "Wave", "duration": 2, "tracks": [
                { "channel": "gaze/x", "interpolation": "step", "keyframes": [
                    { "time": 0, "value": 0 }, { "time": 2, "value": 1, "interpolation": "linear" },
                ] },
            ] } },
        ] });
        let parsed = animations(&bundle).remove(0);
        let alone = serde_json::to_value(&parsed).unwrap();
        assert_eq!(animation(&alone), Some(parsed));
        assert_eq!(animation(&json!({ "name": "no id" })), None);
    }

    #[test]
    fn fields_serialize_in_camel_case() {
        let input = &rig_inputs(&json!({ "metadata": { "vizij": { "inputs": [
            { "path": "x", "defaultValue": 1 },
        ] } } }))[0];
        let json = serde_json::to_value(input).unwrap();
        assert_eq!(json["defaultValue"], 1.0);
        let pose = Pose {
            id: "p".into(),
            name: None,
            description: None,
            group_ids: vec!["g".into()],
            values: BTreeMap::new(),
        };
        assert_eq!(
            serde_json::to_value(pose).unwrap()["groupIds"],
            json!(["g"])
        );
    }
}
