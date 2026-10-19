//! The Vizij face standard: the store paths a compliant face implements.
//!
//! Faces are driven through named controls on the store, under the
//! `standard/vizij/` prefix. The pose vocabulary has three tiers, from coarse
//! to fine; a face implements what it implements, and standard mappings (like
//! the ROS4HRI one) degrade to the tiers a face covers. Beside them, the
//! conversation state tells a face what the dialogue it takes part in is
//! doing. The same vocabulary is declared as data — the `vizij-face` profile
//! in [`crate::profile`] — and a test holds the two equal:
//!
//! - **Gaze & lids** — per-eye position and eyelid controls, the blink, and
//!   the gaze target: the point on the face's frame the eyes look toward, and
//!   how strongly they hold it.
//! - **Semantic** — one weight per named expression and per viseme shape. The
//!   expression names are ROS4HRI's set plus the ones faces implement beyond
//!   it; the viseme shapes are the industry 15-shape set (the Oculus/Meta
//!   convention).
//! - **Muscle** — fine-grained face controls cherry-picked from FACS action
//!   units and ARKit blendshapes: FACS supplies the taxonomy (each control
//!   names an action unit, so an `hri_msgs/FacialActionUnits` message maps
//!   losslessly), ARKit supplies lateralization and naming familiarity (its
//!   blendshape arrays carry left/right where the FACS message cannot, and
//!   assets in the wild ship ARKit-named morph targets). Controls exist for
//!   the union that makes sense on a commanded robot face; ARKit's
//!   tracking-only shapes (`eyeLook*` — redundant with the gaze tier) and
//!   FACS codes a command channel cannot express (visibility, head/eye
//!   movement — owned by the gaze tier) have none.
//! - **Conversation** — whether the face is speaking, its interlocutor is
//!   speaking, or it is thinking: state a face may react to, not a pose.
//!
//! Beside the face's own vocabulary, the **caption** is text the host shows
//! over the face — a command of its display, not of its rig — and the
//! face's **settings** ([`FACE_SETTINGS`]) say what the face is like: its
//! lipsync gain, how far apart its eyes are and how far they turn.
//!
//! Everything is an `f32` weight in [0, 1] unless a constant says otherwise.

/// Prefix of every control path in the Vizij face standard.
pub const VIZIJ_PREFIX: &str = "standard/vizij";

/// Prepend `rig_prefix` to every standard control a graph spec reads or
/// writes (its `input` and `output` nodes on `standard/vizij/…`), leaving
/// any other path alone. Faces namespace their controls under their rig
/// (`rig/quori_latest/`), so a profile or a skill fragment authored against
/// the bare standard takes the face's prefix when it composes; `""` leaves
/// the spec unprefixed.
pub fn prefix_controls(spec: &mut serde_json::Value, rig_prefix: &str) {
    if rig_prefix.is_empty() {
        return;
    }
    for node in spec
        .get_mut("nodes")
        .and_then(serde_json::Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if !matches!(
            node.get("type").and_then(serde_json::Value::as_str),
            Some("output" | "input")
        ) {
            continue;
        }
        if let Some(path) = node.pointer_mut("/params/path") {
            if let Some(p) = path.as_str() {
                if p.starts_with(VIZIJ_PREFIX) {
                    *path = serde_json::Value::String(format!("{rig_prefix}{p}"));
                }
            }
        }
    }
}

// --- Gaze & lids ------------------------------------------------------------

/// Eye position controls, normalized [-1, 1]: `pos/x` is the wearer's
/// left-to-right, `pos/y` is down-to-up.
pub const LEFT_EYE_POS_X: &str = "standard/vizij/left_eye/pos/x";
pub const LEFT_EYE_POS_Y: &str = "standard/vizij/left_eye/pos/y";
pub const RIGHT_EYE_POS_X: &str = "standard/vizij/right_eye/pos/x";
pub const RIGHT_EYE_POS_Y: &str = "standard/vizij/right_eye/pos/y";

