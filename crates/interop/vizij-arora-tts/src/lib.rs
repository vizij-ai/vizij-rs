//! Vizij's text-to-speech as an Arora host module.
//!
//! The **contract** — `say(text, voice) -> Status` with the mutable `viseme`
//! and `speech` out-parameters — is the speech skill's
//! ([`vizij_arora_behavior::speech`], re-exported here): same function id,
//! parameter ids, and signature for every provider, so a behavior references
//! `say` without caring which provider a build registered. This crate ships
//! the **cloud provider** (the Vizij TTS cloud function — AWS Polly behind an
//! HTTP endpoint, no credentials in the app; `API_URL` overrides the
//! deployment). A sibling provider (e.g. the local Piper one) implements the
//! same contract with its own module id.
//!
//! A poll-on-tick action (the arora-sdk `docs/async-functions.md` contract):
//! [`say`] is re-invoked each tick while `Running`; synthesis + playback run
//! off the tick thread, and the closure only polls. The module streams the
//! current viseme as one of the face standard's shapes
//! ([`vizij_arora_host::standard::VISEME_SHAPES`]), Polly's viseme codes
//! mapped by [`polly_shape`]; [`SILENCE_VISEME`] is written at rest. It
//! reports the utterance as `speech` while its audio plays — from the moment
//! playback starts, empty before and after — so the face's speech state
//! follows the voice, not the request. What a shape looks like, and how one
//! blends into the next, is the face's and the speech skill's business, not
//! the provider's.
//!
//! A halt is silence: the interpreter stops re-invoking `say`, and nothing
//! in the module ABI tells the producer so. Every tick refreshes the run's
//! pulse, and a producer that sees no tick for [`IDLE_STOP`] stops the audio
//! and ends the run — the same rule for every producer, so that a halted
//! run goes quiet within that bound whatever plays the audio.

use arora_engine::module::{HostModule, ModuleBuilder};

use std::collections::HashMap;

use uuid::{uuid, Uuid};

pub use vizij_arora_behavior::speech::{
    say_signature, SAY_ID, SAY_SPEECH_PARAM_ID, SAY_TEXT_PARAM_ID, SAY_VISEME_PARAM_ID,
    SAY_VOICE_PARAM_ID, SILENCE_VISEME,
};

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

use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
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
pub const MODULE_ID: Uuid = uuid!("4f6f0b0a-62cb-4a1f-ab0d-08f283485091");

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
    viseme: Arc<Mutex<&'static str>>,
    /// The instant of the last `say` tick — the producer's only sign that the
    /// run is still wanted.
    pulse: Pulse,
    /// Whether the audio is playing — the span the utterance is reported as
    /// `speech`.
    playing: Arc<AtomicBool>,
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
    /// The viseme code (AWS Polly viseme set), mapped to a shape at the playhead.
    value: String,
}

#[derive(Deserialize)]
struct VisemeResponse {
    visemes: Vec<SpeechMark>,
}

/// The tts module: the described `say` action, discoverable over `DescribeMethods`
/// and — via its `Status` return — exposable as a ROS 2 action by a bridge.
pub fn host_module() -> HostModule {
    ModuleBuilder::new(MODULE_ID)
        .described_function(SAY_ID, "say", say_signature(), say)
        .build()
}

/// Speak `text` in `voice`, streaming the current viseme and, while the audio
/// plays, the utterance. Re-invoked each tick while `Running`; keeps its state
/// in [`RUNS`], keyed by content.
pub fn say(call: Call) -> Result<CallResult, CallError> {
    let text = match arg_string(&call, SAY_TEXT_PARAM_ID) {
        Some(text) => text,
        None => return Ok(status_only(task::failure())),
    };
    let voice = arg_string(&call, SAY_VOICE_PARAM_ID).unwrap_or_else(|| DEFAULT_VOICE.to_string());
    let key = utterance_key(&text, &voice);
    let mut runs = match RUNS.lock() {
        Ok(runs) => runs,
        Err(_) => return Ok(status_only(task::failure())),
    };

    // First tick: spawn synthesis + playback off the tick thread. Later ticks
    // find the run and fall through to the poll.
    let run = runs
        .entry(key)
        .or_insert_with(|| spawn_say(text.clone(), voice));
    if let Ok(mut last) = run.pulse.lock() {
        *last = Instant::now();
    }

    // The viseme at the playhead, advanced by the playback task, and the
    // utterance while the audio plays.
    let current = run.viseme.lock().map(|cur| *cur).unwrap_or(SILENCE_VISEME);
    let speech = if run.playing.load(Ordering::Relaxed) {
        text.as_str()
    } else {
        ""
    };

    // Poll the run's `JoinHandle` (a `Future`) once — the tick loop is the
    // executor, a no-op waker suffices, and a terminal result drops the run so
    // the completed handle is never polled again.
    let mut cx = Context::from_waker(Waker::noop());
    match Future::poll(Pin::new(&mut run.handle), &mut cx) {
        Poll::Pending => Ok(streaming(task::running(), current, speech)),
        Poll::Ready(Ok(status)) => {
            runs.remove(&key);
            Ok(streaming(status, SILENCE_VISEME, ""))
        }
        Poll::Ready(Err(_join_error)) => {
            runs.remove(&key);
            Ok(streaming(task::failure(), SILENCE_VISEME, ""))
        }
    }
}

