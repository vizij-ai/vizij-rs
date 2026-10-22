//! Arora's animation module (`arora-animation-module`), registered
//! **host-side** — no wasm, no artifact dependency, so the workspace stays on
//! stable.
//!
//! [`host_module`] registers the module's declared functions over a state of
//! its own, so two devices in one process never share players, and a graph
//! `ExternalFunction` node or a client dispatches to it by the declared ids
//! exactly as it would to the loaded wasm guest. Each function reads its
//! arguments by parameter id: the order a caller sends them in carries no
//! meaning.
//!
//! An animation loads through the declared functions, whoever the client:
//! [`module_animation`] converts a face's authored animation into the clip
//! (format 2.0) that `load_clip` takes, its tracks keyed through the face's
//! rig.

use std::collections::HashMap;

use arora::HostModule;
use arora_animation_module::animation::{self, ids};
use arora_animation_module::{
    exports, AnimationModule, Clip, ClipKeypoint, ClipTrack, ClipTransition,
};
use arora_types::module::declared::AroraModule;
use arora_types::value::Value;
use uuid::Uuid;
use vizij_arora_host::contents::{Animation, AnimationTrack, Keyframe};
use vizij_arora_host::ChannelKeys;

/// The animation module as a host module over a state of its own: every
/// declared function, dispatched in-process under its declared id.
pub fn host_module() -> HostModule {
    HostModule::from_exports(ids::MODULE, exports(AnimationModule::new()))
}

// --- A face's animations, as the module takes them ---------------------------

/// `animation` as the module's [`Clip`]: each track keyed by the store key its
/// channel names (`keys`), its keyframes at signed nanoseconds, its length the
/// animation's duration (to its last keyframe when it has none). A track
/// without keyframes is left out; the keypoints are [`keypoints`]'.
pub fn module_animation(animation: &Animation, keys: &ChannelKeys) -> Clip {
    Clip {
        name: animation
            .name
            .clone()
            .unwrap_or_else(|| animation.id.clone()),
        length_ns: (animation.duration > 0.0).then(|| nanos(animation.duration)),
        tracks: animation
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| !track.keyframes.is_empty())
            .map(|(index, track)| {
                let id = format!("{}:{index}", animation.id);
                ClipTrack {
                    keypoints: keypoints(track, &id),
                    id,
                    name: track.channel.clone(),
                    target: keys.key(&track.channel),
                }
            })
            .collect(),
    }
}

/// Seconds as the clip's whole nanoseconds.
fn nanos(seconds: f64) -> i64 {
    (seconds * 1e9).round() as i64
}

/// How a segment plays, by its left keyframe's interpolation (the track's when
/// the keyframe states none).
enum Segment {
    Linear,
    Step,
    Cubic,
}

fn segment(left: &Keyframe, track: &AnimationTrack) -> Segment {
    let named = left
        .interpolation
        .as_deref()
        .or(track.interpolation.as_deref())
        .map(str::to_ascii_lowercase);
    match named.as_deref() {
        Some("cubic" | "cubicspline" | "smooth") => Segment::Cubic,
        Some("step") => Segment::Step,
        _ => Segment::Linear,
    }
}

