//! Profiles: the **interfaces** a standard declares, as data.
//!
//! A *profile* is an interface: the set of store paths one party exposes to
//! another, each with its type, range, and default. It declares nothing about
//! how those paths are produced or consumed — that is a *mapping*'s job
//! ([`crate::mappings`]): a graph that implements one profile in terms of
//! another. Keeping the two apart is what makes a chain checkable: a mapping's
//! `input` paths can be validated against the profile it claims to consume,
//! and its `output` paths against the one it claims to produce, instead of the
//! mapping being the only surviving record of either.
//!
//! The serialized key shape is `arora-bridge-ws`'s `KeyInfo` — `{ path, kind,
//! value_type, min, max, default_value }`, with `value_type` an arora
//! [`Type`] and `default_value` an arora [`Value`] — so a profile round-trips
//! through the same descriptor the WS registry and the standalone app already
//! speak. A profile is that list plus an id, a version, a description, and
//! the [`Scope`] its paths are addressed in.
//!
//! Like the mapping assets, the committed JSON is generated from the constants
//! and a test fails when the two drift. That direction inverts later: the
//! asset becomes the definition and the constants read it.

use arora_types::value::{Type, Value};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};

use crate::ros4hri;
use crate::standard::{self, EXPRESSION_NAMES, FACE_CONTROLS, VISEME_SHAPES};

/// Where a profile's paths live on the device's store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Paths are absolute: one instance per device, shared by every face
    /// (`standard/ros4hri/*`, as a bridge writes them).
    Device,
    /// Paths are relative to one face and take its rig prefix
    /// (`rig/<faceId>/`) when addressed — the standard face controls, which
    /// every face carries its own copy of.
    Face,
}

/// Standard metadata a key carries beyond a path and a type: the FACS action
/// unit a muscle control expresses, the ARKit blendshape it corresponds to,
/// and the tier it belongs to in profiles that define tiers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KeyMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub au: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arkit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
}

/// One typed path in a profile — `KeyInfo` plus optional standard metadata.
///
/// `kind` is the role of the key for the party implementing the profile:
/// `input` for a key a caller commands. A profile lifted from a mapping
/// ([`surface`]) carries the side it was lifted from instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileKey {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_type: Option<Type>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<KeyMeta>,
}

impl ProfileKey {
    /// A commanded key of `value_type` with no range and no default.
    fn input(path: impl Into<String>, value_type: Type) -> Self {
        ProfileKey {
            path: path.into(),
            kind: Some("input".into()),
            value_type: Some(value_type),
            min: None,
            max: None,
            default_value: None,
            meta: None,
        }
    }

    /// A weight in `[0, 1]` resting at zero — the shape of most face controls.
    fn weight(path: impl Into<String>) -> Self {
        ProfileKey {
            min: Some(0.0),
            max: Some(1.0),
            default_value: Some(Value::F32(0.0)),
            ..ProfileKey::input(path, Type::F32)
        }
    }

    /// A bipolar control in `[-1, 1]` resting at zero.
    fn bipolar(path: impl Into<String>) -> Self {
        ProfileKey {
            min: Some(-1.0),
            ..ProfileKey::weight(path)
        }
    }

    /// A string key resting empty.
    fn text(path: impl Into<String>) -> Self {
        ProfileKey {
            default_value: Some(Value::String(String::new())),
            ..ProfileKey::input(path, Type::String)
        }
    }

    fn with_meta(mut self, meta: KeyMeta) -> Self {
        self.meta = Some(meta);
        self
    }

    fn with_tier(self, tier: &str) -> Self {
        self.with_meta(KeyMeta {
            tier: Some(tier.into()),
            ..KeyMeta::default()
        })
    }

    /// The tier this key declares, if its profile defines tiers.
    pub fn tier(&self) -> Option<&str> {
        self.meta.as_ref().and_then(|m| m.tier.as_deref())
    }
}

/// A profile: its identity, the scope its paths are addressed in, and every
/// typed path it declares.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub version: String,
    pub title: String,
    pub description: String,
    pub scope: Scope,
    pub keys: Vec<ProfileKey>,
}

impl Profile {
    /// The profile addressed to one face: a [`Scope::Face`] profile gets
    /// `rig_prefix` (e.g. `rig/quori_latest/`) prepended to every path; a
    /// [`Scope::Device`] profile is returned unchanged, because its paths are
    /// not per face. An empty prefix leaves either untouched.
    pub fn with_rig_prefix(mut self, rig_prefix: &str) -> Profile {
        if self.scope == Scope::Face && !rig_prefix.is_empty() {
            for key in &mut self.keys {
                key.path = format!("{rig_prefix}{}", key.path);
            }
        }
        self
    }