/// Top-eyelid positions, [0, 1]: 0 fully open, 1 fully closed.
pub const LEFT_EYE_TOP_EYELID_POS_Y: &str = "standard/vizij/left_eye_top_eyelid/pos/y";
pub const RIGHT_EYE_TOP_EYELID_POS_Y: &str = "standard/vizij/right_eye_top_eyelid/pos/y";

/// The blink, [0, 1]: 0 open, 1 both eyes closed. A level, not a trigger: the
/// writer shapes each blink over time (the close and the reopen, ≈0.2 s in
/// all), and the face renders the weight as is, with no transition of its
/// own — the same split as the visemes, which keeps a face's adaptation
/// stateless. It is independent of the eyelid positions, which keep their
/// meaning: a face closes each lid at least as far as the stronger of the
/// two, so a blink passes over lids held half-closed and returns them there.
pub const BLINK: &str = "standard/vizij/blink";

/// The gaze target, a point on the face's frame in screen conventions: `x`
/// from its left edge (0) to its right edge (1), `y` from its top edge (0) to
/// its bottom edge (1), as the viewer sees the face. Rests at the centre,
/// (0.5, 0.5): straight ahead.
///
/// It says where the eyes point, so a caller directs the gaze of a face
/// without knowing how the face moves its eyes. The face implements it as its
/// author chooses, like any control — its program blends the target with its
/// own eye motion, or its standard adaptation adds the target's offset to the
/// eye positions, statelessly — but never both, so that the eye positions
/// keep one writer. Toward the edges of the frame the eyes reach the ends of
/// their range.
pub const GAZE_TARGET_X: &str = "standard/vizij/gaze/target/x";
pub const GAZE_TARGET_Y: &str = "standard/vizij/gaze/target/y";

/// How strongly the eyes hold the gaze target, [0, 1], resting at 0: at 0 the
/// eyes do what the face itself does — its program's motion, or its rest —
/// whatever the target says; at 1 they hold the target, with no more than
/// micro-motion; in between, a blend of the two. Since the target rests at
/// the centre, the weight is what tells "no target" from "look ahead". A face
/// applies it wherever it implements the target, in its program or in its
/// standard adaptation.
pub const GAZE_TARGET_WEIGHT: &str = "standard/vizij/gaze/target/weight";

/// The distance between the centres of the face's eyes, metres, as the
/// robot shows them, at least 0: the baseline a mapping aiming the eyes at a
/// point in space verges them over — each eye turns toward the point from
/// its own centre, half this distance either side of the face's. A setting
/// of the face ([`FACE_SETTINGS`]): rests at the value the face's adaptation
/// is authored with, 0.06 unless it says otherwise, and a deployment whose
/// display shows the face larger or smaller overrides it by writing the key.
pub const GAZE_EYE_SPACING: &str = "standard/vizij/gaze/eye_spacing";

/// How far the face's eyes turn, radians: the gaze angle off the face's axis
/// at which an eye reaches the end of its travel, from 0.01 to 0.9. A
/// mapping aiming the eyes at a point in space turns an angle of ±range into
/// an eye position of ±1. A setting of the face ([`FACE_SETTINGS`]): rests
/// at the value the face's adaptation is authored with, 0.78 unless it says
/// otherwise, and a deployment overrides it by writing the key.
pub const GAZE_RANGE: &str = "standard/vizij/gaze/range";

/// The span [`GAZE_RANGE`] runs over, radians. Above it a mapping's
/// arctangent cannot be trusted to reach the end of the eyes' travel — the
/// ROS4HRI mapping's rational approximation peaks at 0.94 rad — and the
/// floor keeps the normalization finite. The profile declares it as the
/// key's range, and the ROS4HRI mapping reads the range clamped to it.
pub(crate) const GAZE_RANGE_SPAN: (f64, f64) = (0.01, 0.9);

