//! The built-in ROS4HRI mapping: a composable graph source implementing the
//! `ros4hri` profile — the `standard/ros4hri/*` store keys a ROS bridge writes
//! from the ROS4HRI topics, and the one it publishes — in terms of the
//! `vizij-face` profile, the [`crate::standard`] face controls and state.
//! Both interfaces are declared as data in [`crate::profile`]; this module is
//! the operation between them.
//!
//! The mapping is asset-independent by construction — it only writes standard
//! control paths; what an expression or a viseme *looks like* stays with the
//! face (its rig and adaptation graphs). Per channel:
//!
//! - **Expression** — the keys command the `set_expression` skill: each
//!   change of `expression/name`, `valence` or `arousal` spawns a run of it
//!   with the three, and the first evaluation spawns one with the keys as
//!   loaded, so the face rests at neutral from boot. A run, never this
//!   graph, writes the face's expression; the skill computes it with this
//!   module's math ([`expression_targets`]: a non-empty name selects its
//!   expression at the arousal as its intensity, otherwise valence/arousal
//!   blend the named weights by proximity to each expression's circumplex
//!   anchor). The run is exclusive with the expression's setters, so a key
//!   write, a `set_expression` call and a `set_facial_action_units` call
//!   take the face over from one another — the latest command wins, by
//!   whichever path it came — crossfading from where the face is.
//! - **Gaze** — `gaze/target` (vec3, meters, face frame: x forward, y left,
//!   z up) maps to per-eye positions with vergence and a center fallback for
//!   targets at or behind the face plane (x ≤ 0.1). How far apart the eyes
//!   are and how far they turn are the face's own settings
//!   ([`standard::GAZE_EYE_SPACING`], [`standard::GAZE_RANGE`]), read under
//!   its rig prefix: each eye aims from its own centre, and an angle of
//!   ±range is an eye position of ±1 — the incumbent's 6 cm and ±0.78 rad for
//!   a face that authors neither.
//! - **Blink** — an idle generator (≈8 s cycle, deterministically jittered,
//!   0.2 s parabolic pulse) drives the eyelids; the `asleep` expression
//!   — the face's own weight, whoever sets it — closes them and inhibits
//!   the blink in proportion to its weight. The standard's
//!   `standard/vizij/blink` is not written: that key is for a caller's
//!   blinks, which the face layers over these lids.
//! - **Viseme** — `viseme`, the ROS4HRI code of the lip shape at the audio
//!   playhead, plays through the `play_viseme` skill: each new code spawns a
//!   run of that shape at full weight, which takes over from the run before
//!   it. The lips are a player's, never this graph's: a run writes the
//!   face's viseme weights and state, so a streamed viseme shows over the
//!   device's own speaking `say` as any player does, and `sil` is a run that
//!   drives no shape — the lips settle at rest.
//! - **Speech** — the other direction: the face's speech state is relayed
//!   as is to the ROS4HRI speech state a bridge publishes — the utterance a
//!   say run is speaking (`standard/vizij/speech`) to `speech/text`, for
//!   subtitles, and the current viseme (`standard/vizij/viseme`, a shape
//!   name) to `speech/viseme`, for anything that follows the lips. The face's
//!   keys are per face; the ROS4HRI keys are the device's, which is what a
//!   static bridge profile can name.
//! - **Captions** — a closed caption's `caption/text` and `caption/speaker`
//!   are relayed as is to the face's caption (`standard/vizij/caption/*`),
//!   which the host shows over the face.
//!
//! All continuous channels pass through a ~200 ms exponential smoother (the
//! incumbent ROS4HRI face's dynamics), the expression setters' runs
//! included. The smoothing, the blink's cadence and the circumplex are the
//! mapping's own: how a ROS4HRI command plays out in time and which
//! expressions an affect names are the same on every face. What depends on
//! the face — its eyes — is the face's to say. The graph is generated data: it
//! composes exactly like [`crate::animations_source`], after the face's own
//! graphs and before any playing program, so a performance overrides it,
//! last-writer-wins.

use serde_json::{json, Value as Json};

use crate::graph_builder::GraphBuilder;
use crate::skills::{play_viseme, set_expression, viseme_shape_of, SILENCE_VISEME};
use crate::standard::{self, ROS4HRI_EXPRESSION_NAMES};

