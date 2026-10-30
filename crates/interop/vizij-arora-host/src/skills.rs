//! Spawnable skill fragments: task-run behavior as graph data.
//!
//! A skill fragment is the *implementation* of a device task-run method as
//! asset content — a graph spec the interpreter grafts per run
//! (`vizij-arora-behavior`'s task-fragment registry) instead of calling host
//! code. Each skill's exterior contract — its function's id, name and
//! signature — is declared here once, as a trait ([`LookAt`], [`PlayViseme`],
//! [`Say`]): the device describes the method from it, a module implementing
//! it is registered under the implementer's own module id, and an exposure
//! profile binds a ROS 2 action to the method by its name.
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
//! The skills that ship:
//!
//! - **look_at**, the gaze skill behind the ROS4HRI `/skill/look_at` action
//!   (`interaction_skills/LookAt`);
//! - the expression setters, **set_expression** (a named expression or a
//!   valence/arousal point) and **set_facial_action_units** (FACS action
//!   units on the muscle tier): each holds the whole face's expression until
//!   a later run of either takes it over, crossfading from where the face
//!   is ([`hold_expression`]);
//! - the lips' players: **play_viseme**, one viseme shape played through a
//!   lipsync envelope; **set_viseme**, a shape held until the next command;
//!   **play_visemes**, a viseme timeline; and **say**, the text-to-speech
//!   action, whose run hosts the device's `say` module call and drives the
//!   face's lips from the viseme it streams. They share one lipsync driver
//!   ([`viseme_driver`]): the selected shape's weight crossfades in and the
//!   others out, the face's current-viseme state ([`standard::VISEME`]) and
//!   the run's feedback follow.
//!
//! `set_*` skills set a state that holds until changed: their runs never end
//! on their own. `play_*` skills play a timeline that ends. Which runs a new
//! one takes over from — the expression's setters, the lips' players — is
//! the device's to declare when it registers the fragments.

use arora_behavior::{
    Status, STATUS_ENUMERATION_ID, STATUS_FAILURE_VARIANT_ID, STATUS_RUNNING_VARIANT_ID,
    STATUS_SUCCESS_VARIANT_ID,
};
use serde_json::{json, Value as Json};
use uuid::Uuid;
use vizij_api_core::value::{Enumeration, Structure, StructureField, Value};

use crate::graph_builder::GraphBuilder;
use crate::ros4hri::{self, GAZE_FRAME_KEY, GAZE_TARGET_KEY};
use crate::standard::{self, VISEME_SHAPES};

/// The gaze skill: `look_at` directs the gaze at `target`, a point in
/// `frame`, under `policy`. Its fragment ([`generate_look_at`]) implements
/// it; the module a device registers for it only fails, and exists so the
/// method is described.
#[arora_module::contract(name = "look_at")]
pub trait LookAt {
    #[export(id = "21566ce4-59a5-43a8-bf6d-1ea9e205415e")]
    fn look_at(
        &mut self,
        #[param(id = "3a8db272-79a9-4cd3-861a-9fe3b485a8a9")] policy: String,
        #[param(id = "5984f824-c4b8-4bdc-8706-187ac05f3bab")] target: Vec<f32>,
        #[param(id = "b8211f70-7626-47d9-8815-c59357a81d5f")] frame: String,
    ) -> Status;
}

/// The look_at method's parameters, in declared order: the fragment's
/// placeholder inputs, and the parameter names of [`LookAt::look_at`] (a test
/// holds the two equal).
pub const LOOK_AT_PARAMS: [&str; 3] = ["policy", "target", "frame"];

/// How long a `glance`/`reset` fixation holds before the run succeeds,
/// seconds.
const SETTLE_SECONDS: f64 = 0.6;

/// The `std_skills/Result` "not supported" error code, answered for gaze
/// policies this skill does not implement (`social`, `auto`, and anything
/// unknown).
const ROS_ENOTSUP: u8 = 134;

/// The canonical fragment asset, verbatim: the behavior as data. Edit it by
/// regenerating (`vizij-bundle export-skill look_at`) — a test keeps it in
/// sync with [`generate_look_at`].
pub const LOOK_AT_JSON: &str = include_str!("../skills/look_at.json");

/// The viseme skill: `play_viseme` plays one viseme `shape` (one of
/// [`VISEME_SHAPES`]) through the lipsync envelope, driven to `weight` in
/// `[0, 1]` and held `duration` seconds from its start — a streamed
/// viseme's own duration — or, without one, [`VISEME_ATTACK`] plus
/// [`VISEME_HOLD`]. Its fragment ([`generate_play_viseme`]) implements it; the
/// module a device registers for it only fails, and exists so the method is
/// described.
#[arora_module::contract(name = "play_viseme")]
pub trait PlayViseme {
    #[export(id = "43747a8e-2f6c-42ae-9d14-d440c2195b2f")]
    fn play_viseme(
        &mut self,
        #[param(id = "da9fb9bc-12f7-486b-a65b-7ada44eb15f6")] shape: String,
        #[param(id = "2055ee33-cdc7-4eb0-a04e-dc49d9314a61")] weight: f32,
        #[param(id = "48249635-6ecd-4aaa-bd5b-ca570dbba3d0")] duration: Option<f32>,
    ) -> Status;
}

/// The play_viseme method's parameters, in declared order: the fragment's
/// placeholder inputs, and the parameter names of
/// [`PlayViseme::play_viseme`] (a test holds the two equal).
pub const PLAY_VISEME_PARAMS: [&str; 3] = ["shape", "weight", "duration"];

/// The canonical play_viseme fragment asset; regenerate with
/// `vizij-bundle export-skill play_viseme`.
pub const PLAY_VISEME_JSON: &str = include_str!("../skills/play_viseme.json");

