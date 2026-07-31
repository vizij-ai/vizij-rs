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
use arora_behavior_tree_types::STATUS_ENUMERATION_ID;
use arora_types::call::{Call, CallError, CallResult};
use arora_types::gen_uuid_from_str;
use arora_types::record::module::frozen::{Function, Parameter};
use arora_types::record::ty::{FrozenScalar, FrozenTy, PrimitiveKind};
use arora_types::record::{FrozenReference, Version};
use arora_types::value::{StructureField, Value};
use serde::{Deserialize, Serialize};
use soloud::*;
use uuid::Uuid;
use vizij_graph_core::task;

/// Default provider: the same Vizij TTS cloud function the web demo
/// (`@vizij/speech-react`'s `fetchVisemeData`) calls — AWS Polly behind an HTTP
/// endpoint, so the module needs no credentials. Overridable via `API_URL`.
const DEFAULT_API_BASE: &str = "https://us-central1-semio-vizij.cloudfunctions.net/api";
const DEFAULT_VOICE: &str = "Ruth";
/// The rest / silence viseme (AWS Polly viseme set), written when nothing speaks.
const SILENCE_VISEME: &str = "sil";

/// The tts module's id on the device.
pub fn module_id() -> Uuid {
    gen_uuid_from_str("tts-module")
}

/// The `say` function's id.
pub fn say_id() -> Uuid {
    gen_uuid_from_str("say")
}

fn text_param_id() -> Uuid {
    gen_uuid_from_str("say.text")
}
fn voice_param_id() -> Uuid {
    gen_uuid_from_str("say.voice")
}
fn viseme_param_id() -> Uuid {
    gen_uuid_from_str("say.viseme")
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

/// `say(text, voice) -> Status`, with a mutable `viseme` out-parameter. The
/// `Status` return is the task-run marker a bridge exposes as an action.
fn say_signature() -> Function {
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

/// Speak `text` in `voice`, streaming the current viseme. Re-invoked each tick
/// while `Running`; keeps its state in [`RUNS`], keyed by content.
fn say(call: Call) -> Result<CallResult, CallError> {
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

    // First tick: spawn synthesis + playback off the tick thread.
    if !runs.contains_key(&key) {
        runs.insert(key, spawn_say(text, voice));
    }
    let run = runs.get_mut(&key).expect("just inserted");

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
            Some(pair) => pair,
            None => return task::failure(),
        };

        // Play the whole utterance; advance the current viseme by the playhead.
        let sl = match Soloud::default() {
            Ok(sl) => sl,
            Err(_) => return task::failure(),
        };
        let mut wav = audio::WavStream::default();
        if wav.load_mem(&audio).is_err() {
            return task::failure();
        }
        let start = Instant::now();
        sl.play(&wav);
        let mut next = 0usize;
        while sl.voice_count() > 0 {
            let elapsed = start.elapsed().as_millis() as u64;
            while next < marks.len() && marks[next].time <= elapsed {
                if let Ok(mut cur) = viseme_task.lock() {
                    *cur = marks[next].value.clone();
                }
                next += 1;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
        if let Ok(mut cur) = viseme_task.lock() {
            *cur = SILENCE_VISEME.to_string();
        }
        task::success()
    });
    Run { handle, viseme }
}

/// Fetch audio (mp3) + the viseme timeline from the TTS provider — the same two
/// endpoints the web demo's `fetchVisemeData` uses. `None` on any transport or
/// decode error.
async fn synthesize(base: &str, voice: &str, text: &str) -> Option<(Vec<u8>, Vec<SpeechMark>)> {
    let client = reqwest::Client::new();
    let body = TtsRequest { voice, text };
    let audio = client
        .post(format!("{base}/tts/get-audio"))
        .json(&body)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .bytes()
        .await
        .ok()?
        .to_vec();
    let marks = client
        .post(format!("{base}/tts/get-visemes"))
        .json(&body)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<VisemeResponse>()
        .await
        .ok()?
        .visemes;
    Some((audio, marks))
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