/// Source id of the composed mapping (node ids get `ros4hri::` prefixes).
pub const ROS4HRI_SOURCE_ID: &str = "ros4hri";

/// Prefix of every ROS4HRI key the mapping consumes or produces.
pub const ROS4HRI_PREFIX: &str = "standard/ros4hri";

/// Input keys, as a ROS bridge writes them.
pub const EXPRESSION_NAME_KEY: &str = "standard/ros4hri/expression/name";
pub const EXPRESSION_VALENCE_KEY: &str = "standard/ros4hri/expression/valence";
pub const EXPRESSION_AROUSAL_KEY: &str = "standard/ros4hri/expression/arousal";
pub const GAZE_TARGET_KEY: &str = "standard/ros4hri/gaze/target";
pub const GAZE_FRAME_KEY: &str = "standard/ros4hri/gaze/frame";
/// The lip shape at the audio playhead, as ROS4HRI's `hri_msgs/Viseme.value`
/// — an index into [`crate::standard::VISEME_SHAPES`], whose order is
/// ROS4HRI's own (`SIL`, `PP`, `FF`, … `OU`), `0` (`sil`) at rest. The
/// mapping plays each new code through the `play_viseme` skill.
pub const VISEME_KEY: &str = "standard/ros4hri/viseme";
/// Where the streamed viseme falls in the utterance, seconds
/// (`hri_msgs/Viseme.time`): a new time is a new viseme, even of the same
/// shape.
pub const VISEME_TIME_KEY: &str = "standard/ros4hri/viseme/time";
/// How long the streamed viseme's shape is held, seconds
/// (`hri_msgs/Viseme.duration`); `0` when the stream gives none.
pub const VISEME_DURATION_KEY: &str = "standard/ros4hri/viseme/duration";

/// A closed caption's text and who says it
/// (`hri_actions_msgs/ClosedCaption.text` and `.speaker_id`, `__system__`
/// for the robot itself), written together; relayed as is to the face's
/// caption ([`standard::CAPTION_TEXT`], [`standard::CAPTION_SPEAKER`]).
pub const CAPTION_TEXT_KEY: &str = "standard/ros4hri/caption/text";
pub const CAPTION_SPEAKER_KEY: &str = "standard/ros4hri/caption/speaker";

/// The output key, as a ROS bridge publishes it: the utterance being spoken,
/// empty at rest — the face's speech state ([`standard::SPEECH`]) relayed to
/// the device-scoped key the bridge's ROS4HRI profile names.
pub const SPEECH_TEXT_KEY: &str = "standard/ros4hri/speech/text";
/// The lip shape being spoken, one of [`standard::VISEME_SHAPES`] by name,
/// `sil` at rest — the face's current viseme ([`standard::VISEME`]) relayed
/// to the device-scoped key, so one topic follows the lips of the whole
/// robot whichever player is speaking.
pub const SPEECH_VISEME_KEY: &str = "standard/ros4hri/speech/viseme";

/// Circumplex anchor (valence, arousal) per expression name, used to blend
/// the named weights when only `valence`/`arousal` are commanded. Order
/// matches [`ROS4HRI_EXPRESSION_NAMES`].
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
/// The expression setters' runs smooth at it too, so whichever writes the
/// face's expression moves it alike.
pub(crate) const HALF_LIFE: f64 = 0.14;
/// Circumplex kernel radius: anchors farther than this get zero weight.
const ANCHOR_RADIUS: f64 = 0.6;
/// Blink cycle length / pulse width, seconds.
const BLINK_PERIOD: f64 = 8.0;
const BLINK_WIDTH: f64 = 0.2;

/// The composable ROS4HRI mapping source: the canonical mapping asset
/// ([`MAPPING_JSON`]) with `rig_prefix` applied. The prefix is prepended to
/// every face-standard path it writes or reads — the face's controls, its
/// speech state and settings, which faces namespace under their rig (e.g.
/// `rig/quori_latest/`) — and the ROS4HRI keys stay device-scoped; pass
/// `""` for unprefixed controls.
pub fn ros4hri_source(rig_prefix: &str) -> (String, Json) {
    let mut spec: Json = serde_json::from_str(MAPPING_JSON).expect("mappings/ros4hri.json parses");
    apply_rig_prefix(&mut spec, rig_prefix);
    (ROS4HRI_SOURCE_ID.to_string(), spec)
}