/// The speech skill: `say` speaks `text` in `voice` (the provider's own when
/// `None`) and streams through its mutable parameters the viseme at the
/// audio playhead (`viseme`, one of [`VISEME_SHAPES`]) and the utterance
/// while its audio plays (`speech`: the text from the moment playback
/// starts, whether or not synthesis has finished, empty before and after).
/// Each text-to-speech provider implements it under its own module id, and
/// is re-invoked each tick while `Running` (the poll-on-tick contract). The
/// say fragment ([`generate_say`]) hosts the call, drives the lips from the
/// streamed viseme and writes the utterance as the face's speech state.
#[arora_module::contract(name = "say")]
pub trait Say {
    #[export(id = "77bf2798-e7ce-47c6-a45c-3c2e9ba1837d")]
    fn say(
        &mut self,
        #[param(id = "881dc182-d4ba-4ea0-9e81-f4eddab6f669")] text: String,
        #[param(id = "f56ca142-db46-4c58-bc44-7896c4b54d5c")] voice: Option<String>,
        #[param(id = "a1fbf58b-bf66-44a6-a503-9d9078ee5755")] viseme: &mut String,
        #[param(id = "de350f2e-0dc7-455a-b895-af3c61001669")] speech: &mut String,
    ) -> Status;
}

/// The say method's inputs, in declared order: the fragment's placeholder
/// inputs, and the leading parameter names of [`Say::say`] (a test holds the
/// two equal) — its `viseme` and `speech` are outputs.
pub const SAY_PARAMS: [&str; 2] = ["text", "voice"];

/// The rest token every viseme player writes when nothing is speaking — the
/// `sil` shape of [`VISEME_SHAPES`].
pub const SILENCE_VISEME: &str = "sil";

/// The fields of a viseme player's feedback record: the shape being made and
/// how hard it is driven, `[0, 1]` — the names ROS4HRI's `Say` feedback
/// carries them under, as Vizij extends it.
pub const FEEDBACK_VISEME: &str = "viseme";
pub const FEEDBACK_INTENSITY: &str = "intensity";
/// The feedback field the code-addressed players (set_viseme, play_visemes)
/// add: the shape's ROS4HRI code, a `u8` — the `value` of Vizij's
/// `Viseme` message, which their ROS 2 feedback carries.
pub const FEEDBACK_CODE: &str = "value";

/// The canonical say fragment asset; regenerate with
/// `vizij-bundle export-skill say`.
pub const SAY_JSON: &str = include_str!("../skills/say.json");

/// The expression skill: `set_expression` sets the face's expression and
/// holds it until a run of the expression's other setter, or a new one,
/// takes over — the named `expression` (one of the standard's
/// [`standard::expression_names`]) at the `arousal` as its intensity when
/// `expression` is not empty, else the `valence`/`arousal` point blended over
/// ROS4HRI's circumplex anchors, the way the ROS4HRI mapping computes it.
/// Its fragment ([`generate_set_expression`]) implements it; the module a
/// device registers for it only fails, and exists so the method is
/// described.
#[arora_module::contract(name = "set_expression")]
pub trait SetExpression {
    #[export(id = "824ac8fe-111d-4730-91af-413829c31556")]
    fn set_expression(
        &mut self,
        #[param(id = "b19ecf22-87e2-4510-9ce3-b50ba842d13f")] expression: String,
        #[param(id = "c142771f-bb54-4f02-a19f-136afd0f614f")] valence: f32,
        #[param(id = "151091f4-fc8e-4a2c-b553-798d23a94bcc")] arousal: f32,
    ) -> Status;
}

/// The set_expression method's parameters, in declared order.
pub const SET_EXPRESSION_PARAMS: [&str; 3] = ["expression", "valence", "arousal"];

/// The canonical set_expression fragment asset; regenerate with
/// `vizij-bundle export-skill set_expression`.
pub const SET_EXPRESSION_JSON: &str = include_str!("../skills/set_expression.json");

/// The action-unit skill: `set_facial_action_units` sets the face's
/// expression as FACS action units and holds it until a run of the
/// expression's other setter, or a new one, takes over. Sparse: `codes[i]`
/// is driven at `intensities[i]`, in `[0, 1]`, and an action unit not listed
/// rests at zero — the command describes the whole face, as a named
/// expression does. The named expressions hold at `neutral` (its weight 1,
/// the others 0): the action units move the face from its neutral pose, the
/// base a face's poses are authored over. Each code drives the muscle-tier controls expressing it
/// ([`standard::controls_for_au`]; AU 26 also the de-facto
/// [`standard::MOUTH_JAW_OPEN`]); codes the muscle tier does not express are
/// ignored, and mismatched lengths drive nothing. Its fragment
/// ([`generate_set_facial_action_units`]) implements it.
#[arora_module::contract(name = "set_facial_action_units")]
pub trait SetFacialActionUnits {
    #[export(id = "f4cc8ef7-bd1d-4e03-ba9b-1983c73e5fa5")]
    fn set_facial_action_units(
        &mut self,
        #[param(id = "30663b7d-4f51-409c-bc39-c0048c09556c")] codes: Vec<u8>,
        #[param(id = "1a5df11a-d6b6-4daa-8ebc-1aaf05d3d948")] intensities: Vec<f32>,
    ) -> Status;
}

/// The set_facial_action_units method's parameters, in declared order.
pub const SET_FACIAL_ACTION_UNITS_PARAMS: [&str; 2] = ["codes", "intensities"];

/// The canonical set_facial_action_units fragment asset; regenerate with
/// `vizij-bundle export-skill set_facial_action_units`.
pub const SET_FACIAL_ACTION_UNITS_JSON: &str =
    include_str!("../skills/set_facial_action_units.json");

/// The live viseme skill: `set_viseme` holds the shape whose code is
/// `viseme` — an index into [`VISEME_SHAPES`], ROS4HRI's order, `0` (`sil`)
/// at rest — at full weight until a new lips run takes over: the shape at the
/// playhead, for a producer that knows only that. Its fragment
/// ([`generate_set_viseme`]) implements it.
#[arora_module::contract(name = "set_viseme")]
pub trait SetViseme {
    #[export(id = "db273118-d0fe-4e0e-9fb8-884a0e24c625")]
    fn set_viseme(
        &mut self,
        #[param(id = "ccc25d4b-1568-4181-bdfe-bace0d0644a8")] viseme: u8,
    ) -> Status;
}

/// The set_viseme method's parameters, in declared order.
pub const SET_VISEME_PARAMS: [&str; 1] = ["viseme"];

/// The canonical set_viseme fragment asset; regenerate with
/// `vizij-bundle export-skill set_viseme`.
pub const SET_VISEME_JSON: &str = include_str!("../skills/set_viseme.json");

