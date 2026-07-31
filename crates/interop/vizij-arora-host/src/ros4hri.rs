//! The built-in ROS4HRI profile: a composable graph source mapping the
//! `standard/ros4hri/*` store keys (what a ROS bridge writes from the ROS4HRI
//! topics) onto the [`crate::standard`] face controls.
//!
//! The profile is asset-independent by construction — it only writes standard
//! control paths; what an expression or a viseme *looks like* stays with the
//! face (its rig and adaptation graphs). Per channel:
//!
//! - **Expression** — a non-empty `expression/name` one-hots the named weight;
//!   otherwise `valence`/`arousal` blend the named weights by proximity to
//!   each expression's circumplex anchor. Weights are smoothed and written to
//!   `standard/vizij/expression/<name>`.
//! - **Gaze** — `gaze/target` (vec3, meters, face frame: x forward, y left,
//!   z up) maps to per-eye positions with vergence, the incumbent ±0.78 rad →
//!   ±1 normalization, and a center fallback for targets at or behind the
//!   face plane (x ≤ 0.1).
//! - **Action units** — `au/<code>` intensities route to the muscle-tier
//!   controls per [`crate::standard::FACE_CONTROLS`]; the eyes-closed unit
//!   also drives the eyelids, and jaw-open additionally drives the de-facto
//!   `mouth/morph/jaw_open` control.
//! - **Visemes** — `viseme/<shape>` weights pass through, smoothed, to
//!   `standard/vizij/viseme/<shape>`.
//! - **Blink** — an idle generator (≈8 s cycle, deterministically jittered,
//!   0.2 s parabolic pulse) drives the eyelids, inhibited while the eyes are
//!   commanded closed or the face is asleep.
//!
//! All continuous channels pass through a ~200 ms exponential smoother (the
//! incumbent ROS4HRI face's dynamics). The graph is generated data: it
//! composes exactly like [`crate::animations_source`], after the face's own
//! graphs and before any playing program, so a performance overrides it,
//! last-writer-wins.

use serde_json::{json, Value as Json};

use crate::graph_builder::GraphBuilder;
use crate::standard::{self, EXPRESSION_NAMES, FACE_CONTROLS, VISEME_SHAPES};

/// Source id of the composed profile (node ids get `ros4hri::` prefixes).
pub const ROS4HRI_SOURCE_ID: &str = "ros4hri";

/// Prefix of every key the profile consumes.
pub const ROS4HRI_PREFIX: &str = "standard/ros4hri";

/// Input keys, as a ROS bridge writes them.
pub const EXPRESSION_NAME_KEY: &str = "standard/ros4hri/expression/name";
pub const EXPRESSION_VALENCE_KEY: &str = "standard/ros4hri/expression/valence";
pub const EXPRESSION_AROUSAL_KEY: &str = "standard/ros4hri/expression/arousal";
pub const GAZE_TARGET_KEY: &str = "standard/ros4hri/gaze/target";
pub const GAZE_FRAME_KEY: &str = "standard/ros4hri/gaze/frame";

/// The key carrying a FACS action-unit intensity, [0, 1].
pub fn au_key(code: u8) -> String {
    format!("{ROS4HRI_PREFIX}/au/{code}")
}

/// The key carrying a viseme-shape weight, [0, 1].
pub fn viseme_key(shape: &str) -> String {
    format!("{ROS4HRI_PREFIX}/viseme/{shape}")
}

/// Circumplex anchor (valence, arousal) per expression name, used to blend
/// the named weights when only `valence`/`arousal` are commanded. Order
/// matches [`EXPRESSION_NAMES`].
#[rustfmt::skip]
pub const EXPRESSION_ANCHORS: [(f64, f64); 25] = [
    (0.0, 0.0),     // neutral
    (-0.7, 0.7),    // angry
    (-0.7, -0.4),   // sad
    (0.8, 0.4),     // happy
    (0.1, 0.8),     // surprised
    (-0.7, 0.4),    // disgusted
    (-0.7, 0.8),    // scared
    (-0.3, 0.3),    // pleading
    (-0.4, -0.15),  // vulnerable
    (-0.8, -0.5),   // despaired
    (-0.5, -0.35),  // guilty
    (-0.5, -0.25),  // disappointed
    (-0.35, 0.2),   // embarrassed
    (-0.8, 0.8),    // horrified
    (-0.3, 0.1),    // skeptical
    (-0.5, 0.4),    // annoyed
    (-0.8, 0.9),    // furious
    (-0.4, 0.25),   // suspicious
    (-0.6, -0.3),   // rejected
    (-0.35, -0.55), // bored
    (-0.15, -0.7),  // tired
    (0.0, -0.95),   // asleep
    (-0.25, 0.35),  // confused
    (0.5, 0.7),     // amazed
    (0.7, 0.8),     // excited
];