/// The canonical mapping asset, verbatim: the graph as data. This file is
/// what the bundler embeds into GLBs and what the web runtime serves; edit it
/// by regenerating (`vizij-bundle export-mapping ros4hri`) — a test keeps it
/// in sync with [`generate`].
pub const MAPPING_JSON: &str = include_str!("../mappings/ros4hri.json");

/// Regenerate the mapping graph from first principles, unprefixed — the
/// export path behind the canonical asset.
pub fn generate() -> Json {
    build("").1
}

/// Prepend `rig_prefix` to every face-standard path the mapping reads or
/// writes (`standard/vizij/*`, the face-scoped `vizij-face` profile: the
/// controls it writes, the speech state and settings it reads). The ROS4HRI
/// paths — the device-scoped `ros4hri` profile a bridge writes and
/// publishes — stay untouched.
pub fn apply_rig_prefix(spec: &mut Json, rig_prefix: &str) {
    standard::prefix_controls(spec, rig_prefix);
}

/// `atan(num / den)` under `prefix`, via the rational approximation
/// `r / (1 + 0.28 r²)` (≤1 % error up to 0.8 rad), normalized by the face's
/// gaze range — `range` rad → 1 — and clamped.
fn gaze_angle(g: &mut GraphBuilder, prefix: &str, num: &str, den: &str, range: &str) -> String {
    let id = |step: &str| format!("{prefix}/{step}");
    let ratio = g.div(&id("ratio"), num, den);
    let ratio2 = g.mul(&id("ratio2"), &ratio, &ratio);
    let k = g.constant(0.28);
    let correction = g.mul(&id("correction"), &ratio2, &k);
    let one = g.constant(1.0);
    let denominator = g.add(&id("denominator"), &correction, &one);
    let atan = g.div(&id("atan"), &ratio, &denominator);
    let normalized = g.div(&id("normalized"), &atan, range);
    g.clamp(&id("angle"), &normalized, -1.0, 1.0)
}

