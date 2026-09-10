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
//! Three skills ship: **look_at**, the gaze skill behind the ROS4HRI
//! `/skill/look_at` action (`interaction_skills/LookAt`); **play_viseme**,
//! one viseme shape played through a lipsync envelope; and **say**, the
//! text-to-speech action, whose run hosts the device's `say` module call and
//! drives the face's lips from the viseme it streams. The two viseme players
//! share one lipsync driver ([`viseme_driver`]): the selected shape's weight
//! crossfades in and the others out, the face's current-viseme state
//! ([`standard::VISEME`]) and the run's feedback follow.

use arora_behavior::{
    STATUS_ENUMERATION_ID, STATUS_FAILURE_VARIANT_ID, STATUS_RUNNING_VARIANT_ID,
    STATUS_SUCCESS_VARIANT_ID,
};
use serde_json::{json, Value as Json};
use uuid::Uuid;
use vizij_api_core::value::{Enumeration, Value};

use crate::graph_builder::GraphBuilder;
use crate::ros4hri::{GAZE_FRAME_KEY, GAZE_TARGET_KEY};
use crate::standard::{self, VISEME_SHAPES};

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

/// The play_viseme method's parameters: the viseme shape (one of
/// [`VISEME_SHAPES`]) and the weight it is driven to, [0, 1].
pub const PLAY_VISEME_PARAMS: [&str; 2] = ["shape", "weight"];

/// The play_viseme method's name, as the device describes it.
pub const PLAY_VISEME_FUNCTION: &str = "play_viseme";

/// The canonical play_viseme fragment asset; regenerate with
/// `vizij-bundle export-skill play_viseme`.
pub const PLAY_VISEME_JSON: &str = include_str!("../skills/play_viseme.json");

/// The say method's parameters: the text to speak and the voice.
pub const SAY_PARAMS: [&str; 2] = ["text", "voice"];

/// The say method's name, as the device describes it.
pub const SAY_FUNCTION: &str = "say";

/// The say contract's ids — identical across the text-to-speech providers,
/// so a behavior references `say` without caring which provider a build
/// registered. The fragment hosts the module call under [`SAY_ID`] and reads
/// the viseme the provider streams through its mutable parameter
/// [`SAY_VISEME_PARAM_ID`].
pub const SAY_ID: Uuid = uuid::uuid!("77bf2798-e7ce-47c6-a45c-3c2e9ba1837d");
pub const SAY_TEXT_PARAM_ID: Uuid = uuid::uuid!("881dc182-d4ba-4ea0-9e81-f4eddab6f669");
pub const SAY_VOICE_PARAM_ID: Uuid = uuid::uuid!("f56ca142-db46-4c58-bc44-7896c4b54d5c");
pub const SAY_VISEME_PARAM_ID: Uuid = uuid::uuid!("a1fbf58b-bf66-44a6-a503-9d9078ee5755");

/// The rest token every viseme player writes when nothing is speaking — the
/// `sil` shape of [`VISEME_SHAPES`].
pub const SILENCE_VISEME: &str = "sil";

/// The fields of a viseme player's feedback record: the shape being made and
/// how hard it is driven, `[0, 1]` — the names ROS4HRI's `Say` feedback
/// carries them under, as Vizij extends it.
pub const FEEDBACK_VISEME: &str = "viseme";
pub const FEEDBACK_INTENSITY: &str = "intensity";

/// The canonical say fragment asset; regenerate with
/// `vizij-bundle export-skill say`.
pub const SAY_JSON: &str = include_str!("../skills/say.json");

/// A played viseme's envelope, seconds: the weight ramps in over
/// [`VISEME_ATTACK`], holds for [`VISEME_HOLD`], ramps out over
/// [`VISEME_RELEASE`] — about half a second in all, a spoken viseme's span
/// with room to read.
pub const VISEME_ATTACK: f64 = 0.08;
pub const VISEME_HOLD: f64 = 0.25;
pub const VISEME_RELEASE: f64 = 0.15;

/// The crossfade between shapes, as the half-life of each shape weight's
/// smoother, seconds — short enough for speech (a viseme every ~100 ms), long
/// enough that a shape never snaps.
const VISEME_CROSSFADE_HALF_LIFE: f64 = 0.03;

