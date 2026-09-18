//! The viseme players on the browser interpreter: play_viseme through its
//! envelope, and say streaming the wasm provider's visemes — the provider's
//! producer a `spawn_local` future, the playhead a JS function — both as
//! task runs spawned the way a bridge spawns them.

#![cfg(target_arch = "wasm32")]

use std::rc::Rc;
use std::time::Duration;

use arora_behavior::{interpreter_module, RunPolicy, TaskHandle};
use arora_types::call::Call;
use arora_types::data::{Key, StateChange};
use arora_types::value::{StructureField, Value};
use tts_wasm_spike::*;
use vizij_arora_behavior::{task, viseme};
use vizij_arora_host::standard;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// A minimal base graph (the browser device's passthrough proof graph).
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

/// `step_for` with one console line per tick: the tick index, the run status,
/// the current viseme, the `aa`/`PP` weights and the playhead poll count the
/// page's stash carries (`-` before `play` was called). Evidence for "once
/// per tick, never after terminal" is this trace.
fn step_traced(arora: &mut arora::Arora, tick: &mut usize, seconds: f32, status_path: &str) {
    let ticks = (seconds / 0.016).round() as usize;
    for _ in 0..ticks {
        arora.step(Duration::from_millis(16)).expect("step");
        *tick += 1;
        let status = match read_value(arora, status_path) {
            Some(v) if v == task::running() => "Running",
            Some(v) if v == task::success() => "Success",
            Some(v) if v == task::failure() => "Failure",
            Some(_) => "?",
            None => "-",
        };
        let viseme = match read_value(arora, standard::VISEME) {
            Some(Value::String(s)) => s,
            _ => "-".to_string(),
        };
        let weight = |shape: &str| match read_value(arora, &standard::viseme_path(shape)) {
            Some(Value::F32(x)) => format!("{x:.2}"),
            Some(Value::F64(x)) => format!("{x:.2}"),
            _ => "-".to_string(),
        };
        let stash = js_sys::Reflect::get(&js_sys::global(), &"__spike".into()).unwrap();
        let polls = if stash.is_undefined() {
            "-".to_string()
        } else {
            js_sys::Reflect::get(&stash, &"polls".into())
                .unwrap()
                .as_f64()
                .map(|p| format!("{p}"))
                .unwrap_or_default()
        };
        console_log!(
            "tick {:>2}: status={status:<8} viseme={viseme:<4} aa={} PP={} polls={polls}",
            *tick,
            weight("aa"),
            weight("PP")
        );
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

/// Let the microtask queue run: what a `spawn_local` future needs to advance
/// between two ticks driven from a test body.
async fn yield_now() {
    JsFuture::from(js_sys::Promise::resolve(&JsValue::NULL))
        .await
        .unwrap();
}

fn mark(time: u64, value: &str) -> SpeechMark {
    SpeechMark {
        time,
        value: value.to_string(),
    }
}

/// The play_viseme fragment on the browser interpreter: the same envelope
/// and settle the native test asserts
/// (crates/vizij/src/device.rs `a_play_viseme_run_plays_the_shape_through_its_envelope`).
#[wasm_bindgen_test]
fn play_viseme_runs_on_the_browser_interpreter() {
    let provider = host_module(Config {
        api_base: DEFAULT_API_BASE.to_string(),
        play: js_sys::Function::new_no_args("return null"),
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
    assert!(read_f32(&arora, &standard::viseme_path("oh")) < 0.01);
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

/// The say fragment over the wasm provider: synthesis is a scripted future
/// (still awaited on the microtask queue through `spawn_local`), playback a
/// JS callback whose playhead advances 40 ms per poll and ends after 260 ms.
/// The lips follow the marks at the playhead; the run succeeds once the
/// playhead reports the end and the lips settle.
#[wasm_bindgen_test]
async fn say_streams_the_visemes_at_the_page_s_playhead() {
    let play = js_sys::Function::new_with_args(
        "bytes, marks",
        r#"
        globalThis.__spike = { bytes: bytes.length, marks: marks.length, first: marks[0].value, polls: 0 };
        let t = -40;
        return function playhead() {
            t += 40;
            globalThis.__spike.polls += 1;
            return t > 260 ? null : t;
        };
        "#,
    );
    let synth: Synth = Rc::new(|_text, _voice| {
        Box::pin(async {
            Ok((
                vec![1u8, 2, 3, 4],
                vec![mark(0, "p"), mark(100, "a"), mark(200, "sil")],
            ))
        })
    });
    let provider = host_module_with_synth(
        Config {
            api_base: "http://unused.invalid".to_string(),
            play,
        },
        Some(synth),
    );
    let mut arora = compose(&passthrough(), "", provider)
        .expect("compose")
        .build()
        .expect("build arora");
    stage_base_input(&arora);
    let handle = spawn(&mut arora, &say_call("hello"));

    let mut tick = 0usize;
    // Tick 1 spawns the producer; it has not run yet (a microtask).
    step_traced(&mut arora, &mut tick, 0.016, &handle.status.path);
    assert_eq!(
        read_value(&arora, &handle.status.path),
        Some(task::running())
    );
    assert!(js_sys::Reflect::get(&js_sys::global(), &"__spike".into())
        .unwrap()
        .is_undefined());

    // The producer runs on the microtask queue: synthesis, then `play`.
    yield_now().await;
    let stash = js_sys::Reflect::get(&js_sys::global(), &"__spike".into()).unwrap();
    let field = |name: &str| js_sys::Reflect::get(&stash, &name.into()).unwrap();
    assert_eq!(field("bytes").as_f64(), Some(4.0), "the audio bytes reached JS");
    assert_eq!(field("marks").as_f64(), Some(3.0), "the marks reached JS");
    assert_eq!(field("first").as_string().as_deref(), Some("p"));

    // Playhead 0, 40: the `p` mark → PP.
    step_traced(&mut arora, &mut tick, 0.032, &handle.status.path);
    assert_eq!(read_value(&arora, standard::VISEME), Some(text("PP")));
    let pp = read_f32(&arora, &standard::viseme_path("PP"));
    assert!(pp > 0.3, "PP = {pp} rising");

    // Playhead 80, 120, 160: the `a` mark at 100 → aa, PP crossfades out.
    step_traced(&mut arora, &mut tick, 0.048, &handle.status.path);
    assert_eq!(read_value(&arora, standard::VISEME), Some(text("aa")));
    assert!(read_f32(&arora, &standard::viseme_path("aa")) > 0.3);
    assert!(read_f32(&arora, &standard::viseme_path("PP")) < pp);

    // Playhead 200, 240 (sil), then null: the run ends, the lips settle.
    step_traced(&mut arora, &mut tick, 0.048, &handle.status.path);
    assert_eq!(read_value(&arora, standard::VISEME), Some(text("sil")));
    step_traced(&mut arora, &mut tick, 0.3, &handle.status.path);
    assert_eq!(
        read_value(&arora, &handle.status.path),
        Some(task::success())
    );
    assert!(read_f32(&arora, &standard::viseme_path("aa")) < 0.02);
    // The playhead was polled once per tick while playing, never after.
    let polls = field("polls").as_f64().unwrap();
    console_log!("say: {tick} ticks in all, playhead polled {polls} times");
    assert_eq!(polls, 8.0, "0..=280 ms in 40 ms steps, once per tick");
}