/// A track's keyframes as the clip's keypoints, every side stated, since a
/// side left empty plays the clip format's default easing:
///
/// - **`linear`** (the default): `linear` on both sides of the segment.
/// - **`cubic`**: glTF's `CUBICSPLINE`, a Hermite segment, as the Bézier
///   handles that play it exactly — `(Δt/3, m·Δt/3)` leaving, `(−Δt/3,
///   −m·Δt/3)` arriving, `m` the side's authored tangent in value units per
///   second, or the segment's chord slope where the keyframe carries none. A
///   segment untangented at both ends is the straight line between its keys,
///   as the authoring timeline draws it.
/// - **`step`**: the left value held to the next keyframe, by a keypoint
///   carrying it a nanosecond before the keyframe's time: the clip format
///   steps no number. Not at the keyframe's time itself: at an instant two
///   keypoints share, the earlier one's segment still plays, so a step onto
///   an animation's last keyframe would end on the held value.
///
/// The first keypoint's arriving side and the last one's leaving side, which
/// no segment reads, are `linear`.
fn keypoints(track: &AnimationTrack, track_id: &str) -> Vec<ClipKeypoint> {
    let linear = || {
        vec![ClipTransition {
            kind: "linear".into(),
            dt_ns: 0.0,
            dv: 0.0,
        }]
    };
    let handle = |dt_ns: f64, dv: f64| {
        vec![ClipTransition {
            kind: "handle".into(),
            dt_ns,
            dv,
        }]
    };
    let point = |id: String, time_ns: i64, value: f64| ClipKeypoint {
        id,
        time_ns,
        value: Value::F64(value),
        arrive: linear(),
        leave: linear(),
        pairing: String::new(),
    };
    let mut points: Vec<ClipKeypoint> = Vec::with_capacity(track.keyframes.len());
    for (index, keyframe) in track.keyframes.iter().enumerate() {
        let time_ns = nanos(keyframe.time);
        let mut current = point(format!("{track_id}:{index}"), time_ns, keyframe.value);
        if let (Some(left), Some(previous)) = (
            index.checked_sub(1).map(|left| &track.keyframes[left]),
            points.last_mut(),
        ) {
            let span_ns = (time_ns - previous.time_ns) as f64;
            match segment(left, track) {
                Segment::Cubic if span_ns > 0.0 => {
                    let third = span_ns / 1e9 / 3.0;
                    let chord = (keyframe.value - left.value) / (span_ns / 1e9);
                    let leaving = left.out_tangent.unwrap_or(chord);
                    let arriving = keyframe.in_tangent.unwrap_or(chord);
                    previous.leave = handle(span_ns / 3.0, leaving * third);
                    current.arrive = handle(-span_ns / 3.0, -arriving * third);
                }
                Segment::Step if span_ns > 0.0 => {
                    let held = (time_ns - 1).max(previous.time_ns);
                    points.push(point(format!("{track_id}:{index}~"), held, left.value));
                }
                Segment::Cubic | Segment::Step | Segment::Linear => {}
            }
        }
        points.push(current);
    }
    points
}

