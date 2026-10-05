//! Host-side end-to-end proof: load the built `.wasm` into an Arora engine and
//! drive the module through its declared interface — the header from
//! [`animation::header`], the calls through the declaration's own client
//! stubs over the real buffer ABI — and assert a one-track 0->1 ramp advances,
//! the player states come back with their instances, and the animation
//! unloads.
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

use arora_engine::engine::EngineBuilder;
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

fn workspace_target_wasm() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("target/wasm32-wasip1/debug/vizij_animation_module.wasm")
}

fn sampled(output: &TrackOutput) -> f32 {
    match output.value {
        Value::F32(f) => f,
        Value::F64(f) => f as f32,
        ref other => panic!("expected a scalar sample, got {other:?}"),
    }
}

#[ignore = "needs the wasm artifact pre-built (a nested cargo build deadlocks on the build lock); run with --ignored after `cargo build -p vizij-animation-module --target wasm32-wasip1`"]
#[test]
fn ramp_advances_through_the_wasm_module() {
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

    // --- setup: load the clip, create a player, attach an instance ----------
    let anim = animation::client::load_animation(&mut engine, ramp_clip()).expect("load_animation");
    let player =
        animation::client::create_player(&mut engine, Some("p".into())).expect("create_player");
    let instance =
        animation::client::add_instance(&mut engine, player, anim, None).expect("add_instance");

    // --- step twice by 0.25 s: the ramp advances 0 -> 0.25 -> 0.5 -----------
    let quarter_s = 250_000_000u64;
    let first = animation::client::step(&mut engine, quarter_s).expect("step");
    let first = first.first().expect("one track output");
    assert_eq!(first.track_id, "t0");
    assert_eq!(first.default_key, "node/x");
    let second = animation::client::step(&mut engine, quarter_s).expect("step");
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

    // --- the player states come back as records, nested instances included --
    let states = animation::client::player_states(&mut engine).expect("player_states");
    assert_eq!(states.len(), 1);
    assert_eq!((states[0].player, states[0].name.as_str()), (player, "p"));
    assert_eq!(states[0].instances.len(), 1);
    assert_eq!(
        (states[0].instances[0].instance, states[0].instances[0].anim),
        (instance, anim)
    );

    // --- unloading: the player, then the animation ---------------------------
    assert!(animation::client::remove_player(&mut engine, player).expect("remove_player"));
    assert!(animation::client::unload_animation(&mut engine, anim).expect("unload_animation"));
    assert!(!animation::client::unload_animation(&mut engine, anim).expect("unload_animation"));
    assert!(animation::client::player_states(&mut engine)
        .expect("player_states")
        .is_empty());
}
