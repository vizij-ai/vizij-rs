//! The Piper TTS provider: `say(text, voice) -> Status`, fully local.
//!
//! Same contract as the cloud provider ([`vizij_arora_tts`]) — a drop-in swap
//! selected by the `tts-piper` build feature. No network and no credentials at
//! run time: the `vizij-piper` build provisions libpiper and a patched default
//! voice, so a plain `cargo build --features tts-piper` is the whole setup.
//!
//! The `viseme` out-parameter carries the face-standard shape at the audio
//! playhead, the espeak-ng **phoneme** there mapped by [`phoneme_shape`];
//! markers and punctuation — BOS `^`, EOS `$`, stress marks, `.`/`,` — are
//! the rest token `sil`.
//!
//! Piper's voice is chosen at build/run time (`PIPER_VOICE`); the `voice`
//! call parameter names Polly voices and is ignored here (logged), keeping
//! call sites portable across providers.

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
use uuid::{uuid, Uuid};
use vizij_graph_core::task;
use vizij_piper::{PhonemeEvent, Synthesizer};

use vizij_arora_tts::{
    say_signature, SAY_ID, SAY_TEXT_PARAM_ID, SAY_VISEME_PARAM_ID, SAY_VOICE_PARAM_ID,
    SILENCE_VISEME,
};

/// The Piper tts module's id on the device — distinct from the cloud module so
/// DescribeMethods shows which provider this build carries.
pub const MODULE_ID: Uuid = uuid!("31ca2243-5719-4862-aa57-c30d27cab62e");

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

/// The loaded voice, created on first use (a model load) and reused. One
/// utterance synthesizes at a time; the lock serializes access.
static SYNTH: LazyLock<Mutex<Option<Synthesizer>>> = LazyLock::new(|| Mutex::new(None));

/// Live utterances, keyed by content so concurrent `say`s do not share a slot
/// (the module ABI hands the closure no per-run id — same trade-off as the
/// cloud provider).
static RUNS: LazyLock<Mutex<HashMap<u64, Run>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// One live utterance.
struct Run {
    /// The synthesis+playback task; its output is the terminal status `Value`.
    handle: tokio::task::JoinHandle<Value>,
    /// The phoneme at the audio playhead, advanced by the task and sampled by
    /// the closure each tick.
    viseme: Arc<Mutex<String>>,
}

/// The Piper tts module: the described `say` action — the same signature the
/// cloud provider describes, discoverable over `DescribeMethods`.
pub fn host_module() -> HostModule {
    ModuleBuilder::new(MODULE_ID)
        .described_function(SAY_ID, "say", say_signature(), say)
        .build()
}

