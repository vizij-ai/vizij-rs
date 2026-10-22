//! The cost of one `step` (or `step_values`) call through the wasm guest, on
//! an animation of 300 scalar tracks looping on one player: the host encodes
//! the call, the guest steps the engine and encodes the result, the host
//! decodes it — what a device pays per frame for this module.
//!
//! ```text
//! cargo build -p vizij-animation-module --target wasm32-wasip1 --release
//! cargo run --release -p vizij-animation-module --example step_cost [step|step_values] [steps]
//! ```
//!
//! It prints the mean wall time per step. Wall time depends on the host's
//! load; a count of instructions does not: run the built example under
//! `valgrind --tool=callgrind`, or `/usr/bin/time -l` on macOS ("instructions
//! retired"), naming one function, once with `steps` and once with 0, and
//! divide the difference by `steps` — the setup and warm-up, the same in both
//! runs, cancel out.
//!
//! With `STEP_COST_NATIVE=<path to libvizij_animation_module.so/.dylib>`
//! (from `cargo build -p vizij-animation-module --release`), the engine loads
//! the module as a native library instead: the same declared entry points
//! and wire format on both sides of the call, without the wasm runtime —
//! the form callgrind can run, since wasmtime reserves more address space
//! than valgrind maps. Its counts compare one call with another, or one
//! build with another; the guest's own counts are higher, by the bounds
//! checks and copies of the wasm runtime.

use std::path::PathBuf;
use std::time::Instant;

use arora_engine::engine::EngineBuilder;
use arora_engine::executor::native::NativeExecutor;
use arora_engine::executor::wasm::WebAssemblyExecutor;
use arora_types::module::low::{Executor, ModuleDefinition};
use arora_types::value::Value;
use vizij_animation_module::{animation, AnimTrack, AnimationClip, Keypoint};

const TRACKS: usize = 300;
const WARMUP: usize = 200;
const STEPS: u32 = 2_000;

fn main() {
    // The wasm guest, or with `STEP_COST_NATIVE` the module built as a native
    // shared library: the same entry points and wire format, without the
    // wasm runtime, whose address-space reservation callgrind cannot map.
    let (executor, artifact) = match std::env::var_os("STEP_COST_NATIVE") {
        Some(library) => ("native", PathBuf::from(library)),
        None => (
            "wasm",
            std::env::var_os("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target")
                })
                .join("wasm32-wasip1/release/vizij_animation_module.wasm"),
        ),
    };
    let executable = std::fs::read(&artifact).unwrap_or_else(|e| {
        panic!("{}: {e} — build it with `cargo build -p vizij-animation-module --target wasm32-wasip1 --release` (or `--release` alone for the native library)", artifact.display())
    });
    let builder = EngineBuilder::new();
    let mut engine = if executor == "native" {
        builder.add_executor(NativeExecutor::new())
    } else {
        builder.add_executor(WebAssemblyExecutor::new().expect("wasm executor"))
    }
    .build();
    let header = animation::header(Executor {
        name: executor.to_string(),
        min_version: None,
        max_version: None,
    });
    engine
        .load_module(ModuleDefinition {
            schema_version: 0,
            header,
            executable: executable.into_boxed_slice(),
        })
        .expect("load module");

    let keypoint = |id: String, stamp: f32, v: f32| Keypoint {
        id,
        stamp,
        value: Value::F32(v),
        transitions_in: vec![],
        transitions_out: vec![],
    };
    let clip = AnimationClip {
        name: "cost".into(),
        duration: 10_000,
        tracks: (0..TRACKS)
            .map(|i| AnimTrack {
                id: format!("cost:{i}"),
                name: format!("input_{i}"),
                animatable_id: format!("rig/face/input_{i}"),
                points: vec![
                    keypoint(format!("cost:{i}:0"), 0.0, 0.0),
                    keypoint(format!("cost:{i}:1"), 1.0, 1.0),
                ],
            })
            .collect(),
    };
    let anim = animation::client::load_animation(&mut engine, clip).expect("load_animation");
    let player = animation::client::create_player(&mut engine, None).expect("create_player");
    animation::client::add_instance(&mut engine, player, anim).expect("add_instance");

    let mut only = None;
    let mut steps = STEPS;
    for arg in std::env::args().skip(1) {
        match arg.parse() {
            Ok(n) => steps = n,
            Err(_) if ["step", "step_values"].contains(&arg.as_str()) => only = Some(arg),
            Err(_) => panic!("{arg}: expected `step`, `step_values` or a number of steps"),
        }
    }
    let dt = 16_666_667u64;
    let time = |name: &str, step: &mut dyn FnMut()| {
        if only.as_deref().is_some_and(|only| only != name) {
            return;
        }
        for _ in 0..WARMUP {
            step();
        }
        let started = Instant::now();
        for _ in 0..steps {
            step();
        }
        if steps > 0 {
            let per_step = started.elapsed() / steps;
            println!(
                "{name}: {:.1} µs per step ({TRACKS} tracks, mean of {steps})",
                per_step.as_secs_f64() * 1e6
            );
        }
    };
    time("step", &mut || {
        let out = animation::client::step(&mut engine, dt, None).expect("step");
        assert_eq!(out.len(), TRACKS);
    });
    time("step_values", &mut || {
        let out = animation::client::step_values(&mut engine, dt, None).expect("step_values");
        let Value::ArrayValue(values) = out.values else {
            panic!("an array value");
        };
        assert_eq!(values.len(), TRACKS);
    });
}
