//! Spike: the Vizij cloud text-to-speech provider with a wasm32 branch.
//!
//! The contract is unchanged and shared: `say(text, voice) -> Status` with a
//! mutable `viseme` out-parameter, re-invoked each tick while `Running`
//! (vizij-arora-behavior's speech skill; arora-sdk `docs/async-functions.md`).
//! Synthesis (`synthesize`) is target-independent: reqwest 0.13 is `fetch` on
//! wasm32 and hyper natively. What differs per target is the **producer** the
//! tick samples:
//!
//! - native (`platform::native`): a tokio task fetches, a blocking-pool task
//!   plays through rodio; the playhead is `Sink::get_pos()`.
//! - wasm32 (`platform::web`): a `spawn_local` future fetches, then hands
//!   `{audioBytes, marks}` to the page's playback callback, which returns a
//!   `playhead()` function; the tick samples it. The page owns the
//!   `AudioContext` and its user-gesture unlock — the module never touches
//!   Web Audio.
//!
//! The run map is owned by the module closure (a `HostModule` function is a
//! `Box<dyn FnMut>`, no `Send` bound), so a run may hold a `JsValue`; a
//! `static Mutex` map could not. `API_URL` is a constructor parameter.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use arora::{HostModule, ModuleBuilder};
use arora_types::call::{Call, CallError, CallResult};
use arora_types::value::{StructureField, Value};
use serde::{Deserialize, Serialize};
use uuid::{uuid, Uuid};
use vizij_graph_core::task;

pub use vizij_arora_behavior::speech::{
    say_signature, SAY_ID, SAY_TEXT_PARAM_ID, SAY_VISEME_PARAM_ID, SAY_VOICE_PARAM_ID,
    SILENCE_VISEME,
};

/// The tts module's id on the device (the cloud provider's).
pub const MODULE_ID: Uuid = uuid!("4f6f0b0a-62cb-4a1f-ab0d-08f283485091");
pub const DEFAULT_API_BASE: &str = "https://us-central1-semio-vizij.cloudfunctions.net/api";
const DEFAULT_VOICE: &str = "Ruth";

/// The face-standard shape for a Polly viseme code.
pub fn polly_shape(code: &str) -> &'static str {
    match code {
        "sil" => "sil",
        "p" => "PP",
        "f" => "FF",
        "T" => "TH",
        "t" => "DD",
        "k" => "kk",
        "S" => "CH",
        "s" => "SS",
        "l" => "nn",
        "r" => "RR",
        "a" => "aa",
        "e" | "E" => "E",
        "i" | "@" => "ih",
        "o" | "O" => "oh",
        "u" => "ou",
        other => {
            log::warn!("tts: unknown Polly viseme code {other:?}, at rest");
            SILENCE_VISEME
        }
    }
}

/// The request body both TTS endpoints take.
#[derive(Serialize)]
struct TtsRequest<'a> {
    voice: &'a str,
    text: &'a str,
}

/// One AWS Polly speech mark (already filtered to `type == "viseme"`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SpeechMark {
    /// Milliseconds into the audio.
    pub time: u64,
    /// The viseme code.
    pub value: String,
}

#[derive(Deserialize)]
struct VisemeResponse {
    visemes: Vec<SpeechMark>,
}

/// Audio (mp3) + the viseme timeline. Target-independent: reqwest selects
/// `fetch` on wasm32.
pub async fn synthesize(
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

/// The shape at `playhead_ms`: every mark at or before the playhead is
/// consumed, the last one consumed is the current shape. Pure and shared.
pub fn shape_at(
    marks: &[SpeechMark],
    next: &mut usize,
    playhead_ms: u64,
    current: &'static str,
) -> &'static str {
    let mut shape = current;
    while *next < marks.len() && marks[*next].time <= playhead_ms {
        shape = polly_shape(&marks[*next].value);
        *next += 1;
    }
    shape
}

/// A synthesis producer: what `say` awaits for `(audio, marks)`. The default
/// is [`synthesize`] against the configured base; tests script one.
pub type Synth = std::rc::Rc<
    dyn Fn(String, String) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(Vec<u8>, Vec<SpeechMark>), String>>>>,
>;

