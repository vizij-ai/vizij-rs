//! Spawnable skill fragments: task-run behavior as graph data.
//!
//! A skill fragment is the *implementation* of a device task-run method as
//! asset content — a graph spec the interpreter grafts per run
//! (`vizij-arora-behavior`'s task-fragment registry) instead of calling host
//! code. The exterior contract (the method's described signature, and the
//! ROS 2 action an exposure profile binds it to) lives with the device and
//! the bridge; this module owns only the behavior.
//!
//! Fragments speak a placeholder-path convention the interpreter rewrites to
//! each run's key prefix at graft time:
//!
//! - `task/<param>` inputs are the method's parameters, served from the
//!   spawn-time arguments and live-updatable through the run's update keys;
//! - the `task/status` output is the run's lifecycle — the behavior `Status`
//!   enumeration, `Running` until the run ends;
//! - a `task/result` output carries the run's result; an integer there is
//!   the `std_skills` errno the ROS action plane answers verbatim.
//!
//! One skill ships today: **look_at**, the gaze skill behind the ROS4HRI
//! `/skill/look_at` action (`interaction_skills/LookAt`).

use arora_behavior::{
    STATUS_ENUMERATION_ID, STATUS_FAILURE_VARIANT_ID, STATUS_RUNNING_VARIANT_ID,
    STATUS_SUCCESS_VARIANT_ID,
};
use serde_json::{json, Value as Json};
use uuid::Uuid;
use vizij_api_core::value::{Enumeration, Value};

use crate::graph_builder::GraphBuilder;
use crate::ros4hri::{GAZE_FRAME_KEY, GAZE_TARGET_KEY};

/// The look_at method's parameters, in declared order — the fragment's
/// placeholder inputs and the described signature's parameter names, from one
/// list so the contract and the behavior cannot drift.
pub const LOOK_AT_PARAMS: [&str; 3] = ["policy", "target", "frame"];

/// The look_at method's name, as the device describes it.
pub const LOOK_AT_FUNCTION: &str = "look_at";

/// How long a `glance`/`reset` fixation holds before the run succeeds,
/// seconds.
const SETTLE_SECONDS: f64 = 0.6;

/// The `std_skills/Result` "not supported" error code, answered for gaze
/// policies this skill does not implement (`social`, `random`, `auto`, and
/// anything unknown).
const ROS_ENOTSUP: u8 = 134;

/// The canonical fragment asset, verbatim: the behavior as data. Edit it by
/// regenerating (`vizij-bundle export-skill look_at`) — a test keeps it in
/// sync with [`generate_look_at`].
pub const LOOK_AT_JSON: &str = include_str!("../skills/look_at.json");