/// `function id -> module id` over the animation functions — what
/// `ProcessingGraph::set_function_modules` needs so a bare function handle
/// (the animation source's `step`/`player_states` nodes) routes to this module.
pub fn function_modules() -> HashMap<Uuid, Uuid> {
    animation::Module::record(Uuid::nil())
        .exports
        .into_keys()
        .map(|function| (function, ids::MODULE))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_animation_module::{
        BakedAnimation, OutputKeys, PlayerState, StepValues, TrackOutput,
    };
    use arora_types::call::Call;
    use arora_types::value::StructureField;
    use arora_types::AroraType;
    use vizij_arora_host::Bundle;
    use vizij_arora_store::BlackboardStore;

    fn device() -> arora::Arora {
        arora::Arora::builder()
            .with_data_store(Box::new(BlackboardStore::new()))
            .with_host_module(host_module())
            .build()
            .expect("build arora")
    }

    fn field(id: Uuid, value: Value) -> StructureField {
        StructureField {
            id,
            value: Box::new(value),
        }
    }

    fn call(device: &mut arora::Arora, function: Uuid, args: Vec<StructureField>) -> Value {
        device
            .call(Call {
                module_id: Some(ids::MODULE),
                id: function,
                args,
            })
            .expect("the call dispatches")
            .ret
    }

    fn players(device: &mut arora::Arora) -> Vec<PlayerState> {
        call(device, ids::player_states::FUNCTION, Vec::new())
            .into_elements()
            .expect("an array of PlayerState")
            .into_iter()
            .map(|state| PlayerState::try_from(state).expect("a PlayerState"))
            .collect()
    }

    fn create_player(device: &mut arora::Arora, name: &str) -> u32 {
        match call(
            device,
            ids::create_player::FUNCTION,
            vec![field(ids::create_player::NAME, Value::String(name.into()))],
        ) {
            Value::U32(player) => player,
            other => panic!("expected a player id, got {other:?}"),
        }
    }

    fn load(device: &mut arora::Arora, clip: Clip) -> u32 {
        match call(
            device,
            ids::load_clip::FUNCTION,
            vec![field(ids::load_clip::CLIP, clip.into())],
        ) {
            Value::U32(anim) if anim != u32::MAX => anim,
            other => panic!("expected an animation id, got {other:?}"),
        }
    }

    fn add_instance(device: &mut arora::Arora, player: u32, anim: u32) -> u32 {
        match call(
            device,
            ids::add_instance::FUNCTION,
            vec![
                field(ids::add_instance::PLAYER, Value::U32(player)),
                field(ids::add_instance::ANIM, Value::U32(anim)),
            ],
        ) {
            Value::U32(instance) if instance != u32::MAX => instance,
            other => panic!("expected an instance id, got {other:?}"),
        }
    }

    /// `step_values` at device time `time_ns`, `dt_ns` after the last step.
    fn step_values(device: &mut arora::Arora, dt_ns: u64, time_ns: u64) -> Vec<Value> {
        let step = StepValues::try_from(call(
            device,
            ids::step_values::FUNCTION,
            vec![
                field(ids::step_values::DT_NS, Value::U64(dt_ns)),
                field(ids::step_values::TIME_NS, Value::U64(time_ns)),
            ],
        ))
        .expect("a StepValues record");
        let Value::ArrayValue(values) = step.values else {
            panic!("an array value");
        };
        values
    }

    #[test]
    fn two_devices_own_separate_animation_engines() {
        let mut first = device();
        let mut second = device();
        create_player(&mut first, "only-in-first");
        assert_eq!(players(&mut first).len(), 1);
        assert_eq!(players(&mut second).len(), 0);
    }

    fn authored(json: serde_json::Value) -> Animation {
        vizij_arora_host::contents::animation(&json).expect("an animation")
    }

    /// A one-track animation keyed by its channel alone.
    fn one_track(id: &str, track: serde_json::Value) -> Clip {
        module_animation(
            &authored(serde_json::json!({ "id": id, "duration": 1, "tracks": [track] })),
            &ChannelKeys::default(),
        )
    }

    /// VIZ-129: arguments are matched by parameter id, so a caller sending them
    /// in another order makes the same call.
    #[test]
    fn arguments_are_matched_by_id_whatever_their_order() {
        let mut device = device();
        let player = create_player(&mut device, "p");
        let anim = load(
            &mut device,
            one_track(
                "a",
                serde_json::json!({ "channel": "a", "keyframes": [{ "time": 0, "value": 1 }] }),
            ),
        );
        let instance = add_instance(&mut device, player, anim);
        let player_arg = field(ids::set_weight::PLAYER, Value::U32(player));
        let instance_arg = field(ids::set_weight::INSTANCE, Value::U32(instance));
        let weight = field(ids::set_weight::WEIGHT, Value::F32(0.5));
        let declared = call(
            &mut device,
            ids::set_weight::FUNCTION,
            vec![player_arg.clone(), instance_arg.clone(), weight.clone()],
        );
        let reversed = call(
            &mut device,
            ids::set_weight::FUNCTION,
            vec![weight, instance_arg, player_arg],
        );
        assert_eq!(
            declared,
            Value::U32(instance),
            "set_weight answers with the instance"
        );
        assert_eq!(reversed, declared);
    }

    #[test]
    fn a_missing_argument_fails_the_call_by_name() {
        let mut device = device();
        let error = device
            .call(Call {
                module_id: Some(ids::MODULE),
                id: ids::add_instance::FUNCTION,
                args: vec![field(ids::add_instance::ANIM, Value::U32(0))],
            })
            .expect_err("add_instance without its player");
        assert!(
            error.to_string().contains("missing parameter `player`"),
            "{error}"
        );
        let error = device
            .call(Call {
                module_id: Some(ids::MODULE),
                id: ids::add_instance_with_weight::FUNCTION,
                args: vec![
                    field(ids::add_instance_with_weight::PLAYER, Value::U32(0)),
                    field(ids::add_instance_with_weight::ANIM, Value::U32(0)),
                ],
            })
            .expect_err("add_instance_with_weight without its weight");
        assert!(
            error.to_string().contains("missing parameter `weight`"),
            "{error}"
        );
    }

    /// `vizij-arora-host`'s animation source names the module's functions, the
    /// `step` parameters and the `TrackOutput` fields by id; they are the
    /// declaration's.
    #[test]
    fn the_animation_source_names_the_declared_ids() {
        let (_, spec) = vizij_arora_host::animations_source();
        let node = |id: &str| {
            spec["nodes"]
                .as_array()
                .expect("nodes")
                .iter()
                .find(|node| node["id"] == id)
                .unwrap_or_else(|| panic!("node {id}"))
                .clone()
        };
        let arora_types::ty::low::TypeKind::Structure(output) = TrackOutput::arora_type().kind
        else {
            panic!("TrackOutput is a structure");
        };
        let field = |name: &str| {
            output
                .fields
                .iter()
                .find(|(_, field)| field.name == name)
                .map(|(id, _)| id.to_string())
                .unwrap_or_else(|| panic!("field {name}"))
        };
        let step = node("step");
        assert_eq!(step["params"]["function"], ids::step::FUNCTION.to_string());
        assert_eq!(step["params"]["param_ids"][0], ids::step::DT_NS.to_string());
        assert_eq!(
            step["params"]["param_ids"][1],
            ids::step::TIME_NS.to_string()
        );
        assert_eq!(
            node("states")["params"]["function"],
            ids::player_states::FUNCTION.to_string()
        );
        let apply = node("apply");
        assert_eq!(apply["params"]["key_field"], field("default_key"));
        assert_eq!(apply["params"]["value_field"], field("value"));
    }

    /// A side's transition, as `(kind, dt_ns, dv)`.
    type Side<'a> = (&'a str, f64, f64);

    fn side(transitions: &[ClipTransition]) -> Side<'_> {
        assert_eq!(transitions.len(), 1, "every side is stated");
        let t = &transitions[0];
        (t.kind.as_str(), t.dt_ns, t.dv)
    }

    /// Tracks key through the face's rig; keyframes take their nanoseconds;
    /// each segment is played as its left keyframe's interpolation says, every
    /// side of every keypoint stated.
    #[test]
    fn an_animation_keys_its_tracks_through_the_rig_and_states_every_side() {
        let bundle = Bundle::from_bundle_json(&serde_json::json!({
            "metadata": { "faceId": "f" },
            "graphs": [{ "kind": "rig", "spec": { "nodes": [
                { "id": "input_x", "type": "input", "params": { "path": "rig/f/x" } },
            ], "edges": [] } }],
        }));
        let converted = module_animation(
            &authored(serde_json::json!({ "id": "c", "duration": 3, "tracks": [
                { "channel": "x", "interpolation": "step", "keyframes": [
                    { "time": 0, "value": 0 },
                    { "time": 1, "value": 1, "interpolation": "cubic", "outTangent": 3 },
                    { "time": 2.5, "value": 0, "interpolation": "linear" },
                    { "time": 3, "value": 2, "inTangent": 5 },
                ] },
                { "channel": "y", "keyframes": [] },
            ] })),
            &bundle.channel_keys(),
        );
        assert_eq!(converted.name, "c");
        assert_eq!(converted.length_ns, Some(3_000_000_000));
        assert_eq!(
            converted.tracks.len(),
            1,
            "a track without keyframes is left out"
        );
        let track = &converted.tracks[0];
        assert_eq!(track.target, "rig/f/x");
        let points: Vec<(i64, Value, Side, Side)> = track
            .keypoints
            .iter()
            .map(|p| (p.time_ns, p.value.clone(), side(&p.arrive), side(&p.leave)))
            .collect();
        let linear = ("linear", 0.0, 0.0);
        // The track's step holds 0 to 1 s; the cubic leaves at its authored
        // 3/s over 1.5 s and arrives on the chord's -2/3 per second; the
        // linear segment, which ends the track, takes no tangent.
        assert_eq!(
            points,
            [
                (0, Value::F64(0.0), linear, linear),
                (999_999_999, Value::F64(0.0), linear, linear),
                (1_000_000_000, Value::F64(1.0), linear, ("handle", 5e8, 1.5)),
                (
                    2_500_000_000,
                    Value::F64(0.0),
                    ("handle", -5e8, (1.0 / 1.5) * 0.5),
                    linear
                ),
                (3_000_000_000, Value::F64(2.0), linear, linear),
            ]
        );
    }

    /// The Hermite curve a `cubic` segment from (0, `v0`) to (1 s, `v1`) plays
    /// at `u` seconds, leaving at slope `m0` and arriving at `m1`.
    fn hermite(v0: f64, m0: f64, v1: f64, m1: f64, u: f64) -> f64 {
        let (u2, u3) = (u * u, u * u * u);
        (2.0 * u3 - 3.0 * u2 + 1.0) * v0
            + (u3 - 2.0 * u2 + u) * m0
            + (-2.0 * u3 + 3.0 * u2) * v1
            + (u3 - u2) * m1
    }

    /// The values a one-track clip plays at `times` (seconds), on a player
    /// started at device time 0.
    fn played(clip: Clip, times: &[f64]) -> Vec<f64> {
        let mut device = device();
        let player = create_player(&mut device, "p");
        let anim = load(&mut device, clip);
        add_instance(&mut device, player, anim);
        step_values(&mut device, 0, 0);
        let mut last = 0;
        times
            .iter()
            .map(|seconds| {
                let now = nanos(*seconds) as u64;
                let values = step_values(&mut device, now - last, now);
                last = now;
                match values[..] {
                    [Value::F64(v)] => v,
                    ref other => panic!("one number, got {other:?}"),
                }
            })
            .collect()
    }

    const TIMES: [f64; 6] = [0.1, 0.25, 0.5, 0.6, 0.75, 0.9];

    /// A `cubic` segment plays glTF's `CUBICSPLINE` Hermite on its authored
    /// tangents, in value units per second.
    #[test]
    fn a_cubic_segment_plays_its_authored_tangents() {
        let clip = one_track(
            "h",
            serde_json::json!({ "channel": "h", "interpolation": "cubic", "keyframes": [
                { "time": 0, "value": 0.2, "outTangent": 4 },
                { "time": 1, "value": 0.7, "inTangent": -3 },
            ] }),
        );
        for (u, value) in TIMES.iter().zip(played(clip, &TIMES)) {
            let expected = hermite(0.2, 4.0, 0.7, -3.0, *u);
            assert!(
                (value - expected).abs() < 1e-9,
                "at {u} s: {value}, the Hermite gives {expected}"
            );
        }
    }

    /// A `cubic` segment untangented at both ends is the straight line
    /// between its keys, as the authoring timeline draws it; tangented at one
    /// end, the other takes the chord's slope.
    #[test]
    fn an_untangented_cubic_plays_the_chord_slope() {
        let untangented = one_track(
            "c",
            serde_json::json!({ "channel": "c", "interpolation": "cubic", "keyframes": [
                { "time": 0, "value": 1 }, { "time": 1, "value": 3 },
            ] }),
        );
        for (u, value) in TIMES.iter().zip(played(untangented, &TIMES)) {
            assert!((value - (1.0 + 2.0 * u)).abs() < 1e-9, "at {u} s: {value}");
        }
        let one_side = one_track(
            "c",
            serde_json::json!({ "channel": "c", "interpolation": "cubic", "keyframes": [
                { "time": 0, "value": 1, "outTangent": 0 }, { "time": 1, "value": 3 },
            ] }),
        );
        for (u, value) in TIMES.iter().zip(played(one_side, &TIMES)) {
            let expected = hermite(1.0, 0.0, 3.0, 2.0, *u);
            assert!((value - expected).abs() < 1e-9, "at {u} s: {value}");
        }
    }

    /// A `step` segment holds its left value until the next keyframe's time,
    /// which then plays — the last one at its instant.
    #[test]
    fn a_step_holds_its_value_to_the_next_keyframe() {
        let clip = one_track(
            "s",
            serde_json::json!({ "channel": "s", "interpolation": "step", "keyframes": [
                { "time": 0, "value": 0.25 }, { "time": 0.5, "value": 0.75 },
                { "time": 1, "value": 0.5 },
            ] }),
        );
        let values = played(clip, &[0.1, 0.4999, 0.5, 0.9, 1.0]);
        for (value, expected) in values.iter().zip([0.25, 0.25, 0.75, 0.75, 0.5]) {
            assert!((value - expected).abs() < 1e-12, "{values:?}");
        }
    }

    /// `add_instance` adds an instance blending at 1;
    /// `add_instance_with_weight` at its weight, at 0 writing nothing.
    #[test]
    fn an_instance_added_through_a_call_takes_its_weight() {
        let mut device = device();
        let player = create_player(&mut device, "p");
        for (key, weight) in [("loud", None), ("quiet", Some(0.0))] {
            let anim = load(
                &mut device,
                one_track(
                    key,
                    serde_json::json!({ "channel": key, "keyframes": [{ "time": 0, "value": 1 }] }),
                ),
            );
            match weight {
                None => {
                    add_instance(&mut device, player, anim);
                }
                Some(weight) => {
                    call(
                        &mut device,
                        ids::add_instance_with_weight::FUNCTION,
                        vec![
                            field(ids::add_instance_with_weight::PLAYER, Value::U32(player)),
                            field(ids::add_instance_with_weight::ANIM, Value::U32(anim)),
                            field(ids::add_instance_with_weight::WEIGHT, Value::F32(weight)),
                        ],
                    );
                }
            }
        }
        let out = format!(
            "{:?}",
            call(
                &mut device,
                ids::step::FUNCTION,
                vec![field(ids::step::DT_NS, Value::U64(0))],
            )
        );
        assert!(
            out.contains("loud") && !out.contains("quiet"),
            "only the weighted instance writes: {out}"
        );
    }

    /// A `play_at` anchored in the `time_ns` that `step` is given starts the
    /// player at that instant, through the declared functions: until then it
    /// waits at its start.
    #[test]
    fn play_at_starts_at_the_step_time_it_names() {
        let mut device = device();
        let player = create_player(&mut device, "anchored");
        let anim = load(
            &mut device,
            one_track(
                "a",
                serde_json::json!({ "channel": "a", "keyframes": [
                    { "time": 0, "value": 0 }, { "time": 1, "value": 1 },
                ] }),
            ),
        );
        add_instance(&mut device, player, anim);
        let step = |device: &mut arora::Arora, time_ns: u64| {
            call(
                device,
                ids::step::FUNCTION,
                vec![
                    field(ids::step::DT_NS, Value::U64(100_000_000)),
                    field(ids::step::TIME_NS, Value::U64(time_ns)),
                ],
            );
        };
        call(
            &mut device,
            ids::stop::FUNCTION,
            vec![field(ids::stop::PLAYER, Value::U32(player))],
        );
        step(&mut device, 1_000_000_000);
        let anchored = call(
            &mut device,
            ids::play_at::FUNCTION,
            vec![
                field(ids::play_at::PLAYER, Value::U32(player)),
                field(ids::play_at::TIME_NS, Value::U64(1_150_000_000)),
            ],
        );
        assert_eq!(anchored, Value::U32(player));
        step(&mut device, 1_100_000_000);
        let state = players(&mut device).remove(0);
        assert!(state.waiting, "{state:?}");
        assert_eq!(state.time_ns, 0);
        step(&mut device, 1_200_000_000);
        let state = players(&mut device).remove(0);
        assert_eq!(state.state, "playing");
        assert!(!state.waiting);
        assert_eq!(state.time_ns, 50_000_000, "50 ms after the anchor");
    }

    /// A looping player carries its travel past the bound it wraps at, and a
    /// window set while it plays leaves its playhead where it is.
    #[test]
    fn a_loop_carries_past_its_bound_and_a_window_leaves_the_playhead() {
        let mut device = device();
        let player = create_player(&mut device, "p");
        let anim = load(
            &mut device,
            one_track(
                "l",
                serde_json::json!({ "channel": "l", "keyframes": [
                    { "time": 0, "value": 0 }, { "time": 1, "value": 1 },
                ] }),
            ),
        );
        add_instance(&mut device, player, anim);
        step_values(&mut device, 0, 0);
        step_values(&mut device, 700_000_000, 700_000_000);
        step_values(&mut device, 700_000_000, 1_400_000_000);
        assert_eq!(
            players(&mut device)[0].time_ns,
            400_000_000,
            "1.4 s into a 1 s loop"
        );
        step_values(&mut device, 400_000_000, 1_800_000_000);
        let windowed = call(
            &mut device,
            ids::set_window::FUNCTION,
            vec![
                field(ids::set_window::PLAYER, Value::U32(player)),
                field(ids::set_window::START_NS, Value::U64(200_000_000)),
                field(ids::set_window::END_NS, Value::U64(600_000_000)),
            ],
        );
        assert_eq!(windowed, Value::U32(player));
        let state = players(&mut device).remove(0);
        assert_eq!(
            state.time_ns, 800_000_000,
            "outside its new window: {state:?}"
        );
        assert_eq!(
            (state.window_start_ns, state.window_end_ns),
            (200_000_000, Some(600_000_000))
        );
    }

    /// `bake` answers a call with a `BakedAnimation` record, its window
    /// arguments in nanoseconds, and `Value::Option(None)` for an animation
    /// not loaded.
    #[test]
    fn bake_answers_with_a_baked_animation_record() {
        let mut device = device();
        let anim = load(
            &mut device,
            one_track(
                "b",
                serde_json::json!({ "channel": "b", "keyframes": [
                    { "time": 0, "value": 0 }, { "time": 1, "value": 1 },
                ] }),
            ),
        );
        let bake = |device: &mut arora::Arora, anim: u32| {
            call(
                device,
                ids::bake::FUNCTION,
                vec![
                    field(ids::bake::ANIM, Value::U32(anim)),
                    field(ids::bake::FRAME_RATE, Value::F32(10.0)),
                    field(ids::bake::END_NS, Value::U64(500_000_000)),
                ],
            )
        };
        let Value::Option(Some(baked)) = bake(&mut device, anim) else {
            panic!("a baked animation");
        };
        let baked = BakedAnimation::try_from(*baked).expect("decodes");
        assert_eq!((baked.start_ns, baked.end_ns), (0, 500_000_000));
        let Value::ArrayValue(values) = &baked.tracks[0].values else {
            panic!("an array value");
        };
        assert_eq!(values.len(), 6);
        assert_eq!(bake(&mut device, anim + 1), Value::Option(None));

        // A window in seconds, the arguments' former unit, is refused.
        let error = device
            .call(Call {
                module_id: Some(ids::MODULE),
                id: ids::bake::FUNCTION,
                args: vec![
                    field(ids::bake::ANIM, Value::U32(anim)),
                    field(ids::bake::START_NS, Value::F32(0.5)),
                ],
            })
            .expect_err("an f32 start");
        assert!(error.to_string().contains("start_ns"), "{error}");
    }

    /// `step_values` answers with the values by position in the table
    /// `output_keys` answers with, both under one revision; a number plays as
    /// an `f64`.
    #[test]
    fn step_values_follow_the_output_keys() {
        let mut device = device();
        let player = create_player(&mut device, "p");
        let anim = load(
            &mut device,
            module_animation(
                &authored(serde_json::json!({ "id": "v", "duration": 1, "tracks": [
                    { "channel": "a", "keyframes": [{ "time": 0, "value": 1 }] },
                    { "channel": "b", "keyframes": [{ "time": 0, "value": 2 }] },
                ] })),
                &ChannelKeys::default(),
            ),
        );
        add_instance(&mut device, player, anim);
        let keys = OutputKeys::try_from(call(&mut device, ids::output_keys::FUNCTION, Vec::new()))
            .expect("an OutputKeys record");
        assert_eq!(keys.keys.len(), 2);
        let step = StepValues::try_from(call(
            &mut device,
            ids::step_values::FUNCTION,
            vec![field(ids::step_values::DT_NS, Value::U64(0))],
        ))
        .expect("a StepValues record");
        assert_eq!(step.revision, keys.revision);
        let Value::ArrayValue(values) = step.values else {
            panic!("an array value");
        };
        let by_key: Vec<(&str, &Value)> =
            keys.keys.iter().map(String::as_str).zip(&values).collect();
        assert_eq!(
            by_key,
            [("a", &Value::F64(1.0)), ("b", &Value::F64(2.0))],
            "{by_key:?}"
        );
    }

    #[test]
    fn an_animation_reloads_through_a_call() {
        let mut device = device();
        let clip = |key: &str| {
            one_track(
                "wave",
                serde_json::json!({ "channel": key, "keyframes": [{ "time": 0, "value": 1 }] }),
            )
        };
        let player = create_player(&mut device, "p");
        let anim = load(&mut device, clip("before"));
        add_instance(&mut device, player, anim);
        let reload = |device: &mut arora::Arora, anim: u32| {
            call(
                device,
                ids::reload_clip::FUNCTION,
                vec![
                    field(ids::reload_clip::CLIP, clip("after").into()),
                    field(ids::reload_clip::ANIM, Value::U32(anim)),
                ],
            )
        };
        assert_eq!(reload(&mut device, anim), Value::Boolean(true));
        assert_eq!(reload(&mut device, anim + 1), Value::Boolean(false));
        let out = format!(
            "{:?}",
            call(
                &mut device,
                ids::step::FUNCTION,
                vec![field(ids::step::DT_NS, Value::U64(0))],
            )
        );
        assert!(
            out.contains("after") && !out.contains("before"),
            "the instance plays the reloaded tracks: {out}"
        );
    }

    #[test]
    fn every_declared_function_is_registered_and_described() {
        let declared = function_modules();
        assert_eq!(declared.len(), 31);
        let module = host_module();
        let described: Vec<Uuid> = module.descriptions().iter().map(|d| d.id).collect();
        for function in declared.keys() {
            assert!(described.contains(function), "{function} is described");
        }
    }
}