/// Provider configuration — the constructor parameters that replace
/// `std::env::var("API_URL")` and, on wasm32, name the page's playback.
pub struct Config {
    pub api_base: String,
    /// wasm32: `play(audioBytes: Uint8Array, marks: {time, value}[]) ->
    /// playhead: () => number | null`. Milliseconds since the audio started
    /// (0 while decoding or waiting for the context to resume), `null` once
    /// playback ended; a throw is a failure.
    #[cfg(target_arch = "wasm32")]
    pub play: js_sys::Function,
}

/// The tts module: the described `say` action.
pub fn host_module(config: Config) -> HostModule {
    host_module_with_synth(config, None)
}

/// The module with a scripted synthesizer (tests) or the default (`None`).
pub fn host_module_with_synth(config: Config, synth: Option<Synth>) -> HostModule {
    let mut provider = Provider {
        config,
        synth,
        runs: HashMap::new(),
    };
    ModuleBuilder::new(MODULE_ID)
        .described_function(SAY_ID, "say", say_signature(), move |call| provider.say(call))
        .build()
}

/// The module's state: its configuration and the live runs, keyed by content
/// because the ABI hands the closure no per-run id.
struct Provider {
    config: Config,
    synth: Option<Synth>,
    runs: HashMap<u64, platform::Run>,
}

/// What one tick observes of a run.
pub enum Observed {
    Running(&'static str),
    Ended(Value),
}

impl Provider {
    fn say(&mut self, call: Call) -> Result<CallResult, CallError> {
        let Some(text) = arg_string(&call, SAY_TEXT_PARAM_ID) else {
            return Ok(status_only(task::failure()));
        };
        let voice =
            arg_string(&call, SAY_VOICE_PARAM_ID).unwrap_or_else(|| DEFAULT_VOICE.to_string());
        let key = utterance_key(&text, &voice);
        let synth = self.synth.clone();
        let config = &self.config;
        let run = self
            .runs
            .entry(key)
            .or_insert_with(|| platform::spawn(config, synth, text, voice));
        match platform::poll(run) {
            Observed::Running(viseme) => Ok(with_viseme(task::running(), viseme)),
            Observed::Ended(status) => {
                self.runs.remove(&key);
                Ok(with_viseme(status, SILENCE_VISEME))
            }
        }
    }
}

/// The producer per target: `Run`, `spawn`, `poll`.
pub mod platform {
    #[cfg(not(target_arch = "wasm32"))]
    pub use self::native::*;
    #[cfg(target_arch = "wasm32")]
    pub use self::web::*;

    /// The synthesis future for a run: the scripted one or the default fetch.
    /// Only the web producer can await a scripted (`Rc`) synthesizer: the
    /// native producer runs on the tokio runtime, which needs `Send`.
    #[cfg(target_arch = "wasm32")]
    fn synthesis(
        synth: Option<super::Synth>,
        base: String,
        voice: String,
        text: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(Vec<u8>, Vec<super::SpeechMark>), String>>>>
    {
        match synth {
            Some(synth) => synth(text, voice),
            None => Box::pin(async move { super::synthesize(&base, &voice, &text).await }),
        }
    }

    /// Native: a tokio task fetches, a blocking-pool task plays through rodio;
    /// the tick polls the `JoinHandle` with a no-op waker and samples the
    /// shared viseme cell. The playhead is the sink's own position.
    #[cfg(not(target_arch = "wasm32"))]
    mod native {
        use super::super::{shape_at, Config, Observed, SpeechMark, Synth, SILENCE_VISEME};
        use arora_types::value::Value;
        use std::future::Future;
        use std::pin::Pin;
        use std::sync::{Arc, LazyLock, Mutex};
        use std::task::{Context, Poll, Waker};
        use vizij_graph_core::task;

        static TOKIO_HANDLE: LazyLock<tokio::runtime::Handle> = LazyLock::new(|| {
            tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
                Box::leak(Box::new(
                    tokio::runtime::Runtime::new().expect("a tokio runtime"),
                ))
                .handle()
                .clone()
            })
        });

        pub struct Run {
            handle: tokio::task::JoinHandle<Value>,
            viseme: Arc<Mutex<&'static str>>,
        }

