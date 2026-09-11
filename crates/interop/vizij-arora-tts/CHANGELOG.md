# Changelog

All notable changes to `vizij-arora-tts`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

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
