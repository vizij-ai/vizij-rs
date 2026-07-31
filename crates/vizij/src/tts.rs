//! The device's text-to-speech module: `say(text, voice) -> Status`.
//!
//! A poll-on-tick action (the arora-sdk `docs/async-functions.md` contract):
//! it synthesizes speech through the Vizij TTS provider — the same cloud
//! function the web demo (`@vizij/speech-react`) uses — plays the audio, and
//! streams the current viseme. It returns `Running` while speaking and `Success`
//! at the end; the heavy work runs off the tick thread and the closure only
//! polls it, exactly like arora-sdk's `polly::say`.
//!
//! The module *produces* the current viseme (as the `viseme` out-parameter). It
//! does **not** map it to face poses — the vocabulary, the destination key, and
//! the crossfade dynamics belong to the action/graph layer, not here.

use std::collections::HashMap;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use arora::{HostModule, ModuleBuilder};
use arora_types::call::{Call, CallError, CallResult};
use arora_types::value::{StructureField, Value};
use serde::{Deserialize, Serialize};
use uuid::{uuid, Uuid};
use vizij_graph_core::task;

use crate::tts_api::{
    say_id, say_signature, text_param_id, viseme_param_id, voice_param_id, SILENCE_VISEME,
};

/// Default provider: the same Vizij TTS cloud function the web demo
/// (`@vizij/speech-react`'s `fetchVisemeData`) calls — AWS Polly behind an HTTP
/// endpoint, so the module needs no credentials. Overridable via `API_URL`.
const DEFAULT_API_BASE: &str = "https://us-central1-semio-vizij.cloudfunctions.net/api";
const DEFAULT_VOICE: &str = "Ruth";

/// The tts module's id on the device.
pub fn module_id() -> Uuid {
    uuid!("4f6f0b0a-62cb-4a1f-ab0d-08f283485091")
}

/// A handle for spawning: reuse the ambient runtime if one is active, otherwise a
/// dedicated one. Only a `Handle` is needed.
static TOKIO_HANDLE: LazyLock<tokio::runtime::Handle> = LazyLock::new(|| {
    tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
        Box::leak(Box::new(
            tokio::runtime::Runtime::new().expect("a tokio runtime"),
        ))
        .handle()
        .clone()
    })
});

/// Live utterances, keyed by content so concurrent `say`s do not share a slot
/// (the module ABI hands the closure no per-run id). Each run owns the
/// synthesis+playback task and a shared viseme cell the task advances at the
/// audio playhead; the closure polls the handle and samples the cell each tick.
static RUNS: LazyLock<Mutex<HashMap<u64, Run>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// One live utterance.
struct Run {
    /// The synthesis+playback task; its output is the terminal status `Value`.
    handle: tokio::task::JoinHandle<Value>,
    /// The viseme code at the audio playhead, advanced by the task and sampled by
    /// the closure each tick.
    viseme: Arc<Mutex<String>>,
}

/// The request body both TTS endpoints take.
#[derive(Serialize)]
struct TtsRequest<'a> {
    voice: &'a str,
    text: &'a str,
}

/// One AWS Polly speech mark; the endpoint returns them already filtered to
/// `type == "viseme"`, and the other fields are ignored.
#[derive(Deserialize)]
struct SpeechMark {
    /// Milliseconds into the audio.
    time: u64,
    /// The viseme code (AWS Polly viseme set).
    value: String,
}

#[derive(Deserialize)]
struct VisemeResponse {
    visemes: Vec<SpeechMark>,
}

/// The tts module: the described `say` action, discoverable over `DescribeMethods`
/// and — via its `Status` return — exposable as a ROS 2 action by a bridge.
pub fn host_module() -> HostModule {
    ModuleBuilder::new(module_id())
        .described_function(say_id(), "say", say_signature(), say)
        .build()
}

/// Speak `text` in `voice`, streaming the current viseme. Re-invoked each tick
/// while `Running`; keeps its state in [`RUNS`], keyed by content.
pub(crate) fn say(call: Call) -> Result<CallResult, CallError> {
    let text = match arg_string(&call, text_param_id()) {
        Some(text) => text,
        None => return Ok(status_only(task::failure())),
    };
    let voice = arg_string(&call, voice_param_id()).unwrap_or_else(|| DEFAULT_VOICE.to_string());
    let key = utterance_key(&text, &voice);
    let mut runs = match RUNS.lock() {
        Ok(runs) => runs,
        Err(_) => return Ok(status_only(task::failure())),
    };

    // First tick: spawn synthesis + playback off the tick thread. Later ticks
    // find the run and fall through to the poll.
    let run = runs.entry(key).or_insert_with(|| spawn_say(text, voice));

    // The viseme at the playhead, advanced by the playback task.
    let current = run
        .viseme
        .lock()
        .map(|cur| cur.clone())
        .unwrap_or_else(|_| SILENCE_VISEME.to_string());

    // Poll the run's `JoinHandle` (a `Future`) once — the tick loop is the
    // executor, a no-op waker suffices, and a terminal result drops the run so
    // the completed handle is never polled again.
    let mut cx = Context::from_waker(Waker::noop());
    match Future::poll(Pin::new(&mut run.handle), &mut cx) {
        Poll::Pending => Ok(with_viseme(task::running(), &current)),
        Poll::Ready(Ok(status)) => {
            runs.remove(&key);
            Ok(with_viseme(status, SILENCE_VISEME))
        }
        Poll::Ready(Err(_join_error)) => {
            runs.remove(&key);
            Ok(with_viseme(task::failure(), SILENCE_VISEME))
        }
    }
}