/// Where [`GAZE_EYE_SPACING`] rests when the face authors none, metres.
pub(crate) const GAZE_EYE_SPACING_REST: f64 = 0.06;

/// Where [`GAZE_RANGE`] rests when the face authors none, radians: the
/// incumbent ROS4HRI face's ±0.78 rad → ±1.
pub(crate) const GAZE_RANGE_REST: f64 = 0.78;

// --- Semantic tier: expressions --------------------------------------------

/// The named expressions ROS4HRI's `hri_msgs/Expression` defines — the
/// expressions the built-in ROS4HRI mapping commands by name. Part of the
/// standard's set, [`expression_names`].
pub const ROS4HRI_EXPRESSION_NAMES: [&str; 25] = [
    "neutral",
    "angry",
    "sad",
    "happy",
    "surprised",
    "disgusted",
    "scared",
    "pleading",
    "vulnerable",
    "despaired",
    "guilty",
    "disappointed",
    "embarrassed",
    "horrified",
    "skeptical",
    "annoyed",
    "furious",
    "suspicious",
    "rejected",
    "bored",
    "tired",
    "asleep",
    "confused",
    "amazed",
    "excited",
];

/// The named expressions the standard declares beyond ROS4HRI's vocabulary:
/// expressions faces implement that `hri_msgs/Expression` does not name, so
/// no ROS4HRI command reaches them. Part of the standard's set,
/// [`expression_names`].
pub const VIZIJ_EXPRESSION_NAMES: [&str; 2] = ["concerned", "sleepy"];

/// The standard's named expressions: ROS4HRI's ([`ROS4HRI_EXPRESSION_NAMES`])
/// followed by the ones beyond it ([`VIZIJ_EXPRESSION_NAMES`]). A face
/// implements an expression by responding to its weight control at
/// [`expression_path`]; the standard does not prescribe what the expression
/// looks like — that is the face's authored pose.
pub fn expression_names() -> impl Iterator<Item = &'static str> {
    ROS4HRI_EXPRESSION_NAMES
        .into_iter()
        .chain(VIZIJ_EXPRESSION_NAMES)
}

/// ROS4HRI's expression names, which are not the standard's whole set.
#[deprecated(
    since = "4.1.0",
    note = "ROS4HRI's subset of the standard's expressions: use `ROS4HRI_EXPRESSION_NAMES`, or `expression_names()` for the whole set"
)]
pub const EXPRESSION_NAMES: [&str; 25] = ROS4HRI_EXPRESSION_NAMES;

/// The weight control path for a named expression.
pub fn expression_path(name: &str) -> String {
    format!("{VIZIJ_PREFIX}/expression/{name}")
}

// --- Semantic tier: visemes -------------------------------------------------

/// The viseme shapes, the industry 15-shape set (Oculus/Meta naming). `sil`
/// is silence: rest, which is the face's own neutral — every viseme weight
/// at zero, the `sil` weight included (a player writes it, never drives it).
pub const VISEME_SHAPES: [&str; 15] = [
    "sil", "PP", "FF", "TH", "DD", "kk", "CH", "SS", "nn", "RR", "aa", "E", "ih", "oh", "ou",
];

/// The weight control path for a viseme shape. A raw weight: the face maps
/// it onto its poses as is, with no transition of its own — the timing
/// (attack, hold, release, the crossfade to the next shape) is the viseme
/// player's, the `play_viseme` skill and the `say` skill's lipsync.
pub fn viseme_path(shape: &str) -> String {
    format!("{VIZIJ_PREFIX}/viseme/{shape}")
}

/// The current viseme, one of [`VISEME_SHAPES`] as a string, `sil` at rest —
/// the face's lipsync state, written by whichever viseme player is speaking
/// (a `play_viseme` run, a `say` run's lipsync) for anything that follows
/// speech (subtitles, a mirror face, a monitor). Not a command: the players
/// are the actions; this is what they report.
pub const VISEME: &str = "standard/vizij/viseme";

