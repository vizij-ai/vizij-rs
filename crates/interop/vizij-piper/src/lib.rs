//! Piper text-to-speech with phoneme alignments, over `libpiper` (FFI).
//!
//! Fully local: no network and no credentials at run time. The build script
//! provisions everything (libpiper, espeak-ng data, a patched default voice)
//! and bakes the paths in, so [`Synthesizer::new_default`] works with zero
//! configuration; `PIPER_VOICE` / `PIPER_VOICE_CONFIG` / `PIPER_ESPEAK_DATA`
//! override the defaults at run time.
//!
//! Alignments are per-phoneme sample counts produced in the same synthesis
//! pass (no second alignment step). Two libpiper quirks found during
//! validation are handled here:
//! - `PIPER_DONE` can arrive with the final chunk still filled — a naive
//!   break-on-DONE loop drops the last sentence;
//! - the chunk's `phonemes` buffer is cumulative across sentences while
//!   ids/alignments are per-sentence — decode past what previous chunks
//!   consumed.

#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]

mod ffi {
    #![allow(dead_code, clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

use std::ffi::CString;

/// One phoneme (or punctuation/marker pseudo-phoneme) with its timing.
///
/// Phonemes are espeak-ng symbols, not any viseme vocabulary — mapping them to
/// mouth shapes is the caller's decision. Markers (`^` BOS, `$` EOS, stress and
/// punctuation) appear in the stream and carry real durations; treat them as
/// the rest pose.
#[derive(Debug, Clone)]
pub struct PhonemeEvent {
    pub phoneme: String,
    /// Start of the phoneme, in samples from the utterance start.
    pub start_samples: i64,
    /// Duration in samples.
    pub dur_samples: i64,
}

/// Synthesized speech: mono float32 PCM plus the aligned phoneme timeline.
pub struct Synthesis {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub events: Vec<PhonemeEvent>,
}

/// A loaded Piper voice.
pub struct Synthesizer {
    raw: *mut ffi::piper_synthesizer,
}

// The synthesizer is used behind a lock by one caller at a time; libpiper has
// no thread affinity (plain onnxruntime + espeak calls), it is just not
// re-entrant — which the exclusive &mut receiver already guarantees.
unsafe impl Send for Synthesizer {}

impl Synthesizer {
    /// The build-provisioned default voice and espeak data, overridable via
    /// `PIPER_VOICE` / `PIPER_VOICE_CONFIG` / `PIPER_ESPEAK_DATA`.
    pub fn new_default() -> Result<Self, String> {
        let voice = std::env::var("PIPER_VOICE")
            .unwrap_or_else(|_| env!("VIZIJ_PIPER_DEFAULT_VOICE").to_string());
        let config = std::env::var("PIPER_VOICE_CONFIG")
            .unwrap_or_else(|_| env!("VIZIJ_PIPER_DEFAULT_VOICE_CONFIG").to_string());
        let espeak = std::env::var("PIPER_ESPEAK_DATA")
            .unwrap_or_else(|_| env!("VIZIJ_PIPER_DEFAULT_ESPEAK_DATA").to_string());
        Self::new(&voice, Some(&config), &espeak)
    }

    pub fn new(model: &str, config: Option<&str>, espeak_data: &str) -> Result<Self, String> {
        let m = CString::new(model).map_err(|e| e.to_string())?;
        let c = match config {
            Some(c) => Some(CString::new(c).map_err(|e| e.to_string())?),
            None => None,
        };
        let e = CString::new(espeak_data).map_err(|e| e.to_string())?;
        let raw = unsafe {
            ffi::piper_create(
                m.as_ptr(),
                c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
                e.as_ptr(),
            )
        };
        if raw.is_null() {
            Err(format!("piper_create failed (model {model})"))
        } else {
            Ok(Self { raw })
        }
    }