    /// Every path the profile declares — the cheap form for coverage checks.
    pub fn paths(&self) -> Vec<&str> {
        self.keys.iter().map(|k| k.path.as_str()).collect()
    }

    /// The distinct tiers the profile's keys declare, in first-seen order —
    /// the standard's coarse-to-fine progression when the generator lists
    /// them that way.
    pub fn tiers(&self) -> Vec<&str> {
        let mut tiers: Vec<&str> = Vec::new();
        for tier in self.keys.iter().filter_map(ProfileKey::tier) {
            if !tiers.contains(&tier) {
                tiers.push(tier);
            }
        }
        tiers
    }
}

// --- The Vizij face standard ------------------------------------------------

/// The `vizij-face` profile: the standard's own interface — gaze and lids,
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

    for path in [
        standard::LEFT_EYE_POS_X,
        standard::LEFT_EYE_POS_Y,
        standard::RIGHT_EYE_POS_X,
        standard::RIGHT_EYE_POS_Y,
    ] {
        keys.push(ProfileKey::bipolar(path).with_tier("gaze"));
    }
    // Lids are unipolar: 0 open, 1 closed.
    for path in [
        standard::LEFT_EYE_TOP_EYELID_POS_Y,
        standard::RIGHT_EYE_TOP_EYELID_POS_Y,
    ] {
        keys.push(ProfileKey::weight(path).with_tier("gaze"));
    }
    for name in EXPRESSION_NAMES {
        keys.push(ProfileKey::weight(standard::expression_path(name)).with_tier("expression"));
    }
    for shape in VISEME_SHAPES {
        keys.push(ProfileKey::weight(standard::viseme_path(shape)).with_tier("viseme"));
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
        description: "The portable face interface: gaze and lids, 25 named expressions, \
                      15 visemes, and 35 muscle controls keyed to FACS action units and \
                      ARKit blendshapes."
            .into(),
        scope: Scope::Face,
        keys,
    }
}

// --- ROS4HRI ----------------------------------------------------------------

/// The `ros4hri` profile: the keys a ROS bridge writes and the ROS4HRI mapping
/// reads. Generated from [`crate::ros4hri`]'s key contract.
///
/// The shipped ROS 2 exposure preset feeds only the expression and gaze keys;
/// the action-unit and viseme keys are part of the interface and have no topic
/// behind them yet. Declaring the set is what makes that visible.
pub fn ros4hri_profile() -> Profile {
    let mut keys = vec![
        ProfileKey::text(ros4hri::EXPRESSION_NAME_KEY),
        ProfileKey::bipolar(ros4hri::EXPRESSION_VALENCE_KEY),
        ProfileKey::bipolar(ros4hri::EXPRESSION_AROUSAL_KEY),
        // A `vec3` structure (meters, face frame); no rest value — the mapping
        // holds its own far-ahead default until the key is written.
        ProfileKey::input(ros4hri::GAZE_TARGET_KEY, Type::Structure),
        ProfileKey::text(ros4hri::GAZE_FRAME_KEY),
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
        description: "The ROS4HRI face-command interface: expression name with valence and \
                      arousal, a gaze target and its frame, FACS action-unit intensities, \
                      and viseme weights (a Vizij extension)."
            .into(),
        scope: Scope::Device,
        keys,
    }
}

// --- The registry -----------------------------------------------------------

/// A shipped profile: its id, the committed asset, and the generator the
/// asset is regenerated from (`vizij-bundle export-profile <id>`).
pub struct ShippedProfile {
    pub id: &'static str,
    pub asset_json: &'static str,
    pub generate: fn() -> Profile,
}

/// Every profile Vizij ships.
pub const PROFILES: [ShippedProfile; 2] = [
    ShippedProfile {
        id: "vizij-face",
        asset_json: include_str!("../profiles/vizij-face.json"),
        generate: vizij_face_profile,
    },
    ShippedProfile {
        id: "ros4hri",
        asset_json: include_str!("../profiles/ros4hri.json"),
        generate: ros4hri_profile,
    },
];

/// A shipped profile's registry entry by id. `None` for an unknown id.
pub fn shipped(id: &str) -> Option<&'static ShippedProfile> {
    PROFILES.iter().find(|p| p.id == id)
}