/// The face's lipsync gain: how strongly it shows the viseme weights. The
/// face's adaptation scales every viseme weight by it before mapping it onto
/// a pose, each pose clamped to [0, 1], so a face whose mouth shapes read too
/// strong or too weak is tuned once rather than on every call. A caller's
/// weight keeps its meaning; the gain is the face's rendering of it. Rests at
/// the gain the face's adaptation is authored with, 1 unless it says
/// otherwise, and a deployment overrides it by writing the key. A setting,
/// not a control ([`FACE_SETTINGS`]): nothing animates it.
pub const LIPSYNC_GAIN: &str = "standard/vizij/lipsync/gain";

/// The face's settings: what a face is like rather than what it is told to
/// do. Each rests at the value the face's own graphs author — the default of
/// an input node on its path, in the face's adaptation — so the face states
/// it once, as asset content, and a mapping composed over the face reads the
/// face's value instead of carrying one of its own
/// ([`crate::Bundle::sources`]). A deployment overrides one by writing the
/// key. Nothing animates a setting, and none is in a tier: the tiers grade
/// what a face can be posed with.
pub const FACE_SETTINGS: [&str; 3] = [LIPSYNC_GAIN, GAZE_EYE_SPACING, GAZE_RANGE];

/// The utterance being spoken, the face's speech state: the text of a `say`
/// run from the moment its audio starts playing — whether or not synthesis
/// has finished — until it ends, empty at rest. Written by the say player
/// for anything that follows speech (subtitles, a transcript, a ROS4HRI
/// speech topic). Not a command: speaking is the `say` skill; this is what
/// it reports.
pub const SPEECH: &str = "standard/vizij/speech";

// --- Captions ----------------------------------------------------------------

/// The caption the face shows: text overlaid at the bottom of the face's own
/// rectangle, empty at rest. Each new value is a new caption, shown in
/// chunks after the ones before it ([`crate::captions`]); the same caption
/// written again is not shown again. A command of the face's display rather
/// than of its rig: the host draws it over the face, so it is no tier's and
/// no face's graphs read it.
pub const CAPTION_TEXT: &str = "standard/vizij/caption/text";

/// Who says the caption at [`CAPTION_TEXT`], written with it:
/// [`CAPTION_SPEAKER_SELF`] when the face itself speaks, any other id for
/// someone else, empty when unknown. The host colours the caption by it.
pub const CAPTION_SPEAKER: &str = "standard/vizij/caption/speaker";

/// The [`CAPTION_SPEAKER`] of the face's own speech: `__system__`, the id
/// ROS4HRI's `hri_actions_msgs/ClosedCaption` gives the robot itself, taken
/// as is so the ROS4HRI mapping relays a caption's speaker unchanged.
pub const CAPTION_SPEAKER_SELF: &str = "__system__";

// --- Muscle tier: face controls ---------------------------------------------

/// A fine-grained face control: its Vizij name, the FACS action unit it
/// expresses (`hri_msgs/FacialActionUnits` code), and the ARKit blendshape it
/// corresponds to. AU codes repeat across lateralized pairs (FACS does not
/// split left/right at the code level); ARKit names are unique.
pub struct FaceControl {
    pub name: &'static str,
    pub au: Option<u8>,
    pub arkit: &'static str,
}

