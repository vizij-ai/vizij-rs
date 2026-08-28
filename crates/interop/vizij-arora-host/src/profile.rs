//! Profiles: the **names and types** a standard defines, as data.
//!
//! A standard has two halves, and this module owns the first. A *profile* is
//! the set of typed store paths a standard defines — its vocabulary. A
//! *mapping* ([`crate::mappings`]) is the graph that carries one profile's
//! keys onto another's. Keeping them apart is what makes the chain checkable:
//! a mapping's input paths can be validated against the profile it claims to
//! consume, instead of being the only surviving record of what that profile
//! contains.
//!
//! The serialized shape is `arora-bridge-ws`'s `KeyInfo` — `{ path, kind,
//! value_type, min, max, default_value }` — so a profile round-trips through
//! the same descriptor the WS registry and the standalone app already speak.
//! A profile is that list plus an id, a version, and a description.
//!
//! Like the mapping assets, the committed JSON is generated from the constants
//! and a test fails when the two drift. That direction inverts later: the
//! asset becomes the definition and the constants read it.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};

use crate::ros4hri;
use crate::standard::{self, EXPRESSION_NAMES, FACE_CONTROLS, VISEME_SHAPES};

/// Per-control metadata the face standard carries beyond a path and a type:
/// the FACS action unit a muscle control expresses, and the ARKit blendshape
/// it corresponds to. Absent on controls that have neither.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KeyMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub au: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arkit: Option<String>,
    /// The tier this control belongs to, for profiles that define tiers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
}

/// One typed path in a profile — `KeyInfo` plus optional standard metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileKey {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<Json>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<KeyMeta>,
}

impl ProfileKey {
    /// A weight in `[0, 1]` resting at zero — the shape of most face controls.
    fn weight(path: String) -> Self {
        ProfileKey {
            path,
            kind: Some("input".into()),
            value_type: Some("f32".into()),
            min: Some(0.0),
            max: Some(1.0),
            default_value: Some(json!({ "f32": 0.0 })),
            meta: None,
        }
    }

    /// A bipolar control in `[-1, 1]` resting at zero.
    fn bipolar(path: &str) -> Self {
        ProfileKey {
            min: Some(-1.0),
            ..ProfileKey::weight(path.to_string())
        }
    }

    fn with_meta(mut self, meta: KeyMeta) -> Self {
        self.meta = Some(meta);
        self
    }
}

/// A profile's profile: its identity and every typed path it defines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub version: String,
    pub title: String,
    pub description: String,
    pub keys: Vec<ProfileKey>,
}

// --- The Vizij face standard ------------------------------------------------

/// The `vizij-face` profile: the standard's own vocabulary — gaze and lids,
/// the named expressions, the visemes, and the muscle tier — generated from
/// [`crate::standard`].
///
/// This is the full standard, not the subset any one mapping happens to reach.
/// The ROS4HRI mapping covers 33 of the 35 muscle controls (`jaw_left` and
/// `jaw_right` have no FACS code, so its action-unit channel cannot express
/// them); that difference is only computable because this set is declared
/// independently.
pub fn vizij_face_profile() -> Profile {
    let mut keys = Vec::new();

    let gaze = |path: &str, tier: &str| {
        ProfileKey::bipolar(path).with_meta(KeyMeta {
            tier: Some(tier.into()),
            ..KeyMeta::default()
        })
    };
    keys.push(gaze(standard::LEFT_EYE_POS_X, "gaze"));
    keys.push(gaze(standard::LEFT_EYE_POS_Y, "gaze"));
    keys.push(gaze(standard::RIGHT_EYE_POS_X, "gaze"));
    keys.push(gaze(standard::RIGHT_EYE_POS_Y, "gaze"));
    // Lids are unipolar: 0 open, 1 closed.
    for path in [
        standard::LEFT_EYE_TOP_EYELID_POS_Y,
        standard::RIGHT_EYE_TOP_EYELID_POS_Y,
    ] {
        keys.push(ProfileKey::weight(path.to_string()).with_meta(KeyMeta {
            tier: Some("gaze".into()),
            ..KeyMeta::default()
        }));
    }

    for name in EXPRESSION_NAMES {
        keys.push(
            ProfileKey::weight(standard::expression_path(name)).with_meta(KeyMeta {
                tier: Some("expression".into()),
                ..KeyMeta::default()
            }),
        );
    }

    for shape in VISEME_SHAPES {
        keys.push(
            ProfileKey::weight(standard::viseme_path(shape)).with_meta(KeyMeta {
                tier: Some("viseme".into()),
                ..KeyMeta::default()
            }),
        );
    }

    for control in FACE_CONTROLS.iter() {
        keys.push(
            ProfileKey::weight(standard::face_path(control.name)).with_meta(KeyMeta {
                au: control.au,
                arkit: Some(control.arkit.to_string()),
                tier: Some("muscle".into()),
            }),
        );
    }

    Profile {
        id: "vizij-face".into(),
        version: "v1".into(),
        title: "Vizij face standard".into(),
        description: "The portable face vocabulary: gaze and lids, 25 named expressions, \
                      15 visemes, and 35 muscle controls keyed to FACS action units and \
                      ARKit blendshapes."
            .into(),
        keys,
    }
}

