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
use serde_json::{json, Map, Value as Json};

use crate::ros4hri;
use crate::skills;
use crate::standard::{self, CONVERSATION_STATES, FACE_CONTROLS, VISEME_SHAPES};

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
/// `input` for a key a caller commands, `output` for state the implementer
/// reports and a caller follows. A profile lifted from a mapping
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

    /// A string state key the implementer reports, `rest` when nothing is
    /// happening.
    fn state(path: impl Into<String>, rest: &str) -> Self {
        ProfileKey {
            kind: Some("output".into()),
            default_value: Some(Value::String(rest.to_string())),
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

    /// The paths of one `kind` — `input` for the keys a caller commands,
    /// `output` for the state the implementer reports.
    pub fn paths_of(&self, kind: &str) -> Vec<&str> {
        self.keys
            .iter()
            .filter(|k| k.kind.as_deref() == Some(kind))
            .map(|k| k.path.as_str())
            .collect()
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

/// The `vizij-face` profile: the standard's own interface — gaze, lids,
/// blink and the gaze target with its weight, the named expressions, the visemes, the muscle
/// tier, and the conversation state — generated from [`crate::standard`].
/// The keys run in tier order, so [`Profile::tiers`] lists `gaze`,
/// `expression`, `viseme`, `muscle`, `conversation`.
///
/// This is the full standard, not the subset any one mapping happens to reach.
/// The ROS4HRI mapping writes none of the muscle tier, since ROS4HRI defines
/// no channel commanding action units; that difference is only computable
/// because this set is declared independently.
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
    keys.push(ProfileKey::weight(standard::BLINK).with_tier("gaze"));
    // The gaze target: a point on the face's frame, resting at its centre,
    // and how strongly the eyes hold it, resting at 0.
    for path in [standard::GAZE_TARGET_X, standard::GAZE_TARGET_Y] {
        keys.push(
            ProfileKey {
                default_value: Some(Value::F32(0.5)),
                ..ProfileKey::weight(path)
            }
            .with_tier("gaze"),
        );
    }
    keys.push(ProfileKey::weight(standard::GAZE_TARGET_WEIGHT).with_tier("gaze"));
    for name in standard::expression_names() {
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
    // The de-facto jaw-open path: the same muscle as `face/jaw_open`, under
    // the name every current face implements.
    keys.push(
        ProfileKey::weight(standard::MOUTH_JAW_OPEN).with_meta(KeyMeta {
            au: Some(26),
            arkit: Some("jawOpen".into()),
            tier: Some("muscle".into()),
        }),
    );
    for state in CONVERSATION_STATES {
        keys.push(ProfileKey::weight(standard::conversation_path(state)).with_tier("conversation"));
    }
    // The face's settings rather than controls: the gain its adaptation
    // scales the viseme weights by, and how far apart its eyes are and how
    // far they turn, which a mapping aiming them at a point in space reads.
    // Each rests where the face authors it, at these when it authors none.
    // No tier — the tiers grade controls.
    let (min_range, max_range) = standard::GAZE_RANGE_SPAN;
    for (path, rest, min, max) in [
        (standard::LIPSYNC_GAIN, 1.0, 0.0, None),
        (
            standard::GAZE_EYE_SPACING,
            standard::GAZE_EYE_SPACING_REST,
            0.0,
            None,
        ),
        (
            standard::GAZE_RANGE,
            standard::GAZE_RANGE_REST,
            min_range,
            Some(max_range),
        ),
    ] {
        keys.push(ProfileKey {
            min: Some(min),
            max,
            default_value: Some(Value::F32(rest as f32)),
            ..ProfileKey::input(path, Type::F32)
        });
    }
    // The caption the host shows over the face, and who says it: a command
    // of the face's display, read by the host rather than the face's graphs.
    // No tier.
    keys.push(ProfileKey::text(standard::CAPTION_TEXT));
    keys.push(ProfileKey::text(standard::CAPTION_SPEAKER));
    // What the face reports rather than takes: the lipsync state the viseme
    // players write. No tier, for the same reason.
    keys.push(ProfileKey::state(standard::VISEME, skills::SILENCE_VISEME));
    keys.push(ProfileKey::state(standard::SPEECH, ""));

    Profile {
        id: "vizij-face".into(),
        version: "v1".into(),
        title: "Vizij face standard".into(),
        description: "The portable face interface: gaze, lids, blink and the gaze \
                      target with its weight, 27 named expressions, 15 visemes, 36 muscle controls keyed \
                      to FACS action units and ARKit blendshapes (35 named per \
                      FACS/ARKit, plus the de-facto jaw-open path), and the conversation \
                      state (speaking, user speaking, thinking); its settings, the \
                      lipsync gain, the eyes' spacing and how far they turn; the \
                      caption shown over it and who says it; and the speech state the \
                      face reports, its current viseme and the utterance being spoken."
            .into(),
        scope: Scope::Face,
        keys,
    }
}

// --- ROS4HRI ----------------------------------------------------------------

/// The `ros4hri` profile: the keys a ROS bridge writes and the ROS4HRI mapping
/// reads, and the one the mapping writes and the bridge publishes. Generated
/// from [`crate::ros4hri`]'s key contract.
///
/// The inputs are the expression, gaze and viseme keys the shipped ROS 2
/// exposure preset feeds. ROS4HRI publishes facial action units only as
/// perception output (`hri_msgs/FacialActionUnits` per detected face) and
/// defines no command for them, so the profile carries none; a FACS-granular
/// pose is the `vizij-face` muscle tier's. A closed caption — its text and
/// its speaker — is an input too, which the mapping relays to the face's
/// caption. The speech state is the output: what the face is saying, for
/// subtitles, and the lip shape it is saying it with.
pub fn ros4hri_profile() -> Profile {
    let keys = vec![
        ProfileKey::text(ros4hri::EXPRESSION_NAME_KEY),
        ProfileKey::bipolar(ros4hri::EXPRESSION_VALENCE_KEY),
        ProfileKey::bipolar(ros4hri::EXPRESSION_AROUSAL_KEY),
        // A `vec3` structure (meters, face frame); no rest value — the mapping
        // holds its own far-ahead default until the key is written.
        ProfileKey::input(ros4hri::GAZE_TARGET_KEY, Type::Structure),
        ProfileKey::text(ros4hri::GAZE_FRAME_KEY),
        // The lip shape at the audio playhead, as ROS4HRI codes it: an index
        // into the standard's shapes, resting at `sil`.
        ProfileKey {
            min: Some(0.0),
            max: Some((VISEME_SHAPES.len() - 1) as f64),
            default_value: Some(Value::U8(0)),
            ..ProfileKey::input(ros4hri::VISEME_KEY, Type::U8)
        },
        // Where that viseme falls in the utterance, and how long it holds,
        // seconds.
        ProfileKey {
            min: Some(0.0),
            default_value: Some(Value::F32(0.0)),
            ..ProfileKey::input(ros4hri::VISEME_TIME_KEY, Type::F32)
        },
        ProfileKey {
            min: Some(0.0),
            default_value: Some(Value::F32(0.0)),
            ..ProfileKey::input(ros4hri::VISEME_DURATION_KEY, Type::F32)
        },
        // A closed caption: its text and who says it, as the ROS4HRI message
        // carries them.
        ProfileKey::text(ros4hri::CAPTION_TEXT_KEY),
        ProfileKey::text(ros4hri::CAPTION_SPEAKER_KEY),
        ProfileKey::state(ros4hri::SPEECH_TEXT_KEY, ""),
        ProfileKey::state(ros4hri::SPEECH_VISEME_KEY, skills::SILENCE_VISEME),
    ];
    Profile {
        id: "ros4hri".into(),
        version: "v1".into(),
        title: "ROS4HRI face command".into(),
        description: "The ROS4HRI face interface: expression name with valence and \
                      arousal, a gaze target and its frame, the streamed viseme and the \
                      closed caption commanded; the utterance being spoken and its \
                      current viseme reported."
            .into(),
        scope: Scope::Device,
        keys,
    }
}

// --- Coverage ---------------------------------------------------------------

/// One tier of the standard-coverage report.
pub struct TierCoverage {
    pub tier: String,
    pub covered: Vec<String>,
    pub missing: Vec<String>,
}

/// The tiers the compliance level climbs, coarse to fine: L0 gaze & lids, L1
/// expressions, L2 visemes, L3 the muscle tier. The profile's other tiers —
/// the conversation state — are reported but not climbed: they say what a
/// face reacts to, not how finely it can be posed.
const LEVEL_TIERS: [&str; 4] = ["gaze", "expression", "viseme", "muscle"];

/// The paths a tier reports without requiring them for its level: the gaze
/// target and its weight, which say where the eyes point in terms of the eye
/// positions the gaze tier already grades — every gaze they direct, the eye
/// positions reach.
const UNGRADED: [&str; 3] = [
    standard::GAZE_TARGET_X,
    standard::GAZE_TARGET_Y,
    standard::GAZE_TARGET_WEIGHT,
];

/// A face's coverage of the Vizij standard: which paths of each tier of the
/// `vizij-face` profile its graphs listen on. `level` is the highest tier of
/// [`LEVEL_TIERS`] the face fully covers, in order (half is enough for the
/// muscle tier: faces rig the muscles they have), [`UNGRADED`] paths aside.
pub struct Coverage {
    pub face_id: Option<String>,
    pub level: u8,
    pub tiers: Vec<TierCoverage>,
}

/// A face's standard coverage from its input surface — the standard paths its
/// graphs read, rig prefix stripped ([`crate::Bundle::input_paths`]) — tier
/// by tier of the `vizij-face` profile.
pub fn coverage(face_id: Option<String>, inputs: &[String]) -> Coverage {
    let has = |path: &str| inputs.iter().any(|p| p == path);

    let standard = vizij_face_profile();
    let tiers: Vec<TierCoverage> = standard
        .tiers()
        .into_iter()
        .map(|tier| {
            let (covered, missing) = standard
                .keys
                .iter()
                .filter(|k| k.tier() == Some(tier))
                .map(|k| k.path.clone())
                .partition(|p| has(p));
            TierCoverage {
                tier: tier.to_string(),
                covered,
                missing,
            }
        })
        .collect();

    // Each level requires every tier before it in full, and its own tier in
    // full — or, for the muscle tier, at least half — but for the paths it
    // reports ungraded.
    let mut level = 0;
    let ladder = tiers
        .iter()
        .filter(|t| LEVEL_TIERS.contains(&t.tier.as_str()));
    for (i, tier) in ladder.enumerate() {
        let missing = tier
            .missing
            .iter()
            .filter(|p| !UNGRADED.contains(&p.as_str()))
            .count();
        let full = missing == 0;
        let enough = tier.tier == "muscle" && tier.covered.len() >= missing;
        if !(full || enough) {
            break;
        }
        level = i as u8;
    }

    Coverage {
        face_id,
        level,
        tiers,
    }
}

impl Coverage {
    /// The report as JSON (the machine-readable `validate` output).
    pub fn to_json(&self) -> Json {
        let mut tiers = Map::new();
        for t in &self.tiers {
            tiers.insert(
                t.tier.clone(),
                json!({
                    "covered": t.covered.len(),
                    "of": t.covered.len() + t.missing.len(),
                    "missing": t.missing,
                }),
            );
        }
        json!({
            "faceId": self.face_id,
            "level": format!("L{}", self.level),
            "tiers": tiers,
        })
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
/// reaches the gaze and expression tiers, none of the muscle tier). Compare,
/// do not replace.
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
        assert_eq!(tier("gaze"), 10);
        assert_eq!(tier("expression"), 27);
        assert_eq!(tier("viseme"), 15);
        assert_eq!(tier("muscle"), 36);
        assert_eq!(tier("conversation"), 3);
        assert_eq!(face.keys.len(), 98);
        assert_eq!(
            face.tiers(),
            ["gaze", "expression", "viseme", "muscle", "conversation"]
        );
        assert_eq!(
            face.paths_of("output"),
            [standard::VISEME, standard::SPEECH],
            "the speech state the face reports, outside the control tiers"
        );
        // The face's settings: non-negative `f32` inputs outside the control
        // tiers, each resting where a face that authors none rests — the
        // lipsync gain at 1, the eyes 6 cm apart and turning ±0.78 rad, of
        // at most 0.9 rad.
        for (path, rest, min, max) in [
            (standard::LIPSYNC_GAIN, 1.0, 0.0, None),
            (standard::GAZE_EYE_SPACING, 0.06, 0.0, None),
            (standard::GAZE_RANGE, 0.78, 0.01, Some(0.9)),
        ] {
            let setting = face
                .keys
                .iter()
                .find(|k| k.path == path)
                .unwrap_or_else(|| panic!("the face's setting {path}"));
            assert_eq!(setting.kind.as_deref(), Some("input"), "{path}");
            assert_eq!(setting.value_type, Some(Type::F32), "{path}");
            assert_eq!((setting.min, setting.max), (Some(min), max), "{path}");
            assert_eq!(setting.tier(), None, "{path}: a setting, in no tier");
            assert_eq!(setting.default_value, Some(Value::F32(rest)), "{path}");
        }
        assert_eq!(
            standard::FACE_SETTINGS.map(|path| face.paths().contains(&path)),
            [true; 3]
        );
        // The caption is a command of the face's display: a string input, in
        // no tier.
        for path in [standard::CAPTION_TEXT, standard::CAPTION_SPEAKER] {
            let caption = face.keys.iter().find(|k| k.path == path).unwrap();
            assert_eq!(caption.kind.as_deref(), Some("input"), "{path}");
            assert_eq!(caption.value_type, Some(Type::String), "{path}");
            assert_eq!(caption.tier(), None, "{path}");
        }
        assert_eq!(face.scope, Scope::Face);
    }

    /// The blink, the gaze target's weight and the conversation state are
    /// weights resting at zero — open eyes, no target held, nobody speaking,
    /// nothing pending — in their tiers.
    #[test]
    fn blink_gaze_weight_and_conversation_keys_are_weights_resting_at_zero() {
        let face = vizij_face_profile();
        let key = |path: &str| {
            face.keys
                .iter()
                .find(|k| k.path == path)
                .unwrap_or_else(|| panic!("{path} is declared"))
        };
        let mut expected = vec![
            (standard::BLINK.to_string(), "gaze"),
            (standard::GAZE_TARGET_WEIGHT.to_string(), "gaze"),
        ];
        expected.extend(
            standard::VIZIJ_EXPRESSION_NAMES
                .iter()
                .map(|n| (standard::expression_path(n), "expression")),
        );
        expected.extend(
            CONVERSATION_STATES
                .iter()
                .map(|s| (standard::conversation_path(s), "conversation")),
        );
        for (path, tier) in expected {
            let k = key(&path);
            assert_eq!(k.tier(), Some(tier), "{path}");
            assert_eq!(k.kind.as_deref(), Some("input"), "{path}");
            assert_eq!(k.value_type, Some(Type::F32), "{path}");
            assert_eq!((k.min, k.max), (Some(0.0), Some(1.0)), "{path}");
            assert_eq!(k.default_value, Some(Value::F32(0.0)), "{path}");
        }
    }

    /// The gaze target is a point on the face's frame, each coordinate in
    /// `[0, 1]`, resting at the centre: a face nobody directs looks ahead.
    #[test]
    fn the_gaze_target_rests_at_the_centre_of_the_frame() {
        let face = vizij_face_profile();
        for path in [standard::GAZE_TARGET_X, standard::GAZE_TARGET_Y] {
            let k = face
                .keys
                .iter()
                .find(|k| k.path == path)
                .unwrap_or_else(|| panic!("{path} is declared"));
            assert_eq!(k.tier(), Some("gaze"), "{path}");
            assert_eq!(k.kind.as_deref(), Some("input"), "{path}");
            assert_eq!(k.value_type, Some(Type::F32), "{path}");
            assert_eq!((k.min, k.max), (Some(0.0), Some(1.0)), "{path}");
            assert_eq!(k.default_value, Some(Value::F32(0.5)), "{path}");
        }
    }

    /// Every muscle-tier key carries the ARKit name it corresponds to; the AU
    /// code is optional because two of them (`jaw_left`, `jaw_right`) have no
    /// FACS code.
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

    /// The ROS4HRI profile is the mapping's contract: 10 named input keys and
    /// the speech state it reports (the utterance and its current viseme) —
    /// device-global, and no action-unit keys, which ROS4HRI does not command.
    #[test]
    fn the_ros4hri_profile_matches_its_key_contract() {
        let ros = ros4hri_profile();
        assert_eq!(ros.keys.len(), 10 + 2);
        assert!(ros.keys.iter().all(|k| k.meta.is_none()));
        assert_eq!(ros.scope, Scope::Device);
        assert!(ros.paths().contains(&ros4hri::EXPRESSION_NAME_KEY));
        assert_eq!(
            ros.paths_of("output"),
            [ros4hri::SPEECH_TEXT_KEY, ros4hri::SPEECH_VISEME_KEY]
        );
        let target = ros
            .keys
            .iter()
            .find(|k| k.path == ros4hri::GAZE_TARGET_KEY)
            .unwrap();
        assert_eq!(target.value_type, Some(Type::Structure));
        // One ROS4HRI code, not a weight per shape: the face's shapes are the
        // `vizij-face` side of the mapping.
        let viseme = ros
            .keys
            .iter()
            .find(|k| k.path == ros4hri::VISEME_KEY)
            .unwrap();
        assert_eq!(viseme.value_type, Some(Type::U8));
        assert!(!ros.paths().iter().any(|p| {
            VISEME_SHAPES
                .iter()
                .any(|shape| p.ends_with(&format!("/viseme/{shape}")))
        }));
        // With its timing beside it.
        for key in [ros4hri::VISEME_TIME_KEY, ros4hri::VISEME_DURATION_KEY] {
            let timing = ros.keys.iter().find(|k| k.path == key).unwrap();
            assert_eq!(timing.value_type, Some(Type::F32), "{key}");
        }
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

    /// The shipped mapping reads the whole `ros4hri` command surface but
    /// `gaze/frame` (the look_at skill consumes it), plus the face's speech
    /// state and its gaze settings, and
    /// writes the whole `vizij-face` control surface but what ROS4HRI has no
    /// channel for, plus the ROS4HRI speech state — and nothing either profile
    /// does not declare on that side. Which keys are declared and unmapped is
    /// the point of declaring them apart from the mapping.
    #[test]
    fn surface_reconciles_the_ros4hri_mapping_against_the_profiles() {
        let spec: Json = serde_json::from_str(ros4hri::MAPPING_JSON).unwrap();
        let ros = ros4hri_profile();
        let face = vizij_face_profile();

        // What the mapping reads: the ROS4HRI inputs, the face's outputs, and
        // the face's settings its gaze channel aims the eyes with.
        let consumed = surface(&spec, Side::Input, "ros4hri", Scope::Device);
        let unread: Vec<&str> = ros
            .paths_of("input")
            .into_iter()
            .filter(|p| !consumed.paths().contains(p))
            .collect();
        assert_eq!(unread, [ros4hri::GAZE_FRAME_KEY]);
        for setting in [standard::GAZE_EYE_SPACING, standard::GAZE_RANGE] {
            assert!(consumed.paths().contains(&setting), "{setting} unread");
        }
        let readable = [
            ros.paths_of("input"),
            face.paths_of("output"),
            vec![standard::GAZE_EYE_SPACING, standard::GAZE_RANGE],
        ]
        .concat();
        let unreadable: Vec<&str> = consumed
            .paths()
            .into_iter()
            .filter(|p| !readable.contains(p))
            .collect();
        assert!(unreadable.is_empty(), "undeclared inputs: {unreadable:?}");

        // What the mapping writes: the face's inputs and the ROS4HRI outputs.
        let produced = surface(&spec, Side::Output, "vizij-face", Scope::Face);
        let unwritten: Vec<&str> = face
            .paths_of("input")
            .into_iter()
            .filter(|p| !produced.paths().contains(p))
            .collect();
        // Unwritten, in profile order: the caller's blink (the mapping's
        // idle blink drives the lids), the gaze target and its weight (the
        // mapping drives the eye positions from ROS4HRI's own target, a point
        // in space), the expressions ROS4HRI does not name, the lipsync
        // surface (the viseme players own it), the muscle tier (ROS4HRI
        // commands no action units), the conversation state, and the face's
        // settings (what the face is like, not a command). The caption, in
        // no tier, is written: a closed caption relays to it.
        let beyond_ros4hri: Vec<String> = standard::VIZIJ_EXPRESSION_NAMES
            .iter()
            .map(|n| standard::expression_path(n))
            .collect();
        let expected: Vec<&str> =
            face.keys
                .iter()
                .filter(|k| k.kind.as_deref() == Some("input"))
                .filter(|k| match k.tier() {
                    Some("gaze") => [
                        standard::BLINK,
                        standard::GAZE_TARGET_X,
                        standard::GAZE_TARGET_Y,
                        standard::GAZE_TARGET_WEIGHT,
                    ]
                    .contains(&k.path.as_str()),
                    Some("expression") => beyond_ros4hri.contains(&k.path),
                    Some("viseme" | "muscle" | "conversation") => true,
                    None => ![standard::CAPTION_TEXT, standard::CAPTION_SPEAKER]
                        .contains(&k.path.as_str()),
                    Some(other) => panic!("unexpected tier {other}"),
                })
                .map(|k| k.path.as_str())
                .collect();
        assert_eq!(unwritten, expected);
        assert_eq!(
            ros.paths_of("output")
                .into_iter()
                .filter(|p| !produced.paths().contains(p))
                .count(),
            0,
            "the ROS4HRI outputs are all written"
        );
        let writable = [face.paths_of("input"), ros.paths_of("output")].concat();
        let undeclared: Vec<&str> = produced
            .paths()
            .into_iter()
            .filter(|p| !writable.contains(p))
            .collect();
        assert!(undeclared.is_empty(), "undeclared outputs: {undeclared:?}");
    }
}