/// The muscle-tier vocabulary. Ordered by face region, brows to tongue.
#[rustfmt::skip]
pub const FACE_CONTROLS: [FaceControl; 35] = [
    // Brows
    FaceControl { name: "brow_inner_up", au: Some(1), arkit: "browInnerUp" },
    FaceControl { name: "brow_outer_up_left", au: Some(2), arkit: "browOuterUpLeft" },
    FaceControl { name: "brow_outer_up_right", au: Some(2), arkit: "browOuterUpRight" },
    FaceControl { name: "brow_down_left", au: Some(4), arkit: "browDownLeft" },
    FaceControl { name: "brow_down_right", au: Some(4), arkit: "browDownRight" },
    // Eyes
    FaceControl { name: "eye_wide_left", au: Some(5), arkit: "eyeWideLeft" },
    FaceControl { name: "eye_wide_right", au: Some(5), arkit: "eyeWideRight" },
    FaceControl { name: "eye_squint_left", au: Some(7), arkit: "eyeSquintLeft" },
    FaceControl { name: "eye_squint_right", au: Some(7), arkit: "eyeSquintRight" },
    FaceControl { name: "eye_closed_left", au: Some(43), arkit: "eyeBlinkLeft" },
    FaceControl { name: "eye_closed_right", au: Some(43), arkit: "eyeBlinkRight" },
    // Cheeks & nose
    FaceControl { name: "cheek_raise_left", au: Some(6), arkit: "cheekSquintLeft" },
    FaceControl { name: "cheek_raise_right", au: Some(6), arkit: "cheekSquintRight" },
    FaceControl { name: "cheek_puff", au: Some(34), arkit: "cheekPuff" },
    FaceControl { name: "nose_sneer_left", au: Some(9), arkit: "noseSneerLeft" },
    FaceControl { name: "nose_sneer_right", au: Some(9), arkit: "noseSneerRight" },
    // Jaw
    FaceControl { name: "jaw_open", au: Some(26), arkit: "jawOpen" },
    FaceControl { name: "jaw_left", au: None, arkit: "jawLeft" },
    FaceControl { name: "jaw_right", au: None, arkit: "jawRight" },
    FaceControl { name: "jaw_forward", au: Some(29), arkit: "jawForward" },
    // Mouth
    FaceControl { name: "mouth_smile_left", au: Some(12), arkit: "mouthSmileLeft" },
    FaceControl { name: "mouth_smile_right", au: Some(12), arkit: "mouthSmileRight" },
    FaceControl { name: "mouth_frown_left", au: Some(15), arkit: "mouthFrownLeft" },
    FaceControl { name: "mouth_frown_right", au: Some(15), arkit: "mouthFrownRight" },
    FaceControl { name: "mouth_press_left", au: Some(24), arkit: "mouthPressLeft" },
    FaceControl { name: "mouth_press_right", au: Some(24), arkit: "mouthPressRight" },
    FaceControl { name: "mouth_pucker", au: Some(18), arkit: "mouthPucker" },
    FaceControl { name: "mouth_funnel", au: Some(22), arkit: "mouthFunnel" },
    FaceControl { name: "mouth_stretch_left", au: Some(20), arkit: "mouthStretchLeft" },
    FaceControl { name: "mouth_stretch_right", au: Some(20), arkit: "mouthStretchRight" },
    FaceControl { name: "mouth_upper_up_left", au: Some(10), arkit: "mouthUpperUpLeft" },
    FaceControl { name: "mouth_upper_up_right", au: Some(10), arkit: "mouthUpperUpRight" },
    FaceControl { name: "mouth_lower_down_left", au: Some(16), arkit: "mouthLowerDownLeft" },
    FaceControl { name: "mouth_lower_down_right", au: Some(16), arkit: "mouthLowerDownRight" },
    // Tongue
    FaceControl { name: "tongue_out", au: Some(19), arkit: "tongueOut" },
];

/// The weight control path for a muscle-tier face control.
pub fn face_path(control: &str) -> String {
    format!("{VIZIJ_PREFIX}/face/{control}")
}

/// The jaw-open control at the path every current face implements — the
/// same muscle as `face/jaw_open` (AU 26, ARKit `jawOpen`), kept under its
/// de-facto name so a face rigged before the muscle tier existed still
/// opens its mouth. Part of the muscle tier.
pub const MOUTH_JAW_OPEN: &str = "standard/vizij/mouth/morph/jaw_open";