        pub fn spawn(config: &Config, synth: Option<Synth>, text: String, voice: String) -> Run {
            let viseme = Arc::new(Mutex::new(SILENCE_VISEME));
            let cell = viseme.clone();
            // A scripted `Synth` is `Rc` (single-threaded, the wasm shape) and
            // cannot cross onto the runtime; natively the fetch is the producer.
            if synth.is_some() {
                log::warn!("tts: a scripted synthesizer is not exercised natively");
            }
            drop(synth);
            let base = config.api_base.clone();
            let handle = TOKIO_HANDLE.spawn(async move {
                let (audio, marks) = match super::super::synthesize(&base, &voice, &text).await {
                    Ok(pair) => pair,
                    Err(e) => {
                        log::error!("tts: synthesis failed: {e}");
                        return task::failure();
                    }
                };
                match tokio::task::spawn_blocking(move || play(audio, marks, cell)).await {
                    Ok(status) => status,
                    Err(_) => task::failure(),
                }
            });
            Run { handle, viseme }
        }

        pub fn poll(run: &mut Run) -> Observed {
            let current = run.viseme.lock().map(|c| *c).unwrap_or(SILENCE_VISEME);
            let mut cx = Context::from_waker(Waker::noop());
            match Future::poll(Pin::new(&mut run.handle), &mut cx) {
                Poll::Pending => Observed::Running(current),
                Poll::Ready(Ok(status)) => Observed::Ended(status),
                Poll::Ready(Err(_)) => Observed::Ended(task::failure()),
            }
        }

        /// Play the mp3 whole; the viseme cell follows `Sink::get_pos()`.
        fn play(audio: Vec<u8>, marks: Vec<SpeechMark>, viseme: Arc<Mutex<&'static str>>) -> Value {
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
            sink.append(source);
            let mut next = 0usize;
            let mut current = SILENCE_VISEME;
            while !sink.empty() {
                let playhead_ms = sink.get_pos().as_millis() as u64;
                current = shape_at(&marks, &mut next, playhead_ms, current);
                if let Ok(mut cell) = viseme.lock() {
                    *cell = current;
                }
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            if let Ok(mut cell) = viseme.lock() {
                *cell = SILENCE_VISEME;
            }
            task::success()
        }
    }

    /// wasm32: a `spawn_local` future fetches, then hands the page's playback
    /// callback `{audioBytes, marks}` and keeps the `playhead()` it returns;
    /// the tick calls it and maps the marks to the current shape.
    #[cfg(target_arch = "wasm32")]
    mod web {
        use super::super::{shape_at, Config, Observed, SpeechMark, Synth, SILENCE_VISEME};
        use arora_types::value::Value;
        use std::cell::RefCell;
        use std::rc::Rc;
        use vizij_graph_core::task;
        use wasm_bindgen::{JsCast, JsValue};

        enum Phase {
            Fetching,
            Playing {
                playhead: js_sys::Function,
                marks: Vec<SpeechMark>,
                next: usize,
                current: &'static str,
            },
            Ended(Value),
        }

        pub struct Run {
            phase: Rc<RefCell<Phase>>,
        }

        pub fn spawn(config: &Config, synth: Option<Synth>, text: String, voice: String) -> Run {
            let phase = Rc::new(RefCell::new(Phase::Fetching));
            let shared = phase.clone();
            let play = config.play.clone();
            let synthesis = super::synthesis(synth, config.api_base.clone(), voice, text);
            wasm_bindgen_futures::spawn_local(async move {
                let (audio, marks) = match synthesis.await {
                    Ok(pair) => pair,
                    Err(e) => {
                        log::error!("tts: synthesis failed: {e}");
                        *shared.borrow_mut() = Phase::Ended(task::failure());
                        return;
                    }
                };
                let bytes = js_sys::Uint8Array::from(audio.as_slice());
                let marks_js = match serde_wasm_bindgen::to_value(&marks) {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!("tts: marks to JS failed: {e}");
                        *shared.borrow_mut() = Phase::Ended(task::failure());
                        return;
                    }
                };
                let playhead = play
                    .call2(&JsValue::NULL, &bytes, &marks_js)
                    .ok()
                    .and_then(|v| v.dyn_into::<js_sys::Function>().ok());
                *shared.borrow_mut() = match playhead {
                    Some(playhead) => Phase::Playing {
                        playhead,
                        marks,
                        next: 0,
                        current: SILENCE_VISEME,
                    },
                    None => {
                        log::error!("tts: the playback callback returned no playhead");
                        Phase::Ended(task::failure())
                    }
                };
            });
            Run { phase }
        }

