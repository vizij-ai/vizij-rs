# Changelog

All notable changes to `vizij-arora-behavior`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [1.1.0] - 2026-09-10

### Added

- `speech`: the speech skill's contract — `say(text, voice) -> Status` with a
  mutable `viseme` out-parameter (`SAY_ID`, `SAY_TEXT_PARAM_ID`,
  `SAY_VOICE_PARAM_ID`, `SAY_VISEME_PARAM_ID`, `SILENCE_VISEME`,
  `say_signature`) and the `say` fragment, which hosts a provider's call in
  its run and drives the lips from the viseme the provider streams; the
  current `{viseme, intensity}` is the run's feedback. Every text-to-speech
  provider implements this one function under its own module id.
- `viseme`: the `play_viseme(shape, weight)` contract (`MODULE_ID`,
  `PLAY_VISEME_ID`, `play_viseme_signature`, `play_viseme_parameters`) and its
  fragment, which plays one face-standard shape through an envelope and
  succeeds once the lips have settled. Both fragments share one lipsync driver
  whose smoothing state is the face's own weight read back from the store, so
  a run taking over continues each fade from where it was and a run ending
  leaves nothing mid-fade.
- A `TaskFragment` can be `exclusive`: spawning it halts the function's live
  runs first, so one player owns the lips.
- Fragments take the face's rig prefix (`say_fragment(rig_prefix)`,
  `play_viseme_fragment(rig_prefix)`, `TaskFragment::parse_with_rig_prefix`),
  so the standard paths they read and write are the face's own, and a run's
  argument bundle is served on `task/update`.

### Changed

- Built on `vizij-arora-host` 2 (profiles and mappings named apart — see its
  changelog) and `vizij-graph-core` 1.4 (the `taskrun` node's keyed `mutated`
  outputs and `done`).

## [1.0.0] - 2026-07-31

First published release: `ProcessingGraph`, a Vizij node graph driven as an
Arora `BehaviorInterpreter` — loaded whole or edited node-by-node with a
`GraphDiff` through the engine's interpreter module, task runs spawned and
halted as graph structure, `dt` from the runtime's built-in store key — and
the portable `look_at` gaze contract in `gaze` (ids, parameters, signature,
fragment parsing, the embedded override).
