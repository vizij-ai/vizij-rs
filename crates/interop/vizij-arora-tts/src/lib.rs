//! Vizij's text-to-speech as an Arora host module.
//!
//! The **contract** — `say(text, voice) -> Status` with a mutable `viseme`
//! out-parameter — is the speech skill's
//! ([`vizij_arora_behavior::speech`], re-exported here): same function id,
//! parameter ids, and signature for every provider, so a behavior references
//! `say` without caring which provider a build registered. This crate ships
//! the **cloud provider** (the Vizij TTS cloud function — AWS Polly behind an
//! HTTP endpoint, no credentials in the app; [`Config::api_base`] names the
//! deployment). A sibling provider (e.g. the local Piper one) implements the
//! same contract with its own module id.
//!
//! A poll-on-tick action (the arora-sdk `docs/async-functions.md` contract):
//! `say` is re-invoked each tick while `Running`; synthesis + playback run
//! off the tick, and the closure only polls. The module streams the current
//! viseme as one of the face standard's shapes
//! ([`vizij_arora_host::standard::VISEME_SHAPES`]), Polly's viseme codes
//! mapped by [`polly_shape`]; [`SILENCE_VISEME`] is written at rest. What a
//! shape looks like, and how one blends into the next, is the face's and the
//! speech skill's business, not the provider's.
//!
//! Synthesis ([`synthesize`]) is the same on every target — reqwest is
//! `fetch` on wasm32 and hyper natively. What differs per target is the
//! **producer** the tick samples ([`platform`]): natively a tokio task fetches
//! and a blocking-pool task plays through rodio, the playhead the sink's own;
//! in the browser a `spawn_local` future fetches and hands the page's
//! playback hook the bytes and the marks, keeping the `playhead()` it
//! returns. The page owns the `AudioContext` and its user-gesture unlock.
//!
//! A halt is silence: the interpreter stops re-invoking `say`, and nothing
//! in the module ABI tells the producer so. Every tick refreshes the run's
//! pulse, and a producer that sees no tick for the halt bound stops the
//! audio and ends the run — natively in the playback loop, in the browser
//! in the page's player (the `playhead()` poll is its pulse) — so that a
//! halted run goes quiet within that bound whatever plays the audio. The
//! bound is [`IDLE_STOP`], or [`HALT_TICKS`] of the run's own tick interval
//! when the ticks come slower than that ([`Pulse`] keeps the interval): a
//! page rendering at a few frames a second ticks hundreds of milliseconds
//! apart, and a gap of one tick is not a halt. A run gone quiet is dropped
//! from the module's map at the next `say`.

use arora_engine::module::{HostModule, ModuleBuilder};

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arora_types::call::{Call, CallError, CallResult};
use arora_types::value::{StructureField, Value};
use serde::{Deserialize, Serialize};
use uuid::{uuid, Uuid};
use vizij_graph_core::task;

pub use vizij_arora_behavior::speech::{
    say_signature, SAY_ID, SAY_TEXT_PARAM_ID, SAY_VISEME_PARAM_ID, SAY_VOICE_PARAM_ID,
    SILENCE_VISEME,
};

/// The default provider: the Vizij TTS cloud function (AWS Polly behind an
/// HTTP endpoint, no credentials in the app).
pub const DEFAULT_API_BASE: &str = "https://us-central1-semio-vizij.cloudfunctions.net/api";
const DEFAULT_VOICE: &str = "Ruth";

/// The tts module's id on the device.
pub const MODULE_ID: Uuid = uuid!("4f6f0b0a-62cb-4a1f-ab0d-08f283485091");

/// How long a producer keeps playing without a `say` tick before it treats
/// the run as halted and stops the audio — when the ticks come at least
/// this often; slower ticks widen the bound to [`HALT_TICKS`] of their
/// interval.
pub const IDLE_STOP: Duration = Duration::from_millis(250);

/// How many of the run's own tick intervals without a tick mean a halt,
/// where that is longer than [`IDLE_STOP`].
pub const HALT_TICKS: u32 = 4;