/// The viseme timeline skill: `play_visemes` plays an alignment — the shape
/// whose code is `visemes[i]` from `starts[i]` (seconds since the run's
/// start) for `durations[i]` seconds, through the lipsync driver's
/// crossfade, rest between spans — and succeeds once the last span has
/// ended and the lips have settled. Spans are expected not to overlap; where
/// they do, the higher code shows. Its fragment ([`generate_play_visemes`])
/// implements it.
#[arora_module::contract(name = "play_visemes")]
pub trait PlayVisemes {
    #[export(id = "e2a75984-edf3-42d4-a51c-231274dcdd95")]
    fn play_visemes(
        &mut self,
        #[param(id = "e2cc7afe-0916-44f9-b030-8ef2dce0a007")] visemes: Vec<u8>,
        #[param(id = "bb3b52da-872b-4dbd-8a3d-b7ee2cc0665c")] starts: Vec<f32>,
        #[param(id = "81813374-aa60-4d0e-8195-a77f6179549d")] durations: Vec<f32>,
    ) -> Status;
}

/// The play_visemes method's parameters, in declared order.
pub const PLAY_VISEMES_PARAMS: [&str; 3] = ["visemes", "starts", "durations"];

/// The canonical play_visemes fragment asset; regenerate with
/// `vizij-bundle export-skill play_visemes`.
pub const PLAY_VISEMES_JSON: &str = include_str!("../skills/play_visemes.json");

/// A played viseme's envelope, seconds: the weight ramps in over
/// [`VISEME_ATTACK`], holds until the viseme's duration has passed since its
/// start — [`VISEME_ATTACK`] plus [`VISEME_HOLD`] when it gives none — and
/// ramps out over [`VISEME_RELEASE`]: about half a second in all by default,
/// a spoken viseme's span with room to read.
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

/// An exponential smoother's per-step gain, `1 - 0.5^(dt / half_life)`, from
/// the step's duration — nodes under `id`, and the graph's `in/dt` input, so
/// once per graph. Returns the gain node.
pub(crate) fn smoothing_gain(g: &mut GraphBuilder, id: &str, half_life: f64) -> String {
    let dt_ns = g.input("in/dt", DT_KEY, json!(0.0));
    let ns_per_s = g.constant(1e9);
    let dt = g.div(&format!("{id}/dt"), &dt_ns, &ns_per_s);
    let half_life = g.constant(half_life);
    let half_lives = g.div(&format!("{id}/half_lives"), &dt, &half_life);
    let half = g.constant(0.5);
    let decay = g.op(
        &format!("{id}/decay"),
        "power",
        json!({}),
        &[("base", &half), ("exp", &half_lives)],
    );
    let one = g.constant(1.0);
    g.sub(&format!("{id}/gain"), &one, &decay)
}

/// Move the face's control at `path` toward `target` by `gain`
/// ([`smoothing_gain`]): `weight += (target - weight) * gain`, the smoother's
/// state being the control's own value read back from the store. So whoever
/// writes the control next continues from where the face is — a writer
/// taking over never snaps it. Nodes `in/<id>` and under `id`; returns the
/// weight node, for the caller to write to `path`.
pub(crate) fn follow(
    g: &mut GraphBuilder,
    id: &str,
    path: &str,
    target: &str,
    gain: &str,
) -> String {
    let previous = g.input(&format!("in/{id}"), path, json!(0.0));
    let toward = g.sub(&format!("{id}/toward"), target, &previous);
    let step = g.mul(&format!("{id}/step"), &toward, gain);
    g.add(&format!("{id}/weight"), &previous, &step)
}

/// The viseme shape a ROS4HRI code names, as text: the code indexes
/// [`VISEME_SHAPES`] — ROS4HRI numbers them in the order the 15-shape set
/// lists them, so the two tables are one — and a code past the end, or no
/// number, rests like `sil`. Returns the node holding the shape.
pub(crate) fn viseme_shape_of(g: &mut GraphBuilder, code: &str) -> String {
    let mut shape = g.text(SILENCE_VISEME);
    for (index, name) in VISEME_SHAPES.iter().enumerate().skip(1) {
        let id = |step: &str| format!("viseme/{name}/{step}");
        let wanted = g.constant(index as f64);
        let is = g.op(
            &id("is"),
            "equal",
            json!({}),
            &[("lhs", code), ("rhs", &wanted)],
        );
        let text = g.text(name);
        shape = g.select(&id("shape"), &is, &text, &shape);
    }
    shape
}

