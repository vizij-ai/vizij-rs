# Changelog

## 2.1.0

### Minor Changes

- c435435: Add `Runtime.applyGraphEdits`: apply a spec-level graph diff (`upsert_nodes` / `remove_nodes` / `upsert_edges` / `remove_edges`) to the running graph in place (VIZ-79). An edit patches the graph — unchanged nodes keep their runtime state — instead of reloading the whole spec.

All notable changes to `@vizij/runtime`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [2.0.0] - 2026-07-22

### Breaking

- The client-facing API drops the "device"/"Arora" vocabulary for "runtime":
  `startDevice` → `startRuntime`, the `AroraDevice` class → `Runtime`,
  `DeviceModule` → `RuntimeModule`, `DeviceCall` → `RuntimeCall`,
  `DeviceCallResult` → `RuntimeCallResult`. Behavior is unchanged; only the
  names differ. Update imports and the class name at call sites.

## [1.1.0] - 2026-07-22

### Added

- Device graphs can apply a keyed record batch to the store by default: an
  `output` node **without** `path` takes `key_field`/`value_field` params
  (record field ids, UUIDs) and writes each record's value under the path
  its key field names — e.g. an `externalfunction` module call's "what
  changed" applied onto its own keys. An empty batch writes nothing; a
  record missing either field is an evaluation error; a repeated key keeps
  the batch's last entry in the tick's flush.

## [1.0.2] - 2026-07-22

### Changed

- Built on arora 9.1 / arora-web 6.1: the self-paced loop yields to the JS
  event loop even when a step overruns its period, so `run()` no longer
  freezes the page under sustained load.
- A failing behavior tick no longer ends `run()` (and no longer rejects its
  promise): the failure stands as a readable error until a tick recovers,
  and a failed run leaves the device usable.

### Added

- `AroraDevice.behaviorError` — the behavior's standing error: the message
  of its latest failed tick, `undefined` while healthy.
- `AroraDevice.behaviorErrorChanged()` — resolves on the next change of the
  standing error (a message when a failure appears, `undefined` when a tick
  recovers); sequential awaits share one cursor, so no change is missed.

## [1.0.1] - 2026-07-21

### Fixed

- The published dependencies are resolved semver ranges. The 1.0.0 artifact
  on npm carries unresolved `workspace:*` ranges and is not installable.

## [1.0.0] - 2026-07-21

### Changed

- The package is `@vizij/runtime`. Earlier versions were published as
  `@vizij/arora-web-wasm`.
- `AroraDevice.run(periodMs?)` hands the device to its own self-paced loop,
  for good: `step()` is unavailable from then on (the `running` getter
  tells), and the returned promise only ever rejects — when stepping fails.
- `AroraDevice.loadGraph(spec)` swaps the running graph in place: the store,
  the loaded modules, and the device itself all survive. On a device not
  under `run()` a zero-dt step is taken so the swap lands without an
  external driver.
- `step()` returns `void`; a failed step throws.
- The first `drainChanges()` returns the store's whole current state: the
  subscription opens on it, so no separate init snapshot is needed.

## [0.2.0] - 2026-07-16

### Added

- Arora wasm modules load into the browser device: `startDevice(graph, init,
modules)` takes `{ headerJson, wasmBytes }` pairs (e.g. from
  `@vizij/animation-module`) and loads them into the device's engine.
- `AroraDevice.call(call)` calls a loaded module's function through the
  device: the call dispatches inside the next `step` — the same phase a
  remote bridge command executes in — and resolves with the `CallResult`.
  Loaded exports also feed the graph's function → module map, so
  `ExternalFunction` nodes reach module functions with no extra wiring.

## [0.1.2] - 2026-07-11

### Changed

- Store writes (`setValue`/`writeValues`) accept the vizij value shorthand
  (`{"float": 0.5}`, `{"vec3": [1, 2, 3]}`, …); the wasm normalizes it to the
  canonical Arora `Value` form.

## [0.1.1] - 2026-07-10

### Changed

- arora-web 5.2.1 floor: the store accessors return plain JS objects, so the
  wrapper forwards them as-is (the deep Map→object conversion is gone).

## [0.1.0] - 2026-07-10

### Added

- Initial release: run a Vizij runtime in the browser as an Arora device.
  `init()` loads the wasm once; `startDevice(graphSpec?)` boots the device
  with the graph as its behavior; `AroraDevice` exposes `step(dtMs)`,
  `setValue`/`writeValues` (ValueInput in), `readValues`/`snapshot`/
  `drainChanges` (ValueJSON out).