/// The face-standard shape for a Polly viseme code (the AWS Polly viseme
/// set), `sil` for silence and for a code this table does not know (logged).
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

/// The provider's configuration.
pub struct Config {
    /// The deployment of the Vizij TTS cloud function, [`DEFAULT_API_BASE`]
    /// unless a build points elsewhere (`API_URL` in the desktop app).
    pub api_base: String,
    /// The page's playback hook, `play(audioBytes: Uint8Array, marks:
    /// {time, value}[]) -> playhead: () => number | null`: milliseconds since
    /// the audio started (0 while decoding or while the `AudioContext` waits
    /// for its unlock), `null` once playback ended or was stopped; a throw
    /// fails the run. The hook is polled once per tick while the run plays
    /// — its pulse: a player that is not polled for [`IDLE_STOP`] stops the
    /// audio, which is how a halted run goes quiet in the browser.
    #[cfg(target_arch = "wasm32")]
    pub play: js_sys::Function,
}

impl Config {
    /// The cloud provider at its default deployment.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn cloud() -> Self {
        Self {
            api_base: DEFAULT_API_BASE.to_string(),
        }
    }
}

/// The tts module: the described `say` action, discoverable over
/// `DescribeMethods` and — via its `Status` return — exposable as a ROS 2
/// action by a bridge.
pub fn host_module(config: Config) -> HostModule {
    host_module_with_synth(config, None)
}

/// A synthesis producer: what a run awaits for `(audio, marks)`. The default
/// is [`synthesize`] against the configured base; a test scripts one. Only
/// the browser producer can await a scripted one: the native producer runs
/// on the tokio runtime, which needs `Send`.
pub type Synth = std::rc::Rc<
    dyn Fn(
        String,
        String,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(Vec<u8>, Vec<SpeechMark>), String>>>,
    >,
>;

/// The module with a scripted synthesizer (`Some`, tests) or the default.
pub fn host_module_with_synth(config: Config, synth: Option<Synth>) -> HostModule {
    let mut provider = Provider {
        config,
        synth,
        runs: HashMap::new(),
    };
    ModuleBuilder::new(MODULE_ID)
        .described_function(SAY_ID, "say", say_signature(), move |call| {
            provider.say(call)
        })
        .build()
}

/// The module's state: its configuration and the live runs, keyed by content
/// because the ABI hands the closure no per-run id (distinct `(text, voice)`
/// never collide; the same one twice shares a run).
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
    /// Speak `text` in `voice`, streaming the current viseme. Re-invoked each
    /// tick while `Running`.
    fn say(&mut self, call: Call) -> Result<CallResult, CallError> {
        // A run nobody ticks any more was halted; its producer has stopped or
        // is stopping on its own, and its slot goes.
        self.runs.retain(|_, run| !is_halted(platform::pulse(run)));

        let Some(text) = arg_string(&call, SAY_TEXT_PARAM_ID) else {
            return Ok(status_only(task::failure()));
        };
        let voice =
            arg_string(&call, SAY_VOICE_PARAM_ID).unwrap_or_else(|| DEFAULT_VOICE.to_string());
        let key = utterance_key(&text, &voice);
        let synth = self.synth.clone();
        let config = &self.config;
        // First tick: spawn synthesis + playback off the tick. Later ticks
        // find the run, refresh its pulse and fall through to the poll.
        let run = self
            .runs
            .entry(key)
            .or_insert_with(|| platform::spawn(config, synth, text, voice));
        platform::pulse(run).beat();
        match platform::poll(run) {
            Observed::Running(viseme) => Ok(with_viseme(task::running(), viseme)),
            Observed::Ended(status) => {
                self.runs.remove(&key);
                Ok(with_viseme(status, SILENCE_VISEME))
            }
        }
    }
}

/// The last instant a run was ticked, shared between the tick and the
/// producer: the producer's only sign that the run is still wanted. It also
/// keeps the interval the ticks come at, so the halt bound follows a slow
/// ticker.
#[derive(Clone)]
pub struct Pulse(Arc<Mutex<Beats>>);