/// Smoothing half-life, seconds — ≈200 ms time constant, the incumbent's.
const HALF_LIFE: f64 = 0.14;
/// Circumplex kernel radius: anchors farther than this get zero weight.
const ANCHOR_RADIUS: f64 = 0.6;
/// Blink cycle length / pulse width, seconds.
const BLINK_PERIOD: f64 = 8.0;
const BLINK_WIDTH: f64 = 0.2;
/// Half the interocular distance, meters — the vergence offset.
const HALF_IOD: f64 = 0.03;
/// The incumbent's gaze normalization: ±0.78 rad maps to ±1.
const GAZE_RANGE_RAD: f64 = 0.78;

/// `atan(num / den)` via the rational approximation `r / (1 + 0.28 r²)`
/// (≤1 % error inside the clamped gaze range), normalized by the
/// incumbent's ±0.78 rad → ±1 and clamped.
fn gaze_angle(g: &mut GraphBuilder, num: &str, den: &str) -> String {
    let r = g.div(num, den);
    let r2 = g.mul(&r, &r);
    let k = g.constant(0.28);
    let kr2 = g.mul(&r2, &k);
    let one = g.constant(1.0);
    let d = g.add2(&kr2, &one);
    let atan = g.div(&r, &d);
    let range = g.constant(GAZE_RANGE_RAD);
    let norm = g.div(&atan, &range);
    let lo = g.constant(-1.0);
    let hi = g.constant(1.0);
    g.op(
        "clamp",
        json!({}),
        &[("in", &norm), ("min", &lo), ("max", &hi)],
    )
}

/// The composable ROS4HRI profile source: the canonical profile asset
/// ([`PROFILE_JSON`]) with `rig_prefix` applied. The prefix is prepended to
/// every written control path (faces namespace their rig inputs, e.g.
/// `rig/quori_latest/`); pass `""` for unprefixed controls.
pub fn ros4hri_source(rig_prefix: &str) -> (String, Json) {
    let mut spec: Json = serde_json::from_str(PROFILE_JSON).expect("profiles/ros4hri.json parses");
    apply_rig_prefix(&mut spec, rig_prefix);
    (ROS4HRI_SOURCE_ID.to_string(), spec)
}

/// The canonical profile asset, verbatim: the graph as data. This file is
/// what the bundler embeds into GLBs and what the web runtime serves; edit it
/// by regenerating (`vizij-bundle export-profile ros4hri`) — a test keeps it
/// in sync with [`generate`].
pub const PROFILE_JSON: &str = include_str!("../profiles/ros4hri.json");

/// Regenerate the profile graph from first principles, unprefixed — the
/// export path behind the canonical asset.
pub fn generate() -> Json {
    build("").1
}

/// Prepend `rig_prefix` to every path the profile writes (its `output`
/// nodes). Input paths — the `standard/ros4hri/*` keys a bridge writes — are
/// device-global and stay untouched.
pub fn apply_rig_prefix(spec: &mut Json, rig_prefix: &str) {
    if rig_prefix.is_empty() {
        return;
    }
    for node in spec
        .get_mut("nodes")
        .and_then(Json::as_array_mut)
        .into_iter()
        .flatten()
    {
        if node.get("type").and_then(Json::as_str) == Some("output") {
            if let Some(path) = node.pointer_mut("/params/path") {
                if let Some(p) = path.as_str() {
                    *path = Json::String(format!("{rig_prefix}{p}"));
                }
            }
        }
    }
}

