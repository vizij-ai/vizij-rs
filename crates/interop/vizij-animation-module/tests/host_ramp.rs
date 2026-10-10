//! Host-side end-to-end proof: load the built `.wasm` into an Arora engine and
//! drive the module through its declared interface — the header from
//! [`animation::header`], the calls through the declaration's own client
//! stubs over the real buffer ABI — and assert a one-track 0->1 ramp advances,
//! the player states come back with their instances and weights, a paused
//! player keeps its speed, a reloaded animation writes its new tracks, the
//! animation unloads, the transport (a window, a reversed speed, instance
//! timing, an anchored start) reaches the guest, and a number track's values
//! come back in the width of its keypoints.
//!
//! What it proves is the `arora_call` boundary contract: the guest entry points
//! the declaration generates and the client stubs it generates agree, arrays of
//! structures marshal in both directions (the clip's `tracks`/`points` in, the
//! `[TrackOutput]` step result out), and a track carries its authored key. The
//! module's own logic is verified natively in `src/lib.rs` `tests`.
//!
//! Ignored by default (it needs the wasm artifact pre-built — a nested
//! `cargo build` would deadlock on the build lock). To reproduce:
//!
//! ```text
//! cargo build -p vizij-animation-module --target wasm32-wasip1
//! cargo test  -p vizij-animation-module --test host_ramp -- --ignored
//! ```

use std::path::PathBuf;
use std::pin::Pin;

use arora_engine::engine::{Engine, EngineBuilder};
use arora_engine::executor::wasm::WebAssemblyExecutor;
use arora_types::module::low::{Executor, ModuleDefinition};
use arora_types::value::Value;
use vizij_animation_module::{animation, AnimTrack, AnimationClip, Keypoint, TrackOutput};

/// A one-track 0->1 ramp over 1000 ms (keyframes at 0.0 and 1.0) targeting
/// `node/x`. The keyframe `value`s are dynamic scalars; with no authored timing
/// handles, intermediate samples follow vizij-animation-core's default ease.
fn ramp_clip() -> AnimationClip {
    let keypoint = |id: &str, stamp: f32, v: f32| Keypoint {
        id: id.into(),
        stamp,
        value: Value::F32(v),
        transitions_in: vec![],
        transitions_out: vec![],
    };
    AnimationClip {
        name: "ramp".into(),
        duration: 1000,
        tracks: vec![AnimTrack {
            id: "t0".into(),
            name: "ramp".into(),
            animatable_id: "node/x".into(),
            points: vec![keypoint("k0", 0.0, 0.0), keypoint("k1", 1.0, 1.0)],
        }],
    }
}

/// The debug wasm artifact, under `CARGO_TARGET_DIR` when the environment
/// sets it (cargo builds there), else the workspace's `target/`.
fn workspace_target_wasm() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target"))
        .join("wasm32-wasip1/debug/vizij_animation_module.wasm")
}

fn sampled(output: &TrackOutput) -> f32 {
    match output.value {
        Value::F32(f) => f,
        Value::F64(f) => f as f32,
        ref other => panic!("expected a scalar sample, got {other:?}"),
    }
}

/// An engine with the built guest loaded under its declared header.
fn engine_with_the_guest() -> Pin<Box<Engine>> {
    let mut engine = EngineBuilder::new()
        .add_executor(WebAssemblyExecutor::new().expect("wasm executor"))
        .build();
    let header = animation::header(Executor {
        name: "wasm".to_string(),
        min_version: None,
        max_version: None,
    });
    let wasm = std::fs::read(workspace_target_wasm()).expect(
        "wasm artifact missing — run `cargo build -p vizij-animation-module --target wasm32-wasip1`",
    );
    engine
        .load_module(ModuleDefinition {
            schema_version: 0,
            header,
            executable: wasm.into_boxed_slice(),
        })
        .expect("load module");
    engine
}