// --- ROS4HRI ----------------------------------------------------------------

/// The `ros4hri` profile: what a ROS bridge writes and the ROS4HRI mapping
/// reads. Generated from [`crate::ros4hri`]'s key contract.
///
/// Note that the shipped ROS 2 exposure preset writes only the expression and
/// gaze keys — the action-unit and viseme keys are part of the contract and
/// have no topic feeding them. Declaring the set is what makes that visible.
pub fn ros4hri_profile() -> Profile {
    let mut keys = vec![
        ProfileKey {
            kind: Some("input".into()),
            value_type: Some("str".into()),
            min: None,
            max: None,
            default_value: Some(json!({ "str": "" })),
            ..ProfileKey::weight(ros4hri::EXPRESSION_NAME_KEY.to_string())
        },
        ProfileKey::bipolar(ros4hri::EXPRESSION_VALENCE_KEY),
        ProfileKey::bipolar(ros4hri::EXPRESSION_AROUSAL_KEY),
        ProfileKey {
            value_type: Some("value".into()),
            min: None,
            max: None,
            default_value: None,
            ..ProfileKey::weight(ros4hri::GAZE_TARGET_KEY.to_string())
        },
        ProfileKey {
            value_type: Some("str".into()),
            min: None,
            max: None,
            default_value: Some(json!({ "str": "" })),
            ..ProfileKey::weight(ros4hri::GAZE_FRAME_KEY.to_string())
        },
    ];

    // The action units the standard's muscle tier can express, in the order
    // the mapping enumerates them.
    let mut au_codes: Vec<u8> = FACE_CONTROLS.iter().filter_map(|c| c.au).collect();
    au_codes.sort_unstable();
    au_codes.dedup();
    for code in au_codes {
        keys.push(
            ProfileKey::weight(ros4hri::au_key(code)).with_meta(KeyMeta {
                au: Some(code),
                ..KeyMeta::default()
            }),
        );
    }

    for shape in VISEME_SHAPES {
        keys.push(ProfileKey::weight(ros4hri::viseme_key(shape)));
    }

    Profile {
        id: "ros4hri".into(),
        version: "v1".into(),
        title: "ROS4HRI face command".into(),
        description: "The ROS4HRI face-command surface: expression name with valence and \
                      arousal, a gaze target and its frame, FACS action-unit intensities, \
                      and viseme weights (a Vizij extension)."
            .into(),
        keys,
    }
}

// --- The registry -----------------------------------------------------------

/// The committed `vizij-face` asset.
pub const VIZIJ_FACE_JSON: &str = include_str!("../profiles/vizij-face.json");
/// The committed `ros4hri` asset.
pub const ROS4HRI_JSON: &str = include_str!("../profiles/ros4hri.json");