struct Beats {
    last: f64,
    /// The tick interval in milliseconds: the widest recent gap, forgetting
    /// a one-off hiccup over the ticks that follow.
    interval: f64,
}

impl Default for Pulse {
    fn default() -> Self {
        Self::new()
    }
}

impl Pulse {
    /// A pulse beating now.
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Beats {
            last: now_ms(),
            interval: 0.0,
        })))
    }

    /// Record a tick.
    pub fn beat(&self) {
        if let Ok(mut beats) = self.0.lock() {
            let now = now_ms();
            let gap = (now - beats.last).max(0.0);
            beats.interval = if gap > beats.interval {
                gap
            } else {
                0.9 * beats.interval + 0.1 * gap
            };
            beats.last = now;
        }
    }

    /// Time since the last tick.
    pub fn since(&self) -> Duration {
        self.0
            .lock()
            .map(|beats| Duration::from_secs_f64(((now_ms() - beats.last) / 1000.0).max(0.0)))
            .unwrap_or(Duration::MAX)
    }

    /// How long without a tick means a halt: [`IDLE_STOP`], or
    /// [`HALT_TICKS`] intervals of the ticks seen so far when they come
    /// slower than that.
    pub fn halt_bound(&self) -> Duration {
        let interval = self.0.lock().map(|beats| beats.interval).unwrap_or(0.0);
        IDLE_STOP.max(Duration::from_secs_f64(
            f64::from(HALT_TICKS) * interval / 1000.0,
        ))
    }
}

/// Whether a pulse has gone quiet for longer than its halt bound.
pub fn is_halted(pulse: &Pulse) -> bool {
    pulse.since() > pulse.halt_bound()
}

/// A monotonic-enough clock in milliseconds: `Instant` natively, the page's
/// clock in the browser (`Instant` is unavailable on `wasm32-unknown-unknown`).
#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    use std::sync::LazyLock;
    use std::time::Instant;
    static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);
    EPOCH.elapsed().as_secs_f64() * 1000.0
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    js_sys::Date::now()
}

/// The request body both TTS endpoints take.
#[derive(Serialize)]
struct TtsRequest<'a> {
    voice: &'a str,
    text: &'a str,
}

/// One AWS Polly speech mark; the endpoint returns them already filtered to
/// `type == "viseme"`, and the other fields are ignored.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SpeechMark {
    /// Milliseconds into the audio.
    pub time: u64,
    /// The viseme code (AWS Polly viseme set), mapped to a shape at the playhead.
    pub value: String,
}

#[derive(Deserialize)]
struct VisemeResponse {
    visemes: Vec<SpeechMark>,
}

/// Fetch audio (mp3) + the viseme timeline from the TTS provider — the same
/// two endpoints the web demo's `fetchVisemeData` uses. The same on every
/// target: reqwest selects `fetch` on wasm32.
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

/// A shape change on the audio timeline, in milliseconds from its start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cue {
    pub time_ms: u64,
    pub shape: &'static str,
}

/// The marks as cues, their codes mapped to shapes.
pub fn cues(marks: &[SpeechMark]) -> Vec<Cue> {
    marks
        .iter()
        .map(|mark| Cue {
            time_ms: mark.time,
            shape: polly_shape(&mark.value),
        })
        .collect()
}

/// The shape at `playhead_ms`: every cue at or before the playhead is
/// consumed, and the last one consumed is the current shape.
pub fn shape_at(
    cues: &[Cue],
    next: &mut usize,
    playhead_ms: u64,
    current: &'static str,
) -> &'static str {
    let mut shape = current;
    while *next < cues.len() && cues[*next].time_ms <= playhead_ms {
        shape = cues[*next].shape;
        *next += 1;
    }
    shape
}

/// How a followed playback ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Playback {
    /// The audio ran out.
    Ended,
    /// The pulse went quiet while audio was still queued; the caller stops it.
    Halted,
}