#[ignore = "needs the wasm artifact pre-built (a nested cargo build deadlocks on the build lock); run with --ignored after `cargo build -p vizij-animation-module --target wasm32-wasip1`"]
#[test]
fn ramp_advances_through_the_wasm_module() {
    let mut engine = engine_with_the_guest();

    // --- setup: load the clip, create a player, attach an instance ----------
    let anim = animation::client::load_animation(&mut engine, ramp_clip()).expect("load_animation");
    let player =
        animation::client::create_player(&mut engine, Some("p".into())).expect("create_player");
    let instance =
        animation::client::add_instance(&mut engine, player, anim).expect("add_instance");

    // --- step twice by 0.25 s: the ramp advances 0 -> 0.25 -> 0.5 -----------
    let quarter_s = 250_000_000u64;
    let first = animation::client::step(&mut engine, quarter_s, None).expect("step");
    let first = first.first().expect("one track output");
    assert_eq!(first.track_id, "t0");
    assert_eq!(first.default_key, "node/x");
    let second = animation::client::step(&mut engine, quarter_s, None).expect("step");
    let second = second.first().expect("one track output");
    assert_eq!(second.default_key, "node/x");

    // The sampled magnitude follows the default ease, so the assertions are the
    // interpolation-agnostic facts: strictly upward from 0, and ~0.5 at the
    // 0.5 s midpoint (the symmetric checkpoint the native unit test confirms).
    assert!(
        sampled(first) > 0.0 && sampled(first) < sampled(second),
        "ramp should advance strictly upward, got {} then {}",
        sampled(first),
        sampled(second)
    );
    assert!(
        (sampled(second) - 0.5).abs() < 1e-3,
        "expected ~0.5 at the 0.5 s midpoint, got {}",
        sampled(second)
    );

    // --- a second instance, added at weight 0 --------------------------------
    let silent = animation::client::add_instance_with_weight(&mut engine, player, anim, 0.0)
        .expect("add_instance_with_weight");

    // --- the player states come back as records, nested instances included --
    let states = animation::client::player_states(&mut engine).expect("player_states");
    assert_eq!(states.len(), 1);
    assert_eq!((states[0].player, states[0].name.as_str()), (player, "p"));
    let instances: Vec<(u32, u32, f32)> = states[0]
        .instances
        .iter()
        .map(|i| (i.instance, i.anim, i.weight))
        .collect();
    assert_eq!(instances, [(instance, anim, 1.0), (silent, anim, 0.0)]);

    // --- a paused player keeps the speed it was given -------------------------
    animation::client::set_speed(&mut engine, player, 0.5).expect("set_speed");
    animation::client::pause(&mut engine, player).expect("pause");
    animation::client::step(&mut engine, quarter_s, None).expect("step");
    let states = animation::client::player_states(&mut engine).expect("player_states");
    assert_eq!((states[0].state.as_str(), states[0].speed), ("paused", 0.5));

    // --- reloading: the same id, a track more, the playback kept --------------
    let mut reloaded = ramp_clip();
    reloaded.tracks.push(AnimTrack {
        id: "t1".into(),
        animatable_id: "node/y".into(),
        ..reloaded.tracks[0].clone()
    });
    assert!(
        animation::client::reload_animation(&mut engine, anim, reloaded).expect("reload_animation")
    );
    let outputs = animation::client::step(&mut engine, 0, None).expect("step");
    let y = outputs
        .iter()
        .find(|o| o.default_key == "node/y")
        .expect("the added track writes");
    assert_eq!(y.track_id, "t1");
    let states = animation::client::player_states(&mut engine).expect("player_states");
    assert_eq!((states[0].state.as_str(), states[0].speed), ("paused", 0.5));
    assert_eq!(states[0].instances.len(), 2, "the instances stay");

    // --- unloading: the player, then the animation ---------------------------
    assert!(animation::client::remove_player(&mut engine, player).expect("remove_player"));
    assert!(animation::client::unload_animation(&mut engine, anim).expect("unload_animation"));
    assert!(!animation::client::unload_animation(&mut engine, anim).expect("unload_animation"));
    assert!(animation::client::player_states(&mut engine)
        .expect("player_states")
        .is_empty());
}

/// The transport exports through the guest: a `once` window held at its end,
/// a reversed speed, an instance's timing read back, and a start anchored in
/// the step time.
#[ignore = "needs the wasm artifact pre-built (a nested cargo build deadlocks on the build lock); run with --ignored after `cargo build -p vizij-animation-module --target wasm32-wasip1`"]
#[test]
fn the_transport_reaches_the_wasm_module() {
    use animation::client;
    let mut engine = engine_with_the_guest();
    let anim = client::load_animation(&mut engine, ramp_clip()).expect("load_animation");
    let player = client::create_player(&mut engine, None).expect("create_player");
    let instance = client::add_instance(&mut engine, player, anim).expect("add_instance");
    let state = |engine: &mut Pin<Box<Engine>>| {
        client::player_states(engine)
            .expect("player_states")
            .remove(0)
    };

    client::set_loop(&mut engine, player, "once".into()).expect("set_loop");
    client::set_window(&mut engine, player, 0, Some(500_000_000)).expect("set_window");
    client::step(&mut engine, 750_000_000, None).expect("step");
    let held = state(&mut engine);
    assert!(held.ended, "held at the window end");
    assert_eq!(held.window_end_ns, Some(500_000_000));

    client::set_speed(&mut engine, player, -1.0).expect("set_speed");
    client::step(&mut engine, 250_000_000, None).expect("step");
    let reversed = state(&mut engine);
    assert!(!reversed.ended);
    assert!(reversed.time_ns.abs_diff(250_000_000) < 1_000);

    client::set_start_offset(&mut engine, player, instance, -100_000_000)
        .expect("set_start_offset");
    client::set_time_scale(&mut engine, player, instance, 2.0).expect("set_time_scale");
    assert_eq!(
        client::set_time_scale(&mut engine, player, instance, 0.0).expect("set_time_scale"),
        u32::MAX
    );
    client::stop(&mut engine, player).expect("stop");
    client::step(&mut engine, 0, Some(1_000_000_000)).expect("step");
    let timed = state(&mut engine);
    assert!(timed.instances[0].start_offset_ns.abs_diff(-100_000_000) < 1_000);
    assert_eq!(timed.instances[0].time_scale, 2.0);

    client::set_speed(&mut engine, player, 1.0).expect("set_speed");
    client::play_at(&mut engine, player, 1_150_000_000).expect("play_at");
    client::step(&mut engine, 100_000_000, Some(1_100_000_000)).expect("step");
    assert_eq!(state(&mut engine).state, "stopped", "waits for its start");
    client::step(&mut engine, 100_000_000, Some(1_200_000_000)).expect("step");
    let started = state(&mut engine);
    assert_eq!(started.state, "playing");
    assert!(started.time_ns.abs_diff(50_000_000) < 1_000);
}

