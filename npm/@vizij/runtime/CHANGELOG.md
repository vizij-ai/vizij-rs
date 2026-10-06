# Changelog

## 4.1.0

### Minor Changes

- 9dc784c: The view renders faces authored in the authoring app, exported from Blender, or exported from Semio Studio. A `phong` or `lambert` element is shaded as three's `MeshPhongMaterial` and `MeshLambertMaterial` are under the ambient light — diffuse plus emissive, no metalness — and `shininess` and `specular` bind (they shape nothing without a direct light). Every mesh of a face is drawn under that model, a mesh no element declares shaded as three's glTF loader makes its material, so a GLB without RobotData (a plain Blender export, its colors in `emissiveFactor`) renders its colors; a GLB declaring no `rootBounds` is framed on its scene's bounding box, which `describe()` reports as its `rootBounds`. Studio's per-axis RobotData features (`translation.x`, `rotation.r`, `color.g`) bind, and `setStaticFeature` takes their names; `describe()` reports a per-axis animatable's `feature` by that name. A RobotData `ellipse` or `rectangle` draws as a shape with a unit circle or plane scaled by its `width` and `height`, its stroke dropped with a warning. An element on an unnamed glTF node is found.
- d838ba2: The ROS4HRI mapping shows a named expression (`standard/ros4hri/expression/name`) at the commanded arousal (`standard/ros4hri/expression/arousal`) as its intensity, clamped to 0..1, instead of at full weight: a name with an arousal of 0 or below shows no expression. An empty name still blends the expressions by valence and arousal on the circumplex.
- 5bfa181: The `ros4hri` profile declares `standard/ros4hri/viseme`, the lip shape a ROS4HRI TTS node streams (a `u8` index into the face standard's 15 shapes, resting at `sil`): 27 keys in all. Nothing maps it to the lips yet.
- 346c6fd: A device's animation module (1.1.0) adds `set_window`, `play_at`, `set_start_offset` and `set_time_scale`, callable with `Runtime.call`, plays backwards at a negative `set_speed`, and reports `ended`, the play window and each instance's timing in the player states at `vizij/animations/players` (see `@vizij/animation-module`). The device's animation source steps the module without `time_ns`, so there `play_at` counts from the module's first step.
- 0971559: The face reports what it is saying: while a `say` run's audio plays, the utterance is the face's speech state, `standard/vizij/speech` (empty before playback starts and once it ends), readable like any other key. The browser provider counts the audio as playing once the page's playback hook reports a playhead past 0.

## 4.0.0

### Major Changes

- 9f576e1: `Runtime` is a client of the Vizij's device with the surface any client of an Arora device has: `call(call)` (a `Call` names its `module_id`), `invoke(method, args, moduleId?)` — a described method by name, its arguments by parameter name; a task method starts a run and resolves to its `RunHandle` (`{ run, status, feedback, result, update }`, as any Arora client is given it) — `spawn(call)`, `halt(runId)`, `listKeys(prefix?)` (each key with its meta, `KeyInfo`), `describeMethods(prefix?)` (`MethodDescription`), the store's reads and writes, and the interpreter's LOAD and EDIT as `loadGraph` and `applyGraphEdits`. Every operation that reaches the device applies at its next step, which its promise waits for; none steps the device itself.

  What the face does goes through the methods its modules and interpreter declare, by name: the skills (`invoke("say", { text })`, `look_at`, `play_viseme`), the rest module's `reset` and `reset_keys`, the animation module's functions, and programs. A face's programs are runs of its interpreter's `run_behavior(name, behavior)`: `behaviorValue(graph)` makes a graph spec its `behavior` argument, `runEdits(runId, from, to)` builds the `applyGraphEdits` edits that change a run's behavior in place (the nodes it keeps keep their state), and `BEHAVIOR_RUNS` is the key prefix under which every run's `status` and `name` are listed. `loadVizij`'s `program` option starts its program as such a run, beside the face's graph rather than composed into it.

  Breaking, against 3.0.0: `spawnSkill(name, args)` is `invoke(name, args)`; a run handle is `RunHandle` rather than `TaskHandle`, and `halt` takes its `run` id; `call` no longer infers a missing `module_id`; `loadGraph` and `applyGraphEdits` no longer step a device that is not under `run()`.

### Minor Changes

- 74fd512: A Vizij's animations load into its device's animation module with the face (unless `loadVizij(id, glb, { animations: false })`): each on a player named after the animation's id, its instance at weight 0, stopped, each track writing the rig input its channel names. Every client plays, loads and unloads animations through the module's declared functions, by name (`invoke("play", { player })`, `set_weight`, `load_animation`, `reload_animation`, …). `moduleAnimation(animation)` turns an animation in `describe()`'s shape into the module's `AnimationClip` on the face's rig keys, for `load_animation`; `decodePlayerStates` reads the player states the animation source writes to `ANIMATION_PLAYERS_PATH` each step. `ModuleAnimation`, `PlayerState` and `InstanceState` are exported types.
- 7645c96: The calls an authoring viewport makes. `drainPicks()` reports a press on no element as a miss (`elementId: null`), and finds the element over its mesh as drawn, morphs applied. `setSelection(vizijId, elementIds)` outlines the chosen elements with the selection glow. `Runtime.hold(paths)` / `release(paths?)` keep rig outputs, whole or per component, at what the view shows while the device runs. `setStaticFeature(vizijId, elementId, feature, value)` sets a static feature in place, with no reload. `loadVizij` over a Vizij already shown keeps its previous scene on screen until the new one is ready, so a structural edit reloads from exported GLB bytes without a blank frame; `ready` reads false from the call until then. A GLB carrying static features in its RobotData loads.
- cab6392: `describe(glb)` returns what a face's bundle carries for an app to build its controls from, parsed once in the wasm: `poses` and `poseGroups` (each pose with its description, the ids of the groups it belongs to and its input values), `rigInputs` (each rig input's path relative to the rig prefix, label, group, default and range), `programs` (each program's id, label and graph, as authored), `animations` (ids, names, durations in seconds, tracks of time-ordered keyframes) and `metadata`, the bundle's open-ended metadata as authored (`speechConfig`, `activeMotionGraphId`, …). `Program`, `Pose`, `PoseGroup`, `RigInput`, `Animation`, `AnimationTrack` and `AnimationKeyframe` are exported types (`Animation` is `@vizij/runtime`'s, not the DOM's Web Animations `Animation`), and `VizijDescription` documents every field.
- f4f25e4: A face returns to rest at any time through its device's `rest` module, which any client invokes by name: `reset` puts every key the device describes back to the value it rests at — each free input of the composed graph to its authored default, each input the bundle's neutral pose names to its neutral — and `reset_keys` only the keys it names. A device from `startRuntime` rests its graph's free inputs at their defaults.

  `setView(vizijId, { bounds, fit, zoom, toneMapping })` frames and tone-maps one face on a canvas several faces share: `bounds` (`{ center, size }`, the shape `describe` reports `rootBounds` in) replaces the GLB's authored bounds, `fit` and `zoom` override `mount`'s for that face, and `toneMapping` is one of the authoring app's curves (`none`, `agx`, `aces`, `neutral`, as three.js computes them). Each call replaces the face's previous view, and the view survives the face's reloads. `safeArea(vizijId)` gives the rectangle the face's framed bounds cover on the canvas, in CSS pixels from its top-left corner, for a DOM overlay. `Bounds`, `Fit`, `ToneMapping` and `VizijView` are exported types.

- b0994fd: `profile("vizij-face")` declares 88 keys: the blink `standard/vizij/blink` (tier `gaze`; 0 open, 1 both eyes closed — a level the writer shapes over time, independent of the eyelid positions), the expressions `standard/vizij/expression/concerned` and `…/sleepy` (tier `expression`), and the conversation state `standard/vizij/conversation/{speaking,user_speaking,thinking}` (tier `conversation`, written 0 or 1 by the agent the face speaks for). Each is an `f32` weight in [0, 1] resting at 0. `mapping("ros4hri")` is unchanged: ROS4HRI names none of them.

## 3.0.0

3.0.0 follows 2.4.0: 2.5.0 was never published, and its changes ship here.

### Breaking

- The wasm is the Bevy view and the Arora device together: 25 MB, 7 MB
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
- Profiles and mappings are two things, and the API now says so.

  A **profile** is an interface: the set of store paths one party exposes to another, each with its type, range, and default. `profiles()` lists the shipped ones (`{ id, version, title, description, scope, keys }` — currently `vizij-face`, 81 paths, and `ros4hri`, 25) and `profile(id, rigPrefix)` returns one in full, every path with its arora type, range, default, and standard metadata (FACS action unit, ARKit blendshape, tier). `scope` says where the paths live: a `face` profile is addressed to one face with its rig prefix, a `device` profile is absolute and ignores the prefix.

  A **mapping** is a graph that implements one profile in terms of another. `mappings()` and `mapping(id, rigPrefix)` are the renamed `standardProfiles()` / `standardProfile(id, rigPrefix)`, which stay as deprecated aliases (with `StandardProfile` aliasing `Mapping`); nothing that consumes them moves.

  This is the API an authoring app's profile import consumes: pick a profile, get its paths already addressed to the open face, and declare it on the GLB (`bundle.profiles`) so the interface a face is authored against travels with the asset.

- Visemes leave the ROS4HRI profile and become the viseme players' business. `skills()` lists `play_viseme` (one shape through a lipsync envelope; a new call takes the lips over) and `say` (text-to-speech with the lips driven from the streamed visemes) next to `look_at`, with `skillSource(id)` serving their fragments; the `ros4hri` profile no longer declares `standard/ros4hri/viseme/*`, and the mapping no longer writes the lipsync surface. The face standard's `standard/vizij/viseme/<shape>` weights stay raw, the current viseme is state at `standard/vizij/viseme`, and a player's run feeds back `{viseme, intensity}` — the pair ROS4HRI's `Say` feedback carries as Vizij extends it. In the node graph, the `taskrun` node gains keyed `mutated` outputs (one per out parameter named in `record_keys`, by parameter id) and a `done` (terminality) output, and integer values count as scalars to the arithmetic nodes.
- The `vizij-face` profile declares `standard/vizij/mouth/morph/jaw_open` — the de-facto jaw-open path every current face implements, the same muscle as `face/jaw_open` (AU 26, ARKit `jawOpen`) — so `profile("vizij-face")` lists 82 keys and every path the ROS4HRI mapping writes is declared.

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
- The skills are declared from contracts: `say`'s `voice` is optional, and
  its fragment states the `viseme` it passes to the provider (`sil`) in its
  task-run node's `value`. `look_at` and `play_viseme` are described by the
  device's interpreter and listed under the interpreter module, with no gaze
  or viseme module of their own; `spawnSkill` spawns them through it.
- A `taskrun` node calls with its `value` param's fields, then the fields of
  its `args` input, which win for a parameter both name; the `args` input
  used to replace `value` whole.

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