/// A shipped profile by id, parsed from its committed asset. `None` for an
/// unknown id.
pub fn profile(id: &str) -> Option<Profile> {
    let entry = shipped(id)?;
    serde_json::from_str(entry.asset_json).ok()
}

/// The registry as JSON — id, version, title, description, scope and key
/// count per entry. What a CLI prints and an authoring picker lists.
pub fn profiles_json() -> Json {
    Json::Array(
        PROFILES
            .iter()
            .filter_map(|entry| profile(entry.id))
            .map(|p| {
                json!({
                    "id": p.id,
                    "version": p.version,
                    "title": p.title,
                    "description": p.description,
                    "scope": p.scope,
                    "keys": p.keys.len(),
                })
            })
            .collect(),
    )
}

// --- Lifting a profile out of a mapping ------------------------------------

/// Which side of a mapping graph [`surface`] lifts: the paths its `input`
/// nodes read, or the paths its `output` nodes write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Input,
    Output,
}

impl Side {
    /// The graph node type this side reads, and the `kind` the lifted keys
    /// carry.
    fn node_type(self) -> &'static str {
        match self {
            Side::Input => "input",
            Side::Output => "output",
        }
    }
}

impl std::str::FromStr for Side {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "input" => Ok(Side::Input),
            "output" => Ok(Side::Output),
            other => Err(format!("expected input or output, got {other}")),
        }
    }
}