/// Inputs that used to trap the guest are refused with a value: a bake of
/// more samples than the core bakes comes back empty, a window past the
/// clip's end bakes its end, and the arguments the module rejects come back
/// as `u32::MAX`, before anything reaches the engine. The guest answers on.
#[ignore = "needs the wasm artifact pre-built (a nested cargo build deadlocks on the build lock); run with --ignored after `cargo build -p vizij-animation-module --target wasm32-wasip1`"]
#[test]
fn bad_input_is_refused_without_a_trap() {
    use animation::client;
    let mut engine = engine_with_the_guest();
    let anim = client::load_animation(&mut engine, ramp_clip()).expect("load_animation");
    let player = client::create_player(&mut engine, Some("p".into())).expect("create_player");
    client::add_instance(&mut engine, player, anim).expect("add_instance");

    let refused = client::bake(&mut engine, anim, Some(1e9), None, None).expect("bake");
    assert!(refused.is_none(), "a billion samples are refused");
    let late = client::bake(&mut engine, anim, Some(30.0), Some(5_000_000_000), None)
        .expect("bake")
        .expect("a window past the end bakes the end");
    assert_eq!((late.start_ns, late.end_ns), (1_000_000_000, 1_000_000_000));
    assert_eq!(
        late.tracks[0].values,
        Value::ArrayValue(vec![Value::F32(1.0)])
    );

    let states = client::player_states(&mut engine).expect("player_states");
    assert_eq!((states.len(), states[0].name.as_str()), (1, "p"));
    assert_eq!(states[0].instances.len(), 1);
    let out = client::step(&mut engine, 250_000_000, None).expect("step");
    assert_eq!(out.len(), 1);

    for rejected in [f32::NAN, f32::INFINITY] {
        assert_eq!(
            client::set_speed(&mut engine, player, rejected).expect("set_speed"),
            u32::MAX
        );
    }
    assert_eq!(
        client::add_instance_with_weight(&mut engine, player, anim, -1.0)
            .expect("add_instance_with_weight"),
        u32::MAX
    );
}

/// A bake through the guest comes back as a typed `BakedAnimation`: its
/// window in nanoseconds, each track's samples as `Value`s, and the
/// derivatives as optional `Value`s when asked for.
#[ignore = "needs the wasm artifact pre-built (a nested cargo build deadlocks on the build lock); run with --ignored after `cargo build -p vizij-animation-module --target wasm32-wasip1`"]
#[test]
fn a_bake_returns_typed_samples_through_the_wasm_module() {
    use animation::client;
    let mut engine = engine_with_the_guest();
    let anim = client::load_animation(&mut engine, ramp_clip()).expect("load_animation");
    let baked = client::bake(&mut engine, anim, Some(10.0), None, Some(500_000_000))
        .expect("bake")
        .expect("a loaded animation bakes");
    assert_eq!(
        (baked.frame_rate, baked.start_ns, baked.end_ns),
        (10.0, 0, 500_000_000)
    );
    assert_eq!(baked.tracks[0].animatable_id, "node/x");
    let Value::ArrayValue(values) = &baked.tracks[0].values else {
        panic!("an array value, got {:?}", baked.tracks[0].values);
    };
    assert_eq!(values.len(), 6, "0 to 0.5 s at 10 Hz, both ends");
    assert_eq!(baked.tracks[0].derivatives, Value::ArrayValue(Vec::new()));

    let with = client::bake_with_derivatives(&mut engine, anim, Some(10.0), None, None)
        .expect("bake_with_derivatives")
        .expect("a loaded animation bakes");
    let Value::ArrayValue(derivatives) = &with.tracks[0].derivatives else {
        panic!("an array value");
    };
    assert_eq!(derivatives.len(), 11);
    assert!(matches!(derivatives[5], Value::Option(Some(_))));
    assert!(client::bake(&mut engine, anim + 1, None, None, None)
        .expect("bake")
        .is_none());
}