/// Drive the viseme cell from a playhead until the audio ends (`None`) or the
/// pulse goes quiet, sampling every 15 ms; the cell is at rest on return.
/// Every native provider plays through this loop, so the halt bound is the
/// same whatever synthesizes the audio.
#[cfg(not(target_arch = "wasm32"))]
pub fn follow(
    cues: &[Cue],
    viseme: &Mutex<&'static str>,
    pulse: &Pulse,
    mut playhead: impl FnMut() -> Option<Duration>,
) -> Playback {
    let mut next = 0usize;
    let mut current = SILENCE_VISEME;
    let outcome = loop {
        if is_halted(pulse) {
            break Playback::Halted;
        }
        let Some(position) = playhead() else {
            break Playback::Ended;
        };
        current = shape_at(cues, &mut next, position.as_millis() as u64, current);
        if let Ok(mut cell) = viseme.lock() {
            *cell = current;
        }
        std::thread::sleep(Duration::from_millis(15));
    };
    if let Ok(mut cell) = viseme.lock() {
        *cell = SILENCE_VISEME;
    }
    outcome
}

/// The producer per target: `Run`, `spawn`, `poll`, `pulse`.
pub mod platform {
    #[cfg(not(target_arch = "wasm32"))]
    pub use self::native::*;
    #[cfg(target_arch = "wasm32")]
    pub use self::web::*;

    /// Native: a tokio task fetches, a blocking-pool task plays through rodio;
    /// the tick polls the `JoinHandle` with a no-op waker and samples the
    /// shared viseme cell. The playhead is the sink's own position.
    #[cfg(not(target_arch = "wasm32"))]
    mod native {
        use super::super::{
            cues, follow, is_halted, synthesize, Config, Observed, Playback, Pulse, SpeechMark,
            Synth, SILENCE_VISEME,
        };
        use arora_types::value::Value;
        use std::future::Future;
        use std::pin::Pin;
        use std::sync::{Arc, LazyLock, Mutex};
        use std::task::{Context, Poll, Waker};
        use vizij_graph_core::task;