    /// Synthesize `text` in one pass: audio plus the aligned phoneme timeline.
    ///
    /// Blocking (model inference) — call it off any latency-sensitive thread.
    pub fn synthesize(&mut self, text: &str) -> Result<Synthesis, String> {
        let t = CString::new(text).map_err(|e| e.to_string())?;
        let opts = unsafe { ffi::piper_default_synthesize_options(self.raw) };
        let rc = unsafe { ffi::piper_synthesize_start(self.raw, t.as_ptr(), &opts) };
        if rc != ffi::PIPER_OK as i32 {
            return Err(format!("piper_synthesize_start failed: {rc}"));
        }

        let mut pcm: Vec<f32> = Vec::new();
        let mut events: Vec<PhonemeEvent> = Vec::new();
        let mut rate: u32 = 0;
        let mut cursor: i64 = 0;
        let mut phon_seen: usize = 0;

        loop {
            let mut chunk: ffi::piper_audio_chunk = unsafe { std::mem::zeroed() };
            let rc = unsafe { ffi::piper_synthesize_next(self.raw, &mut chunk) };
            // PIPER_DONE can carry the final chunk — consume before breaking.
            if rc == ffi::PIPER_DONE as i32 && chunk.num_samples == 0 {
                break;
            }
            if rc != ffi::PIPER_OK as i32 && rc != ffi::PIPER_DONE as i32 {
                return Err(format!("piper_synthesize_next failed: {rc}"));
            }
            let done = rc == ffi::PIPER_DONE as i32;

            rate = chunk.sample_rate as u32;
            // The chunk's pointers die on the next call — copy out immediately.
            let s = unsafe { std::slice::from_raw_parts(chunk.samples, chunk.num_samples) };
            pcm.extend_from_slice(s);

            if chunk.num_alignments > 0 {
                if chunk.num_alignments != chunk.num_phoneme_ids {
                    return Err("alignments and phoneme_ids are not parallel".into());
                }
                let phon_all =
                    unsafe { std::slice::from_raw_parts(chunk.phonemes, chunk.num_phonemes) };
                let aligns =
                    unsafe { std::slice::from_raw_parts(chunk.alignments, chunk.num_alignments) };
                // The phonemes buffer is cumulative — take only the new tail.
                let phon = &phon_all[phon_seen.min(phon_all.len())..];
                phon_seen = phon_all.len();
                decode_groups(phon, aligns, &mut cursor, &mut events)?;
            }

            if done {
                break;
            }
        }
        Ok(Synthesis {
            samples: pcm,
            sample_rate: rate,
            events,
        })
    }
}

impl Drop for Synthesizer {
    fn drop(&mut self) {
        unsafe { ffi::piper_free(self.raw) }
    }
}

/// The documented grouping scheme (piper.h): `phonemes` repeats each codepoint
/// once per corresponding id, groups separated by 0; `alignments` holds one
/// sample count per id, so a phoneme's duration is the SUM of its group.
fn decode_groups(
    phon: &[u32],
    aligns: &[std::os::raw::c_int],
    cursor: &mut i64,
    out: &mut Vec<PhonemeEvent>,
) -> Result<(), String> {
    let mut i = 0usize;
    let mut a = 0usize;
    while i < phon.len() {
        if phon[i] == 0 {
            i += 1;
            continue; // group separator
        }
        let cp = phon[i];
        let mut n = 0usize;
        while i + n < phon.len() && phon[i + n] == cp {
            n += 1;
        }
        if a + n > aligns.len() {
            return Err("alignment array shorter than phoneme groups".into());
        }
        let dur: i64 = aligns[a..a + n].iter().map(|&x| x as i64).sum();
        out.push(PhonemeEvent {
            phoneme: char::from_u32(cp).map(String::from).unwrap_or_default(),
            start_samples: *cursor,
            dur_samples: dur,
        });
        *cursor += dur;
        i += n;
        a += n;
    }
    // Trailing ids past the final phoneme group (if any) still occupy time.
    if a < aligns.len() {
        let dur: i64 = aligns[a..].iter().map(|&x| x as i64).sum();
        *cursor += dur;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end over the build-provisioned artifacts: the Rust ONNX patch
    /// yields alignments, and they reconcile exactly against the audio.
    #[test]
    fn default_voice_aligns_exactly() {
        let mut synth = Synthesizer::new_default().expect("default synthesizer");
        let s = synth.synthesize("The blue fish swam.").expect("synthesize");
        assert!(s.sample_rate > 0);
        assert!(
            !s.events.is_empty(),
            "the patched voice must emit alignments"
        );
        let aligned: i64 = s.events.iter().map(|e| e.dur_samples).sum();
        assert_eq!(aligned, s.samples.len() as i64, "alignments must reconcile");
        // Real phonemes present, not only markers.
        assert!(s
            .events
            .iter()
            .any(|e| e.phoneme == "f" || e.phoneme == "b"));
    }
}