/// Spawn synthesis (the Vizij TTS provider) + playback and return the run. The
/// task fetches audio + a viseme timeline, plays the audio, and advances the
/// shared viseme cell at the playhead. The heavy work never runs on the tick
/// thread; `say` only samples the cell and polls the handle.
fn spawn_say(text: String, voice: String) -> Run {
    let viseme = Arc::new(Mutex::new(SILENCE_VISEME.to_string()));
    let viseme_task = viseme.clone();
    let handle = TOKIO_HANDLE.spawn(async move {
        let base = std::env::var("API_URL").unwrap_or_else(|_| DEFAULT_API_BASE.to_string());
        let (audio, marks) = match synthesize(&base, &voice, &text).await {
            Ok(pair) => pair,
            Err(e) => {
                log::error!("tts: synthesis failed: {e}");
                return task::failure();
            }
        };
        // Playback blocks and rodio's stream is thread-bound, so it runs on
        // the blocking pool; this task just awaits the outcome.
        match tokio::task::spawn_blocking(move || play(audio, marks, viseme_task)).await {
            Ok(status) => status,
            Err(_join_error) => task::failure(),
        }
    });
    Run { handle, viseme }
}

/// Play the mp3 whole, advancing the shared viseme cell at the playhead.
fn play(audio: Vec<u8>, marks: Vec<SpeechMark>, viseme: Arc<Mutex<String>>) -> Value {
    let (_stream, handle) = match rodio::OutputStream::try_default() {
        Ok(pair) => pair,
        Err(e) => {
            log::error!("tts: audio output init failed: {e}");
            return task::failure();
        }
    };
    let sink = match rodio::Sink::try_new(&handle) {
        Ok(sink) => sink,
        Err(e) => {
            log::error!("tts: audio sink failed: {e}");
            return task::failure();
        }
    };
    let source = match rodio::Decoder::new(std::io::Cursor::new(audio)) {
        Ok(source) => source,
        Err(e) => {
            log::error!("tts: audio decode failed: {e}");
            return task::failure();
        }
    };
    let start = Instant::now();
    sink.append(source);
    let mut next = 0usize;
    while !sink.empty() {
        let elapsed = start.elapsed().as_millis() as u64;
        while next < marks.len() && marks[next].time <= elapsed {
            if let Ok(mut cur) = viseme.lock() {
                *cur = marks[next].value.clone();
            }
            next += 1;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    if let Ok(mut cur) = viseme.lock() {
        *cur = SILENCE_VISEME.to_string();
    }
    task::success()
}

/// Fetch audio (mp3) + the viseme timeline from the TTS provider — the same two
/// endpoints the web demo's `fetchVisemeData` uses.
async fn synthesize(
    base: &str,
    voice: &str,
    text: &str,
) -> Result<(Vec<u8>, Vec<SpeechMark>), String> {
    let client = reqwest::Client::new();
    let body = TtsRequest { voice, text };
    let audio = client
        .post(format!("{base}/tts/get-audio"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("get-audio: {e}"))?
        .error_for_status()
        .map_err(|e| format!("get-audio: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("get-audio body: {e}"))?
        .to_vec();
    let marks = client
        .post(format!("{base}/tts/get-visemes"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("get-visemes: {e}"))?
        .error_for_status()
        .map_err(|e| format!("get-visemes: {e}"))?
        .json::<VisemeResponse>()
        .await
        .map_err(|e| format!("get-visemes decode: {e}"))?
        .visemes;
    Ok((audio, marks))
}

/// A result carrying only the status (no viseme output).
fn status_only(status: Value) -> CallResult {
    CallResult {
        ret: status,
        mutated: Vec::new(),
    }
}

/// A result carrying the status plus the current viseme in the out-parameter.
fn with_viseme(status: Value, viseme: &str) -> CallResult {
    CallResult {
        ret: status,
        mutated: vec![StructureField {
            id: viseme_param_id(),
            value: Box::new(Value::String(viseme.to_string())),
        }],
    }
}

/// Read a string argument by its parameter id (order-independent).
fn arg_string(call: &Call, id: Uuid) -> Option<String> {
    match call.args.iter().find(|field| field.id == id) {
        Some(field) => match field.value.as_ref() {
            Value::String(s) => Some(s.clone()),
            _ => None,
        },
        None => None,
    }
}

/// The run key for an utterance. The module ABI hands `say` only its arguments —
/// no per-invocation identity — so runs are keyed by content: distinct (text,
/// voice) never collide, same (text, voice) share one run. Proper per-caller
/// keying needs a run id in the ABI (arora-sdk `docs/async-functions.md`).
fn utterance_key(text: &str, voice: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    voice.hash(&mut h);
    h.finish()
}
