# Changelog

All notable changes to `vizij-graph-core`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [3.1.1] - 2026-10-19

### Fixed

- `Spring` moves by the exact solution of a damped spring over each step,
  for a target that holds through the step. One long step lands where as
  many short ones would, for any stiffness, damping and mass, so a late
  frame — a page shown again after its timers were throttled, a slow load —
  leaves the spring where that much time would have. It used to integrate
  with semi-implicit Euler, which diverges once a step is longer than a
  bound its parameters set — 0.15 s for the mouth and brow springs of
  Quori's program, 0.23 s for its eye spring: under steps of a second they
  grew to infinity and then NaN, which the spring then kept for good.
- `Spring` keeps a finite state. A component whose target is not finite
  holds its position and velocity, and one whose state is not finite —
  seeded from a target that was not — starts again at rest on its target.
  Its doc no longer calls it critically damped; how it is damped is its
  parameters'.

## [3.1.0] - 2026-10-19

### Added

- `GraphSpec::validate`: refuses a graph that would fail every evaluation,
  before it is installed. The checks are the ones evaluation would fail on
  anyway: an edge to a node the graph lacks, two edges into one input, two
  nodes sharing an id, a cycle, and a node leaving a required input with
  neither an edge nor an inline default. The error names the node, and the
  port for a required input. What a graph needs only at evaluation time,
  such as a value staged on an input path, is not checked.
- `schema::required_inputs(kind)` and `NodeSignature::required_inputs`: the
  input ports a node of `kind` cannot evaluate without — every input of
  `Clamp`, `Remap`, `CenteredRemap` and `PiecewiseRemap`, `UrdfFk`'s
  `joints`, `UrdfIkPosition`'s `target_pos`, and `UrdfIkPose`'s `target_pos`
  and `target_rot`. Every other input reads a default when unwired. A test
  holds the list to the evaluator: each node kind fails without each input
  the list names and evaluates without any other. A registry serialized to
  JSON marks each of these ports `"required": true`.

### Changed

- Licensed under Apache-2.0, where it was MIT OR Apache-2.0. Releases up
  to 3.0.0 keep the licence they were published under.

## [3.0.0] - 2026-10-17

### Changed

- **Breaking:** a step with no time in it moves nothing that time drives. At
  a `dt` of zero, or a negative or non-finite one, `Spring` records its new
  target and keeps its position and velocity, and `Damp` and `Slew` keep
  their value; each moves on from there at the next step with time in it,
  exactly as if the zero step had not been. They used to jump to their
  target and zero a spring's velocity. `Time` and `Oscillator` read the
  unchanged `t`, so an evaluation at `dt = 0` carries a changed input through
  every node that does not depend on time and moves nothing else. A `Damp`
  with no half-life and a `Slew` with no rate limit pass their target
  through at any `dt`, as before. A smoother whose input changes shape
  starts again from the new input, at any `dt`, as before.
- **Breaking:** an IK node (`UrdfIkPosition`, `UrdfIkPose`) answers a request
  equal to its last one — target, seed, weights and solver settings — with
  its last solution rather than solving again. An unseeded solve starts from
  the node's last solution, and solving a redundant chain again from it lands
  on another solution, so the joints moved on every evaluation with nothing
  they depend on changed.

## [2.6.0] - 2026-10-13

### Changed

- The plan resolves what a selector reads from a `constant` when it is
  built: the input's binding holds the selected value, and an evaluation
  neither reads nor copies the constant for it. A constant every read of
  which is resolved is not evaluated by `evaluate_writes_with_functions` at
  all. A face's pose driver reads its neutral-pose record through 94 such
  selectors; its device step takes 3.4 M fewer instructions per frame.
- The `value` of a constant read through a selector is therefore plan
  structure: a new value takes a new plan. `fingerprint_spec` covers it, so
  an unversioned spec (`version` 0) edited in place is planned again; a
  versioned spec takes a new version (`GraphSpec::with_cache`), as for any
  structural edit.

## [2.5.0] - 2026-10-13

### Added

- `evaluate_writes_with_functions`: an evaluation for a host that takes its
  results from `GraphRuntime::writes` and `spawns`. It leaves
  `GraphRuntime::outputs`, the copy of every port by node id and port name,
  empty: that copy clones every value the graph computes a second time each
  evaluation, a third of a face's graph evaluation.

### Changed

- A selector projects its source's port in place: an edge selecting a field
  of a record, an item of an array or an enumeration's payload clones what
  it selects, not the source value and its shape. A face's pose driver,
  whose 94 edges each select one field of a 339-field neutral-pose record,
  evaluates in 45 M instructions per frame instead of 271 M.
- An `input` node formats its path only for an error message, not on every
  evaluation.
- A `constant` keeps its port from one evaluation to the next while its
  `value` is unchanged — neither cloned nor its shape inferred again — unless it
  declares output shapes. A change to `value` in place, or a plan change,
  evaluates it again.

## [2.4.0] - 2026-10-10

### Added

- `Spawn` takes an optional `when` input: a new value starts a run even when
  the arguments are unchanged, and it is not passed to the call. An event's
  time on `when` makes a repeated command a new run.

## [2.3.0] - 2026-10-10

### Added