/// The step's duration, the runtime's built-in key (integer nanoseconds).
const DT_KEY: &str = "arora/dt";

/// Below this, every shape weight counts as at rest: a viseme player ends
/// its run only once its crossfade has settled here, because a run's writes
/// stop with its fragment — ending mid-fade would freeze the lips there.
const VISEME_REST: f64 = 0.02;

/// Regenerate the look_at fragment from first principles — the export path
/// behind the canonical asset.
///
/// The behavior, per the `interaction_skills/LookAt` policies:
///
/// - empty policy or `track`: write the goal target (and frame) onto the
///   ROS4HRI gaze keys and stay `Running` — tracking ends when the goal is
///   cancelled or replaced (the halt is the exit);
/// - `glance` / `reset`: write the target (`reset` recenters on the mapping's
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
    let policy = g.input("in/policy", "task/policy", json!(""));
    let target = g.input(
        "in/target",
        "task/target",
        json!({ "x": 10.0, "y": 0.0, "z": 0.0 }),
    );
    let frame = g.input("in/frame", "task/frame", json!(""));

    // Gaze: `reset` recenters on the mapping's far-ahead rest target (the
    // unverged straight-ahead), every other policy tracks the goal. The
    // written keys are the same standard surface the topic plane feeds — the
    // ROS4HRI mapping turns them into eye pose.
    let rest = g.node(
        "gaze/rest_target",
        "constant",
        json!({ "value": { "x": 10.0, "y": 0.0, "z": 0.0 } }),
    );
    let face_frame = g.node("gaze/face_frame", "constant", json!({ "value": "" }));
    let gaze = g.op(
        "gaze/target",
        "case",
        json!({ "case_labels": ["reset"] }),
        &[
            ("selector", &policy),
            ("operand_0", &rest),
            ("default", &target),
        ],
    );
    g.output("out/gaze/target", &gaze, GAZE_TARGET_KEY.to_string());
    let gaze_frame = g.op(
        "gaze/frame",
        "case",
        json!({ "case_labels": ["reset"] }),
        &[
            ("selector", &policy),
            ("operand_0", &face_frame),
            ("default", &frame),
        ],
    );
    g.output("out/gaze/frame", &gaze_frame, GAZE_FRAME_KEY.to_string());

    // The fixation clock: the graph clock, latched through the store on the
    // run's first tick (`task/start` reads back what it wrote), so elapsed
    // time is measured from the spawn.
    let now = g.node("clock", "time", json!({}));
    let start_in = g.input("in/start", "task/start", json!(0.0));
    let zero = g.constant(0.0);
    let started = g.op(
        "fixation/started",
        "greaterthan",
        json!({}),
        &[("lhs", &start_in), ("rhs", &zero)],
    );
    let start = g.select("fixation/start", &started, &start_in, &now);
    g.output("out/start", &start, "task/start".to_string());
    let elapsed = g.sub("fixation/elapsed", &now, &start);
    let dwell = g.constant(SETTLE_SECONDS);
    let settled = g.op(
        "fixation/settled",
        "greaterthan",
        json!({}),
        &[("lhs", &elapsed), ("rhs", &dwell)],
    );

    // The lifecycle: tracking runs until halted; a fixation succeeds once
    // settled; unimplemented policies fail.
    let running = g.node(
        "status/running",
        "constant",
        json!({ "value": status(STATUS_RUNNING_VARIANT_ID) }),
    );
    let success = g.node(
        "status/success",
        "constant",
        json!({ "value": status(STATUS_SUCCESS_VARIANT_ID) }),
    );
    let failure = g.node(
        "status/failure",
        "constant",
        json!({ "value": status(STATUS_FAILURE_VARIANT_ID) }),
    );
    let fixation = g.select("fixation/status", &settled, &success, &running);
    let lifecycle = g.op(
        "status",
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
    g.output("out/status", &lifecycle, "task/status".to_string());

    // The errno: unsupported policies answer ROS_ENOTSUP. On every
    // implemented path the run stays silent — an empty text the action plane
    // ignores, so the goal's lifecycle decides the errno (success, cancel,
    // preemption).
    let silent = g.node("errno/silent", "constant", json!({ "value": "" }));
    let enotsup = g.node(
        "errno/enotsup",
        "constant",
        json!({ "value": { "u8": ROS_ENOTSUP } }),
    );
    let errno = g.op(
        "errno",
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
    g.output("out/result", &errno, "task/result".to_string());

    json!({ "nodes": g.nodes, "edges": g.edges })
}

/// The lipsync driver the viseme players share: `shape` (a node output
/// carrying text — one of [`VISEME_SHAPES`]; `sil` or anything else selects
/// no shape) and `envelope` (a float node, the weight the selected shape is
/// driven to) become the face's viseme weights, each crossfading through its
/// own smoother, plus the face's current-viseme state ([`standard::VISEME`])
/// and the run's `task/feedback`: the shape while it is driven, `sil`
/// otherwise. Rest is every weight at zero — the face's own neutral — so the
/// `sil` weight is written but never driven. Returns the `settled` node:
/// whether every weight is at rest ([`VISEME_REST`]), what a player's
/// lifecycle waits for before ending.
fn viseme_driver(g: &mut GraphBuilder, shape: (&str, &str), envelope: &str) -> String {
    let (shape_node, shape_port) = shape;
    let zero = g.constant(0.0);
    // The crossfade's per-step gain, 1 - 0.5^(dt / half-life), from the
    // step's duration.
    let dt_ns = g.input("in/dt", DT_KEY, json!(0.0));
    let ns_per_s = g.constant(1e9);
    let dt = g.div("crossfade/dt", &dt_ns, &ns_per_s);
    let half_life = g.constant(VISEME_CROSSFADE_HALF_LIFE);
    let half_lives = g.div("crossfade/half_lives", &dt, &half_life);
    let half = g.constant(0.5);
    let decay = g.op(
        "crossfade/decay",
        "power",
        json!({}),
        &[("base", &half), ("exp", &half_lives)],
    );
    let one = g.constant(1.0);
    let gain = g.sub("crossfade/gain", &one, &decay);
    let mut loudest: Option<String> = None;
    for name in VISEME_SHAPES {
        if name == SILENCE_VISEME {
            g.output(
                &format!("out/viseme/{name}"),
                &zero,
                standard::viseme_path(name),
            );
            continue;
        }
        let select = g.node(
            &format!("viseme/{name}/target"),
            "case",
            json!({ "case_labels": [name] }),
        );
        g.edge_from(shape_node, shape_port, &select, "selector");
        g.edge(envelope, &select, "operand_0");
        g.edge(&zero, &select, "default");
        // The smoother's state is the face's own weight, read back from the
        // store: `weight += (target - weight) * gain`. So whichever run
        // writes a shape continues its fade from where the last one left it
        // — a run taking over never snaps, and one ending leaves nothing
        // mid-fade behind.
        let previous = g.input(
            &format!("in/viseme/{name}"),
            &standard::viseme_path(name),
            json!(0.0),
        );
        let toward = g.sub(&format!("viseme/{name}/toward"), &select, &previous);
        let step = g.mul(&format!("viseme/{name}/step"), &toward, &gain);
        let weight = g.add(&format!("viseme/{name}/weight"), &previous, &step);
        g.output(
            &format!("out/viseme/{name}"),
            &weight,
            standard::viseme_path(name),
        );
        loudest = Some(match loudest {
            Some(so_far) => g.max(&format!("viseme/loudest/{name}"), &so_far, &weight),
            None => weight,
        });
    }
    let loudest = loudest.expect("the standard has shapes");
    let at_rest = g.constant(VISEME_REST);
    let settled = g.op(
        "viseme/settled",
        "lessthan",
        json!({}),
        &[("lhs", &loudest), ("rhs", &at_rest)],
    );
    // The state: the shape while the envelope drives it (a driven `sil` is
    // rest already), `sil` otherwise.
    let driven = g.op(
        "viseme/driven",
        "greaterthan",
        json!({}),
        &[("lhs", envelope), ("rhs", &zero)],
    );
    let silence = g.text(SILENCE_VISEME);
    let current = g.node("viseme/current", "if", json!({}));
    g.edge(&driven, &current, "cond");
    g.edge_from(shape_node, shape_port, &current, "then");
    g.edge(&silence, &current, "else");
    g.output("out/viseme/state", &current, standard::VISEME.to_string());
    // The run's feedback pairs the shape with how hard it is driven — the
    // pair a client needs to mirror the lips, and the shape of ROS4HRI's
    // `Say` feedback as Vizij extends it. Rest reports zero: `sil` is the
    // absence of a shape, whatever the envelope holds. (A `case` on the
    // text — `equal` compares numbers.)
    let intensity = g.node(
        "viseme/intensity",
        "case",
        json!({ "case_labels": [SILENCE_VISEME] }),
    );
    g.edge(&current, &intensity, "selector");
    g.edge(&zero, &intensity, "operand_0");
    g.edge(envelope, &intensity, "default");
    let report = g.node(
        "viseme/report",
        "buildrecord",
        json!({ "record_keys": [FEEDBACK_VISEME, FEEDBACK_INTENSITY] }),
    );
    g.edge(&current, &report, "field_0");
    g.edge(&intensity, &report, "field_1");
    g.output("out/feedback", &report, "task/feedback".to_string());
    settled
}

/// The run's clock: the graph time latched through the store on the first
/// tick (`task/start` reads back what it wrote), so elapsed time counts from
/// the spawn. Returns the elapsed-seconds node.
fn elapsed_since_spawn(g: &mut GraphBuilder) -> String {
    let now = g.node("clock", "time", json!({}));
    let start_in = g.input("in/start", "task/start", json!(0.0));
    let zero = g.constant(0.0);
    let started = g.op(
        "envelope/started",
        "greaterthan",
        json!({}),
        &[("lhs", &start_in), ("rhs", &zero)],
    );
    let start = g.select("envelope/start", &started, &start_in, &now);
    g.output("out/start", &start, "task/start".to_string());
    g.sub("envelope/elapsed", &now, &start)
}

/// A behavior `Status` constant node.
fn status_node(g: &mut GraphBuilder, id: &str, variant: Uuid) -> String {
    let status = serde_json::to_value(Value::Enumeration(Enumeration {
        id: STATUS_ENUMERATION_ID,
        variant_id: variant,
        value: Box::new(Value::Unit),
    }))
    .expect("a status value serializes");
    g.node(id, "constant", json!({ "value": status }))
}

/// Regenerate the play_viseme fragment from first principles — the export
/// path behind the canonical asset.
///
/// The run plays `shape` at `weight` through the lipsync envelope: the
/// weight ramps in over [`VISEME_ATTACK`], holds for [`VISEME_HOLD`], ramps
/// out over [`VISEME_RELEASE`], and the run succeeds once the envelope has
/// closed and the lips have settled. A new run for the same shape restarts
/// the envelope: grafted later, it writes last.
pub fn generate_play_viseme() -> Json {
    let g = &mut GraphBuilder::new();
    let shape = g.input("in/shape", "task/shape", json!(SILENCE_VISEME));
    let weight = g.input("in/weight", "task/weight", json!(1.0));

    let elapsed = elapsed_since_spawn(g);
    let attack = g.constant(VISEME_ATTACK);
    let release = g.constant(VISEME_RELEASE);
    let total = g.constant(VISEME_ATTACK + VISEME_HOLD + VISEME_RELEASE);
    // The envelope: min(ramp in, ramp out), each clamped to [0, 1].
    let rising = g.div("envelope/rising", &elapsed, &attack);
    let rise = g.clamp("envelope/rise", &rising, 0.0, 1.0);
    let remaining = g.sub("envelope/remaining", &total, &elapsed);
    let falling = g.div("envelope/falling", &remaining, &release);
    let fall = g.clamp("envelope/fall", &falling, 0.0, 1.0);
    let gate = g.min("envelope/gate", &rise, &fall);
    let envelope = g.mul("envelope/weighted", &weight, &gate);
    let settled = viseme_driver(g, (&shape, "out"), &envelope);

    // The lifecycle: running until the envelope has closed and the lips
    // have settled.
    let closed = g.op(
        "lifecycle/closed",
        "greaterthan",
        json!({}),
        &[("lhs", &elapsed), ("rhs", &total)],
    );
    let ended = g.op(
        "lifecycle/ended",
        "and",
        json!({}),
        &[("lhs", &closed), ("rhs", &settled)],
    );
    let running = status_node(g, "lifecycle/running", STATUS_RUNNING_VARIANT_ID);
    let success = status_node(g, "lifecycle/success", STATUS_SUCCESS_VARIANT_ID);
    let lifecycle = g.select("lifecycle/status", &ended, &success, &running);
    g.output("out/status", &lifecycle, "task/status".to_string());

    json!({ "nodes": g.nodes, "edges": g.edges })
}

/// Regenerate the say fragment from first principles — the export path
/// behind the canonical asset.
///
/// The run hosts the device's `say` module call ([`SAY_ID`]) on the run's
/// own argument bundle (`task/update`, live-updatable) and drives the lips
/// from the viseme the provider streams through its mutable parameter — the
/// lipsync driver at full weight, so the current viseme's shape is on and
/// the others fade. The run reports the call's status as its own once the
/// call has ended and the lips have settled; until then it is running.
pub fn generate_say() -> Json {
    let g = &mut GraphBuilder::new();
    let args = g.input("in/args", "task/update", Json::Null);
    let run = g.node(
        "say/call",
        "taskrun",
        json!({ "function": SAY_ID.to_string() }),
    );
    g.edge(&args, &run, "args");

    let viseme = g.node(
        "say/viseme",
        "readrecord",
        json!({ "record_keys": [SAY_VISEME_PARAM_ID.to_string()] }),
    );
    g.edge_from(&run, "mutated", &viseme, "in");
    let full = g.constant(1.0);
    let settled = viseme_driver(g, (&viseme, "field_0"), &full);

    let ended = g.node("lifecycle/ended", "and", json!({}));
    g.edge_from(&run, "done", &ended, "lhs");
    g.edge(&settled, &ended, "rhs");
    let running = status_node(g, "lifecycle/running", STATUS_RUNNING_VARIANT_ID);
    let lifecycle = g.node("lifecycle/status", "if", json!({}));
    g.edge(&ended, &lifecycle, "cond");
    g.edge(&run, &lifecycle, "then");
    g.edge(&running, &lifecycle, "else");
    g.output("out/status", &lifecycle, "task/status".to_string());

    json!({ "nodes": g.nodes, "edges": g.edges })
}

/// The parsed canonical asset — what the device registers as the look_at
/// task fragment.
pub fn look_at_source() -> Json {
    serde_json::from_str(LOOK_AT_JSON).expect("skills/look_at.json parses")
}

/// The parsed canonical play_viseme asset.
pub fn play_viseme_source() -> Json {
    serde_json::from_str(PLAY_VISEME_JSON).expect("skills/play_viseme.json parses")
}

/// The parsed canonical say asset.
pub fn say_source() -> Json {
    serde_json::from_str(SAY_JSON).expect("skills/say.json parses")
}

/// The bundle graph kind under which a skill fragment embeds in a GLB — the
/// override a face ships of a built-in skill's behavior.
pub const SKILL_KIND: &str = "skill";

/// A skill in the shipped registry: its identity, the contract's parameter
/// names, and the fragment asset behind it.
pub struct Skill {
    /// Registry id — the described function name, also the id everywhere a
    /// user opts in (bundle graph ids, npm lookups).
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// The described signature's parameter names, in declared order — the
    /// fragment's `task/<param>` inputs.
    pub parameters: &'static [&'static str],
    /// The canonical fragment asset, verbatim JSON.
    pub asset_json: &'static str,
}