        pub fn poll(run: &mut Run) -> Observed {
            let mut phase = run.phase.borrow_mut();
            match &mut *phase {
                Phase::Fetching => Observed::Running(SILENCE_VISEME),
                Phase::Ended(status) => Observed::Ended(status.clone()),
                Phase::Playing {
                    playhead,
                    marks,
                    next,
                    current,
                } => match playhead.call0(&JsValue::NULL) {
                    Ok(v) if v.is_null() || v.is_undefined() => {
                        Observed::Ended(task::success())
                    }
                    Ok(v) => {
                        let ms = v.as_f64().unwrap_or(0.0).max(0.0) as u64;
                        *current = shape_at(marks, next, ms, current);
                        Observed::Running(current)
                    }
                    Err(e) => {
                        log::error!("tts: playhead threw: {e:?}");
                        Observed::Ended(task::failure())
                    }
                },
            }
        }
    }
}

/// The device composition the browser device would perform for speech: the
/// say and play_viseme fragments on the interpreter, the provider as a host
/// module, the say call routed to it. `spec` is the base graph (any form the
/// spec normalizer accepts).
pub fn compose(
    spec: &str,
    rig_prefix: &str,
    provider: HostModule,
) -> Result<arora::AroraBuilder, String> {
    use vizij_arora_behavior::{parse_spec, speech, viseme, ProcessingGraph};
    use vizij_arora_store::BlackboardStore;
    let spec = parse_spec(spec)?;
    let mut graph = ProcessingGraph::from_spec(spec)?;
    graph.set_function_modules(HashMap::from([(SAY_ID, provider.id())]));
    graph.set_task_fragment(
        viseme::PLAY_VISEME_ID,
        viseme::play_viseme_fragment(rig_prefix),
    );
    graph.set_task_fragment(SAY_ID, speech::say_fragment(rig_prefix));
    Ok(arora::Arora::builder()
        .with_data_store(Box::new(BlackboardStore::new()))
        .with_behavior_interpreter(Box::new(graph))
        .with_host_module(viseme_host_module())
        .with_host_module(provider))
}

/// The viseme module (crates/vizij/src/viseme.rs's shape): the described
/// play_viseme contract; the behavior is the fragment.
pub fn viseme_host_module() -> HostModule {
    use vizij_arora_behavior::viseme::{play_viseme_signature, MODULE_ID, PLAY_VISEME_ID};
    ModuleBuilder::new(MODULE_ID)
        .described_function(
            PLAY_VISEME_ID,
            vizij_arora_host::skills::PLAY_VISEME_FUNCTION,
            play_viseme_signature(),
            |_call| {
                Ok(CallResult {
                    ret: task::failure(),
                    mutated: Vec::new(),
                })
            },
        )
        .build()
}

fn status_only(status: Value) -> CallResult {
    CallResult {
        ret: status,
        mutated: Vec::new(),
    }
}

fn with_viseme(status: Value, viseme: &str) -> CallResult {
    CallResult {
        ret: status,
        mutated: vec![StructureField {
            id: SAY_VISEME_PARAM_ID,
            value: Box::new(Value::String(viseme.to_string())),
        }],
    }
}

fn arg_string(call: &Call, id: Uuid) -> Option<String> {
    match call.args.iter().find(|field| field.id == id) {
        Some(field) => match field.value.as_ref() {
            Value::String(s) => Some(s.clone()),
            _ => None,
        },
        None => None,
    }
}

fn utterance_key(text: &str, voice: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    voice.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(time: u64, value: &str) -> SpeechMark {
        SpeechMark {
            time,
            value: value.to_string(),
        }
    }

    #[test]
    fn the_shape_follows_the_playhead_through_the_marks() {
        let marks = vec![mark(0, "p"), mark(100, "a"), mark(200, "sil")];
        let mut next = 0;
        assert_eq!(shape_at(&marks, &mut next, 0, SILENCE_VISEME), "PP");
        assert_eq!(shape_at(&marks, &mut next, 50, "PP"), "PP");
        assert_eq!(shape_at(&marks, &mut next, 150, "PP"), "aa");
        // A playhead that jumps consumes every passed mark; the last wins.
        assert_eq!(shape_at(&marks, &mut next, 900, "aa"), "sil");
        assert_eq!(next, 3);
    }
}
