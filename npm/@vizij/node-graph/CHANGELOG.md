# @vizij/node-graph

## 0.13.0

### Minor Changes

- 61d41d6: A graph runs on arora-behavior-tree. `WasmGraph` imports its spec as a behavior tree whose nodes call the node functions, and `evalAll` still evaluates every node and reports every port, each computing what it did. A node calls its function again only when its inputs, its parameters or the clock it reads changed. `setParam` sets a parameter's value in place. An edit that changes a node's ports (a split's sizes, a record's keys), an input's or output's path, or a parameter the node did not set, loads the whole graph again, as `loadGraph` does, and every node keeps its state. Measured natively, a frame of a graph whose inputs hold costs 1.6 to 5 times less. A frame where every input moves costs up to 1.5 times more below a few thousand nodes, and 2 to 2.6 times less at 19,000. Loading a graph costs 2 to 10 times more.

## 0.12.2

### Patch Changes

- 32eddfa: A `Spring` node moves by the exact solution of a damped spring over each step, so one long step lands where as many short ones would, whatever its stiffness, damping and mass. A face whose page was hidden — its steps throttled to one a second, then one a minute — shows its springs where that much time leaves them when it shows again. Before, a step longer than a bound its parameters set (0.15 s for the mouth and brow springs of Quori's program, 0.23 s for its eye spring) made a spring diverge: under steps of a second Quori's springs grew to infinity and then NaN, and its mouth froze at its clamps' floors until the face was reloaded. A spring's state also stays finite: at a target that is not a number it holds, and a state seeded from one starts again at rest on the next target.

## 0.12.1

### Patch Changes

- 0528ebe: Licensed under Apache-2.0, where the package said MIT. The animation module's header (`@vizij/animation-module`'s `artifact/header.json`) states `"license": "Apache-2.0"` too, where it said `"Proprietary"`. Releases already published keep the licence they were published under.
- 9b51cc2: `loadGraph` and `applyGraphEdits` refuse a graph that would fail every evaluation, and the promise rejects with an error naming the node at fault. The graph is refused if it has a node that leaves a required input with neither an edge nor an inline default (a `remap` with no `in`, say), an edge to a node it lacks, two edges into one input, two nodes sharing an id, or a cycle. A refused edit is refused whole: the running graph, its runs and their state go on as they were. Such a graph used to be accepted, then failed the device's every behavior tick, base graph and every run with it, until another load or edit replaced it. A graph that evaluates is accepted as before, including one whose input path has no value staged yet.

  In `@vizij/node-graph`'s registry, an input port a node cannot evaluate without carries `required: true`: the inputs of `clamp`, `remap`, `centeredremap` and `piecewiseremap`, `urdffk`'s `joints`, `urdfikposition`'s `target_pos`, and `urdfikpose`'s `target_pos` and `target_rot`. A port that is neither `optional` nor `required` reads a default (most often zero) when unwired.

- Updated dependencies [0528ebe]
  - @vizij/value-json@0.2.1
  - @vizij/wasm-loader@0.1.6

## 0.12.0

### Minor Changes

- c54800c: A step with no time in it moves nothing that time drives. In `@vizij/runtime`, `step(0)` carries changes through the Vizij's graphs and runs and moves nothing that time drives: the springs, dampers and slews of its graphs, programs and skills keep their value (and a spring its velocity), as the clocks, oscillators, noise and animations already did. They used to jump to their targets, so a paused face snapped each smoothed value on every edit. An edit whose way to the rig crosses a spring, a damper or a slew becomes that smoother's new target, which the face shows once time passes. A `say` run follows its audio, which plays in real time. In `@vizij/node-graph`, the same holds for an `evalAll` with no `step` before it. An IK node answers a request equal to its last one with its last solution: solving a redundant chain again from its own solution moved its joints with nothing changed.

## 0.11.1

### Patch Changes

- 0bdbf32: A graph edge that selects from a constant is resolved when the graph is planned, so evaluating the graph neither reads nor copies the constant: a face's device step takes 12 % fewer instructions. In `@vizij/node-graph`, `set_param` on the `value` of a constant read through a selector replans the graph.
- be363f7: A graph's constants keep their value from one evaluation to the next instead of being copied each time: a face's device step takes 42 % fewer instructions.
- 670cf16: A graph edge with a selector clones the part it selects, not its whole source value: a face whose pose driver selects fields of a large record steps in about a quarter of the instructions it took.

## 0.11.0

### Minor Changes

- 7730edb: The `spawn` node takes an optional `when` input: a new value starts a run even when the arguments are unchanged. `play_viseme` takes an optional `duration`, and the ROS4HRI mapping plays each streamed viseme for its duration and starts a new run for each new time, so a repeated shape plays again.

## 0.10.0

### Minor Changes

- d95ff01: A `spawn` node starts a task run of its function each time its arguments change: the `value` param's fields, then the keyed `args` inputs, one per parameter id in `record_keys`. The first evaluation only records them. The host serves the request after the evaluation and owns the run.