/// The face controls expressing a FACS action unit — the lateralized pair
/// where the control splits left/right, a single control otherwise, empty for
/// codes the muscle tier does not express (visibility codes, head and eye
/// movement — those belong to the gaze tier).
pub fn controls_for_au(au: u8) -> impl Iterator<Item = &'static FaceControl> {
    // AU 45 (blink) and AU 43 (eyes closed) command the same lid controls.
    let au = if au == 45 { 43 } else { au };
    FACE_CONTROLS.iter().filter(move |c| c.au == Some(au))
}

/// The face control corresponding to an ARKit blendshape name, if the
/// vocabulary carries it.
pub fn control_for_arkit(arkit: &str) -> Option<&'static FaceControl> {
    FACE_CONTROLS.iter().find(|c| c.arkit == arkit)
}

// --- Conversation state ------------------------------------------------------

/// The conversation states, each a weight at [`conversation_path`] written 0
/// or 1 by whoever runs the dialogue — the agent the face speaks for:
///
/// - `speaking` — the face is speaking: 1 from the moment its utterance
///   starts (a `say` run is spawned) until it ends or is stopped.
/// - `user_speaking` — the person the face is talking with is speaking.
/// - `thinking` — the agent is preparing its reply.
///
/// They describe the dialogue, not the face: a face reacts as its author
/// chooses (a listening pose, an averted gaze while thinking) or not at all,
/// and nothing in the standard writes them on its own.
pub const CONVERSATION_STATES: [&str; 3] = ["speaking", "user_speaking", "thinking"];

/// The weight path of a conversation state.
pub fn conversation_path(state: &str) -> String {
    format!("{VIZIJ_PREFIX}/conversation/{state}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_control_names_and_arkit_names_are_unique() {
        let mut names: Vec<_> = FACE_CONTROLS.iter().map(|c| c.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), FACE_CONTROLS.len());
        let mut arkit: Vec<_> = FACE_CONTROLS.iter().map(|c| c.arkit).collect();
        arkit.sort_unstable();
        arkit.dedup();
        assert_eq!(arkit.len(), FACE_CONTROLS.len());
    }

    /// The names beyond ROS4HRI's are disjoint from it, so the ROS4HRI
    /// mapping's one-hot over its own names never shadows them.
    #[test]
    fn expression_names_extend_ros4hri_without_overlap() {
        let mut names: Vec<_> = expression_names().collect();
        assert_eq!(names.len(), 27);
        assert_eq!(names[..25], ROS4HRI_EXPRESSION_NAMES);
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 27);
    }

    #[test]
    fn lateralized_aus_route_to_their_pair() {
        let brows: Vec<_> = controls_for_au(4).map(|c| c.name).collect();
        assert_eq!(brows, ["brow_down_left", "brow_down_right"]);
        let jaw: Vec<_> = controls_for_au(26).map(|c| c.name).collect();
        assert_eq!(jaw, ["jaw_open"]);
    }

    #[test]
    fn blink_aliases_eyes_closed() {
        let blink: Vec<_> = controls_for_au(45).map(|c| c.name).collect();
        let closed: Vec<_> = controls_for_au(43).map(|c| c.name).collect();
        assert_eq!(blink, closed);
        assert_eq!(blink, ["eye_closed_left", "eye_closed_right"]);
    }

    #[test]
    fn head_and_eye_movement_aus_have_no_muscle_control() {
        for au in [51u8, 55, 61, 63, 69, 70, 73] {
            assert_eq!(controls_for_au(au).count(), 0, "AU {au}");
        }
    }

    #[test]
    fn arkit_lookup_round_trips() {
        let c = control_for_arkit("jawOpen").expect("jawOpen");
        assert_eq!(c.name, "jaw_open");
        assert_eq!(c.au, Some(26));
        assert!(control_for_arkit("eyeLookUpLeft").is_none());
    }
}
