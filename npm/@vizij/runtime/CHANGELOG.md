# Changelog

All notable changes to `@vizij/runtime`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

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