- `Spawn` node: spawns a task run of its function (`module` and `function`
  params) each time its arguments change. The arguments are the `value`
  param's fields, then the keyed `args` inputs, one per parameter id in
  `record_keys`, which win for a parameter both name; the first evaluation
  only records them, so a graph spawns on a change it observes, never on
  being loaded. The request is queued on `GraphRuntime::spawns` (a
  `SpawnRequest`: module, function, args by parameter id) for the host to
  serve after the evaluation — the graph keeps no handle. `out` says whether
  the evaluation spawned.

## [2.2.0] - 2026-10-09

### Added

- `PlanCache::append_component` and `PlanCache::remove_nodes` change a plan in
  place for a component the rest of the graph neither feeds nor reads — the
  shape of a grafted task run — giving the plan a rebuild would give, or
  refusing (plan untouched) so the caller rebuilds.

### Fixed

- The evaluation order is the graph's alone. `topo_order` seeded its ready
  set from a randomly hashed map, so the order of independent nodes — and
  with it which of two outputs writing one path won — changed from process
  to process and from rebuild to rebuild. Of the orders the edges allow it
  now takes the earliest-listed ready node first: the later-listed writer of
  a path is the last, every time, and a component listed after the rest runs
  after all of it. Two nodes sharing an id are refused.

## [2.1.0] - 2026-09-28

### Changed

- A `TaskRun` node calls with its `value` param's fields, then the fields of
  its `args` input, which win for a parameter both name. The `args` input
  used to replace `value` whole; a graph now states a fixed argument once —
  a mutable argument's initial value — under the live argument bundle.

## [2.0.0] - 2026-09-26

### Changed

- **Breaking:** depends on arora-behavior 9 and vizij-api-core 2.

## [1.4.1] - 2026-09-19

### Fixed

- The crate compiles with `default-features = false`: the `urdfik` module and
  its imports are gated on `urdf_ik` as one unit, so a consumer that wants no
  kinematics (a browser bundle) drops `k` and `urdf-rs` from its build.

## [1.4.0] - 2026-09-10

### Added

- `TaskRun` exposes the call's mutable (out) parameters as keyed variadic
  `mutated` outputs — one slot per parameter id listed in `record_keys`, the
  shape `ReadRecord` already has — and a `done` output, true once the run's
  status is terminal (and latched). A fragment wires the out-parameter it
  routes directly, without knowing the function's signature.
- Integer values count as scalars to the arithmetic nodes.

## [1.3.0] - 2026-07-30

### Changed

- The behavior `Status` ids the `task` module speaks now come from
  `arora-behavior` (where `Status` moved, ARORA-82) instead of
  `arora-behavior-tree-types` — the node graph no longer depends on a
  behavior-*tree* crate for the cross-interpreter run-status contract. Same
  ids, same wire form.

## [1.2.0] - 2026-07-30

### Added

- `TaskRun` gains an optional `args` input: when wired it supplies the call's
  argument bundle (overriding the `value` param), so a live goal update on a
  running task's update key flows into the call each tick — the node-graph
  side of ARORA-84.

## [1.1.0] - 2026-07-29

### Added

- `TaskRun` node: hosts one task run — a self-contained module call (module,
  function, argument bundle as params) invoked each evaluation, emitting the
  run's behavior `Status` and latching once terminal. Grafted per spawned run
  by the interpreter.
- `task` module: the behavior `Status` vocabulary on the value plane
  (constructors, terminality, return coercion), ids from
  `arora-behavior-tree-types`.
- `NodeFunctions::call_module`: module-addressed dispatch with a default that
  falls back to `call`, so registry-style hosts need no wiring.
- `ExternalFunction` node plus the `NodeFunctions` call-bridge seam: a graph
  node can call out to host-provided functions.
- The path-less `Output` node applies a keyed record batch to its keys: an
  `output` without `path` takes `key_field`/`value_field` params (record field
  ids) and writes each record's value under the path its key field names.

### Changed

- Freshened workspace dependencies to current majors.

## [1.0.0] - 2026-07-10

### Breaking

- Built on the unified Value (vizij-api-core 1.0.0 = `arora_types` `Value`): the
  graph evaluates on PODs with `Value` only at node boundaries; node-registry
  defaults and evaluation output are in arora serde form. `List`/`Tuple`/`Array`
  collapse into one `ArrayValue`; enums ride arora's `Enumeration`.

### Added

- Record nodes; noise generator nodes; to/from vector nodes; a new blend-node
  default.

### Changed

- Graph plan cache and warm WASM fast paths for lower evaluation overhead;
  slot-based port layouts; delta staging tightened; defaults included in
  fingerprinting; caching policy documented.

### Fixed

- Transition nodes not receiving `dt` updates; piecewise-remap plateau handling;
  hardened `evaluate_all_cached` plan validation and selector projection errors;
  clippy `into_iter` lint.

## [0.3.0] - 2025-09-28

### Changed

- Cleaned up and extended graph support for IK; added comprehensive blend-node
  test coverage.

## [0.2.0] - 2025-09-23

### Added

- IK nodes and a demo IK node; an FK node; transition nodes; multi-slider node;
  external inputs to the graph with stronger shape selection by nodes; range-node
  params.

### Changed

- Refactored the node-eval monolith and the node schema; extracted the shared
  types and interfaces into a common API (vizij-api-core); typed paths with
  refined shape/value handling; refactor around vector data.

### Fixed

- Oscillator and join node fixes; new-node evaluation fixes.

## [0.1.0] - 2025-09-05

### Added

- Initial release: core node-graph engine for Vizij.
