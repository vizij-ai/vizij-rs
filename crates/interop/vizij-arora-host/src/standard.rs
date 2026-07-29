//! The Vizij face standard: the store paths a compliant face implements.
//!
//! Faces are driven through named controls on the store, under the
//! `standard/vizij/` prefix. The vocabulary has three tiers, from coarse to
//! fine; a face implements what it implements, and standard profiles (like the
//! ROS4HRI one) degrade to the tiers a face covers:
//!
//! - **Gaze & lids** — per-eye position and eyelid controls.
//! - **Semantic** — one weight per named expression and per viseme shape. The
//!   expression names are ROS4HRI's set; the viseme shapes are the industry
//!   15-shape set (the Oculus/Meta convention).
//! - **Muscle** — fine-grained face controls cherry-picked from FACS action
//!   units and ARKit blendshapes: FACS supplies the taxonomy (each control
//!   names an action unit, so ROS4HRI's `FacialActionUnits` maps losslessly),
//!   ARKit supplies lateralization and naming familiarity (its blendshape
//!   arrays carry left/right where the FACS message cannot, and assets in the
//!   wild ship ARKit-named morph targets). Controls exist for the union that
//!   makes sense on a commanded robot face; ARKit's tracking-only shapes
//!   (`eyeLook*` — redundant with the gaze tier) and FACS codes a command
//!   channel cannot express (visibility, head/eye movement — owned by the
//!   gaze tier) have none.
//!
//! Everything is an `f32` weight in [0, 1] unless a constant says otherwise.

/// Prefix of every control path in the Vizij face standard.
pub const VIZIJ_PREFIX: &str = "standard/vizij";

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

// --- Semantic tier: expressions --------------------------------------------

/// The named expressions, from ROS4HRI's `hri_msgs/Expression` vocabulary.
/// A face implements an expression by responding to its weight control at
/// [`expression_path`]; the standard does not prescribe what the expression
/// looks like — that is the face's authored pose.
pub const EXPRESSION_NAMES: [&str; 25] = [
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

/// The weight control path for a named expression.
pub fn expression_path(name: &str) -> String {
    format!("{VIZIJ_PREFIX}/expression/{name}")
}

// --- Semantic tier: visemes -------------------------------------------------

/// The viseme shapes, the industry 15-shape set (Oculus/Meta naming). `sil`
/// is silence — the closed-mouth rest shape.
pub const VISEME_SHAPES: [&str; 15] = [
    "sil", "PP", "FF", "TH", "DD", "kk", "CH", "SS", "nn", "RR", "aa", "E", "ih", "oh", "ou",
];

/// The weight control path for a viseme shape.
pub fn viseme_path(shape: &str) -> String {
    format!("{VIZIJ_PREFIX}/viseme/{shape}")
}

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