/// Spawn synthesis (the Vizij TTS provider) + playback and return the run. The
/// task fetches audio + a viseme timeline, plays the audio, and advances the
/// shared viseme cell at the playhead. The heavy work never runs on the tick
/// thread; `say` only samples the cell and polls the handle.
fn spawn_say(text: String, voice: String) -> Run {
    let viseme = Arc::new(Mutex::new(SILENCE_VISEME));
    let viseme_task = viseme.clone();
    let pulse: Pulse = Arc::new(Mutex::new(Instant::now()));
    let pulse_task = pulse.clone();
    let playing = Arc::new(AtomicBool::new(false));
    let playing_task = playing.clone();
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
        // A halt during synthesis shows as a quiet pulse: nothing to stop yet,
        // but nothing to play either.
        if is_halted(&pulse_task) {
            return task::failure();
        }
        match tokio::task::spawn_blocking(move || {
            play(audio, marks, viseme_task, pulse_task, playing_task)
        })
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
        playing,
    }
}

/// Play the mp3 whole, holding `playing` up for its duration and advancing
/// the shared viseme cell at the sink's own playhead; a quiet pulse stops the
/// sink.
fn play(
    audio: Vec<u8>,
    marks: Vec<SpeechMark>,
    viseme: Arc<Mutex<&'static str>>,
    pulse: Pulse,
    playing: Arc<AtomicBool>,
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
    let cues: Vec<Cue> = marks
        .iter()
        .map(|mark| Cue {
            time_ms: mark.time,
            shape: polly_shape(&mark.value),
        })
        .collect();
    playing.store(true, Ordering::Relaxed);
    let outcome = follow(&cues, &viseme, &pulse, || {
        (!sink.empty()).then(|| sink.get_pos())
    });
    playing.store(false, Ordering::Relaxed);
    match outcome {
        Playback::Ended => task::success(),
        Playback::Halted => {
            sink.stop();
            task::failure()
        }
    }
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

/// A shape change on the audio timeline, in milliseconds from its start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cue {
    pub time_ms: u64,
    pub shape: &'static str,
}

/// How long a producer keeps playing without a `say` tick before it treats
/// the run as halted and stops the audio.
pub const IDLE_STOP: Duration = Duration::from_millis(250);

/// The last instant a run was ticked, shared between the tick and the
/// producer: the producer's only sign that the run is still wanted, because
/// a halt is the interpreter ceasing to re-invoke `say`.
pub type Pulse = Arc<Mutex<Instant>>;

/// Whether a pulse has gone quiet for longer than [`IDLE_STOP`].
pub fn is_halted(pulse: &Pulse) -> bool {
    pulse
        .lock()
        .map(|last| last.elapsed() > IDLE_STOP)
        .unwrap_or(true)
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

/// A result carrying only the status (no viseme output).
fn status_only(status: Value) -> CallResult {
    CallResult {
        ret: status,
        mutated: Vec::new(),
    }
}

/// A result carrying the status plus what the run streams: the current
/// viseme and the utterance being spoken (empty while no audio plays), in
/// the out-parameters.
fn streaming(status: Value, viseme: &str, speech: &str) -> CallResult {
    CallResult {
        ret: status,
        mutated: vec![
            StructureField {
                id: SAY_VISEME_PARAM_ID,
                value: Box::new(Value::String(viseme.to_string())),
            },
            StructureField {
                id: SAY_SPEECH_PARAM_ID,
                value: Box::new(Value::String(speech.to_string())),
            },
        ],
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

#[cfg(test)]
mod tests {
    use super::*;

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

    /// A run nobody ticks any more is a halted run: the producer stops within
    /// `IDLE_STOP` of the last tick and leaves the lips at rest, however long
    /// the audio still had to play.
    #[test]
    fn a_run_without_ticks_goes_silent_within_the_idle_bound() {
        let cues = vec![cue(0, "aa")];
        let viseme = Mutex::new(SILENCE_VISEME);
        let last_tick = Instant::now();
        let pulse: Pulse = Arc::new(Mutex::new(last_tick));
        let started = Instant::now();
        // An endless playhead: the audio never runs out on its own.
        let outcome = follow(&cues, &viseme, &pulse, || Some(started.elapsed()));
        assert_eq!(outcome, Playback::Halted);
        assert!(last_tick.elapsed() < IDLE_STOP + Duration::from_millis(100));
        assert_eq!(*viseme.lock().unwrap(), SILENCE_VISEME);
    }

    /// Ticks keep a run alive past the idle bound, and the run ends with the
    /// audio.
    #[test]
    fn a_ticked_run_plays_to_the_end() {
        let cues = vec![cue(0, "aa"), cue(300, "sil")];
        let viseme = Mutex::new(SILENCE_VISEME);
        let pulse: Pulse = Arc::new(Mutex::new(Instant::now()));
        let started = Instant::now();
        let audio = Duration::from_millis(400);
        let outcome = follow(&cues, &viseme, &pulse, || {
            *pulse.lock().unwrap() = Instant::now();
            let position = started.elapsed();
            (position < audio).then_some(position)
        });
        assert_eq!(outcome, Playback::Ended);
        assert!(started.elapsed() >= audio);
    }
}
