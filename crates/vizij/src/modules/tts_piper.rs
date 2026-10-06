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

use arora::HostModule;
use arora_behavior::Status;
use uuid::{uuid, Uuid};
use vizij_piper::Synthesizer;

use vizij_arora_tts::{follow, is_halted, say, Cue, Playback, Pulse, Say, SILENCE_VISEME};

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

/// One live utterance.
struct Run {
    /// The synthesis+playback task; its output is the terminal status.
    handle: tokio::task::JoinHandle<Status>,
    /// The shape at the audio playhead, advanced by the task and sampled by
    /// `say` each tick.
    viseme: Arc<Mutex<&'static str>>,
    /// The instant of the last `say` tick; the producer stops on a quiet one.
    pulse: Pulse,
}

/// The Piper tts module: the `say` contract — the same signature the cloud
/// provider describes, discoverable over `DescribeMethods`.
pub fn host_module() -> HostModule {
    HostModule::from_exports(
        MODULE_ID,
        say::exports(Piper {
            runs: HashMap::new(),
            ticks: Pulse::new(),
        }),
    )
}

/// The module's state: the live utterances, keyed by content so concurrent
/// `say`s do not share a slot (the module ABI hands `say` no per-run id —
/// same trade-off as the cloud provider).
struct Piper {
    runs: HashMap<u64, Run>,
    /// Beaten on every `say` tick, whichever run: where the tick interval a
    /// run's halt bound follows is learned.
    ticks: Pulse,
}

impl Say for Piper {
    /// Speak `text`, streaming the phoneme at the playhead. Re-invoked each
    /// tick while `Running`.
    fn say(&mut self, text: String, voice: Option<String>, viseme: &mut String) -> Status {
        if let Some(voice) = voice.filter(|voice| !voice.is_empty()) {
            log::debug!(
                "tts-piper: the voice parameter ({voice}) is ignored — \
                 the Piper voice is chosen at build/run time (PIPER_VOICE)"
            );
        }
        let key = utterance_key(&text);
        self.ticks.beat();

        // First tick: spawn synthesis + playback off the tick thread. Later
        // ticks find the run and fall through to the poll.
        let ticks = &self.ticks;
        let run = self
            .runs
            .entry(key)
            .or_insert_with(|| spawn_say(text, ticks.sharing_interval()));
        run.pulse.beat();

        // The shape at the playhead, advanced by the playback task.
        let current = run.viseme.lock().map(|cur| *cur).unwrap_or(SILENCE_VISEME);

        // Poll the run's `JoinHandle` (a `Future`) once — the tick loop is the
        // executor, a no-op waker suffices, and a terminal result drops the
        // run so the completed handle is never polled again.
        let mut cx = Context::from_waker(Waker::noop());
        let status = match Future::poll(Pin::new(&mut run.handle), &mut cx) {
            Poll::Pending => {
                *viseme = current.to_string();
                return Status::Running;
            }
            Poll::Ready(Ok(status)) => status,
            Poll::Ready(Err(_join_error)) => Status::Failure,
        };
        self.runs.remove(&key);
        *viseme = SILENCE_VISEME.to_string();
        status
    }
}

/// Spawn synthesis (local, blocking inference on the blocking pool) + playback,
/// and return the run. The playback loop advances the shared phoneme cell at
/// the playhead; the tick only samples the cell and polls the handle.
fn spawn_say(text: String, pulse: Pulse) -> Run {
    let viseme = Arc::new(Mutex::new(SILENCE_VISEME));
    let viseme_task = viseme.clone();
    let pulse_task = pulse.clone();
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
            None => return Status::Failure,
        };
        // A halt during synthesis shows as a quiet pulse: nothing to play.
        if is_halted(&pulse_task) {
            return Status::Failure;
        }

        // Playback blocks and rodio's stream is thread-bound, so it runs on
        // the blocking pool; this task just awaits the outcome.
        match tokio::task::spawn_blocking(move || play(synthesis, viseme_task, pulse_task)).await {
            Ok(status) => status,
            Err(_join_error) => Status::Failure,
        }
    });
    Run {
        handle,
        viseme,
        pulse,
    }
}

/// Play the synthesized PCM whole, advancing the shared shape cell at the
/// sink's own playhead; a quiet pulse stops the sink.
fn play(
    synthesis: vizij_piper::Synthesis,
    viseme: Arc<Mutex<&'static str>>,
    pulse: Pulse,
) -> Status {
    let (_stream, handle) = match rodio::OutputStream::try_default() {
        Ok(pair) => pair,
        Err(e) => {
            log::error!("tts-piper: audio output init failed: {e}");
            return Status::Failure;
        }
    };
    let sink = match rodio::Sink::try_new(&handle) {
        Ok(sink) => sink,
        Err(e) => {
            log::error!("tts-piper: audio sink failed: {e}");
            return Status::Failure;
        }
    };
    let rate = synthesis.sample_rate;
    let cues: Vec<Cue> = synthesis
        .events
        .iter()
        .map(|event| Cue {
            time_ms: event.start_samples as u64 * 1000 / rate as u64,
            shape: phoneme_shape(&event.phoneme),
        })
        .collect();
    let source = rodio::buffer::SamplesBuffer::new(1, rate, synthesis.samples);
    sink.append(source);
    let outcome = follow(&cues, &viseme, &pulse, || {
        (!sink.empty()).then(|| sink.get_pos())
    });
    match outcome {
        Playback::Ended => Status::Success,
        Playback::Halted => {
            sink.stop();
            Status::Failure
        }
    }
}

/// Drop the loaded voice (running `piper_free`), so a host that is about to
/// exit tears libpiper down deliberately instead of leaving it to C++
/// static-destruction order (which aborts). The `say` example calls this; the
/// app itself exits through the process teardown and does not yet.
pub fn shutdown() {
    if let Ok(mut guard) = SYNTH.lock() {
        guard.take();
    }
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