/// Every skill Vizij ships.
pub const SKILLS: [Skill; 3] = [
    Skill {
        id: LOOK_AT_FUNCTION,
        title: "Look At",
        description: "The ROS4HRI gaze skill (interaction_skills/LookAt on /skill/look_at): \
                      tracks a target on the standard gaze surface until cancelled, holds a \
                      glance/reset fixation then succeeds, and answers ROS_ENOTSUP for the \
                      social/random policies.",
        parameters: &LOOK_AT_PARAMS,
        asset_json: LOOK_AT_JSON,
    },
    Skill {
        id: PLAY_VISEME_FUNCTION,
        title: "Play Viseme",
        description: "Plays one viseme shape of the face standard's 15-shape set at a weight, \
                      through the lipsync envelope (ramp in, hold, ramp out, about half a \
                      second), crossfading the other shapes out; reports the current viseme.",
        parameters: &PLAY_VISEME_PARAMS,
        asset_json: PLAY_VISEME_JSON,
    },
    Skill {
        id: SAY_FUNCTION,
        title: "Say",
        description: "Speaks a text: hosts the device's text-to-speech `say` call and drives \
                      the lips from the viseme it streams, through the same lipsync driver as \
                      play_viseme; the current viseme is the run's feedback.",
        parameters: &SAY_PARAMS,
        asset_json: SAY_JSON,
    },
];