/// Regenerate the look_at fragment from first principles — the export path
/// behind the canonical asset.
///
/// The behavior, per the `interaction_skills/LookAt` policies:
///
/// - empty policy or `track`: write the goal target (and frame) onto the
///   ROS4HRI gaze keys and stay `Running` — tracking ends when the goal is
///   cancelled or replaced;
/// - `idle`: continuously wander slowly around the forward direction;
/// - `random`: continuously generate a more active random gaze trajectory;
/// - `glance` / `reset`: write the target, hold the fixation for
///   [`SETTLE_SECONDS`], then `Success`;
/// - unsupported/unknown policies: `Failure`, with the `ROS_ENOTSUP` errno.
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
    let zero = g.constant(0.0);

    // The method's parameters, staged from the run's keys (the spawn-time
    // arguments become these inputs' defaults at graft time).
    let policy = g.input("in/policy", "task/policy", json!(""));
    let target = g.input(
        "in/target",
        "task/target",
        json!({ "x": 10.0, "y": 0.0, "z": 0.0 }),
    );
    let frame = g.input("in/frame", "task/frame", json!(""));

    // Gaze:
    // - `track` / `glance`: use the supplied target.
    // - `reset`: recenter on the straight-ahead rest target.
    // - `idle`: slowly wander around the forward direction.
    // - `random`: wander faster and wider around it.
    //
    // Y controls horizontal gaze and Z controls vertical gaze. X stays fixed
    // at 10 m so the eyes look around a distant point in front of the face.
    let rest = g.node(
        "gaze/rest_target",
        "constant",
        json!({ "value": { "x": 10.0, "y": 0.0, "z": 0.0 } }),
    );

    let face_frame = g.node("gaze/face_frame", "constant", json!({ "value": "" }));

    let idle_time = g.node("gaze/idle/time", "time", json!({}));
    let idle_x = g.constant(10.0);
    // Random gaze: a more active wandering pattern than idle.
    let random_y_noise = g.op(
        "gaze/random/y/noise",
        "simplenoise",
        json!({
            "noise_seed": 101.0,
            "frequency": 0.45,
            "octaves": 1.0
        }),
        &[("x", &idle_time), ("y", &zero)],
    );
    let random_y_amp = g.constant(2.5);
    let random_y = g.mul("gaze/random/y", &random_y_noise, &random_y_amp);

    let random_z_noise = g.op(
        "gaze/random/z/noise",
        "simplenoise",
        json!({
            "noise_seed": 203.0,
            "frequency": 0.32,
            "octaves": 1.0
        }),
        &[("x", &idle_time), ("y", &zero)],
    );
    let random_z_amp = g.constant(1.5);
    let random_z = g.mul("gaze/random/z", &random_z_noise, &random_z_amp);

    let random_target = g.op(
        "gaze/random/target",
        "join",
        json!({}),
        &[
            ("operand_0", &idle_x),
            ("operand_1", &random_y),
            ("operand_2", &random_z),
        ],
    );

    // Slowly varying horizontal noise: approximately +/- 2 m.
    let idle_y_noise = g.op(
        "gaze/idle/y/noise",
        "simplenoise",
        json!({
            "noise_seed": 17.0,
            "frequency": 0.25,
            "octaves": 1.0
        }),
        &[("x", &idle_time), ("y", &zero)],
    );
    let idle_y_amp = g.constant(2.0);
    let idle_y_raw = g.mul("gaze/idle/y/raw", &idle_y_noise, &idle_y_amp);
    let idle_y = g.damp("gaze/idle/y/smooth", &idle_y_raw, 1.5);

    // Slowly varying vertical noise: approximately +/- 1 m.
    let idle_z_noise = g.op(
        "gaze/idle/z/noise",
        "simplenoise",
        json!({
            "noise_seed": 31.0,
            "frequency": 0.20,
            "octaves": 1.0
        }),
        &[("x", &idle_time), ("y", &zero)],
    );
    let idle_z_amp = g.constant(1.0);
    let idle_z_raw = g.mul("gaze/idle/z/raw", &idle_z_noise, &idle_z_amp);
    let idle_z = g.damp("gaze/idle/z/smooth", &idle_z_raw, 1.5);

    // Build the ROS4HRI Point target: X forward, Y horizontal, Z vertical.
    let idle_target = g.op(
        "gaze/idle/target",
        "join",
        json!({}),
        &[
            ("operand_0", &idle_x),
            ("operand_1", &idle_y),
            ("operand_2", &idle_z),
        ],
    );

    let gaze = g.op(
        "gaze/target",
        "case",
        json!({ "case_labels": ["reset", "idle", "random"] }),
        &[
            ("selector", &policy),
            ("operand_0", &rest),
            ("operand_1", &idle_target),
            ("operand_2", &random_target),
            ("default", &target),
        ],
    );
    g.output("out/gaze/target", &gaze, GAZE_TARGET_KEY.to_string());

    let gaze_frame = g.op(
        "gaze/frame",
        "case",
        json!({ "case_labels": ["reset", "idle", "random"] }),
        &[
            ("selector", &policy),
            ("operand_0", &face_frame),
            ("operand_1", &face_frame),
            ("operand_2", &face_frame),
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

    // The lifecycle: tracking, idle and random run until halted; glance/reset succeed
    // once the target has settled; unimplemented policies fail.
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
        json!({ "case_labels": ["", "track", "glance", "reset", "idle", "random"] }),
        &[
            ("selector", &policy),
            ("operand_0", &running),
            ("operand_1", &running),
            ("operand_2", &fixation),
            ("operand_3", &fixation),
            ("operand_4", &running),
            ("operand_5", &running),
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
        json!({ "case_labels": ["", "track", "glance", "reset", "idle", "random"] }),
        &[
            ("selector", &policy),
            ("operand_0", &silent),
            ("operand_1", &silent),
            ("operand_2", &silent),
            ("operand_3", &silent),
            ("operand_4", &silent),
            ("operand_5", &silent),
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
/// lifecycle waits for before ending. With `report_code`, the feedback also
/// carries the shape's code ([`FEEDBACK_CODE`]), for the players a client
/// addresses by code.
fn viseme_driver(
    g: &mut GraphBuilder,
    shape: (&str, &str),
    envelope: &str,
    report_code: bool,
) -> String {
    let (shape_node, shape_port) = shape;
    let zero = g.constant(0.0);
    let gain = smoothing_gain(g, "crossfade", VISEME_CROSSFADE_HALF_LIFE);
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
        // Whichever run writes a shape continues its fade from where the
        // last one left it — a run taking over never snaps, and one ending
        // leaves nothing mid-fade behind.
        let weight = follow(
            g,
            &format!("viseme/{name}"),
            &standard::viseme_path(name),
            &select,
            &gain,
        );
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
    let mut keys = vec![FEEDBACK_VISEME, FEEDBACK_INTENSITY];
    if report_code {
        keys.push(FEEDBACK_CODE);
    }
    let report = g.node(
        "viseme/report",
        "buildrecord",
        json!({ "record_keys": keys }),
    );
    g.edge(&current, &report, "field_0");
    g.edge(&intensity, &report, "field_1");
    if report_code {
        // The shape's code, typed `u8`: a `case` on the shape's name over
        // the standard's shapes in code order.
        let code = g.node(
            "viseme/code",
            "case",
            json!({ "case_labels": VISEME_SHAPES }),
        );
        g.edge(&current, &code, "selector");
        for (index, name) in VISEME_SHAPES.iter().enumerate() {
            let constant = g.node(
                &format!("viseme/{name}/code"),
                "constant",
                json!({ "value": { "u8": index } }),
            );
            g.edge(&constant, &code, &format!("operand_{index}"));
        }
        let silence = g.node(
            "viseme/silence/code",
            "constant",
            json!({ "value": { "u8": 0 } }),
        );
        g.edge(&silence, &code, "default");
        g.edge(&code, &report, "field_2");
    }
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
/// weight ramps in over [`VISEME_ATTACK`], holds until `duration` has passed
/// since the start (no shorter than the attack; [`VISEME_ATTACK`] plus
/// [`VISEME_HOLD`] when `duration` is not positive), ramps out over
/// [`VISEME_RELEASE`], and the run succeeds once the envelope has closed and
/// the lips have settled. A new run for the same shape restarts the
/// envelope: grafted later, it writes last.
pub fn generate_play_viseme() -> Json {
    let g = &mut GraphBuilder::new();
    let shape = g.input("in/shape", "task/shape", json!(SILENCE_VISEME));
    let weight = g.input("in/weight", "task/weight", json!(1.0));
    let duration = g.input("in/duration", "task/duration", json!(0.0));

    let elapsed = elapsed_since_spawn(g);
    let attack = g.constant(VISEME_ATTACK);
    let release = g.constant(VISEME_RELEASE);
    // How long the shape holds, from the start: the viseme's duration when
    // it gives one, the default otherwise, and never less than the attack.
    let zero = g.constant(0.0);
    let given = g.op(
        "envelope/given",
        "greaterthan",
        json!({}),
        &[("lhs", &duration), ("rhs", &zero)],
    );
    let default_held = g.constant(VISEME_ATTACK + VISEME_HOLD);
    let chosen = g.select("envelope/chosen", &given, &duration, &default_held);
    let held = g.max("envelope/held", &chosen, &attack);
    let total = g.add("envelope/total", &held, &release);
    // The envelope: min(ramp in, ramp out), each clamped to [0, 1].
    let rising = g.div("envelope/rising", &elapsed, &attack);
    let rise = g.clamp("envelope/rise", &rising, 0.0, 1.0);
    let remaining = g.sub("envelope/remaining", &total, &elapsed);
    let falling = g.div("envelope/falling", &remaining, &release);
    let fall = g.clamp("envelope/fall", &falling, 0.0, 1.0);
    let gate = g.min("envelope/gate", &rise, &fall);
    let envelope = g.mul("envelope/weighted", &weight, &gate);
    let settled = viseme_driver(g, (&shape, "out"), &envelope, false);

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
/// The run hosts the device's [`Say::say`] call on the run's own argument
/// bundle (`task/update`, live-updatable), with the viseme it passes in
/// stated as [`SILENCE_VISEME`] and the utterance as empty, and drives the
/// lips from the viseme the provider streams back — the lipsync driver at
/// full weight, so the current viseme's shape is on and the others fade.
/// The utterance the provider reports while its audio plays is written as
/// the face's speech state ([`standard::SPEECH`]), empty before and after.
/// The run reports the call's status as its own once the call has ended and
/// the lips have settled; until then it is running.
pub fn generate_say() -> Json {
    let g = &mut GraphBuilder::new();
    let args = g.input("in/args", "task/update", Json::Null);
    let initial_outputs = serde_json::to_value(Value::Structure(Structure {
        id: say::ids::say::FUNCTION,
        fields: vec![
            StructureField {
                id: say::ids::say::VISEME,
                value: Box::new(Value::String(SILENCE_VISEME.to_string())),
            },
            StructureField {
                id: say::ids::say::SPEECH,
                value: Box::new(Value::String(String::new())),
            },
        ],
    }))
    .expect("an argument bundle serializes");
    // The run's keyed `mutated` slots are the provider's out-parameters, in
    // this order: the viseme, then the utterance being spoken.
    let run = g.node(
        "say/call",
        "taskrun",
        json!({
            "function": say::ids::say::FUNCTION.to_string(),
            "record_keys": [
                say::ids::say::VISEME.to_string(),
                say::ids::say::SPEECH.to_string()
            ],
            "value": initial_outputs
        }),
    );
    g.edge(&args, &run, "args");
    g.output_from(
        "out/speech",
        &run,
        "mutated_1",
        standard::SPEECH.to_string(),
    );

    let full = g.constant(1.0);
    let settled = viseme_driver(g, (&run, "mutated_0"), &full, false);

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

/// The face's expression, as a run owning it writes it: every named
/// expression's weight ([`standard::expression_names`]), every muscle-tier
/// control ([`standard::FACE_CONTROLS`]) and the de-facto jaw
/// ([`standard::MOUTH_JAW_OPEN`]), by path, unprefixed. Both expression
/// setters write all of it.
pub fn expression_surface() -> Vec<String> {
    standard::expression_names()
        .map(standard::expression_path)
        .chain(
            standard::FACE_CONTROLS
                .iter()
                .map(|control| standard::face_path(control.name)),
        )
        .chain(std::iter::once(standard::MOUTH_JAW_OPEN.to_string()))
        .collect()
}

/// Hold the face's expression ([`expression_surface`]) at `targets` — each
/// control's target node by path, zero for one it does not name — each
/// control following its target from where the face is, at the ROS4HRI
/// mapping's smoothing; and run until halted. Both expression setters end
/// this way, so the run taking the face over crossfades it from whatever
/// the one before left: the expression it set fades out as the new one
/// fades in.
fn hold_expression(g: &mut GraphBuilder, targets: &[(String, String)]) {
    let zero = g.constant(0.0);
    let gain = smoothing_gain(g, "smoothing", ros4hri::HALF_LIFE);
    for path in expression_surface() {
        let id = path
            .strip_prefix(standard::VIZIJ_PREFIX)
            .and_then(|p| p.strip_prefix('/'))
            .unwrap_or(&path)
            .to_string();
        let target = targets
            .iter()
            .find(|(named, _)| *named == path)
            .map_or(zero.as_str(), |(_, node)| node.as_str())
            .to_string();
        let weight = follow(g, &id, &path, &target, &gain);
        g.output(&format!("out/{id}"), &weight, path);
    }
    let running = status_node(g, "lifecycle/running", STATUS_RUNNING_VARIANT_ID);
    g.output("out/status", &running, "task/status".to_string());
}

/// Regenerate the set_expression fragment from first principles — the
/// export path behind the canonical asset.
///
/// The run computes the expression's weights from its parameters with the
/// ROS4HRI mapping's math ([`ros4hri::expression_targets`]) over every name
/// the standard declares, rests the muscle tier, and holds that until
/// halted ([`hold_expression`]).
pub fn generate_set_expression() -> Json {
    let g = &mut GraphBuilder::new();
    let name = g.input("in/expression", "task/expression", json!(""));
    let valence = g.input("in/valence", "task/valence", json!(0.0));
    let arousal = g.input("in/arousal", "task/arousal", json!(0.0));
    let names: Vec<&str> = standard::expression_names().collect();
    let weights = ros4hri::expression_targets(g, &names, &name, &valence, &arousal);
    let targets: Vec<(String, String)> = names
        .iter()
        .map(|expr| standard::expression_path(expr))
        .zip(weights)
        .collect();
    hold_expression(g, &targets);
    json!({ "nodes": g.nodes, "edges": g.edges })
}

/// Regenerate the set_facial_action_units fragment from first principles —
/// the export path behind the canonical asset.
///
/// Per action unit the muscle tier expresses, its intensity is the sum of
/// the intensities listed under its code — `codes` and `intensities` taken
/// as vectors, the code matched elementwise (`max(0, 1 - |codes - au|)`, the
/// codes being integers) and dotted with the intensities — clamped to
/// `[0, 1]`, so an unlisted unit, or lists of mismatched lengths, read zero.
/// AU 45 (blink) counts as AU 43 (eyes closed). Each control follows its
/// unit's intensity, the named expressions hold at `neutral`, and the run
/// holds that until halted ([`hold_expression`]).
pub fn generate_set_facial_action_units() -> Json {
    let g = &mut GraphBuilder::new();
    let codes = g.input("in/codes", "task/codes", json!({ "u8s": [] }));
    let intensities = g.input("in/intensities", "task/intensities", json!({ "f32s": [] }));
    let one = g.constant(1.0);
    // Where `codes` lists `au`: 1 at its entries, 0 elsewhere.
    let matches = |g: &mut GraphBuilder, au: u8| -> String {
        let id = |step: &str| format!("au/{au}/{step}");
        let code = g.constant(f64::from(au));
        let offset = g.sub(&id("offset"), &codes, &code);
        let distance = g.op(&id("distance"), "abs", json!({}), &[("in", &offset)]);
        let near = g.sub(&id("near"), &one, &distance);
        g.clamp(&id("match"), &near, 0.0, 1.0)
    };
    let mut units: Vec<u8> = standard::FACE_CONTROLS
        .iter()
        .filter_map(|control| control.au)
        .collect();
    units.sort_unstable();
    units.dedup();
    let mut intensity_of = std::collections::BTreeMap::new();
    for au in units {
        let mut matched = matches(g, au);
        if au == 43 {
            let blink = matches(g, 45);
            matched = g.add("au/43/or_45", &matched, &blink);
        }
        let sum = g.op(
            &format!("au/{au}/sum"),
            "vectordot",
            json!({}),
            &[("a", &matched), ("b", &intensities)],
        );
        let intensity = g.clamp(&format!("au/{au}/intensity"), &sum, 0.0, 1.0);
        intensity_of.insert(au, intensity);
    }
    let mut targets: Vec<(String, String)> = standard::FACE_CONTROLS
        .iter()
        .filter_map(|control| {
            let intensity = intensity_of.get(&control.au?)?;
            Some((standard::face_path(control.name), intensity.clone()))
        })
        .collect();
    targets.push((
        standard::MOUTH_JAW_OPEN.to_string(),
        intensity_of[&26].clone(),
    ));
    targets.push((standard::expression_path("neutral"), one.clone()));
    hold_expression(g, &targets);
    json!({ "nodes": g.nodes, "edges": g.edges })
}

/// Regenerate the set_viseme fragment from first principles — the export
/// path behind the canonical asset.
///
/// The run drives the shape its code names ([`viseme_shape_of`]) at full
/// weight through the lipsync driver — the others crossfade out — and holds
/// it until halted; `sil` drives no shape, so the lips settle at rest.
pub fn generate_set_viseme() -> Json {
    let g = &mut GraphBuilder::new();
    let code = g.input("in/viseme", "task/viseme", json!(0.0));
    let shape = viseme_shape_of(g, &code);
    let full = g.constant(1.0);
    viseme_driver(g, (&shape, "out"), &full, true);
    let running = status_node(g, "lifecycle/running", STATUS_RUNNING_VARIANT_ID);
    g.output("out/status", &running, "task/status".to_string());
    json!({ "nodes": g.nodes, "edges": g.edges })
}

/// Regenerate the play_visemes fragment from first principles — the export
/// path behind the canonical asset.
///
/// The timeline as vectors: a span is on while `starts <= t < starts +
/// durations` (`t` the run's elapsed time), each bound a clamped sign
/// elementwise, and the shape shown is the highest code among the spans on
/// (the one on, for a well-formed alignment), `sil` between them. It drives
/// the lipsync driver at full weight; the run succeeds once `t` has passed
/// the last span's end and the lips have settled.
pub fn generate_play_visemes() -> Json {
    let g = &mut GraphBuilder::new();
    let visemes = g.input("in/visemes", "task/visemes", json!({ "u8s": [] }));
    let starts = g.input("in/starts", "task/starts", json!({ "f32s": [] }));
    let durations = g.input("in/durations", "task/durations", json!({ "f32s": [] }));
    let elapsed = elapsed_since_spawn(g);
    let zero = g.constant(0.0);
    let one = g.constant(1.0);

    let ends = g.add("timeline/ends", &starts, &durations);
    // Started: sign(t - start) + 1 is 0 before, 1 at, 2 after.
    let since = g.sub("timeline/since_start", &elapsed, &starts);
    let since_sign = g.op(
        "timeline/since_start/sign",
        "sign",
        json!({}),
        &[("in", &since)],
    );
    let started_raw = g.add("timeline/started/raw", &since_sign, &one);
    let started = g.clamp("timeline/started", &started_raw, 0.0, 1.0);
    // Not ended: sign(end - t) is 1 before the end, 0 or -1 from it.
    let left = g.sub("timeline/left", &ends, &elapsed);
    let left_sign = g.op("timeline/left/sign", "sign", json!({}), &[("in", &left)]);
    let ongoing = g.clamp("timeline/ongoing", &left_sign, 0.0, 1.0);
    let on = g.mul("timeline/on", &started, &ongoing);
    let spoken = g.mul("timeline/spoken", &on, &visemes);
    let code = g.op("timeline/code", "vectormax", json!({}), &[("in", &spoken)]);
    let shape = viseme_shape_of(g, &code);
    let full = g.constant(1.0);
    let settled = viseme_driver(g, (&shape, "out"), &full, true);

    // The end: the last span's (an empty timeline ends at once — the max of
    // nothing is no number, and `max` keeps the zero).
    let last = g.op("timeline/last", "vectormax", json!({}), &[("in", &ends)]);
    let end = g.max("timeline/end", &last, &zero);
    let closed = g.op(
        "lifecycle/closed",
        "greaterthan",
        json!({}),
        &[("lhs", &elapsed), ("rhs", &end)],
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

/// The parsed canonical set_expression asset.
pub fn set_expression_source() -> Json {
    serde_json::from_str(SET_EXPRESSION_JSON).expect("skills/set_expression.json parses")
}

/// The parsed canonical set_facial_action_units asset.
pub fn set_facial_action_units_source() -> Json {
    serde_json::from_str(SET_FACIAL_ACTION_UNITS_JSON)
        .expect("skills/set_facial_action_units.json parses")
}

/// The parsed canonical set_viseme asset.
pub fn set_viseme_source() -> Json {
    serde_json::from_str(SET_VISEME_JSON).expect("skills/set_viseme.json parses")
}

/// The parsed canonical play_visemes asset.
pub fn play_visemes_source() -> Json {
    serde_json::from_str(PLAY_VISEMES_JSON).expect("skills/play_visemes.json parses")
}

/// The bundle graph kind under which a skill fragment embeds in a GLB — the
/// override a face ships of a built-in skill's behavior.
pub const SKILL_KIND: &str = "skill";

/// A skill in the shipped registry: its identity, the contract's parameter
/// names, and the fragment asset behind it.
pub struct Skill {
    /// Registry id — the skill contract's name, which is also its function's
    /// (the described method name), and the id everywhere a user opts in
    /// (bundle graph ids, npm lookups).
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
pub const SKILLS: [Skill; 7] = [
    Skill {
        id: look_at::NAME,
        title: "Look At",
        description: "The ROS4HRI gaze skill (interaction_skills/LookAt on /skill/look_at): \
                      tracks a target on the standard gaze surface until cancelled, holds a \
                      glance/reset fixation then succeeds, wanders around the forward \
                      direction until cancelled for the idle/random policies, and answers \
                      ROS_ENOTSUP for the social/auto policies.",
        parameters: &LOOK_AT_PARAMS,
        asset_json: LOOK_AT_JSON,
    },
    Skill {
        id: play_viseme::NAME,
        title: "Play Viseme",
        description: "Plays one viseme shape of the face standard's 15-shape set at a weight, \
                      through the lipsync envelope (ramp in, hold, ramp out, about half a \
                      second), crossfading the other shapes out; reports the current viseme.",
        parameters: &PLAY_VISEME_PARAMS,
        asset_json: PLAY_VISEME_JSON,
    },
    Skill {
        id: say::NAME,
        title: "Say",
        description: "Speaks a text: hosts the device's text-to-speech `say` call and drives \
                      the lips from the viseme it streams, through the same lipsync driver as \
                      play_viseme; the current viseme is the run's feedback, and the utterance \
                      is the face's speech state while its audio plays.",
        parameters: &SAY_PARAMS,
        asset_json: SAY_JSON,
    },
    Skill {
        id: set_expression::NAME,
        title: "Set Expression",
        description: "Sets the face's expression and holds it until changed: a named expression \
                      at the arousal as its intensity, or a valence/arousal point blended over \
                      the circumplex; the latest of set_expression and set_facial_action_units \
                      takes the face over, crossfading from where it is.",
        parameters: &SET_EXPRESSION_PARAMS,
        asset_json: SET_EXPRESSION_JSON,
    },
    Skill {
        id: set_facial_action_units::NAME,
        title: "Set Facial Action Units",
        description: "Sets the face's expression as FACS action units (sparse: an unlisted unit \
                      rests) on the muscle tier, over the neutral expression, and holds it \
                      until changed; the latest of set_expression and set_facial_action_units \
                      takes the face over.",
        parameters: &SET_FACIAL_ACTION_UNITS_PARAMS,
        asset_json: SET_FACIAL_ACTION_UNITS_JSON,
    },
    Skill {
        id: set_viseme::NAME,
        title: "Set Viseme",
        description: "Holds one viseme shape, by its ROS4HRI code, at full weight until the next \
                      lips command — the shape at the playhead, for a live producer.",
        parameters: &SET_VISEME_PARAMS,
        asset_json: SET_VISEME_JSON,
    },
    Skill {
        id: play_visemes::NAME,
        title: "Play Visemes",
        description: "Plays a viseme timeline — each shape from its start for its duration, \
                      through the lipsync crossfade — and ends after the last; reports the \
                      current viseme.",
        parameters: &PLAY_VISEMES_PARAMS,
        asset_json: PLAY_VISEMES_JSON,
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
    use arora_types::record::module::frozen::{ExportKind, Module};

    /// A contract's one function: its name and parameter names, in order.
    fn declared(record: Module, function: Uuid) -> (String, Vec<String>) {
        let export = &record.exports[&function];
        let ExportKind::Function(signature) = &export.kind;
        let parameters = signature
            .parameter_ordering
            .iter()
            .map(|id| signature.parameters[id].name.clone())
            .collect();
        (export.name.clone(), parameters)
    }

    /// Each skill's contract is named after its one function, which the
    /// registry and the ROS profile name the skill by, and the fragment's
    /// inputs are the function's parameters.
    #[test]
    fn each_contract_declares_the_skill_s_method() {
        let parent = Uuid::nil();
        let (name, parameters) = declared(look_at::record(parent), look_at::ids::look_at::FUNCTION);
        assert_eq!(name, look_at::NAME);
        assert_eq!(parameters, LOOK_AT_PARAMS);

        let (name, parameters) = declared(
            play_viseme::record(parent),
            play_viseme::ids::play_viseme::FUNCTION,
        );
        assert_eq!(name, play_viseme::NAME);
        assert_eq!(parameters, PLAY_VISEME_PARAMS);

        let (name, parameters) = declared(say::record(parent), say::ids::say::FUNCTION);
        assert_eq!(name, say::NAME);
        assert_eq!(parameters, ["text", "voice", "viseme", "speech"]);
        assert_eq!(parameters[..SAY_PARAMS.len()], SAY_PARAMS);

        let (name, parameters) = declared(
            set_expression::record(parent),
            set_expression::ids::set_expression::FUNCTION,
        );
        assert_eq!(name, set_expression::NAME);
        assert_eq!(parameters, SET_EXPRESSION_PARAMS);

        let (name, parameters) = declared(
            set_facial_action_units::record(parent),
            set_facial_action_units::ids::set_facial_action_units::FUNCTION,
        );
        assert_eq!(name, set_facial_action_units::NAME);
        assert_eq!(parameters, SET_FACIAL_ACTION_UNITS_PARAMS);

        let (name, parameters) = declared(
            set_viseme::record(parent),
            set_viseme::ids::set_viseme::FUNCTION,
        );
        assert_eq!(name, set_viseme::NAME);
        assert_eq!(parameters, SET_VISEME_PARAMS);

        let (name, parameters) = declared(
            play_visemes::record(parent),
            play_visemes::ids::play_visemes::FUNCTION,
        );
        assert_eq!(name, play_visemes::NAME);
        assert_eq!(parameters, PLAY_VISEMES_PARAMS);
    }

    /// `say` streams the viseme and the utterance through mutable
    /// out-parameters; its inputs are not mutable.
    #[test]
    fn say_streams_the_viseme_and_the_utterance_as_out_parameters() {
        let record = say::record(Uuid::nil());
        let ExportKind::Function(signature) = &record.exports[&say::ids::say::FUNCTION].kind;
        let mutable = |id: Uuid| signature.parameters[&id].mutable;
        assert!(mutable(say::ids::say::VISEME));
        assert!(mutable(say::ids::say::SPEECH));
        assert!(!mutable(say::ids::say::TEXT));
        assert!(!mutable(say::ids::say::VOICE));
    }

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
        assert_eq!(
            set_viseme_source(),
            generate_set_viseme(),
            "skills/set_viseme.json is stale"
        );
        assert_eq!(
            play_visemes_source(),
            generate_play_visemes(),
            "skills/play_visemes.json is stale"
        );
    }

    /// The expression setters' committed assets, likewise: regenerate with
    /// `vizij-bundle export-skill set_expression` / `…
    /// set_facial_action_units`.
    #[test]
    fn committed_expression_assets_match_their_generators() {
        assert_eq!(
            set_expression_source(),
            generate_set_expression(),
            "skills/set_expression.json is stale"
        );
        assert_eq!(
            set_facial_action_units_source(),
            generate_set_facial_action_units(),
            "skills/set_facial_action_units.json is stale"
        );
    }

    /// Both expression setters write the face's whole expression — every
    /// named expression, the muscle tier and the de-facto jaw — so the one
    /// that takes the face over leaves nothing of the other's behind.
    #[test]
    fn the_expression_setters_write_the_whole_expression() {
        for source in [set_expression_source(), set_facial_action_units_source()] {
            let outputs: Vec<&str> = source["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|n| n["type"] == "output")
                .filter_map(|n| n["params"]["path"].as_str())
                .collect();
            for path in expression_surface() {
                assert!(outputs.contains(&path.as_str()), "missing {path}");
            }
            assert!(outputs.contains(&"task/status"));
        }
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
        assert_eq!(
            ids,
            [
                "look_at",
                "play_viseme",
                "say",
                "set_expression",
                "set_facial_action_units",
                "set_viseme",
                "play_visemes"
            ]
        );
        assert_eq!(listed[0]["parameters"][1], "target");
        assert_eq!(
            listed[1]["parameters"],
            json!(["shape", "weight", "duration"])
        );
        assert_eq!(listed[2]["parameters"], json!(["text", "voice"]));
        assert!(skill("look_at").is_some());
        assert!(skill("nope").is_none());
        assert_eq!(embedded_graph_id("look_at"), "skill::look_at");
        assert_eq!(skill_source("look_at"), Some(look_at_source()));
        assert_eq!(skill_source("say"), Some(say_source()));
    }

    /// The viseme players write the face standard's lipsync surface — every
    /// shape weight and the current-viseme state — and report their run.
    #[test]
    fn the_viseme_players_write_the_lipsync_surface() {
        for source in [
            play_viseme_source(),
            say_source(),
            set_viseme_source(),
            play_visemes_source(),
        ] {
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
            .any(|n| n["type"] == "taskrun"
                && n["params"]["function"] == say::ids::say::FUNCTION.to_string()));
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

    /// Idle and random are continuous gaze policies: each steers the gaze
    /// target onto its own trajectory, keeps the run Running like tracking
    /// (rather than settling like glance/reset), and answers no errno.
    #[test]
    fn look_at_continuous_policies_are_present() {
        let spec = generate_look_at();
        let nodes = spec["nodes"].as_array().expect("nodes");
        let edges = spec["edges"].as_array().expect("edges");
        // The node feeding `input` of `node`, for the case operand a label
        // selects.
        let operand = |node: &str, label: &str| -> String {
            let case = nodes
                .iter()
                .find(|n| n["id"] == node)
                .unwrap_or_else(|| panic!("node {node}"));
            let index = case["params"]["case_labels"]
                .as_array()
                .expect("case labels")
                .iter()
                .position(|l| l == label)
                .unwrap_or_else(|| panic!("{node} has no case {label:?}"));
            let input = format!("operand_{index}");
            edges
                .iter()
                .find(|e| e["to"]["node_id"] == node && e["to"]["input"] == input.as_str())
                .unwrap_or_else(|| panic!("{node}.{input} is unwired"))["from"]["node_id"]
                .as_str()
                .expect("source node id")
                .to_string()
        };
        for policy in ["idle", "random"] {
            assert_eq!(
                operand("gaze/target", policy),
                format!("gaze/{policy}/target")
            );
            assert_eq!(operand("gaze/frame", policy), "gaze/face_frame");
            assert_eq!(operand("status", policy), "status/running");
            assert_eq!(operand("errno", policy), operand("errno", "track"));
        }
    }
}
