//! The provider on the browser: `say` streaming the visemes the page's
//! playhead reports, as a task run spawned the way a bridge spawns one —
//! synthesis a scripted future (awaited on the microtask queue through
//! `spawn_local`), playback a JS function whose playhead advances 40 ms per
//! poll and ends after 260 ms. The lips follow the marks at the playhead;
//! the run succeeds once the playhead reports the end and the lips settle.
//! The playhead is polled once per tick while playing, never after.
//!
//! `wasm-pack test --headless --chrome crates/interop/vizij-arora-tts`.

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use arora_behavior::{interpreter_module, RunPolicy, TaskHandle};
use arora_types::call::{Call, CallResult};
use arora_types::data::{Key, StateChange};
use arora_types::value::{StructureField, Value};
use vizij_arora_behavior::{parse_spec, speech, task, viseme, ProcessingGraph};
use vizij_arora_host::standard;
use vizij_arora_store::BlackboardStore;
use vizij_arora_tts::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// The passthrough proof graph as the base behavior.
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

/// The device composition the browser device performs for speech: the say
/// and play_viseme fragments on the interpreter, the provider as a host
/// module, the say call routed to it.
fn compose(provider: arora::HostModule) -> arora::Arora {
    let spec = parse_spec(&passthrough()).expect("parse");
    let mut graph = ProcessingGraph::from_spec(spec).expect("encode");
    graph.set_function_modules(HashMap::from([(SAY_ID, provider.id())]));
    graph.set_task_fragment(viseme::PLAY_VISEME_ID, viseme::play_viseme_fragment(""));
    graph.set_task_fragment(SAY_ID, speech::say_fragment(""));
    let viseme_module = arora::ModuleBuilder::new(viseme::MODULE_ID)
        .described_function(
            viseme::PLAY_VISEME_ID,
            "play_viseme",
            viseme::play_viseme_signature(),
            |_call| {
                Ok(CallResult {
                    ret: task::failure(),
                    mutated: Vec::new(),
                })
            },
        )
        .build();
    let arora = arora::Arora::builder()
        .with_data_store(Box::new(BlackboardStore::new()))
        .with_behavior_interpreter(Box::new(graph))
        .with_host_module(viseme_module)
        .with_host_module(provider)
        .build()
        .expect("build arora");
    // The base graph reads `sensor/x`; an input node with neither a staged
    // value nor a default fails the whole evaluation, fragments included.
    arora
        .store()
        .write(StateChange::set("sensor/x", Value::F32(0.0)))
        .expect("stage the base graph input");
    arora
}

fn spawn(arora: &mut arora::Arora, call: &Call) -> TaskHandle {
    let spawned = arora
        .call(interpreter_module::encode_spawn(
            call,
            RunPolicy::Concurrent,
        ))
        .expect("SPAWN dispatches through the engine");
    interpreter_module::decode_spawn_result(&spawned.ret).expect("a TaskHandle comes back")
}

fn step_for(arora: &mut arora::Arora, seconds: f32) {
    let ticks = (seconds / 0.016).round() as usize;
    for _ in 0..ticks {
        arora.step(Duration::from_millis(16)).expect("step");
    }
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

fn stash_field(name: &str) -> JsValue {
    let stash = js_sys::Reflect::get(&js_sys::global(), &"__tts".into()).unwrap();
    js_sys::Reflect::get(&stash, &name.into()).unwrap()
}

#[wasm_bindgen_test]
async fn say_streams_the_visemes_at_the_page_s_playhead() {
    let play = js_sys::Function::new_with_args(
        "bytes, marks",
        r#"
        globalThis.__tts = { bytes: bytes.length, marks: marks.length, first: marks[0].value, polls: 0 };
        let t = -40;
        return function playhead() {
            t += 40;
            globalThis.__tts.polls += 1;
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
    let mut arora = compose(provider);
    let handle = spawn(&mut arora, &say_call("hello"));

    // Tick 1 spawns the producer; it has not run yet (a microtask).
    step_for(&mut arora, 0.016);
    assert_eq!(
        read_value(&arora, &handle.status.path),
        Some(task::running())
    );
    assert!(js_sys::Reflect::get(&js_sys::global(), &"__tts".into())
        .unwrap()
        .is_undefined());

    // The producer runs on the microtask queue: synthesis, then `play`.
    yield_now().await;
    assert_eq!(
        stash_field("bytes").as_f64(),
        Some(4.0),
        "the audio bytes reached JS"
    );
    assert_eq!(
        stash_field("marks").as_f64(),
        Some(3.0),
        "the marks reached JS"
    );
    assert_eq!(stash_field("first").as_string().as_deref(), Some("p"));

    // Playhead 0, 40: the `p` mark → PP.
    step_for(&mut arora, 0.032);
    assert_eq!(read_value(&arora, standard::VISEME), Some(text("PP")));
    let pp = read_f32(&arora, &standard::viseme_path("PP"));
    assert!(pp > 0.3, "PP = {pp} rising");

    // Playhead 80, 120, 160: the `a` mark at 100 → aa, PP crossfades out.
    step_for(&mut arora, 0.048);
    assert_eq!(read_value(&arora, standard::VISEME), Some(text("aa")));
    assert!(read_f32(&arora, &standard::viseme_path("aa")) > 0.3);
    assert!(read_f32(&arora, &standard::viseme_path("PP")) < pp);

    // Playhead 200, 240 (sil), then null: the run ends, the lips settle.
    step_for(&mut arora, 0.048);
    assert_eq!(read_value(&arora, standard::VISEME), Some(text("sil")));
    step_for(&mut arora, 0.3);
    assert_eq!(
        read_value(&arora, &handle.status.path),
        Some(task::success())
    );
    assert!(read_f32(&arora, &standard::viseme_path("aa")) < 0.02);
    // The playhead was polled once per tick while playing, never after.
    assert_eq!(
        stash_field("polls").as_f64(),
        Some(8.0),
        "0..=280 ms in 40 ms steps, once per tick"
    );
}

/// A playhead that throws fails the run; the lips end at rest.
#[wasm_bindgen_test]
async fn a_failing_playback_fails_the_run() {
    let play = js_sys::Function::new_with_args(
        "bytes, marks",
        r#"return function playhead() { throw new Error("no audio output"); };"#,
    );
    let synth: Synth =
        Rc::new(|_text, _voice| Box::pin(async { Ok((vec![1u8], vec![mark(0, "a")])) }));
    let provider = host_module_with_synth(
        Config {
            api_base: "http://unused.invalid".to_string(),
            play,
        },
        Some(synth),
    );
    let mut arora = compose(provider);
    let handle = spawn(&mut arora, &say_call("hello"));
    step_for(&mut arora, 0.016);
    yield_now().await;
    step_for(&mut arora, 0.4);
    assert_eq!(
        read_value(&arora, &handle.status.path),
        Some(task::failure())
    );
    assert_eq!(read_value(&arora, standard::VISEME), Some(text("sil")));
}