### Patch Changes

- e355a0b: The evaluation order is the graph's alone: where two outputs write one path, the later-listed one wins every time, instead of a winner that changed between loads. In a device, a task run writes after the main behavior and a later run after an earlier one, and grafting or pruning a run no longer re-plans the whole graph.

## 0.9.0

0.9.0 follows 0.7.1: 0.8.0 was never published, and its changes ship here.

### Minor Changes

- A `taskrun` node calls with its `value` param's fields, then the fields of its `args` input, which win for a parameter both name; the `args` input used to replace `value` whole.
- Visemes leave the ROS4HRI profile and become the viseme players' business. `skills()` lists `play_viseme` (one shape through a lipsync envelope; a new call takes the lips over) and `say` (text-to-speech with the lips driven from the streamed visemes) next to `look_at`, with `skillSource(id)` serving their fragments; the `ros4hri` profile no longer declares `standard/ros4hri/viseme/*`, and the mapping no longer writes the lipsync surface. The face standard's `standard/vizij/viseme/<shape>` weights stay raw, the current viseme is state at `standard/vizij/viseme`, and a player's run feeds back `{viseme, intensity}` — the pair ROS4HRI's `Say` feedback carries as Vizij extends it. In the node graph, the `taskrun` node gains keyed `mutated` outputs (one per out parameter named in `record_keys`, by parameter id) and a `done` (terminality) output, and integer values count as scalars to the arithmetic nodes.

## 0.7.1

### Patch Changes

- Renamed to `@vizij/node-graph` from `@vizij/node-graph-wasm`: a bare-domain
  npm name — the `-wasm` suffix described the Rust crate, not the npm package.
  Same package and API; update your dependency. The old name is deprecated and
  points here.

## 0.7.0

### Minor Changes

- Values emitted by graph evaluation and the node registry defaults are now in
  arora serde form. Read them through `@vizij/value-json` accessors (which
  decode both the new and the legacy forms); code that pattern-matched the raw
  JSON shape must switch to the accessors.

## 0.6.1

### Patch Changes

- 7f68b05: Fix wasm init normalization so wrapper packages accept `{ module_or_path: ... }`
  inputs without double-wrapping them, and make the shared loader normalize that
  object form consistently in Node and browser contexts.
- Updated dependencies [7f68b05]
  - @vizij/wasm-loader@0.1.5

## 0.6.0

### Minor Changes

- 0fa5752: Add noise generator and to/from vector node support in the graph runtime,
  including runtime param handling updates for noise-related node params.

## 0.5.0

### Minor Changes

- a157545: Add performance-focused wrapper APIs for delta snapshots, hot-path staging, and structural edits.

  - node-graph-wasm: add slot/hot-path staging helpers, delta-aware eval paths, and batch output APIs; expose specVersion/fingerprint in GraphSpec.

  - orchestrator-wasm: add replaceGraph, stepDelta, and smart/hot input staging helpers; surface GraphSpec cache fields and GraphReplaceConfig.

## 0.4.9

### Patch Changes

- 9bd0189: Fix publishing to include dist again
- Updated dependencies [9bd0189]
  - @vizij/wasm-loader@0.1.4
  - @vizij/value-json@0.1.2

## 0.4.8

### Patch Changes

- f6cba9e: Release process testing patch bump
- Updated dependencies [f6cba9e]
  - @vizij/wasm-loader@0.1.3
  - @vizij/value-json@0.1.1

## 0.4.7

### Patch Changes

- Add the math-toolbox expansion: bundle the new abs/sqrt/sign/min/max/round/modulo nodes plus the piecewise remap helper, refresh the baked registry metadata, and extend the wasm tests/fixtures so the new nodes stay covered in CI.

## 0.4.6

### Patch Changes

- Fix package entry points to target the new "dist/<pkg>/src" layout generated by TypeScript so bundlers and TypeScript can resolve the browser-friendly builds.

## 0.4.5

### Patch Changes

- Switch node-graph-wasm to the browser-aware loader split, ensuring bundlers avoid `fs/promises` while keeping Node support, and bump the loader dependency accordingly.

## 0.4.4

### Patch Changes

- Fix wasm entrypoints to resolve their generated JS shim from pkg/… when packaged. Adds ambient module shims so TypeScript accepts the path.

## 0.4.3

### Patch Changes

- Replaced dynamic Node specifiers with literal imports and allow defaultWasmUrl() to return either a string or URL, preventing webpack’s RelativeURL fallback from exploding. Prefer a static ESM import (with fallback), cache the wasm path as a string, and document the bundler setup needed for async WebAssembly in Next.js/Webpack.
- Updated dependencies
  - @vizij/wasm-loader@0.1.1

## 0.4.2

### Patch Changes

- Added baked node regestry metadata helpers

Release notes are generated via [Changesets](../../../.changeset/README.md). Queue a new entry with `pnpm changeset` before publishing.
