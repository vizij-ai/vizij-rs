# Changelog

All notable changes to `vizij-arora-tts`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [3.0.0] - 2026-09-19

### Breaking

- `host_module` takes a `Config`: the deployment (`api_base`, no more
  `API_URL` read inside the crate) and, in the browser, the page's playback
  hook. The module keeps its runs in the closure, not in a process-wide map.
- `follow`, `Pulse` and the playback pieces changed shape: `Pulse` is a value
  (`new`, `beat`, `since`, `halt_bound`), `cues` maps marks to `Cue`s,
  `SpeechMark` is public and serializable.
- The halt bound follows a slow ticker: `IDLE_STOP`, or `HALT_TICKS` (4) of
  the run's own tick interval when the ticks come slower than that — a page
  at a few frames a second ticks hundreds of milliseconds apart, and one
  missed tick is not a halt. The page's player keeps the same rule.

### Added

- The browser producer (`wasm32`): a `spawn_local` future fetches and hands
  the page's `play(audioBytes, marks) -> playhead()` hook the audio and the
  marks; the tick polls the playhead and maps the marks to the shape. The
  page's player enforces the halt bound: a playhead not polled for
  `IDLE_STOP` stops the audio. `synthesize` is public and the same on every
  target.
- `host_module_with_synth` for tests: a scripted synthesizer the browser
  producer awaits.
- The wasm-bindgen tests (`tests/browser.rs`, `wasm-pack test --headless
  --chrome`): the visemes at the page's playhead, a failing playback.

## [2.1.0] - 2026-09-19

### Changed

- A halt is silence: the interpreter stops re-invoking `say`, nothing in the
  module ABI tells the producer, so every tick refreshes the run's pulse and
  a producer that sees no tick for `IDLE_STOP` (250 ms) stops the audio and
  ends the run. Audio still queued at that point is cut, not played out.
- The viseme follows the sink's own playhead (`Sink::get_pos()`) rather than
  wall-clock time since `append`, so a late audio start no longer shifts the
  lips ahead of the sound.

### Added

- The playback loop every native provider shares, public so a sibling
  provider (the Piper one in `vizij`) plays under the same halt rule: `Cue`,
  `Pulse`, `IDLE_STOP`, `is_halted`, `follow`, `Playback` and `shape_at`.

## [2.0.0] - 2026-09-10

### Breaking

- The contract is the speech skill's, re-exported from
  `vizij_arora_behavior::speech`: `SAY_ID`, `SAY_TEXT_PARAM_ID`,
  `SAY_VOICE_PARAM_ID`, `SAY_VISEME_PARAM_ID` and `SILENCE_VISEME` are
  constants where `say_id()`, `text_param_id()`, `voice_param_id()` and
  `viseme_param_id()` were functions, and the cloud provider's `module_id()`
  is the constant `MODULE_ID`. Same function id, parameter ids and signature
  for every provider, each under its own module id, so a behavior references
  `say` without caring which provider a build registered.
- The streamed viseme is a face-standard shape — one of
  `vizij_arora_host::standard::VISEME_SHAPES` — not a Polly code:
  `polly_shape` maps the AWS Polly set onto it, `sil` for silence and for a
  code the table does not know. What a shape looks like, and how one blends
  into the next, is the face's and the speech skill's business, not the
  provider's.

## [1.0.0] - 2026-07-31

First published release: Vizij's text-to-speech as an Arora host module —
`say(text, voice) -> Status` over the Vizij TTS cloud function (AWS Polly
behind an HTTP endpoint, no credentials in the app, `API_URL` overriding the
deployment), a poll-on-tick action that runs synthesis and playback off the
tick thread and streams the current viseme through the mutable `viseme`
out-parameter while the audio plays.
