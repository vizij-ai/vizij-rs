//! Vizij's text-to-speech as an Arora host module.
//!
//! The **contract** — `say(text, voice) -> Status` with a mutable `viseme`
//! out-parameter — is shared by every provider: same function id, parameter
//! ids, and signature, so a behavior references `say` without caring which
//! provider a build registered. This crate ships the contract and the **cloud
//! provider** (the Vizij TTS cloud function — AWS Polly behind an HTTP
//! endpoint, no credentials in the app; `API_URL` overrides the deployment).
//! A sibling provider (e.g. the local Piper one) implements the same
//! contract with its own module id.
//!
//! A poll-on-tick action (the arora-sdk `docs/async-functions.md` contract):
//! [`say`] is re-invoked each tick while `Running`; synthesis + playback run
//! off the tick thread, and the closure only polls. The module *produces*
//! the current viseme (AWS Polly viseme codes); mapping it to face poses is
//! the caller's job. [`SILENCE_VISEME`] is written at rest.

use arora_engine::module::{HostModule, ModuleBuilder};

use std::collections::HashMap;

use arora_behavior_tree_types::STATUS_ENUMERATION_ID;
use arora_types::record::module::frozen::{Function, Parameter};
use arora_types::record::ty::{FrozenScalar, FrozenTy, PrimitiveKind};
use arora_types::record::{FrozenReference, Version};
use uuid::{uuid, Uuid};

/// The rest token, written whenever nothing is speaking (both vocabularies).
pub const SILENCE_VISEME: &str = "sil";

/// The `say` function's id — identical across providers.
pub fn say_id() -> Uuid {
    uuid!("77bf2798-e7ce-47c6-a45c-3c2e9ba1837d")
}

pub fn text_param_id() -> Uuid {
    uuid!("881dc182-d4ba-4ea0-9e81-f4eddab6f669")
}
pub fn voice_param_id() -> Uuid {
    uuid!("f56ca142-db46-4c58-bc44-7896c4b54d5c")
}
pub fn viseme_param_id() -> Uuid {
    uuid!("a1fbf58b-bf66-44a6-a503-9d9078ee5755")
}

/// `say(text, voice) -> Status`, with a mutable `viseme` out-parameter. The
/// `Status` return is the task-run marker a bridge exposes as an action.
pub fn say_signature() -> Function {
    let mut parameters = HashMap::new();
    let mut parameter_ordering = Vec::new();
    for (id, name, kind, mutable) in [
        (text_param_id(), "text", PrimitiveKind::String, false),
        (voice_param_id(), "voice", PrimitiveKind::String, false),
        (viseme_param_id(), "viseme", PrimitiveKind::String, true),
    ] {
        parameter_ordering.push(id);
        parameters.insert(
            id,
            Parameter {
                name: name.to_string(),
                ty: FrozenTy::from(kind),
                mutable,
            },
        );
    }
    Function {
        parameters,
        parameter_ordering,
        return_ty: FrozenTy::FrozenScalar(FrozenScalar {
            reference: FrozenReference {
                id: STATUS_ENUMERATION_ID,
                version: Version::parse("1.0.0").expect("a valid version"),
            },
        }),
    }
}

use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use arora_types::call::{Call, CallError, CallResult};
use arora_types::value::{StructureField, Value};
use serde::{Deserialize, Serialize};
use vizij_graph_core::task;

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
pub fn say(call: Call) -> Result<CallResult, CallError> {
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