/// Look a skill up by id.
pub fn skill(id: &str) -> Option<&'static Skill> {
    SKILLS.iter().find(|s| s.id == id)
}

/// The bundle graph id under which `skill_id` embeds in a GLB — stable, so
/// re-adding replaces rather than duplicates.
pub fn embedded_graph_id(skill_id: &str) -> String {
    format!("skill::{skill_id}")
}

/// A skill's canonical fragment as JSON — face-independent by construction
/// (placeholder `task/*` paths), so unlike a mapping source it takes no rig
/// prefix. `None` for an unknown id.
pub fn skill_source(id: &str) -> Option<Json> {
    let skill = skill(id)?;
    serde_json::from_str(skill.asset_json).ok()
}

/// The registry as JSON — what CLIs print and the web runtime serves for
/// pickers (authoring's Skills menu, the standalone's actions view).
pub fn skills_json() -> Json {
    Json::Array(
        SKILLS
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "title": s.title,
                    "description": s.description,
                    "parameters": s.parameters,
                })
            })
            .collect(),
    )
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

    /// The viseme players' committed assets, likewise: regenerate with
    /// `vizij-bundle export-skill play_viseme` / `… say`.
    #[test]
    fn committed_viseme_assets_match_their_generators() {
        assert_eq!(
            play_viseme_source(),
            generate_play_viseme(),
            "skills/play_viseme.json is stale"
        );
        assert_eq!(say_source(), generate_say(), "skills/say.json is stale");
    }

    #[test]
    fn registry_lists_every_skill() {
        let listed = skills_json();
        let ids: Vec<&str> = listed
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["look_at", "play_viseme", "say"]);
        assert_eq!(listed[0]["parameters"][1], "target");
        assert_eq!(listed[1]["parameters"], json!(["shape", "weight"]));
        assert_eq!(listed[2]["parameters"], json!(["text", "voice"]));
        assert!(skill("look_at").is_some());
        assert!(skill("nope").is_none());
        assert_eq!(embedded_graph_id("look_at"), "skill::look_at");
        assert_eq!(skill_source("look_at"), Some(look_at_source()));
        assert_eq!(skill_source("say"), Some(say_source()));
    }

    /// Both viseme players write the face standard's lipsync surface — every
    /// shape weight and the current-viseme state — and report their run.
    #[test]
    fn the_viseme_players_write_the_lipsync_surface() {
        for source in [play_viseme_source(), say_source()] {
            let outputs: Vec<&str> = source["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|n| n["type"] == "output")
                .filter_map(|n| n["params"]["path"].as_str())
                .collect();
            for shape in VISEME_SHAPES {
                let path = standard::viseme_path(shape);
                assert!(outputs.contains(&path.as_str()), "missing {path}");
            }
            assert!(outputs.contains(&standard::VISEME));
            assert!(outputs.contains(&"task/status"));
            assert!(outputs.contains(&"task/feedback"));
        }
        // play_viseme takes its parameters as placeholder inputs; say hosts
        // the module call on the run's own argument bundle.
        let inputs = |source: &Json| -> Vec<String> {
            source["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|n| n["type"] == "input")
                .filter_map(|n| n["params"]["path"].as_str().map(str::to_string))
                .collect()
        };
        let play = inputs(&play_viseme_source());
        assert!(
            play.contains(&"task/shape".to_string()) && play.contains(&"task/weight".to_string())
        );
        assert!(inputs(&say_source()).contains(&"task/update".to_string()));
        assert!(say_source()["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["type"] == "taskrun" && n["params"]["function"] == SAY_ID.to_string()));
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