/// Speak `text`, streaming the phoneme at the playhead. Re-invoked each tick
/// while `Running`; keeps its state in [`RUNS`], keyed by content.
pub(crate) fn say(call: Call) -> Result<CallResult, CallError> {
    let text = match arg_string(&call, SAY_TEXT_PARAM_ID) {
        Some(text) => text,
        None => return Ok(status_only(task::failure())),
    };
    if let Some(voice) = arg_string(&call, SAY_VOICE_PARAM_ID) {
        if !voice.is_empty() {
            log::debug!(
                "tts-piper: the voice parameter ({voice}) is ignored — \
                 the Piper voice is chosen at build/run time (PIPER_VOICE)"
            );
        }
    }
    let key = utterance_key(&text);
    let mut runs = match RUNS.lock() {
        Ok(runs) => runs,
        Err(_) => return Ok(status_only(task::failure())),
    };

    // First tick: spawn synthesis + playback off the tick thread. Later ticks
    // find the run and fall through to the poll.
    let run = runs.entry(key).or_insert_with(|| spawn_say(text));

    // The phoneme at the playhead, advanced by the playback task.
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

/// Spawn synthesis (local, blocking inference on the blocking pool) + playback,
/// and return the run. The playback loop advances the shared phoneme cell at
/// the playhead; the tick only samples the cell and polls the handle.
fn spawn_say(text: String) -> Run {
    let viseme = Arc::new(Mutex::new(SILENCE_VISEME.to_string()));
    let viseme_task = viseme.clone();
    let handle = TOKIO_HANDLE.spawn(async move {
        // Synthesize off the async workers: model inference is CPU-bound.
        let synthesis = tokio::task::spawn_blocking(move || {
            let mut guard = SYNTH.lock().ok()?;
            if guard.is_none() {
                match Synthesizer::new_default() {
                    Ok(s) => *guard = Some(s),
                    Err(e) => {
                        log::error!("tts-piper: {e}");
                        return None;
                    }
                }
            }
            let synth = guard.as_mut().expect("just created");
            match synth.synthesize(&text) {
                Ok(s) => Some(s),
                Err(e) => {
                    log::error!("tts-piper: synthesis failed: {e}");
                    None
                }
            }
        })
        .await
        .ok()
        .flatten();
        let synthesis = match synthesis {
            Some(s) => s,
            None => return task::failure(),
        };

        // Playback blocks and rodio's stream is thread-bound, so it runs on
        // the blocking pool; this task just awaits the outcome.
        match tokio::task::spawn_blocking(move || play(synthesis, viseme_task)).await {
            Ok(status) => status,
            Err(_join_error) => task::failure(),
        }
    });
    Run { handle, viseme }
}

/// Play the synthesized PCM whole, advancing the shared phoneme cell at the
/// playhead.
fn play(synthesis: vizij_piper::Synthesis, viseme: Arc<Mutex<String>>) -> Value {
    let (_stream, handle) = match rodio::OutputStream::try_default() {
        Ok(pair) => pair,
        Err(e) => {
            log::error!("tts-piper: audio output init failed: {e}");
            return task::failure();
        }
    };
    let sink = match rodio::Sink::try_new(&handle) {
        Ok(sink) => sink,
        Err(e) => {
            log::error!("tts-piper: audio sink failed: {e}");
            return task::failure();
        }
    };
    let rate = synthesis.sample_rate;
    let events = synthesis.events;
    let source = rodio::buffer::SamplesBuffer::new(1, rate, synthesis.samples);
    let start = Instant::now();
    sink.append(source);
    let mut next = 0usize;
    while !sink.empty() {
        let elapsed_samples = start.elapsed().as_millis() as u64 * rate as u64 / 1000;
        while next < events.len() && (events[next].start_samples as u64) <= elapsed_samples {
            if let Ok(mut cur) = viseme.lock() {
                *cur = cursor_token(&events[next]);
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

/// Drop the loaded voice (running `piper_free`), so a host that is about to
/// exit tears libpiper down deliberately instead of leaving it to C++
/// static-destruction order (which aborts). The `say` example calls this; the
/// app itself exits through the process teardown and does not yet.
#[allow(dead_code)]
pub(crate) fn shutdown() {
    if let Ok(mut guard) = SYNTH.lock() {
        guard.take();
    }
}

/// The out-param token for an event: the phoneme's shape; markers, stress
/// and punctuation pseudo-phonemes are the rest token.
fn cursor_token(event: &PhonemeEvent) -> String {
    phoneme_shape(&event.phoneme).to_string()
}

/// The face-standard shape for an espeak-ng phoneme (IPA symbols, possibly
/// with a length or stress mark, e.g. `oʊ`, `ɜː`, `tʃ`). A mouth-shape
/// approximation by articulation: bilabials to `PP`, labiodentals to `FF`,
/// dentals to `TH`, alveolar stops to `DD`, velars to `kk`, postalveolars to
/// `CH`, sibilants to `SS`, nasals and laterals to `nn`, rhotics to `RR`,
/// then the vowels by openness and rounding. Anything else — markers,
/// punctuation, an unknown symbol — is the rest token.
pub(crate) fn phoneme_shape(phoneme: &str) -> &'static str {
    let phoneme = phoneme.trim_matches(|c: char| matches!(c, 'ˈ' | 'ˌ' | 'ː' | '.' | ' '));
    let mut chars = phoneme.chars();
    let (Some(first), second) = (chars.next(), chars.next()) else {
        return SILENCE_VISEME;
    };
    match (first, second) {
        ('t', Some('ʃ')) | ('d', Some('ʒ')) => "CH",
        ('p' | 'b' | 'm', _) => "PP",
        ('f' | 'v', _) => "FF",
        ('θ' | 'ð', _) => "TH",
        ('t' | 'd' | 'ɾ', _) => "DD",
        ('k' | 'g' | 'ɡ', _) => "kk",
        ('ʃ' | 'ʒ', _) => "CH",
        ('s' | 'z', _) => "SS",
        ('n' | 'ŋ' | 'l' | 'ɫ', _) => "nn",
        ('r' | 'ɹ' | 'ɻ', _) => "RR",
        ('a' | 'ɑ' | 'ʌ' | 'æ' | 'ɐ', _) => "aa",
        ('e' | 'ɛ' | 'ɜ', _) => "E",
        ('i' | 'ɪ' | 'ɨ' | 'j' | 'ə' | 'ɚ' | 'h', _) => "ih",
        ('o' | 'ɔ' | 'ɒ', _) => "oh",
        ('u' | 'ʊ' | 'ʉ' | 'w', _) => "ou",
        _ => SILENCE_VISEME,
    }
}

/// A result carrying only the status (no viseme output).
fn status_only(status: Value) -> CallResult {
    CallResult {
        ret: status,
        mutated: Vec::new(),
    }
}

/// A result carrying the status plus the current token in the out-parameter.
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

/// The run key for an utterance — content-keyed, same trade-off as the cloud
/// provider (no per-run id in the module ABI).
fn utterance_key(text: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::phoneme_shape;

    #[test]
    fn phonemes_map_to_the_standard_shapes() {
        assert_eq!(phoneme_shape("tʃ"), "CH");
        assert_eq!(phoneme_shape("t"), "DD");
        assert_eq!(phoneme_shape("oʊ"), "oh");
        assert_eq!(phoneme_shape("ɜː"), "E");
        assert_eq!(phoneme_shape("ˈaɪ"), "aa");
        assert_eq!(phoneme_shape("^"), "sil");
        assert_eq!(phoneme_shape(""), "sil");
    }
}