/// The expression's target weights for `names` (expressions of the
/// standard, [`standard::expression_names`]), in their order, from the
/// `name`, `valence` and `arousal` nodes: a non-empty name selects its
/// expression alone at the arousal as its intensity — its negative half
/// (calm) reading as no intensity rather than an inverted pose — and an
/// empty one blends the ROS4HRI names by proximity of the (valence, arousal)
/// point to each one's circumplex anchor ([`EXPRESSION_ANCHORS`]), resting
/// exactly neutral near the origin. A name without an anchor takes no share
/// of the blend. Nodes under `expression/`.
pub(crate) fn expression_targets(
    g: &mut GraphBuilder,
    names: &[&str],
    name: &str,
    valence: &str,
    arousal: &str,
) -> Vec<String> {
    let zero = g.constant(0.0);
    let one = g.constant(1.0);
    let intensity = g.clamp("expression/intensity", arousal, 0.0, 1.0);

    // The commanded affect as a point (valence, arousal) on the circumplex.
    let affect = g.op(
        "expression/affect",
        "join",
        json!({}),
        &[("operand_0", valence), ("operand_1", arousal)],
    );
    // Dead zone: near-zero affect rests exactly neutral instead of a blend of
    // every origin-adjacent anchor; commanded affect fades the blend in over
    // the first 0.15 of circumplex magnitude.
    let magnitude2 = g.op(
        "expression/affect/magnitude2",
        "vectordot",
        json!({}),
        &[("a", &affect), ("b", &affect)],
    );
    let active = g.remap(
        "expression/affect/active",
        &magnitude2,
        (0.0, 0.15 * 0.15),
        (0.0, 1.0),
    );
    let rest = g.sub("expression/affect/rest", &one, &active);

    // Per anchor, the raw kernel weight max(0, 1 − dist² / R²).
    let kernels: Vec<String> = ROS4HRI_EXPRESSION_NAMES
        .iter()
        .zip(EXPRESSION_ANCHORS)
        .map(|(expr, (av, aa))| {
            let id = |step: &str| format!("expression/{expr}/{step}");
            let anchor = g.vector_constant(&id("anchor"), &[av, aa]);
            let offset = g.op(
                &id("offset"),
                "vectorsubtract",
                json!({}),
                &[("a", &affect), ("b", &anchor)],
            );
            let distance2 = g.op(
                &id("distance2"),
                "vectordot",
                json!({}),
                &[("a", &offset), ("b", &offset)],
            );
            g.remap(
                &id("kernel"),
                &distance2,
                (0.0, ANCHOR_RADIUS * ANCHOR_RADIUS),
                (1.0, 0.0),
            )
        })
        .collect();
    // Normalize so the blend sums to 1 (ε floors the empty-neighborhood case).
    let sum = {
        let ports: Vec<String> = (0..kernels.len()).map(|i| format!("operand_{i}")).collect();
        let refs: Vec<(&str, &str)> = ports
            .iter()
            .zip(&kernels)
            .map(|(p, k)| (p.as_str(), k.as_str()))
            .collect();
        g.op("expression/kernel/sum", "add", json!({}), &refs)
    };
    let eps = g.constant(1e-6);
    let denominator = g.max("expression/kernel/denominator", &sum, &eps);

    names
        .iter()
        .map(|expr| {
            let id = |step: &str| format!("expression/{expr}/{step}");
            let kernel = ROS4HRI_EXPRESSION_NAMES
                .iter()
                .position(|anchored| anchored == expr)
                .map(|i| kernels[i].clone());
            let blend = match kernel {
                Some(kernel) => {
                    let share = g.div(&id("share"), &kernel, &denominator);
                    if *expr == "neutral" {
                        // Neutral also absorbs the dead zone's rest share.
                        let affect_blend = g.mul(&id("affect_blend"), &active, &share);
                        g.add(&id("blend"), &affect_blend, &rest)
                    } else {
                        g.mul(&id("blend"), &active, &share)
                    }
                }
                None => zero.clone(),
            };
            // The commanded name selects: this expression → its intensity,
            // no name → the circumplex blend, another name → 0.
            g.op(
                &id("target"),
                "case",
                json!({ "case_labels": [expr, ""] }),
                &[
                    ("selector", name),
                    ("operand_0", &intensity),
                    ("operand_1", &blend),
                    ("default", &zero),
                ],
            )
        })
        .collect()
}