/// Lift a profile out of a mapping graph: its `input` nodes are the interface
/// it consumes, its `output` nodes the interface it produces. `scope` is the
/// caller's: a graph does not say whether the paths it names are per face.
///
/// This is the bootstrap direction — how an existing mapping is reconciled
/// against a declared profile, and how a face's own adaptation reports the
/// interface it actually implements. It is deliberately *not* the source of
/// truth: a mapping only touches the part of a profile it needs, so a surface
/// lifted this way can be a strict subset of the profile it claims (ROS4HRI
/// reaches 33 of the standard's 35 muscle controls). Compare, do not replace.
pub fn surface(spec: &Json, side: Side, id: &str, scope: Scope) -> Profile {
    let mut keys: Vec<ProfileKey> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for node in spec
        .get("nodes")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        if node.get("type").and_then(Json::as_str) != Some(side.node_type()) {
            continue;
        }
        let Some(path) = node.pointer("/params/path").and_then(Json::as_str) else {
            continue;
        };
        if !seen.insert(path.to_string()) {
            continue;
        }
        // An input node's declared default types the key; an output node
        // declares nothing, so its type stays open.
        let value_type = match node.pointer("/params/value") {
            Some(v) if v.is_string() => Some(Type::String),
            Some(v) if v.is_boolean() => Some(Type::Boolean),
            Some(v) if v.is_number() => Some(Type::F32),
            Some(v) if v.is_object() => Some(Type::Structure),
            _ => None,
        };
        keys.push(ProfileKey {
            path: path.to_string(),
            kind: Some(side.node_type().to_string()),
            value_type,
            min: None,
            max: None,
            default_value: None,
            meta: None,
        });
    }
    keys.sort_by(|a, b| a.path.cmp(&b.path));
    let side_name = side.node_type();
    Profile {
        id: id.to_string(),
        version: "v1".to_string(),
        title: format!("{id} ({side_name} surface)"),
        description: format!(
            "The {side_name} surface lifted from a mapping graph — the paths it actually \
             touches, which may be a subset of the profile it claims."
        ),
        scope,
        keys,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed assets must equal what the generators produce, or the
    /// files on disk quietly stop describing the code that ships.
    #[test]
    fn committed_profiles_match_the_generators() {
        for entry in &PROFILES {
            let committed: Profile =
                serde_json::from_str(entry.asset_json).expect("the asset parses");
            assert_eq!(
                committed,
                (entry.generate)(),
                "profiles/{}.json is stale",
                entry.id
            );
        }
    }

    /// The counts are the standard's contract, so they are worth pinning
    /// independently of the generator that produces them.
    #[test]
    fn the_vizij_face_profile_covers_every_tier_in_full() {
        let face = vizij_face_profile();
        let tier = |name: &str| face.keys.iter().filter(|k| k.tier() == Some(name)).count();
        assert_eq!(tier("gaze"), 6);
        assert_eq!(tier("expression"), 25);
        assert_eq!(tier("viseme"), 15);
        assert_eq!(tier("muscle"), 35);
        assert_eq!(face.keys.len(), 81);
        assert_eq!(face.tiers(), ["gaze", "expression", "viseme", "muscle"]);
        assert_eq!(face.scope, Scope::Face);
    }

    /// Every muscle control carries the ARKit name it corresponds to; the AU
    /// code is optional because two of them (`jaw_left`, `jaw_right`) have no
    /// FACS code — which is exactly why ROS4HRI cannot reach them.
    #[test]
    fn muscle_controls_carry_their_arkit_and_au_metadata() {
        let face = vizij_face_profile();
        let muscle: Vec<&ProfileKey> = face
            .keys
            .iter()
            .filter(|k| k.tier() == Some("muscle"))
            .collect();
        assert!(muscle
            .iter()
            .all(|k| k.meta.as_ref().and_then(|m| m.arkit.as_deref()).is_some()));
        let without_au = muscle
            .iter()
            .filter(|k| k.meta.as_ref().and_then(|m| m.au).is_none())
            .count();
        assert_eq!(without_au, 2, "only the two jaw-shift controls lack an AU");
    }

    /// The ROS4HRI profile is the mapping's input contract: 5 named keys, one
    /// per distinct action unit, and one per viseme shape — device-global.
    #[test]
    fn the_ros4hri_profile_matches_its_key_contract() {
        let ros = ros4hri_profile();
        assert_eq!(ros.keys.len(), 5 + 20 + 15);
        assert_eq!(ros.scope, Scope::Device);
        assert!(ros.paths().contains(&ros4hri::EXPRESSION_NAME_KEY));
        assert!(ros.paths().contains(&"standard/ros4hri/viseme/sil"));
        let target = ros
            .keys
            .iter()
            .find(|k| k.path == ros4hri::GAZE_TARGET_KEY)
            .unwrap();
        assert_eq!(target.value_type, Some(Type::Structure));
    }

    /// Only a face-scoped profile is addressed to a face; a device-scoped
    /// one keeps its absolute paths whatever prefix the caller passes.
    #[test]
    fn the_rig_prefix_applies_to_face_scoped_profiles_only() {
        let face = vizij_face_profile().with_rig_prefix("rig/f/");
        assert!(face
            .paths()
            .iter()
            .all(|p| p.starts_with("rig/f/standard/vizij/")));
        let ros = ros4hri_profile().with_rig_prefix("rig/f/");
        assert_eq!(ros, ros4hri_profile());
        assert_eq!(
            vizij_face_profile().with_rig_prefix(""),
            vizij_face_profile()
        );
    }

    #[test]
    fn the_registry_lists_both_profiles() {
        let listed = profiles_json();
        assert_eq!(listed[0]["id"], "vizij-face");
        assert_eq!(listed[0]["scope"], "face");
        assert_eq!(listed[1]["id"], "ros4hri");
        assert_eq!(listed[1]["scope"], "device");
        assert!(profile("vizij-face").is_some());
        assert!(profile("nope").is_none());
        assert!(shipped("nope").is_none());
    }

    /// The shipped mapping reads the whole `ros4hri` profile but `gaze/frame`
    /// (the look_at skill consumes it) and writes the whole `vizij-face`
    /// profile but the two AU-less jaw controls — plus one path no profile
    /// declares.
    #[test]
    fn surface_reconciles_the_ros4hri_mapping_against_the_profiles() {
        let spec: Json = serde_json::from_str(ros4hri::MAPPING_JSON).unwrap();

        let consumed = surface(&spec, Side::Input, "ros4hri", Scope::Device);
        let declared = ros4hri_profile();
        let unread: Vec<&str> = declared
            .paths()
            .into_iter()
            .filter(|p| !consumed.paths().contains(p))
            .collect();
        assert_eq!(unread, [ros4hri::GAZE_FRAME_KEY]);
        assert!(consumed
            .paths()
            .iter()
            .all(|p| declared.paths().contains(p)));

        let produced = surface(&spec, Side::Output, "vizij-face", Scope::Face);
        let declared = vizij_face_profile();
        let unwritten: Vec<&str> = declared
            .paths()
            .into_iter()
            .filter(|p| !produced.paths().contains(p))
            .collect();
        assert_eq!(
            unwritten,
            [
                "standard/vizij/face/jaw_left",
                "standard/vizij/face/jaw_right"
            ]
        );
        let undeclared: Vec<&str> = produced
            .paths()
            .into_iter()
            .filter(|p| !declared.paths().contains(p))
            .collect();
        assert_eq!(undeclared, ["standard/vizij/mouth/morph/jaw_open"]);
    }
}
