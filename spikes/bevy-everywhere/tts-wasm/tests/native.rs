//! The play_viseme fragment on the native interpreter through the same
//! composition the browser test uses — the product's own version of this
//! test is crates/vizij/src/device.rs
//! `a_play_viseme_run_plays_the_shape_through_its_envelope`.

#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use arora_behavior::{interpreter_module, RunPolicy, TaskHandle};
use arora_types::call::Call;
use arora_types::data::{Key, StateChange};
use arora_types::value::{StructureField, Value};
use tts_wasm_spike::*;
use vizij_arora_behavior::{task, viseme};
use vizij_arora_host::standard;

fn passthrough() -> String {
    serde_json::json!({
        "nodes": [
            { "id": "in",  "type": "input",  "params": { "path": "sensor/x" } },
            { "id": "out", "type": "output", "params": { "path": "actuator/y" } }
        ],
        "edges": [
            { "from": { "node_id": "in" }, "to": { "node_id": "out", "input": "in" } }
        ]
    })
    .to_string()
}

fn spawn(arora: &mut arora::Arora, call: &Call) -> TaskHandle {
    let spawned = arora
        .call(interpreter_module::encode_spawn(call, RunPolicy::Concurrent))
        .expect("SPAWN dispatches through the engine");
    interpreter_module::decode_spawn_result(&spawned.ret).expect("a TaskHandle comes back")
}

fn step_for(arora: &mut arora::Arora, seconds: f32) {
    let ticks = (seconds / 0.016).round() as usize;
    for _ in 0..ticks {
        arora.step(Duration::from_millis(16)).expect("step");
    }
}

/// The base graph reads `sensor/x`; an input node with neither a staged value
/// nor a default fails the whole evaluation, fragments included, so the store
/// carries one before the first tick.
fn stage_base_input(arora: &arora::Arora) {
    arora
        .store()
        .write(StateChange::set("sensor/x", Value::F32(0.0)))
        .expect("stage the base graph input");
}

fn read_value(arora: &arora::Arora, path: &str) -> Option<Value> {
    arora
        .store()
        .read(&[Key::from(path)])
        .into_iter()
        .next()
        .flatten()
}

fn read_f32(arora: &arora::Arora, path: &str) -> f32 {
    match read_value(arora, path) {
        Some(Value::F32(x)) => x,
        Some(Value::F64(x)) => x as f32,
        other => panic!("{path}: not a float: {other:?}"),
    }
}

fn text(s: &str) -> Value {
    Value::String(s.to_string())
}

fn play_viseme_call(shape: &str, weight: f32) -> Call {
    let parameters = viseme::play_viseme_parameters();
    let arg = |name: &str, value: Value| StructureField {
        id: *parameters
            .iter()
            .find(|(_, n)| n == &name)
            .expect("a declared parameter")
            .0,
        value: Box::new(value),
    };
    Call {
        module_id: Some(viseme::MODULE_ID),
        id: viseme::PLAY_VISEME_ID,
        args: vec![
            arg("shape", Value::String(shape.to_string())),
            arg("weight", Value::F32(weight)),
        ],
    }
}

#[test]
fn play_viseme_runs_on_the_native_interpreter() {
    let provider = host_module(Config {
        api_base: DEFAULT_API_BASE.to_string(),
    });
    let mut arora = compose(&passthrough(), "", provider)
        .expect("compose")
        .build()
        .expect("build arora");
    stage_base_input(&arora);
    let handle = spawn(&mut arora, &play_viseme_call("aa", 1.0));

    step_for(&mut arora, 0.2);
    let aa = read_f32(&arora, &standard::viseme_path("aa"));
    assert!(aa > 0.8, "aa = {aa} after the attack");
    assert_eq!(read_value(&arora, standard::VISEME), Some(text("aa")));
    assert_eq!(
        read_value(&arora, &handle.status.path),
        Some(task::running())
    );

    step_for(&mut arora, 0.45);
    let aa = read_f32(&arora, &standard::viseme_path("aa"));
    assert!(aa < 0.1, "aa = {aa} after the release");
    assert_eq!(read_value(&arora, standard::VISEME), Some(text("sil")));
    assert_eq!(
        read_value(&arora, &handle.status.path),
        Some(task::success())
    );
}

fn say_call(what: &str) -> Call {
    Call {
        module_id: Some(MODULE_ID),
        id: SAY_ID,
        args: vec![
            StructureField {
                id: SAY_TEXT_PARAM_ID,
                value: Box::new(text(what)),
            },
            StructureField {
                id: SAY_VOICE_PARAM_ID,
                value: Box::new(text("Ruth")),
            },
        ],
    }
}

/// The say fragment on the native interpreter over the native producer: the
/// taskrun node dispatches `say` to the provider each tick, the tokio task
/// runs the fetch, and the run ends with the producer's terminal status. The
/// synthesis base is a closed local port, so the producer fails without the
/// cloud or an audio device; the lips stay at rest throughout.
#[test]
fn say_ends_with_the_native_producer_s_status() {
    let provider = host_module(Config {
        api_base: "http://127.0.0.1:1".to_string(),
    });
    let mut arora = compose(&passthrough(), "", provider)
        .expect("compose")
        .build()
        .expect("build arora");
    stage_base_input(&arora);
    let handle = spawn(&mut arora, &say_call("hello"));

    arora.step(Duration::from_millis(16)).expect("step");
    assert_eq!(
        read_value(&arora, &handle.status.path),
        Some(task::running())
    );
    assert_eq!(read_value(&arora, standard::VISEME), Some(text("sil")));

    // The fetch fails on its own thread; the ticks sample it.
    let started = std::time::Instant::now();
    let mut ticks = 1;
    while read_value(&arora, &handle.status.path) == Some(task::running()) {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the run is still Running after {ticks} ticks"
        );
        std::thread::sleep(Duration::from_millis(16));
        arora.step(Duration::from_millis(16)).expect("step");
        ticks += 1;
    }
    assert_eq!(
        read_value(&arora, &handle.status.path),
        Some(task::failure())
    );
    assert_eq!(read_value(&arora, standard::VISEME), Some(text("sil")));
    assert!(read_f32(&arora, &standard::viseme_path("aa")) < 0.01);
    eprintln!("say ended Failure after {ticks} ticks, {:?}", started.elapsed());
}