/// `output_keys` and `step_values` through the guest: the values come back
/// by position in the key table, under its revision.
#[ignore = "needs the wasm artifact pre-built (a nested cargo build deadlocks on the build lock); run with --ignored after `cargo build -p vizij-animation-module --target wasm32-wasip1`"]
#[test]
fn step_values_follow_the_output_keys_through_the_wasm_module() {
    use animation::client;
    let mut engine = engine_with_the_guest();
    let anim = client::load_animation(&mut engine, ramp_clip()).expect("load_animation");
    let player = client::create_player(&mut engine, None).expect("create_player");
    client::add_instance(&mut engine, player, anim).expect("add_instance");
    let keys = client::output_keys(&mut engine).expect("output_keys");
    assert_eq!(keys.keys, ["node/x"]);
    let step = client::step_values(&mut engine, 500_000_000, None).expect("step_values");
    assert_eq!(step.revision, keys.revision);
    let Value::ArrayValue(values) = step.values else {
        panic!("an array value, got {:?}", step.values);
    };
    let Value::F32(sample) = values[0] else {
        panic!("a scalar, got {:?}", values[0]);
    };
    assert!(
        (sample - 0.5).abs() < 1e-3,
        "~0.5 at the midpoint, got {sample}"
    );
}

/// A number track's values come back through the guest in the width of its
/// keypoints, from `step`, `step_values` and `bake`: `F64` keypoints give
/// `F64`, a double no `f32` holds unchanged, and `F32` keypoints give `F32`.
#[ignore = "needs the wasm artifact pre-built (a nested cargo build deadlocks on the build lock); run with --ignored after `cargo build -p vizij-animation-module --target wasm32-wasip1`"]
#[test]
fn number_tracks_keep_their_width_through_the_wasm_module() {
    use animation::client;
    let mut engine = engine_with_the_guest();
    let mut clip = ramp_clip();
    let mut held = clip.tracks[0].clone();
    for point in &mut clip.tracks[0].points {
        let Value::F32(v) = point.value else {
            panic!("an F32 keypoint");
        };
        point.value = Value::F64(f64::from(v));
    }
    held.id = "held".into();
    held.animatable_id = "node/held".into();
    held.points.drain(1..);
    held.points[0].value = Value::F64(0.1);
    clip.tracks.push(held);
    let single = client::load_animation(&mut engine, ramp_clip()).expect("load_animation");
    let double = client::load_animation(&mut engine, clip).expect("load_animation");
    let player = client::create_player(&mut engine, None).expect("create_player");
    client::add_instance(&mut engine, player, double).expect("add_instance");

    let keys = client::output_keys(&mut engine).expect("output_keys");
    assert_eq!(keys.keys, ["node/x", "node/held"]);
    let step = client::step_values(&mut engine, 500_000_000, None).expect("step_values");
    let Value::ArrayValue(values) = step.values else {
        panic!("an array value, got {:?}", step.values);
    };
    let Value::F64(sample) = values[0] else {
        panic!("an F64 track gives F64, got {:?}", values[0]);
    };
    assert!(
        (sample - 0.5).abs() < 1e-3,
        "~0.5 at the midpoint, got {sample}"
    );
    assert_eq!(values[1], Value::F64(0.1));
    let records = client::step(&mut engine, 0, None).expect("step");
    let record_values: Vec<&Value> = records.iter().map(|o| &o.value).collect();
    assert!(
        matches!(record_values[..], [Value::F64(_), Value::F64(_)]),
        "{record_values:?}"
    );
    assert_eq!(*record_values[1], Value::F64(0.1));

    for (anim, f64_width) in [(single, false), (double, true)] {
        let baked = client::bake(&mut engine, anim, Some(10.0), None, None)
            .expect("bake")
            .expect("a loaded animation bakes");
        let Value::ArrayValue(values) = &baked.tracks[0].values else {
            panic!("an array value, got {:?}", baked.tracks[0].values);
        };
        assert!(
            values.iter().all(|v| match v {
                Value::F64(_) => f64_width,
                Value::F32(_) => !f64_width,
                _ => false,
            }),
            "{values:?}"
        );
    }
}
