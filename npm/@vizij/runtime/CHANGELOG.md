# Changelog

## 3.0.0-alpha.0

### Breaking

- The wasm is the Bevy view and the Arora device together: 31 MB, 9.2 MB
  gzipped, against 2.4.0's 2.5 MB and 0.8 MB. Every import of this package
  fetches it, including one that only lists `skills()` / `profiles()` or runs
  a device with nothing drawn (`startRuntime`). No device-only artifact is
  built — the `vizij-arora-web` crate is gone — so a page that cannot carry
  the view stays on `@vizij/runtime@2`.
- The wasm file is `vizij_bg.wasm`, no longer `vizij_arora_web_bg.wasm`, and
  it ships once, under `dist/pkg/` (`pkg/` is not in the package). An
  `init(input)` that names the file by URL or path must name the new one.
- `Runtime.run()` returns `Promise<void>` and resolves when `stop()` reclaims
  the device (it was `Promise<never>`): a caller that took the promise's
  settlement for a failure now takes a `stop()` for one. `running` reads the
  device, so it turns false after `stop()`.

### Added

- The view: `mount(canvas, options?)` creates the page's one App;
  `loadVizij(vizijId, glb, options?)` starts a Vizij's device and shows it;
  `placeVizij` / `placeVizijIn` / `fillCanvas` confine it to a rectangle of
  the canvas; `unloadVizij` takes it down; `ready` / `whenReady` say when its
  scene shows; `drainPicks` reports pointer presses as
  `{ vizijId, elementId }`; `describe(glb)` reads a GLB's elements,
  animatables, bounds and programs; `memoryBytes` reads the module's linear
  memory. A Vizij's paths are its own (`runtime.rigPrefix`,
  `runtime.path(relative)`).
- `Runtime` is a Vizij's Arora, or `startRuntime(graph)`'s with no Vizij:
  `stop()`, `spawn(call)` (a task run, resolving to its `TaskHandle`),
  `halt(handle)`.

### Changed

- The API's noun is the Vizij — what the authoring app exports, a face most
  often, not always: `composeFace` is `composeVizij`, `ComposeFaceOptions`
  `ComposeVizijOptions`. The GLB bundle's own field keeps its name
  (`describe(glb).faceId`, the `rig/<faceId>/` prefix).
- `RuntimeModule` is `AroraModule`: an Arora module as `arora-web` loads it,
  header JSON plus executable.
- The animation module is host-linked into every device, its functions
  called by id: `composeVizij({ animations: true })` dispatches without a
  guest. Arora wasm modules still load as guests — `startRuntime(graph, init,
  modules)` and `loadVizij`'s `options.modules` take `AroraModule`s — and a
  guest under a host-linked module's id (`@vizij/animation-module`'s) is
  served by the host-linked one.

## 2.3.0

### Minor Changes

- 3d4406a: Add `composeVizij(gltf, options?)`: the composed behavior graph of a face bundle — base graphs, embedded standard profiles (each suppressing the built-in of the same id), the built-in ROS4HRI profile unless opted out, and the selected program — exactly as the native `vizij` app deploys it. The returned spec feeds `startRuntime`/`Runtime.loadGraph`, so an exported GLB can be deployed and verified in JS without the native app (VIZ-93's autonomous verification loop).

## 2.2.0

### Minor Changes

- 9f1b568: Expose the standard-profile registry to JS: `standardProfiles()` lists the shipped profiles (`{ id, title, description }` — currently `ros4hri`) and `standardProfile(id, rigPrefix)` returns a profile's graph with the face's rig prefix applied, ready to compose or to embed into a GLB as a `standard-profile` bundle graph (`standard::<id>`). The API an authoring app's opt-in picker consumes (VIZ-92).

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
  `DeviceModule` → `AroraModule`, `DeviceCall` → `RuntimeCall`,
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