/// Every profile Vizij ships, as `(id, asset json)`.
pub const PROFILES: [(&str, &str); 2] = [
    ("vizij-face", VIZIJ_FACE_JSON),
    ("ros4hri", ROS4HRI_JSON),
];

/// A shipped profile by id, parsed. `None` for an unknown id.
pub fn profile(id: &str) -> Option<Profile> {
    let (_, json) = PROFILES.iter().find(|(known, _)| *known == id)?;
    serde_json::from_str(json).ok()
}

/// The registry as JSON — id, version, title, description and key count per
/// entry. What a CLI prints and an authoring picker lists.
pub fn profiles_json() -> Json {
    Json::Array(
        PROFILES
            .iter()
            .filter_map(|(id, _)| profile(id))
            .map(|set| {
                json!({
                    "id": set.id,
                    "version": set.version,
                    "title": set.title,
                    "description": set.description,
                    "keys": set.keys.len(),
                })
            })
            .collect(),
    )
}

/// Every path a profile defines — the cheap form for coverage checks.
pub fn paths(set: &Profile) -> Vec<&str> {
    set.keys.iter().map(|k| k.path.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed assets must equal what the generators produce, or the
    /// files on disk quietly stop describing the code that ships.
    #[test]
    fn committed_profiles_match_the_generators() {
        let vizij: Profile = serde_json::from_str(VIZIJ_FACE_JSON).expect("vizij-face parses");
        assert_eq!(vizij, vizij_face_profile(), "profiles/vizij-face.json is stale");
        let ros: Profile = serde_json::from_str(ROS4HRI_JSON).expect("ros4hri parses");
        assert_eq!(ros, ros4hri_profile(), "profiles/ros4hri.json is stale");
    }

    /// The counts are the standard's contract, so they are worth pinning
    /// independently of the generator that produces them.
    #[test]
    fn the_vizij_face_set_covers_every_tier_in_full() {
        let set = vizij_face_profile();
        let tier = |name: &str| {
            set.keys
                .iter()
                .filter(|k| k.meta.as_ref().and_then(|m| m.tier.as_deref()) == Some(name))
                .count()
        };
        assert_eq!(tier("gaze"), 6);
        assert_eq!(tier("expression"), 25);
        assert_eq!(tier("viseme"), 15);
        assert_eq!(tier("muscle"), 35);
        assert_eq!(set.keys.len(), 81);
    }

    /// Every muscle control carries the ARKit name it corresponds to; the AU
    /// code is optional because two of them (`jaw_left`, `jaw_right`) have no
    /// FACS code — which is exactly why ROS4HRI cannot reach them.
    #[test]
    fn muscle_controls_carry_their_arkit_and_au_metadata() {
        let set = vizij_face_profile();
        let muscle: Vec<&ProfileKey> = set
            .keys
            .iter()
            .filter(|k| k.meta.as_ref().and_then(|m| m.tier.as_deref()) == Some("muscle"))
            .collect();
        assert!(muscle.iter().all(|k| k
            .meta
            .as_ref()
            .and_then(|m| m.arkit.as_deref())
            .is_some()));
        let without_au = muscle
            .iter()
            .filter(|k| k.meta.as_ref().and_then(|m| m.au).is_none())
            .count();
        assert_eq!(without_au, 2, "only the two jaw-shift controls lack an AU");
    }

    /// The ROS4HRI set is the mapping's input contract: 5 named keys, one per
    /// distinct action unit, and one per viseme shape.
    #[test]
    fn the_ros4hri_set_matches_its_key_contract() {
        let set = ros4hri_profile();
        assert_eq!(set.keys.len(), 5 + 20 + 15);
        assert!(paths(&set).contains(&ros4hri::EXPRESSION_NAME_KEY));
        assert!(paths(&set).contains(&"standard/ros4hri/viseme/sil"));
    }

    #[test]
    fn the_registry_lists_both_profiles() {
        let listed = profiles_json();
        assert_eq!(listed[0]["id"], "vizij-face");
        assert_eq!(listed[1]["id"], "ros4hri");
        assert!(profile("vizij-face").is_some());
        assert!(profile("nope").is_none());
    }
}