/// The mapping, section by section. Node ids are hierarchical — `in/…` the
/// consumed profile, `out/…` the produced one, and `expression/…`,
/// `gaze/…`, `blink/…`, `viseme/…` the channel that computes each
/// output — so the asset reads by channel and every node says what it holds.
fn build(rig_prefix: &str) -> (String, Json) {
    let g = &mut GraphBuilder::new();
    let out = |path: String| format!("{rig_prefix}{path}");
    // Output node ids name the standard control they write, prefix dropped.
    let out_id = |path: &str| {
        format!(
            "out/{}",
            path.strip_prefix(standard::VIZIJ_PREFIX)
                .and_then(|p| p.strip_prefix('/'))
                .unwrap_or(path)
        )
    };

    let zero = g.constant(0.0);
    let one = g.constant(1.0);

    // --- Expression: the keys command set_expression. Each change spawns a
    // run of it with the three keys, which takes the face over from the
    // expression setter's run before it, whichever command started that one;
    // the first evaluation spawns one too, so the face rests at what the keys
    // hold as loaded — neutral, at their defaults. The run, not this graph,
    // writes the expression. ---
    let name = g.input("in/expression/name", EXPRESSION_NAME_KEY, json!(""));
    let valence = g.input("in/expression/valence", EXPRESSION_VALENCE_KEY, json!(0.0));
    let arousal = g.input("in/expression/arousal", EXPRESSION_AROUSAL_KEY, json!(0.0));
    let set = g.node(
        "expression/set",
        "spawn",
        json!({
            "function": set_expression::ids::set_expression::FUNCTION.to_string(),
            "record_keys": [
                set_expression::ids::set_expression::EXPRESSION.to_string(),
                set_expression::ids::set_expression::VALENCE.to_string(),
                set_expression::ids::set_expression::AROUSAL.to_string(),
            ],
            "spawn_on_load": true,
        }),
    );
    g.edge(&name, &set, "args_0");
    g.edge(&valence, &set, "args_1");
    g.edge(&arousal, &set, "args_2");

    // --- Gaze: target vec3 → per-eye positions with vergence ---------------
    // The resting target sits far ahead so the unverged eyes read straight.
    let target = g.input(
        "in/gaze/target",
        GAZE_TARGET_KEY,
        json!({ "x": 10.0, "y": 0.0, "z": 0.0 }),
    );
    let axis = |g: &mut GraphBuilder, name: &str, index: f64| {
        let index = g.constant(index);
        g.op(
            &format!("gaze/{name}"),
            "vectorindex",
            json!({}),
            &[("v", &target), ("index", &index)],
        )
    };
    let gx = axis(g, "x", 0.0);
    let gy = axis(g, "y", 1.0);
    let gz = axis(g, "z", 2.0);

    // Targets at or behind the face plane recenter the eyes (the incumbent
    // drops them; a pure graph cannot hold the previous pose, so it holds
    // center — a noted deviation).
    let min_x = g.constant(0.1);
    let valid = g.op(
        "gaze/valid",
        "greaterthan",
        json!({}),
        &[("lhs", &gx), ("rhs", &min_x)],
    );

    // The face's eyes: how far apart they are, which each eye aims from half
    // of either side of the face's centre, and how far they turn, the angle
    // an eye position of ±1 stands for — settings the face authors, read
    // under its prefix.
    // Each is read within the span the profile declares for it: a spacing
    // below 0 would cross the eyes' lines of sight, a range outside its span
    // would leave the eyes short of their travel's end or divide by zero.
    let authored_spacing = g.input(
        "in/gaze/eye_spacing",
        standard::GAZE_EYE_SPACING,
        json!(standard::GAZE_EYE_SPACING_REST),
    );
    let spacing = g.max("gaze/eye_spacing", &authored_spacing, &zero);
    let half = g.constant(0.5);
    let half_spacing = g.mul("gaze/half_spacing", &spacing, &half);
    let authored_range = g.input(
        "in/gaze/range",
        standard::GAZE_RANGE,
        json!(standard::GAZE_RANGE_REST),
    );
    let (min_range, max_range) = standard::GAZE_RANGE_SPAN;
    let range = g.clamp("gaze/range", &authored_range, min_range, max_range);

    let y_left = g.add("gaze/left/y", &gy, &half_spacing);
    let y_right = g.sub("gaze/right/y", &gy, &half_spacing);
    let yaw_left = gaze_angle(g, "gaze/left/yaw", &y_left, &gx, &range);
    let yaw_right = gaze_angle(g, "gaze/right/yaw", &y_right, &gx, &range);
    let pitch = gaze_angle(g, "gaze/pitch", &gz, &gx, &range);

    for (eye, yaw, path_x, path_y) in [
        (
            "left",
            &yaw_left,
            standard::LEFT_EYE_POS_X,
            standard::LEFT_EYE_POS_Y,
        ),
        (
            "right",
            &yaw_right,
            standard::RIGHT_EYE_POS_X,
            standard::RIGHT_EYE_POS_Y,
        ),
    ] {
        let id = |step: &str| format!("gaze/{eye}/{step}");
        let gated_x = g.select(&id("x/gated"), &valid, yaw, &zero);
        let gated_y = g.select(&id("y/gated"), &valid, &pitch, &zero);
        let smooth_x = g.damp(&id("x/smooth"), &gated_x, HALF_LIFE);
        let smooth_y = g.damp(&id("y/smooth"), &gated_y, HALF_LIFE);
        g.output(&out_id(path_x), &smooth_x, out(path_x.to_string()));
        g.output(&out_id(path_y), &smooth_y, out(path_y.to_string()));
    }

    // --- Blink: jittered idle pulse, inhibited while the face is asleep ----
    let t = g.node("blink/time", "time", json!({}));
    let period = g.constant(BLINK_PERIOD);
    let cycle = g.div("blink/cycle", &t, &period);
    let cycle_index = g.op(
        "blink/cycle/index",
        "round",
        json!({ "round_mode": "floor" }),
        &[("in", &cycle)],
    );
    // Deterministic per-cycle jitter shifts the blink ±2 s within its cycle.
    let jitter = g.op(
        "blink/jitter",
        "simplenoise",
        json!({ "noise_seed": 7.0, "frequency": 1.0, "octaves": 1.0 }),
        &[("x", &cycle_index), ("y", &zero)],
    );
    let two = g.constant(2.0);
    let shift = g.mul("blink/shift", &jitter, &two);
    let shifted = g.add("blink/shifted_time", &t, &shift);
    let phase = g.op(
        "blink/phase",
        "modulo",
        json!({}),
        &[("lhs", &shifted), ("rhs", &period)],
    );
    // Parabolic pulse 4u(1−u) over the first BLINK_WIDTH seconds of the cycle.
    let width = g.constant(BLINK_WIDTH);
    let progress = g.div("blink/progress", &phase, &width);
    let four = g.constant(4.0);
    let rise = g.mul("blink/pulse/rise", &four, &progress);
    let fall = g.sub("blink/pulse/fall", &one, &progress);
    let parabola = g.mul("blink/pulse/parabola", &rise, &fall);
    let in_window = g.op(
        "blink/pulse/in_window",
        "lessthan",
        json!({}),
        &[("lhs", &progress), ("rhs", &one)],
    );
    let gated = g.select("blink/pulse/gated", &in_window, &parabola, &zero);
    let pulse = g.clamp("blink/pulse", &gated, 0.0, 1.0);
    // Asleep is the one commanded close: it inhibits the blink in proportion
    // to its weight, and the lids follow the stronger of the two. The weight
    // is the face's own, read back — whoever set the expression.
    let asleep = g.input(
        "in/expression/asleep",
        &out(standard::expression_path("asleep")),
        json!(0.0),
    );
    let open_share = g.sub("blink/open_share", &one, &asleep);
    let idle = g.mul("blink/idle", &pulse, &open_share);
    let lid = g.max("blink/lid", &idle, &asleep);
    for path in [
        standard::LEFT_EYE_TOP_EYELID_POS_Y,
        standard::RIGHT_EYE_TOP_EYELID_POS_Y,
    ] {
        g.output(&out_id(path), &lid, out(path.to_string()));
    }

    // --- Viseme: each new code spawns a play_viseme run of its shape -------
    let code = g.input("in/viseme", VISEME_KEY, json!(0.0));
    let shape = viseme_shape_of(g, &code);
    // Full weight, held as long as the stream says: the stream names the
    // shape, and its timing is the stream's. A new time is a new viseme, so a
    // repeated shape plays again. The run, not this graph, writes the lips —
    // so the graph never competes with a player for them.
    let full = g.constant(1.0);
    let duration = g.input("in/viseme/duration", VISEME_DURATION_KEY, json!(0.0));
    let time = g.input("in/viseme/time", VISEME_TIME_KEY, json!(0.0));
    let spawn = g.node(
        "viseme/play",
        "spawn",
        json!({
            "function": play_viseme::ids::play_viseme::FUNCTION.to_string(),
            "record_keys": [
                play_viseme::ids::play_viseme::SHAPE.to_string(),
                play_viseme::ids::play_viseme::WEIGHT.to_string(),
                play_viseme::ids::play_viseme::DURATION.to_string(),
            ],
        }),
    );
    g.edge(&shape, &spawn, "args_0");
    g.edge(&full, &spawn, "args_1");
    g.edge(&duration, &spawn, "args_2");
    g.edge(&time, &spawn, "when");

    // --- Captions: a closed caption, relayed as is to the face's caption,
    // which the host shows. ----------------------------------------------
    for (key, control) in [
        (CAPTION_TEXT_KEY, standard::CAPTION_TEXT),
        (CAPTION_SPEAKER_KEY, standard::CAPTION_SPEAKER),
    ] {
        let id = out_id(control);
        let caption = g.input(&id.replacen("out/", "in/", 1), key, json!(""));
        g.output(&id, &caption, out(control.to_string()));
    }

    // --- Speech: the face's speech state, relayed as is to the ROS4HRI
    // speech keys a bridge publishes. -----------------------------------
    let speech = g.input("in/speech", &out(standard::SPEECH.to_string()), json!(""));
    g.output("out/speech/text", &speech, SPEECH_TEXT_KEY.to_string());
    let spoken = g.input(
        "in/speech/viseme",
        &out(standard::VISEME.to_string()),
        json!(SILENCE_VISEME),
    );
    g.output("out/speech/viseme", &spoken, SPEECH_VISEME_KEY.to_string());

    (
        ROS4HRI_SOURCE_ID.to_string(),
        json!({ "nodes": g.nodes, "edges": g.edges }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::standard::VISEME_SHAPES;

    fn spec() -> Json {
        ros4hri_source("rig/test_face/").1
    }

    /// The committed asset must equal what the generator produces — otherwise
    /// `export-mapping` was not re-run after editing the builder, and the file
    /// the bundler and web runtime serve is stale. Regenerate with
    /// `vizij-bundle export-mapping ros4hri -o crates/interop/vizij-arora-host/mappings/ros4hri.json`.
    #[test]
    fn committed_asset_matches_the_generator() {
        let committed: Json = serde_json::from_str(MAPPING_JSON).expect("asset parses");
        assert_eq!(committed, generate(), "mappings/ros4hri.json is stale");
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
        // The expression is the set_expression runs': the mapping spawns
        // them, and writes none of its weights.
        assert!(!paths.iter().any(|p| p.contains("/expression/")));
        // Visemes are not the profile's: ROS4HRI has no viseme channel, and the
        // face's lipsync is the viseme players' (the play_viseme and say skills).
        assert!(!paths.iter().any(|p| p.contains("/viseme/")));
        // Nor is the muscle tier: ROS4HRI commands no action units.
        assert!(!paths.iter().any(|p| p.contains("/standard/vizij/face/")));
        for path in [
            "rig/test_face/standard/vizij/left_eye/pos/x",
            "rig/test_face/standard/vizij/right_eye/pos/y",
            "rig/test_face/standard/vizij/left_eye_top_eyelid/pos/y",
        ] {
            assert!(paths.contains(&path), "missing {path}");
        }
    }

    /// The mapping reads the ROS4HRI command keys and writes the face's
    /// controls; the speech channel runs the other way, reading the face's
    /// speech state (per face, under the rig prefix) and writing the
    /// device-scoped ROS4HRI speech keys; and the gaze channel reads the
    /// face's eye settings, per face too — nothing else crosses over but the
    /// face's `asleep` weight, which inhibits the blink, and the step's
    /// duration.
    #[test]
    fn source_reads_ros4hri_keys_and_relays_the_speech_state() {
        let spec = spec();
        let asleep = standard::expression_path("asleep");
        let face_in: Vec<String> = [
            standard::SPEECH,
            standard::VISEME,
            standard::GAZE_EYE_SPACING,
            standard::GAZE_RANGE,
            asleep.as_str(),
        ]
        .map(|path| format!("rig/test_face/{path}"))
        .into_iter()
        .chain(["arora/dt".to_string()])
        .collect();
        for node in spec["nodes"].as_array().unwrap() {
            let path = node["params"]["path"].as_str();
            match node["type"].as_str() {
                Some("input") => {
                    let path = path.unwrap();
                    assert!(
                        path.starts_with(ROS4HRI_PREFIX) || face_in.iter().any(|p| p == path),
                        "unexpected input {path}"
                    );
                }
                Some("output") => {
                    let path = path.unwrap();
                    assert!(
                        path.starts_with("rig/test_face/standard/vizij/")
                            || path == SPEECH_TEXT_KEY
                            || path == SPEECH_VISEME_KEY,
                        "unexpected output {path}"
                    );
                }
                _ => {}
            }
        }
    }

    /// ROS4HRI numbers the visemes in the order the 15-shape set lists them,
    /// so a code a TTS node publishes indexes the standard's shapes directly
    /// and the mapping keeps no code table of its own.
    #[test]
    fn ros4hri_viseme_codes_index_the_standard_shapes() {
        let ros4hri = [
            "SIL", "PP", "FF", "TH", "DD", "KK", "CH", "SS", "NN", "RR", "AA", "E", "IH", "OH",
            "OU",
        ];
        assert_eq!(VISEME_SHAPES.len(), ros4hri.len());
        for (shape, code_name) in VISEME_SHAPES.iter().zip(ros4hri) {
            assert!(
                shape.eq_ignore_ascii_case(code_name),
                "{shape} vs {code_name}"
            );
        }
    }

    /// The viseme channel writes nothing itself: it spawns play_viseme with
    /// the shape, a full weight and the stream's duration, by the contract's
    /// parameter ids, and a new time spawns again.
    #[test]
    fn the_viseme_channel_spawns_play_viseme() {
        let spec = spec();
        let spawn = spawn_of(&spec, play_viseme::ids::play_viseme::FUNCTION);
        assert_eq!(
            spawn["params"]["record_keys"],
            json!([
                play_viseme::ids::play_viseme::SHAPE.to_string(),
                play_viseme::ids::play_viseme::WEIGHT.to_string(),
                play_viseme::ids::play_viseme::DURATION.to_string(),
            ])
        );
        let when = spec["edges"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["to"]["node_id"] == spawn["id"] && e["to"]["input"] == "when")
            .expect("a new time spawns again");
        let source = spec["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == when["from"]["node_id"])
            .unwrap();
        assert_eq!(source["params"]["path"], VISEME_TIME_KEY);
    }

    /// The gaze channel aims the eyes with the face's own settings, under its
    /// prefix, resting where a face that authors none rests: the incumbent's
    /// eyes 6 cm apart, turning ±0.78 rad. The mapping carries no eye
    /// geometry of its own.
    #[test]
    fn the_gaze_reads_the_face_s_eye_settings() {
        let spec = spec();
        let input = |path: &str| {
            spec["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["type"] == "input" && n["params"]["path"] == path)
                .unwrap_or_else(|| panic!("the mapping reads {path}"))["params"]["value"]
                .as_f64()
                .unwrap()
        };
        assert_eq!(
            input(&format!("rig/test_face/{}", standard::GAZE_EYE_SPACING)),
            0.06
        );
        assert_eq!(
            input(&format!("rig/test_face/{}", standard::GAZE_RANGE)),
            0.78
        );
        for constant in ["const/0.03", "const/0.78"] {
            assert!(
                !spec["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|n| n["id"] == constant),
                "the mapping holds {constant}"
            );
        }
    }

    /// The spawn node of `function`.
    fn spawn_of(spec: &Json, function: uuid::Uuid) -> Json {
        spec["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["type"] == "spawn" && n["params"]["function"] == function.to_string())
            .unwrap_or_else(|| panic!("the mapping spawns {function}"))
            .clone()
    }

    /// The expression channel writes nothing itself: it spawns
    /// set_expression with the three expression keys, by the contract's
    /// parameter ids — on every change, and once as loaded.
    #[test]
    fn the_expression_channel_spawns_set_expression() {
        use set_expression::ids::set_expression as ids;
        let spec = spec();
        let spawn = spawn_of(&spec, ids::FUNCTION);
        assert_eq!(
            spawn["params"]["record_keys"],
            json!([
                ids::EXPRESSION.to_string(),
                ids::VALENCE.to_string(),
                ids::AROUSAL.to_string(),
            ])
        );
        assert_eq!(spawn["params"]["spawn_on_load"], true);
        for (port, key) in [
            ("args_0", EXPRESSION_NAME_KEY),
            ("args_1", EXPRESSION_VALENCE_KEY),
            ("args_2", EXPRESSION_AROUSAL_KEY),
        ] {
            let edge = spec["edges"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["to"]["node_id"] == spawn["id"] && e["to"]["input"] == port)
                .unwrap_or_else(|| panic!("{port} is wired"));
            let source = spec["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["id"] == edge["from"]["node_id"])
                .unwrap();
            assert_eq!(source["params"]["path"], key, "{port}");
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
        assert_eq!(EXPRESSION_ANCHORS.len(), ROS4HRI_EXPRESSION_NAMES.len());
        // Neutral anchors the origin so idle valence/arousal rests neutral.
        assert_eq!(EXPRESSION_ANCHORS[0], (0.0, 0.0));
    }
}
