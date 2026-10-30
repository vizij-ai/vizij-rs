# Changelog

All notable changes to `vizij-arora-behavior`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [9.0.1] - 2026-10-30

### Changed

- Requires vizij-arora-host 8, the major of the skill contracts on
  arora-behavior 10's `Status` that 9.0.0 already built against.

## [9.0.0] - 2026-10-30

### Changed

- **Breaking:** `ProcessingGraph` is an arora-behavior 10 `BehaviorInterpreter`
  (arora 17 and later): `load` takes and `graph` holds the lean graph model,
  a node holding its own bindings, and `apply` takes its `GraphDiff`
  (`remove`, `set`, `unbind`, `bind`, `root`). `graph_codec` encodes a Vizij
  edge as a binding of the target's input slot, a parameter or an inline
  default as a value bound on its slot, and carries the names a decode needs
  (node id, kind, each slot's name) as value bindings on the node using them,
  so `spec_diff_to_graph_diff` needs no view of the running graph.
- Requires vizij-graph-core 5 and arora-behavior-tree-types 3.

## [8.0.0] - 2026-10-22

### Changed

- `ProcessingGraph` evaluates its graph on arora-behavior-tree.
  vizij-graph-core imports the lowered spec as a behavior tree whose
  detached nodes call arora-dataflow-nodes' node functions, and each tick
  runs the nodes the graph's outputs read whose arguments changed. A graph
  computes what it computed before: the face fixtures' tests pass
  unchanged. A
  spec of a new version (a LOAD, an EDIT, a grafted or pruned run) is
  imported again at the next tick, and every node it keeps keeps its state.
- An `ExternalFunction` node is placed by the description the device's
  function index gives of its function, which `tick` reads: a call to a
  function that returns nothing and writes nothing back is left out, and
  any other is called at every tick, as before: a module's function may
  keep state its arguments do not show (the animation module's
  `step(dt)`). `tick_store_with` takes the index; `tick_store` describes
  no function, so each such node is called at every tick.
- A tick costs less when few of a graph's inputs change, and more when many
  do on graphs of a few thousand nodes or fewer. Measured against the
  write-only evaluation this replaces, a 950-node graph ticks 2.2 times
  faster at rest and 1.6 times slower with every input moving; a
  19,000-node graph ticks 8 and 3.3 times faster. Importing a graph costs
  2 to 3.5 times what lowering it did.
- The graph types are vizij-graph-core 4's, whose node vocabulary is
  arora-dataflow-nodes', and the value types vizij-api-core 3's. `task`
  re-exports `vizij_graph_core::task`, which is arora-dataflow-nodes'.
- `ProcessingGraph` is neither `Send` nor `Sync`, because a behavior tree
  holds its nodes in `Rc`s. A device builds and ticks it on one thread.
  `KeyHolds` stays `Send` and `Sync`, so a client on another thread still
  holds keys through it.
- Requires vizij-graph-core 4 and vizij-api-core 3.

## [7.3.0] - Unreleased

### Changed

- A path-less `output` writes each record's number in the type the store
  declares for its key (`KeyMeta::ty`), so a reader of a key its records
  write reads the key's type: the animation module's `F64` lands on an `F32`
  rig input as an `F32`. Arora checks a declared type on the writes that
  come from outside the device only; the interpreter holds these records to
  it. An integer converts to an integer type exactly, held within the
  type's range; a float rounds to the nearest integer, held within the range,
  and a NaN, which names no integer, writes nothing, so the key keeps its
  value. A key that declares no type takes an `F64` as an `F32`, Vizij's
  number type, which keeps pages that read `.f32` working. Anything but a
  number is written as it is, and a path `output`'s value is not converted.
  The store's meta is read once a tick, for the records alone.

## [7.2.0] - 2026-10-19

### Changed

- `ProcessingGraph`'s LOAD and EDIT lower the graph they leave at once and
  refuse it when vizij-graph-core's `GraphSpec::validate` does, with the
  error naming the node at fault (a `BehaviorError`, which reaches a caller
  of the interpreter module as the call's error). An EDIT is refused whole:
  the running graph, its runs and the lowered spec stay as they were. Such a
  graph used to be installed, then failed every tick, base graph and runs
  together, until a later LOAD or EDIT replaced it. A graph that evaluates
  is accepted as before. Requires vizij-graph-core 3.1.
- Licensed under Apache-2.0, where it was MIT OR Apache-2.0. Releases up
  to 7.1.0 keep the licence they were published under.

## [7.1.0] - 2026-10-18

### Added

- `ProcessingGraph::key_holds`: a `KeyHolds` handle on the store keys the
  graph holds. A held key is left out of what the graph writes to the
  store — its behavior's outputs, its runs', and the unset of a spawned
  run's keys when it ends — so it keeps whatever a client writes to it. The
  graph reads a held key like any other, so what it computes from the key
  follows the client's value. A hold or a release takes effect at the
  graph's next tick: released, a key the graph writes every tick takes its
  value again. A halted run's status is written whatever holds it. Holding
  a key the graph does not write changes nothing.

## [7.0.0] - 2026-10-17

### Changed

- **Breaking:** built on vizij-graph-core 3. The graph types in this crate's
  API — the `GraphSpec` of `parse_spec`, `graph_codec` and `run`, and the
  node and edge specs a `GraphSpecDiff` carries — are vizij-graph-core 3's,
  so they do not unify with those of a crate built on vizij-graph-core 2. A tick with a zero `dt` moves nothing
  that time drives: a device stepped at zero carries a change through its
  graph and its runs, and their springs, dampers, slews, clocks and noise
  hold.

## [6.0.2] - 2026-10-13

### Changed

- The interpreter evaluates its graph for writes
  (`vizij_graph_core::evaluate_writes_with_functions`): it keeps no copy of
  the graph's ports by name, which it never read. A face's device step takes
  30 % fewer instructions.
- The interpreter reads its graph's inputs from the store in one read per
  tick, each input path once, under keys formatted when the graph is
  lowered rather than on every tick.

## [6.0.1] - 2026-10-12

### Fixed

- A run of a function no fragment and no module implements is refused at
  spawn, whoever spawns it: SPAWN answers an error, and a spawn node's request
  starts nothing (logged). Grafted, such a run's node could never call
  anything and failed every evaluation, stopping the whole graph.

## [6.0.0] - 2026-10-12

### Changed

- **Breaking:** built on vizij-arora-host 7. The skill contracts re-exported
  here (`speech::{say, Say}`, `viseme::{play_viseme, PlayViseme}`,
  `gaze::{look_at, LookAt}`) are vizij-arora-host 7's types, so they do not
  unify with those of a crate built on vizij-arora-host 6. The contracts
  themselves are unchanged.

## [5.4.0] - 2026-10-10

### Changed

- The play_viseme fragment serves the contract's `duration` parameter as its
  `task/duration` input. Requires vizij-arora-host 6.5.

## [5.3.0] - 2026-10-10

### Added

- `run::outputs(spec)`: the store keys a `run_behavior` run of `spec` writes
  and leaves holding when halted — each output node's path, once, sorted —
  what a client returns to rest after the halt with the rest module's
  `reset_keys`. The outputs on `task/…` paths, which write the run's own
  keys, are not listed, nor are path-less outputs, whose keyed batch names
  its keys as it runs.

## [5.2.0] - 2026-10-10

### Added

- `ProcessingGraph` serves the runs a graph's `spawn` nodes request: each
  one is grafted once the evaluation that asked for it is done, like a
  SPAWN. Nothing holds its handle, so the run is the graph's own: when it
  ends, or an exclusive spawn halts it, its fragment is pruned and the keys
  it held under `arora/tasks/…` are unset — a graph spawning on every change
  leaves nothing behind in the store. A request that does not graft is
  logged and the step goes on. Requires vizij-graph-core 2.3.

## [5.1.0] - 2026-10-10

### Added

- A graph carries its order in its structure: `root` and each node's ordered
  `children`, over two composites with functions of their own. A `flow` holds
  a dataflow network, a node after every node it reads from and otherwise in
  listed order; a `layers` holds parts of a behavior, one after the other.
  `graph_codec::encode` makes a spec one `flow` of its nodes in listed order,
  and `decode` lists the nodes in a pre-order walk of the structure, then any
  node no composite holds, by id.
- The interpreter's root is a `layers` runner: the main behavior, then each
  live run's `flow` in spawn order. A LOAD replaces the main behavior under
  the runs (a graph with no structure becomes a `flow` of its nodes by id),
  and a node an EDIT adds without placing it joins the end of its run or of
  the main behavior.

### Fixed

- Of two writers of one path, the later-listed wins. Decoding sorted nodes by
  id, so a composed behavior's sources evaluated in the order of their
  prefixes rather than the order they were composed in: on a face device the
  animations came first and lost every path they share with the mappings.

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