        /// A handle for spawning: the ambient runtime if one is active,
        /// otherwise a dedicated one. Only a `Handle` is needed.
        static TOKIO_HANDLE: LazyLock<tokio::runtime::Handle> = LazyLock::new(|| {
            tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
                Box::leak(Box::new(
                    tokio::runtime::Runtime::new().expect("a tokio runtime"),
                ))
                .handle()
                .clone()
            })
        });

        /// One live utterance.
        pub struct Run {
            /// The synthesis+playback task; its output is the terminal status.
            handle: tokio::task::JoinHandle<Value>,
            /// The shape at the audio playhead, advanced by the task and
            /// sampled by the tick.
            viseme: Arc<Mutex<&'static str>>,
            pulse: Pulse,
        }

        pub fn pulse(run: &Run) -> &Pulse {
            &run.pulse
        }

        pub fn spawn(config: &Config, synth: Option<Synth>, text: String, voice: String) -> Run {
            let viseme = Arc::new(Mutex::new(SILENCE_VISEME));
            let cell = viseme.clone();
            let pulse = Pulse::new();
            let pulse_task = pulse.clone();
            // A scripted `Synth` is `Rc` (single-threaded, the browser's shape)
            // and cannot cross onto the runtime; natively the fetch produces.
            if synth.is_some() {
                log::warn!("tts: a scripted synthesizer is not exercised natively");
            }
            drop(synth);
            let base = config.api_base.clone();
            let handle = TOKIO_HANDLE.spawn(async move {
                let (audio, marks) = match synthesize(&base, &voice, &text).await {
                    Ok(pair) => pair,
                    Err(e) => {
                        log::error!("tts: synthesis failed: {e}");
                        return task::failure();
                    }
                };
                // A halt during synthesis shows as a quiet pulse: nothing to
                // stop yet, but nothing to play either.
                if is_halted(&pulse_task) {
                    return task::failure();
                }
                // Playback blocks and rodio's stream is thread-bound, so it
                // runs on the blocking pool; this task just awaits the outcome.
                match tokio::task::spawn_blocking(move || play(audio, marks, cell, pulse_task))
                    .await
                {
                    Ok(status) => status,
                    Err(_join_error) => task::failure(),
                }
            });
            Run {
                handle,
                viseme,
                pulse,
            }
        }

        /// Poll the run's `JoinHandle` (a `Future`) once — the tick loop is
        /// the executor, a no-op waker suffices.
        pub fn poll(run: &mut Run) -> Observed {
            let current = run.viseme.lock().map(|c| *c).unwrap_or(SILENCE_VISEME);
            let mut cx = Context::from_waker(Waker::noop());
            match Future::poll(Pin::new(&mut run.handle), &mut cx) {
                Poll::Pending => Observed::Running(current),
                Poll::Ready(Ok(status)) => Observed::Ended(status),
                Poll::Ready(Err(_join_error)) => Observed::Ended(task::failure()),
            }
        }

        /// Play the mp3 whole, advancing the shared viseme cell at the sink's
        /// own playhead; a quiet pulse stops the sink.
        fn play(
            audio: Vec<u8>,
            marks: Vec<SpeechMark>,
            viseme: Arc<Mutex<&'static str>>,
            pulse: Pulse,
        ) -> Value {
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
            let cues = cues(&marks);
            let outcome = follow(&cues, &viseme, &pulse, || {
                (!sink.empty()).then(|| sink.get_pos())
            });
            match outcome {
                Playback::Ended => task::success(),
                Playback::Halted => {
                    sink.stop();
                    task::failure()
                }
            }
        }
    }

    /// The browser: a `spawn_local` future fetches (the scripted synthesizer
    /// or the default), then hands the page's playback hook the bytes and the
    /// marks and keeps the `playhead()` it returns; the tick calls it and
    /// maps the marks to the current shape.
    #[cfg(target_arch = "wasm32")]
    mod web {
        use super::super::{
            cues, shape_at, synthesize, Config, Cue, Observed, Pulse, Synth, SILENCE_VISEME,
        };
        use arora_types::value::Value;
        use std::cell::RefCell;
        use std::rc::Rc;
        use vizij_graph_core::task;
        use wasm_bindgen::{JsCast, JsValue};

        enum Phase {
            Fetching,
            Playing {
                playhead: js_sys::Function,
                cues: Vec<Cue>,
                next: usize,
                current: &'static str,
            },
            Ended(Value),
        }

        /// One live utterance.
        pub struct Run {
            phase: Rc<RefCell<Phase>>,
            pulse: Pulse,
        }

        pub fn pulse(run: &Run) -> &Pulse {
            &run.pulse
        }

        pub fn spawn(config: &Config, synth: Option<Synth>, text: String, voice: String) -> Run {
            let phase = Rc::new(RefCell::new(Phase::Fetching));
            let shared = phase.clone();
            let play = config.play.clone();
            let base = config.api_base.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let synthesis = match synth {
                    Some(synth) => synth(text, voice).await,
                    None => synthesize(&base, &voice, &text).await,
                };
                let (audio, marks) = match synthesis {
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
                        cues: cues(&marks),
                        next: 0,
                        current: SILENCE_VISEME,
                    },
                    None => {
                        log::error!("tts: the playback hook returned no playhead function");
                        Phase::Ended(task::failure())
                    }
                };
            });
            Run {
                phase,
                pulse: Pulse::new(),
            }
        }

        pub fn poll(run: &mut Run) -> Observed {
            let mut phase = run.phase.borrow_mut();
            match &mut *phase {
                Phase::Fetching => Observed::Running(SILENCE_VISEME),
                Phase::Ended(status) => Observed::Ended(status.clone()),
                Phase::Playing {
                    playhead,
                    cues,
                    next,
                    current,
                } => match playhead.call0(&JsValue::NULL) {
                    Ok(v) if v.is_null() || v.is_undefined() => Observed::Ended(task::success()),
                    Ok(v) => {
                        let ms = v.as_f64().unwrap_or(0.0).max(0.0) as u64;
                        *current = shape_at(cues, next, ms, current);
                        Observed::Running(current)
                    }
                    Err(e) => {
                        log::error!("tts: the playhead threw: {e:?}");
                        Observed::Ended(task::failure())
                    }
                },
            }
        }
    }
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
            id: SAY_VISEME_PARAM_ID,
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::time::Instant;

    fn cue(time_ms: u64, shape: &'static str) -> Cue {
        Cue { time_ms, shape }
    }

    #[test]
    fn the_shape_follows_the_playhead_through_the_cues() {
        let cues = vec![cue(0, "PP"), cue(100, "aa"), cue(200, "sil")];
        let mut next = 0;
        assert_eq!(shape_at(&cues, &mut next, 0, SILENCE_VISEME), "PP");
        assert_eq!(shape_at(&cues, &mut next, 50, "PP"), "PP");
        assert_eq!(shape_at(&cues, &mut next, 150, "PP"), "aa");
        // A playhead that jumps consumes every passed cue; the last wins.
        assert_eq!(shape_at(&cues, &mut next, 900, "aa"), "sil");
        assert_eq!(next, 3);
    }

    #[test]
    fn polly_codes_map_to_the_standard_shapes() {
        let marks = vec![
            SpeechMark {
                time: 0,
                value: "p".into(),
            },
            SpeechMark {
                time: 10,
                value: "sil".into(),
            },
        ];
        assert_eq!(cues(&marks), vec![cue(0, "PP"), cue(10, "sil")]);
    }

    /// A run nobody ticks any more is a halted run: the producer stops within
    /// `IDLE_STOP` of the last tick and leaves the lips at rest, however long
    /// the audio still had to play.
    #[test]
    fn a_run_without_ticks_goes_silent_within_the_idle_bound() {
        let cues = vec![cue(0, "aa")];
        let viseme = Mutex::new(SILENCE_VISEME);
        let last_tick = Instant::now();
        let pulse = Pulse::new();
        let started = Instant::now();
        // An endless playhead: the audio never runs out on its own.
        let outcome = follow(&cues, &viseme, &pulse, || Some(started.elapsed()));
        assert_eq!(outcome, Playback::Halted);
        assert!(last_tick.elapsed() < IDLE_STOP + Duration::from_millis(100));
        assert_eq!(*viseme.lock().unwrap(), SILENCE_VISEME);
    }

    /// A slow ticker widens the halt bound: ticks 100 ms apart keep the
    /// bound at `IDLE_STOP`, ticks 200 ms apart raise it to four intervals.
    #[test]
    fn the_halt_bound_follows_a_slow_ticker() {
        let pulse = Pulse::new();
        assert_eq!(pulse.halt_bound(), IDLE_STOP);
        std::thread::sleep(Duration::from_millis(100));
        pulse.beat();
        assert!(pulse.halt_bound() <= IDLE_STOP + Duration::from_millis(200));
        std::thread::sleep(Duration::from_millis(200));
        pulse.beat();
        let bound = pulse.halt_bound();
        assert!(bound >= Duration::from_millis(800), "{bound:?}");
        assert!(!is_halted(&pulse));
        std::thread::sleep(Duration::from_millis(400));
        assert!(!is_halted(&pulse), "a gap of two intervals is not a halt");
    }

    /// Ticks keep a run alive past the idle bound, and the run ends with the
    /// audio.
    #[test]
    fn a_ticked_run_plays_to_the_end() {
        let cues = vec![cue(0, "aa"), cue(300, "sil")];
        let viseme = Mutex::new(SILENCE_VISEME);
        let pulse = Pulse::new();
        let started = Instant::now();
        let audio = Duration::from_millis(400);
        let outcome = follow(&cues, &viseme, &pulse, || {
            pulse.beat();
            let position = started.elapsed();
            (position < audio).then_some(position)
        });
        assert_eq!(outcome, Playback::Ended);
        assert!(started.elapsed() >= audio);
    }
}