/// Regenerate the look_at fragment from first principles — the export path
/// behind the canonical asset.
///
/// The behavior, per the `interaction_skills/LookAt` policies:
///
/// - empty policy or `track`: write the goal target (and frame) onto the
///   ROS4HRI gaze keys and stay `Running` — tracking ends when the goal is
///   cancelled or replaced (the halt is the exit);
/// - `glance` / `reset`: write the target (`reset` recenters on the profile's
///   far-ahead rest), hold the fixation for [`SETTLE_SECONDS`], then
///   `Success`;
/// - anything else (`social`, `random`, `auto`, unknown): `Failure`, with
///   the `ROS_ENOTSUP` errno on the result key.
pub fn generate_look_at() -> Json {
    let status = |variant: Uuid| -> Json {
        serde_json::to_value(Value::Enumeration(Enumeration {
            id: STATUS_ENUMERATION_ID,
            variant_id: variant,
            value: Box::new(Value::Unit),
        }))
        .expect("a status value serializes")
    };

    let g = &mut GraphBuilder::new();

    // The method's parameters, staged from the run's keys (the spawn-time
    // arguments become these inputs' defaults at graft time).
    let policy = g.input("in-policy", "task/policy", json!(""));
    let target = g.input(
        "in-target",
        "task/target",
        json!({ "x": 10.0, "y": 0.0, "z": 0.0 }),
    );
    let frame = g.input("in-frame", "task/frame", json!(""));

    // Gaze: `reset` recenters on the profile's far-ahead rest target (the
    // unverged straight-ahead), every other policy tracks the goal. The
    // written keys are the same standard surface the topic plane feeds — the
    // ROS4HRI profile turns them into eye pose.
    let rest = g.node(
        "rest-target",
        "constant",
        json!({ "value": { "x": 10.0, "y": 0.0, "z": 0.0 } }),
    );
    let face_frame = g.node("face-frame", "constant", json!({ "value": "" }));
    let gaze = g.op(
        "case",
        json!({ "case_labels": ["reset"] }),
        &[
            ("selector", &policy),
            ("operand_0", &rest),
            ("default", &target),
        ],
    );
    g.output(&gaze, GAZE_TARGET_KEY.to_string());
    let gaze_frame = g.op(
        "case",
        json!({ "case_labels": ["reset"] }),
        &[
            ("selector", &policy),
            ("operand_0", &face_frame),
            ("default", &frame),
        ],
    );
    g.output(&gaze_frame, GAZE_FRAME_KEY.to_string());

    // The fixation clock: the graph clock, latched through the store on the
    // run's first tick (`task/start` reads back what it wrote), so elapsed
    // time is measured from the spawn.
    let now = g.node("clock", "time", json!({}));
    let start_in = g.input("in-start", "task/start", json!(0.0));
    let zero = g.constant(0.0);
    let started = g.op(
        "greaterthan",
        json!({}),
        &[("lhs", &start_in), ("rhs", &zero)],
    );
    let start = g.op(
        "if",
        json!({}),
        &[("cond", &started), ("then", &start_in), ("else", &now)],
    );
    g.output(&start, "task/start".to_string());
    let elapsed = g.sub(&now, &start);
    let dwell = g.constant(SETTLE_SECONDS);
    let settled = g.op(
        "greaterthan",
        json!({}),
        &[("lhs", &elapsed), ("rhs", &dwell)],
    );

    // The lifecycle: tracking runs until halted; a fixation succeeds once
    // settled; unimplemented policies fail.
    let running = g.node(
        "st-running",
        "constant",
        json!({ "value": status(STATUS_RUNNING_VARIANT_ID) }),
    );
    let success = g.node(
        "st-success",
        "constant",
        json!({ "value": status(STATUS_SUCCESS_VARIANT_ID) }),
    );
    let failure = g.node(
        "st-failure",
        "constant",
        json!({ "value": status(STATUS_FAILURE_VARIANT_ID) }),
    );
    let fixation = g.op(
        "if",
        json!({}),
        &[("cond", &settled), ("then", &success), ("else", &running)],
    );
    let lifecycle = g.op(
        "case",
        json!({ "case_labels": ["", "track", "glance", "reset"] }),
        &[
            ("selector", &policy),
            ("operand_0", &running),
            ("operand_1", &running),
            ("operand_2", &fixation),
            ("operand_3", &fixation),
            ("default", &failure),
        ],
    );
    g.output(&lifecycle, "task/status".to_string());

    // The errno: unsupported policies answer ROS_ENOTSUP. On every
    // implemented path the run stays silent — an empty text the action plane
    // ignores, so the goal's lifecycle decides the errno (success, cancel,
    // preemption).
    let silent = g.node("no-errno", "constant", json!({ "value": "" }));
    let enotsup = g.node(
        "errno-enotsup",
        "constant",
        json!({ "value": { "u8": ROS_ENOTSUP } }),
    );
    let errno = g.op(
        "case",
        json!({ "case_labels": ["", "track", "glance", "reset"] }),
        &[
            ("selector", &policy),
            ("operand_0", &silent),
            ("operand_1", &silent),
            ("operand_2", &silent),
            ("operand_3", &silent),
            ("default", &enotsup),
        ],
    );
    g.output(&errno, "task/result".to_string());

    json!({ "nodes": g.nodes, "edges": g.edges })
}

/// The parsed canonical asset — what the device registers as the look_at
/// task fragment.
pub fn look_at_source() -> Json {
    serde_json::from_str(LOOK_AT_JSON).expect("skills/look_at.json parses")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed asset must equal what the generator produces — otherwise
    /// `export-skill` was not re-run after editing the builder. Regenerate
    /// with `vizij-bundle export-skill look_at -o
    /// crates/interop/vizij-arora-host/skills/look_at.json`.
    #[test]
    fn committed_asset_matches_the_generator() {
        let committed: Json = serde_json::from_str(LOOK_AT_JSON).expect("asset parses");
        assert_eq!(
            committed,
            generate_look_at(),
            "skills/look_at.json is stale"
        );
    }

    /// The fragment holds the placeholder contract the interpreter grafts
    /// against: one input per method parameter, the status output, the errno
    /// result output, and the gaze writes.
    #[test]
    fn the_fragment_speaks_the_placeholder_contract() {
        let spec = generate_look_at();
        let nodes = spec["nodes"].as_array().unwrap();
        let paths_of = |ty: &str| -> Vec<&str> {
            nodes
                .iter()
                .filter(|n| n["type"] == ty)
                .filter_map(|n| n["params"]["path"].as_str())
                .collect()
        };
        let inputs = paths_of("input");
        for param in LOOK_AT_PARAMS {
            let path = format!("task/{param}");
            assert!(inputs.contains(&path.as_str()), "missing input {path}");
        }
        let outputs = paths_of("output");
        for path in [
            "task/status",
            "task/result",
            GAZE_TARGET_KEY,
            GAZE_FRAME_KEY,
        ] {
            assert!(outputs.contains(&path), "missing output {path}");
        }
    }
}