fn build(rig_prefix: &str) -> (String, Json) {
    let g = &mut GraphBuilder::new();
    let out = |path: String| format!("{rig_prefix}{path}");

    // --- Expression: name one-hot, else valence/arousal circumplex blend ---
    let name = g.input("in-name", EXPRESSION_NAME_KEY, json!(""));
    let valence = g.input("in-valence", EXPRESSION_VALENCE_KEY, json!(0.0));
    let arousal = g.input("in-arousal", EXPRESSION_AROUSAL_KEY, json!(0.0));

    let zero = g.constant(0.0);
    let one = g.constant(1.0);

    // 1 when a name is commanded, 0 while the name key is empty.
    let has_name = g.op(
        "case",
        json!({ "case_labels": [""] }),
        &[("selector", &name), ("operand_0", &zero), ("default", &one)],
    );
    let no_name = g.sub(&one, &has_name);

    // Raw circumplex weight per anchor: max(0, 1 − dist² / R²).
    let radius_sq = g.constant(ANCHOR_RADIUS * ANCHOR_RADIUS);
    let raw: Vec<String> = EXPRESSION_ANCHORS
        .iter()
        .map(|(av, aa)| {
            let cv = g.constant(*av);
            let ca = g.constant(*aa);
            let dv = g.sub(&valence, &cv);
            let da = g.sub(&arousal, &ca);
            let dv2 = g.mul(&dv, &dv);
            let da2 = g.mul(&da, &da);
            let d2 = g.add2(&dv2, &da2);
            let frac = g.div(&d2, &radius_sq);
            let w = g.sub(&one, &frac);
            g.max2(&w, &zero)
        })
        .collect();
    // Normalize so the blend sums to 1 (ε floors the empty-neighborhood case).
    let sum = {
        let ports: Vec<String> = (0..raw.len()).map(|i| format!("operand_{i}")).collect();
        let refs: Vec<(&str, &str)> = ports
            .iter()
            .zip(&raw)
            .map(|(p, f)| (p.as_str(), f.as_str()))
            .collect();
        g.op("add", json!({}), &refs)
    };
    let eps = g.constant(1e-6);
    let denom = g.max2(&sum, &eps);

    // Dead zone: near-zero valence/arousal rests exactly neutral instead of a
    // blend of every origin-adjacent anchor; commanded affect fades the blend
    // in over the first 0.15 of circumplex magnitude.
    let va_active = {
        let v2 = g.mul(&valence, &valence);
        let a2 = g.mul(&arousal, &arousal);
        let mag2 = g.add2(&v2, &a2);
        let dz2 = g.constant(0.15 * 0.15);
        let ratio = g.div(&mag2, &dz2);
        let lo = g.constant(0.0);
        let hi = g.constant(1.0);
        g.op(
            "clamp",
            json!({}),
            &[("in", &ratio), ("min", &lo), ("max", &hi)],
        )
    };
    let va_rest = g.sub(&one, &va_active);

    let mut asleep_weight = None;
    for (i, expr) in EXPRESSION_NAMES.iter().enumerate() {
        // One-hot from the commanded name.
        let onehot = g.op(
            "case",
            json!({ "case_labels": [expr] }),
            &[("selector", &name), ("operand_0", &one), ("default", &zero)],
        );
        let named = g.mul(&has_name, &onehot);
        let va_n = g.div(&raw[i], &denom);
        let va = g.mul(&va_active, &va_n);
        let va = if *expr == "neutral" {
            g.add2(&va, &va_rest)
        } else {
            va
        };
        let blended = g.mul(&no_name, &va);
        let w = g.add2(&named, &blended);
        let smooth = g.damp(&w, HALF_LIFE);
        if *expr == "asleep" {
            asleep_weight = Some(smooth.clone());
        }
        g.output(&smooth, out(standard::expression_path(expr)));
    }

    // --- Gaze: target vec3 → per-eye positions with vergence ---------------
    // The resting target sits far ahead so the unverged eyes read straight.
    let target = g.input(
        "in-gaze",
        GAZE_TARGET_KEY,
        json!({ "x": 10.0, "y": 0.0, "z": 0.0 }),
    );
    let (c0, c1, c2) = (g.constant(0.0), g.constant(1.0), g.constant(2.0));
    let gx = g.op("vectorindex", json!({}), &[("v", &target), ("index", &c0)]);
    let gy = g.op("vectorindex", json!({}), &[("v", &target), ("index", &c1)]);
    let gz = g.op("vectorindex", json!({}), &[("v", &target), ("index", &c2)]);

    // Targets at or behind the face plane recenter the eyes (the incumbent
    // drops them; a pure graph cannot hold the previous pose, so it holds
    // center — a noted deviation).
    let min_x = g.constant(0.1);
    let valid = g.op("greaterthan", json!({}), &[("lhs", &gx), ("rhs", &min_x)]);

    let iod = g.constant(HALF_IOD);
    let y_left = g.add2(&gy, &iod);
    let y_right = g.sub(&gy, &iod);
    let yaw_l = gaze_angle(g, &y_left, &gx);
    let yaw_r = gaze_angle(g, &y_right, &gx);
    let pitch = gaze_angle(g, &gz, &gx);

    for (angle, path_x, path_y) in [
        (&yaw_l, standard::LEFT_EYE_POS_X, standard::LEFT_EYE_POS_Y),
        (&yaw_r, standard::RIGHT_EYE_POS_X, standard::RIGHT_EYE_POS_Y),
    ] {
        let gated_x = g.op(
            "if",
            json!({}),
            &[("cond", &valid), ("then", angle), ("else", &zero)],
        );
        let gated_y = g.op(
            "if",
            json!({}),
            &[("cond", &valid), ("then", &pitch), ("else", &zero)],
        );
        let sx = g.damp(&gated_x, HALF_LIFE);
        let sy = g.damp(&gated_y, HALF_LIFE);
        g.output(&sx, out(path_x.to_string()));
        g.output(&sy, out(path_y.to_string()));
    }

    // --- Action units → muscle-tier controls -------------------------------
    let mut au_codes: Vec<u8> = FACE_CONTROLS.iter().filter_map(|c| c.au).collect();
    au_codes.sort_unstable();
    au_codes.dedup();
    let mut eyes_closed = String::new();
    for code in au_codes {
        let input_id = format!("in-au-{code}");
        let raw = g.input(&input_id, &au_key(code), json!(0.0));
        let smooth = g.damp(&raw, HALF_LIFE);
        if code == 43 {
            eyes_closed = smooth.clone();
        }
        for control in standard::controls_for_au(code) {
            g.output(&smooth, out(standard::face_path(control.name)));
        }
        // Jaw-open also drives the de-facto mouth control every current face
        // implements, so the AU channel moves faces without a muscle tier.
        if code == 26 {
            g.output(
                &smooth,
                out("standard/vizij/mouth/morph/jaw_open".to_string()),
            );
        }
    }

    // --- Visemes: pass-through, smoothed -----------------------------------
    for shape in VISEME_SHAPES {
        let input_id = format!("in-vis-{shape}");
        let raw = g.input(&input_id, &viseme_key(shape), json!(0.0));
        let smooth = g.damp(&raw, HALF_LIFE);
        g.output(&smooth, out(standard::viseme_path(shape)));
    }

    // --- Blink: jittered idle pulse, inhibited when lids are commanded -----
    let t = g.node("blink-time", "time", json!({}));
    let period = g.constant(BLINK_PERIOD);
    let cycle = g.div(&t, &period);
    let cycle_floor = g.op("round", json!({ "round_mode": "floor" }), &[("in", &cycle)]);
    // Deterministic per-cycle jitter shifts the blink ±2 s within its cycle.
    let jitter = g.op(
        "simplenoise",
        json!({ "noise_seed": 7.0, "frequency": 1.0, "octaves": 1.0 }),
        &[("x", &cycle_floor), ("y", &zero)],
    );
    let two = g.constant(2.0);
    let shift = g.mul(&jitter, &two);
    let shifted = g.add2(&t, &shift);
    let phase = g.op("modulo", json!({}), &[("lhs", &shifted), ("rhs", &period)]);
    // Parabolic pulse 4u(1−u) over the first BLINK_WIDTH seconds of the cycle.
    let width = g.constant(BLINK_WIDTH);
    let u = g.div(&phase, &width);
    let four = g.constant(4.0);
    let a = g.mul(&four, &u);
    let b = g.sub(&one, &u);
    let parabola = g.mul(&a, &b);
    let in_window = g.op("lessthan", json!({}), &[("lhs", &u), ("rhs", &one)]);
    let pulse = g.op(
        "if",
        json!({}),
        &[("cond", &in_window), ("then", &parabola), ("else", &zero)],
    );
    let pulse = g.op(
        "clamp",
        json!({}),
        &[("in", &pulse), ("min", &zero), ("max", &one)],
    );
    // Inhibit while the eyes are commanded closed or the face is asleep.
    let asleep = asleep_weight.expect("asleep is in EXPRESSION_NAMES");
    let commanded = g.max2(&eyes_closed, &asleep);
    let open_share = g.sub(&one, &commanded);
    let blink = g.mul(&pulse, &open_share);
    // Lids follow the strongest closer: the blink pulse or the commanded close.
    let lid = g.max2(&blink, &commanded);
    g.output(&lid, out(standard::LEFT_EYE_TOP_EYELID_POS_Y.to_string()));
    g.output(&lid, out(standard::RIGHT_EYE_TOP_EYELID_POS_Y.to_string()));

    (
        ROS4HRI_SOURCE_ID.to_string(),
        json!({ "nodes": g.nodes, "edges": g.edges }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> Json {
        ros4hri_source("rig/test_face/").1
    }

    /// The committed asset must equal what the generator produces — otherwise
    /// `export-profile` was not re-run after editing the builder, and the file
    /// the bundler and web runtime serve is stale. Regenerate with
    /// `vizij-bundle export-profile ros4hri -o crates/interop/vizij-arora-host/profiles/ros4hri.json`.
    #[test]
    fn committed_asset_matches_the_generator() {
        let committed: Json = serde_json::from_str(PROFILE_JSON).expect("asset parses");
        assert_eq!(committed, generate(), "profiles/ros4hri.json is stale");
    }

    #[test]
    fn source_covers_every_standard_output() {
        let spec = spec();
        let paths: Vec<&str> = spec["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["type"] == "output")
            .filter_map(|n| n["params"]["path"].as_str())
            .collect();
        for expr in EXPRESSION_NAMES {
            let path = format!("rig/test_face/standard/vizij/expression/{expr}");
            assert!(paths.contains(&path.as_str()), "missing {path}");
        }
        for shape in VISEME_SHAPES {
            let path = format!("rig/test_face/standard/vizij/viseme/{shape}");
            assert!(paths.contains(&path.as_str()), "missing {path}");
        }
        for control in &FACE_CONTROLS {
            if control.au.is_some() {
                let path = format!("rig/test_face/standard/vizij/face/{}", control.name);
                assert!(paths.contains(&path.as_str()), "missing {path}");
            }
        }
        for path in [
            "rig/test_face/standard/vizij/left_eye/pos/x",
            "rig/test_face/standard/vizij/right_eye/pos/y",
            "rig/test_face/standard/vizij/left_eye_top_eyelid/pos/y",
            "rig/test_face/standard/vizij/mouth/morph/jaw_open",
        ] {
            assert!(paths.contains(&path), "missing {path}");
        }
    }

    #[test]
    fn source_reads_only_ros4hri_keys() {
        let spec = spec();
        for node in spec["nodes"].as_array().unwrap() {
            if node["type"] == "input" {
                let path = node["params"]["path"].as_str().unwrap();
                assert!(path.starts_with(ROS4HRI_PREFIX), "unexpected input {path}");
            }
        }
    }

    #[test]
    fn edges_reference_existing_nodes() {
        let spec = spec();
        let ids: std::collections::HashSet<&str> = spec["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect();
        for edge in spec["edges"].as_array().unwrap() {
            for end in [&edge["from"]["node_id"], &edge["to"]["node_id"]] {
                assert!(ids.contains(end.as_str().unwrap()), "dangling {end}");
            }
        }
    }

    #[test]
    fn anchors_pair_with_names() {
        assert_eq!(EXPRESSION_ANCHORS.len(), EXPRESSION_NAMES.len());
        // Neutral anchors the origin so idle valence/arousal rests neutral.
        assert_eq!(EXPRESSION_ANCHORS[0], (0.0, 0.0));
    }
}
