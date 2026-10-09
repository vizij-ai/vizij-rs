# Changelog

All notable changes to `vizij-arora-behavior`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [5.0.1] - 2026-10-09

### Fixed

- A task run writes after the main behavior, and a later run after an
  earlier one: where they write one path, the run's value stands. Lowering
  orders the nodes that way — the behavior's, then each live run's in spawn
  order — where the winner used to vary from process to process.
- Grafting or pruning a run no longer lowers and re-plans the whole graph.
  A run is a component of its own, so its nodes and plan are added after the
  rest, or taken out, in place; anything else still lowers in full. In a
  face device that is about 30 M instructions less per run, roughly a third
  of what a run cost. Requires vizij-graph-core 2.2.

## [5.0.0] - 2026-10-06

### Changed

- **Breaking:** depends on vizij-arora-host 6: the re-exported `Say` contract
  has the mutable `speech` out-parameter, and the say fragment writes it as
  the face's speech state, `standard/vizij/speech`.

## [4.1.0] - 2026-10-05

### Added

- `run::RunBehavior::run_behavior(name, behavior) -> Status`, a task method
  the `ProcessingGraph` implements itself and lists among its
  `described_methods`: spawned, it runs `behavior` — a Vizij graph as the
  interpreter module's LOAD carries one (`run::call` builds the call from a
  spec) — beside the main behavior until halted. The graph grafts as the
  run's fragment; the run writes `Running` on its status key (unless the
  graph writes `task/status` itself) and its `name` on `run::name_key`.
  `run::runs(store)` lists the runs a store holds keys of, by name, with
  their handles (`run::handle`); `run::edit(task, from, to)` is the EDIT
  that changes a running behavior in place; `run::behavior(spec)` is a spec
  as `run_behavior`'s `behavior` argument, for a client calling it by name.

### Fixed

- A LOAD of the main behavior leaves the live task runs in place: their
  nodes carry over, with their state. A run's nodes are the graph's nodes
  under `task/<run id>`, so an EDIT that adds or removes nodes there changes
  the run, and a halt prunes the run's nodes as they stand.

## [4.0.0] - 2026-10-05

### Changed

- **Breaking:** built on vizij-arora-host 5. The skill contracts re-exported
  here (`speech::{say, Say}`, `viseme::{play_viseme, PlayViseme}`,
  `gaze::{look_at, LookAt}`) are vizij-arora-host 5's types, so they do not
  unify with those of a crate built on vizij-arora-host 4.

## [3.0.0] - 2026-09-28

### Changed

- **Breaking:** `speech::say_signature`, `viseme::play_viseme_signature` and
  `gaze::look_at_signature` are gone: the contracts of vizij-arora-host 4
  declare the signatures, re-exported here as `speech::{say, Say}`,
  `viseme::{play_viseme, PlayViseme}` and `gaze::{look_at, LookAt}`. Their
  ids replace the re-exported `SAY_*` ids and `viseme::PLAY_VISEME_ID`.
- **Breaking:** no module implements look_at or play_viseme any more:
  `gaze::module_id()`, `gaze::look_at_id()` and `viseme::MODULE_ID` are gone.
  Their fragments are described, so the interpreter describes the methods
  under the interpreter module (arora 10.3), and a remote spawns them through
  it. The function ids are `look_at::ids::look_at::FUNCTION` and
  `play_viseme::ids::play_viseme::FUNCTION`.
- Depends on arora-behavior 9.1.

### Added

- `TaskFragment::described(export)`: the function a fragment implements,
  named and signed, for a function no module implements. `ProcessingGraph`
  lists the described fragments' functions as its `described_methods`.
- `speech::say_parameters()` is public, like its gaze and viseme
  counterparts.

## [2.0.0] - 2026-09-26

### Changed

- **Breaking:** depends on arora-types 3, arora-behavior 9,
  arora-behavior-tree-types 2, arora-simple-data-store 3, vizij-api-core 2,
  vizij-graph-core 2 and vizij-arora-host 3.

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
