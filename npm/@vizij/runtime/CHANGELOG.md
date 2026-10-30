# Changelog

## 10.0.0

### Major Changes

- 27ec269: The device runs on arora 18, through arora-web 15: its behavior interpreter reads arora-behavior 10's graphs, a node holding its own bindings, and its behavior trees are `behavior-tree` 2.0 graphs. A graph `invoke`, `spawn` or the interpreter module's `LOAD` and `EDIT` carry in arora-behavior 9's form is refused. `loadGraph`, `applyGraphEdits` and `runEdits` take and give Vizij specs and spec diffs as before; `behaviorValue` gives the graph in the form the device's `run_behavior` now takes.

## 9.2.0

### Minor Changes

- 61d41d6: A face's graphs run on arora-behavior-tree: each step runs the nodes the graphs' outputs read whose inputs changed, and the graphs compute what they did. Measured natively against the evaluation this replaces, a step of a 950-node graph costs 2.2 times less when its inputs hold and 1.6 times more when every input moves, and a step of a 19,000-node graph costs 8 and 3.3 times less. Loading a graph, or editing it, costs 2 to 3.5 times more.

## 9.1.0

### Minor Changes

- ac62f8e: `describe(glb)` reads the face's versions from its bundle: `hardwareVersion`, its `metadata.hardwareVersion` — the version of the face itself, its shapes, what can move in them and the layout giving those movements their meaning, such as `quori-1.3` — and `softwareVersion`, its `metadata.softwareVersion` — the version of what it runs, its profiles, exposed inputs, programs, extra rigs and animations, such as `1.2`. Each is `null` when the bundle states none or a blank one. A Vizij's device describes itself as the `vizij` model family, of the face's hardware version, else its `faceId`, and of its software version when it has one.

## 9.0.2

### Patch Changes

- 6ff76f3: The module runs its constructors once, when it is instantiated, so a call into it runs only its own code. `invoke`, which builds its answer as JavaScript values and calls into the module for each one, answers in about the time `call` takes for the same value: its answer used to take 10 to 30 times as long as `call`'s, the module's constructors running on each of those calls. Every other operation sheds the same cost per call into the module.
- 8ab9850: `invoke`, `setValue`, `writeValues` and `moduleAnimation` cost linearly in the size of the values they carry. A large record, such as a long animation converted by `moduleAnimation` and passed to `load_clip` or `reload_clip`, used to cost superlinearly the first time: it went through an intermediate JSON tree holding about sixty times its text in the wasm heap. It is now read straight into an Arora value, and written straight from one.

## 9.0.1

### Patch Changes

- 4736708: An ellipse's or a rectangle's fill named per axis, as Semio Studio writes it (`fillColor.r`, `fillColor.g`, `fillColor.b`), colours it: a static component states that component of the fill, an animated one moves it. Such a fill used to draw white and move with nothing.

## 9.0.0

### Major Changes

- 7d073b4: A device runs on arora 15, and loads a WebAssembly guest as a component only.

  - `startRuntime(graph, init, modules)` and `loadVizij`'s `options.modules` take a `{ headerJson, wasmBytes }` pair. Its header names the `wasm-component` executor, and its bytes are a component of Arora's `arora:module` world built for `wasm32-wasip2`. Arora's guests, `arora-animation-module` 2 among them, are built that way.
  - **Breaking:** a `wasm32-wasip1` core module, whose header names `wasm`, no longer loads. Rebuild such a guest for `wasm32-wasip2`.
  - **Breaking:** a call to a guest whose arguments do not match the parameters its header declares fails, naming the parameter, and reaches no guest.
  - A guest under the id of a module the device host-links, such as Arora's animation module, fails the build, naming both modules. A device loads one module per id; the host-linked module used to take the id in dispatch while the guest's functions stayed listed.

## 8.0.0

### Major Changes

- 01400b2: A device's animations play through Arora's animation module, `@arora/animation` 3.0.0 (`arora-animation-module` 1), host-linked with the same module, function and parameter ids. `moduleAnimation(animation)` returns the module's format 2.0 `Clip`, which `load_clip` takes (and `reload_clip`), where it returned the `AnimationClip` `load_animation` takes. Each segment plays as its left keyframe's interpolation says: `linear`, `step`, or `cubic` as glTF's `CUBICSPLINE` Hermite on the keyframes' `inTangent` and `outTangent` (value units per second), the segment's chord slope where one is absent — so an untangented `cubic` plays the straight line its keys make, where it played an ease-in-out. The module plays numbers as `f64`; an animation writes each key in the type the store declares for it (`KeyMeta::ty`), so a Vizij's rig inputs keep holding `f32`. A key with no declared type takes `f32`, Vizij's number type, which keeps pages that read `.f32` working. What the module changes, as a page sees it: a player `play_at` holds before its time reports `waiting` with the state `playing`; a command takes effect at the device time of the last step, so the player states read right after it show it; a window no longer moves the playhead, and a player outside it plays toward it; travel past a bound carries on past the wrap or the turn; `set_loop` takes `bounce`. `describe()`'s keyframes carry `inTangent` and `outTangent` (`AnimationKeyframe`). `decodePlayerStates` reads `PlayerState` 1.3.0: a player's `ended`, `window`, `reversed` and `waiting`, an instance's `startOffset` and `rate`.

## 7.2.0

### Minor Changes

- a79956e: The animation source steps the animation module with the device's `arora/time` as well as `arora/dt`, so `play_at(player, time_ns)` starts a player at that device time, where it counted from the module's first step.
- 6d44aec: A Vizij's device keeps no time until the Vizij is first drawn (`ready`). Until then every `step(dtMs)` is a `step(0)`, whatever `dtMs` says, so it still carries the page's operations and writes through the face's graphs. The first step after advances the time since the frame that drew the Vizij, at most its `dtMs`, and the steps after that advance their `dtMs`. A `run()` called before then starts its loop at that frame; a `stop()` before it gives the run up, and its promise resolves. Before, a page stepping a face from `loadVizij` on — `@vizij/runtime-react`'s provider does — ran the face's springs, programs and animations through the load and the first draw, in steps as long as the frames the load blocked (0.8 s on an M1), and the face first showed mid-motion. A device with no Vizij (`startRuntime`) keeps time from its first step, as before.
- 54f1590: The `vizij-face` profile declares two more settings of the face, `standard/vizij/gaze/eye_spacing` (metres, resting at 0.06) and `standard/vizij/gaze/range` (radians, resting at 0.78), and has 98 keys. The ROS4HRI mapping aims the eyes with them, read under the face's rig prefix, where it held the incumbent's 6 cm and ±0.78 rad as constants; a face authors its own in its adaptation, as the default of an input node on the setting's path, and a face authoring neither is aimed as before. A face's settings rest at the value the face authors, so `reset` returns them to the face's own.

### Patch Changes

- 077e1a1: The page's first `say` plays on a page that steps its device slowly. The page's player stops a playback that goes unpolled for longer than its halt bound, `IDLE_STOP_MS` (250 ms) or four of the page's poll intervals when the page polls slower than that, and it now measures that interval only between two polls of one playback. Until the page has measured it, no playback stops unpolled. On a page stepping about once a second, as under a software GPU, the first `say` was stopped before its first poll, so its run ended at once and no viseme played; it now plays through, and a later playback whose device stops stepping still stops within four of the page's poll intervals.
- 32eddfa: A `Spring` node moves by the exact solution of a damped spring over each step, so one long step lands where as many short ones would, whatever its stiffness, damping and mass. A face whose page was hidden — its steps throttled to one a second, then one a minute — shows its springs where that much time leaves them when it shows again. Before, a step longer than a bound its parameters set (0.15 s for the mouth and brow springs of Quori's program, 0.23 s for its eye spring) made a spring diverge: under steps of a second Quori's springs grew to infinity and then NaN, and its mouth froze at its clamps' floors until the face was reloaded. A spring's state also stays finite: at a target that is not a number it holds, and a state seeded from one starts again at rest on the next target.

## 7.1.0

### Minor Changes

- 9b51cc2: `loadGraph` and `applyGraphEdits` refuse a graph that would fail every evaluation, and the promise rejects with an error naming the node at fault. The graph is refused if it has a node that leaves a required input with neither an edge nor an inline default (a `remap` with no `in`, say), an edge to a node it lacks, two edges into one input, two nodes sharing an id, or a cycle. A refused edit is refused whole: the running graph, its runs and their state go on as they were. Such a graph used to be accepted, then failed the device's every behavior tick, base graph and every run with it, until another load or edit replaced it. A graph that evaluates is accepted as before, including one whose input path has no value staged yet.

  In `@vizij/node-graph`'s registry, an input port a node cannot evaluate without carries `required: true`: the inputs of `clamp`, `remap`, `centeredremap` and `piecewiseremap`, `urdffk`'s `joints`, `urdfikposition`'s `target_pos`, and `urdfikpose`'s `target_pos` and `target_rot`. A port that is neither `optional` nor `required` reads a default (most often zero) when unwired.

- ff1c1f2: `releaseKeys` settles as the device's operations do. On a device with no `run()` going and no `step` from the page, the release asks for a `step(0)` in the next microtask, shared with the same turn's operations and `setValue`/`writeValues`. In that step, a released key that the face's behavior writes takes its writer's value again. Before, the key kept the page's value until something else stepped the device. A page that calls `step` in the same turn applies the release with its own step instead, and under `run()` the run's next step applies it, with no step added.
- a6eb6ed: A device's operations settle when nothing steps it. `loadGraph`, `applyGraphEdits`, `call`, `invoke`, `spawn` and `halt` resolve on a device with no `run()` going and no `step` from the page. Each is queued before it returns, and a `step(0)` in the next microtask applies it, so `await runtime.loadGraph(spec)` on a `startRuntime()` device resolves with the new graph already run once. That step is shared with the `setValue` and `writeValues` of the same turn, and applies everything in the order it was issued. A `step(0)` moves nothing that time drives: springs, dampers, slews, animations and program clocks hold. A `say` run follows its real-time audio. A page that calls `step` in the same turn applies the operations with its own step instead. Under `run()`, the run's next step applies them, and the page adds no step. `listKeys` and `describeMethods` answer at once, with no step, and do not see an operation still queued. A call issued from a callback the device makes during a step, such as the `audio` hook's playhead, is an ordinary call: a step cannot nest one, so the next step applies it, a `step(0)` once the current step has ended or, under `run()`, the run's next step.

### Patch Changes

- 0528ebe: Licensed under Apache-2.0, where the package said MIT. The animation module's header (`@vizij/animation-module`'s `artifact/header.json`) states `"license": "Apache-2.0"` too, where it said `"Proprietary"`. Releases already published keep the licence they were published under.
- d7abdab: The browser module builds on arora 13 and arora-web 10. Its surface and what a device does are unchanged.
- eca26b7: An animatable that several elements share — shapes sharing a material share its colour and opacity — moves every one of them, as the three.js view does; it used to move only the last element the GLB lists. `describe`'s `animatables` entries list every node they drive in `nodes`.
- Updated dependencies [0528ebe]
  - @vizij/value-json@0.2.1
  - @vizij/wasm-loader@0.1.6

## 7.0.0

### Major Changes

- c54800c: A step with no time in it moves nothing that time drives. In `@vizij/runtime`, `step(0)` carries changes through the Vizij's graphs and runs and moves nothing that time drives: the springs, dampers and slews of its graphs, programs and skills keep their value (and a spring its velocity), as the clocks, oscillators, noise and animations already did. They used to jump to their targets, so a paused face snapped each smoothed value on every edit. An edit whose way to the rig crosses a spring, a damper or a slew becomes that smoother's new target, which the face shows once time passes. A `say` run follows its audio, which plays in real time. In `@vizij/node-graph`, the same holds for an `evalAll` with no `step` before it. An IK node answers a request equal to its last one with its last solution: solving a redundant chain again from its own solution moved its joints with nothing changed.

### Minor Changes

- 1a24e58: `channelKeys(gltf, channels)` names the store key each animation track channel drives on the Vizij whose glTF JSON document is `gltf`, without loading it: one key per channel, in order, resolved through the bundle's rig exactly as `Runtime.moduleAnimation` keys a track once the Vizij is loaded. A page that samples an animation on a `startRuntime` device over `composeVizij`'s graph writes these keys to move it as the Vizij's own playback does. It rejects a document that carries no `VIZIJ_bundle`.
- 89e6ee6: `Runtime.holdKeys(keys)` holds store keys at the values clients write: from the next step until `releaseKeys(keys?)` (every held key when omitted), the Vizij's own behavior — its graphs, its program and skill runs, its animations — does not write them, while `setValue`, `writeValues` and the rest module's `reset` still do. What the graphs compute from a held key follows the client's value, so an author's edit of a rig input, a pose weight or a program's input stands while it is made. Holding a key the behavior does not write changes nothing; a reloaded Vizij starts with no holds.

### Patch Changes

- 48c880b: `ready` and `whenReady` report a Vizij once the GPU has finished a frame drawing every mesh visible when it was drawn, each with its pipeline compiled, rather than once its scene is indexed. Until then the Vizij draws under the canvas's clear, so its rectangle shows the previous scene or the background, never a face with meshes missing; a reload's previous scene draws until the new one is ready. Under a software GPU, where a face's first draw compiles its pipelines for seconds, a page no longer stalls after `ready`: it draws on from there. Should something keep a Vizij from being drawn — a material that never loads, a mesh whose pipeline cannot be specialized — the console says what after 120 frames, and the Vizij is ready as it draws after 600 (ten seconds at 60 Hz).
- 81edd3f: A Vizij whose RobotData ids are no UUIDs loads and draws: an element's and an animatable's id is any string, as the published three.js line reads it, where `loadVizij` and `describe` failed with "bad RobotData on node …: UUID parsing failed". Picks report the id as written, and `setSelection`, `setStaticFeature` and `hold` take it.

## 6.2.0

### Minor Changes

- 1eccf06: A face's free input outside the shipped profiles takes its range, as `listKeys` reports it, from: its input node's `params.min` and `params.max`, when both are there and `max` exceeds `min` (both optional); else the `range` the rig declares for it in `metadata.vizij.inputs`, as the authoring app exports for every rig input; else `[0, 1]`. A key a shipped profile declares keeps that profile's range, and types are unchanged. A face's rig inputs that rested outside `[0, 1]` (an eye's scale or translation, say) now report the range their rig declares around their rest.
- c3b6e21: `onInitProgress(listener)` follows `init()`: the wasm module's download in bytes (against its `Content-Length` when the response is not compressed), its compile, its start and `ready`. The listener is called with the current stage at once, then with each change, the download at most every 50 ms. The bytes are counted on a clone of the fetched response, and the response itself is what compiles as it streams in, so Chrome caches the compiled code for the next load. The README says how a host serves the module for that: a content-hashed URL, an immutable `Cache-Control`, `application/wasm`, and brotli.

### Patch Changes

- 7c2fa20: The wasm module is linked with fat LTO and leaves out Bevy's log plugin and multi-threaded executor, which a page never runs: 24.3 MB (6.9 MB gzip, 4.3 MB brotli), from 26.6 MB (7.5 MB, 4.6 MB).

## 6.1.1

### Patch Changes

- af7ebc7: A device holds the store changes a page has not drained as the latest value per key, folded on every step: a page that never calls `drainChanges` no longer grows its memory with every step. `drainChanges` answers as before, each changed key's latest value (`null` for a cleared one) and the whole state on the first drain.

## 6.1.0

### Minor Changes

- 1b2b23c: The `vizij-face` profile declares the caption a host shows over the face, `standard/vizij/caption/text`, and who says it, `standard/vizij/caption/speaker` (`__system__` for the face itself): string inputs resting empty, in no tier. The profile has 96 keys. The `ros4hri` profile declares `standard/ros4hri/caption/{text,speaker}`, which a ROS bridge writes from `hri_actions_msgs/ClosedCaption`, and the ROS4HRI mapping relays them as is to the face's caption.
- 7dfa940: The `vizij-face` profile declares the gaze target, `standard/vizij/gaze/target/x` and `…/y`: a point on the face's frame — origin top-left, `x` to the right, `y` downward, each in `[0, 1]` — resting at the centre, (0.5, 0.5), in the gaze tier. A face whose graphs read it lists it in `listKeys` as an `f32` in `[0, 1]` resting at 0.5. The profile has 93 keys.
- 6905800: The `vizij-face` profile declares the gaze target's weight, `standard/vizij/gaze/target/weight`: how strongly the eyes hold the target, an `f32` in `[0, 1]` resting at 0, in the gaze tier. At 0 the eyes do what the face itself does, whatever the target says; at 1 they hold the target. The profile has 94 keys.
- 9ea8b3a: The `ros4hri` profile declares 10 keys and no `standard/ros4hri/au/<code>` keys, and the ROS4HRI mapping writes none of the face standard's muscle tier: ROS4HRI publishes facial action units only as perception output and defines no command for them. The mapping's idle blink is inhibited by the `asleep` expression alone.

### Patch Changes

- 0bdbf32: A graph edge that selects from a constant is resolved when the graph is planned, so evaluating the graph neither reads nor copies the constant: a face's device step takes 12 % fewer instructions. In `@vizij/node-graph`, `set_param` on the `value` of a constant read through a selector replans the graph.
- be363f7: A graph's constants keep their value from one evaluation to the next instead of being copied each time: a face's device step takes 42 % fewer instructions.
- 23ad7fa: A Vizij's device reads its graph's inputs from the store in one read per step, under keys prepared when the graph loads: its step takes 16 % fewer instructions.
- 670cf16: A graph edge with a selector clones the part it selects, not its whole source value: a face whose pose driver selects fields of a large record steps in about a quarter of the instructions it took.
- 740518d: A Vizij's device evaluates its graph without copying every port by name each step, a copy nothing read: its step takes 30 % fewer instructions.
- 997ed10: The view's cameras build one light cluster instead of a grid each frame: the faces are lit by the ambient term alone.
- 27fea02: The view lands only the pose values that changed since its last frame, read from the device in place: applying a Vizij's pose takes a tenth of the instructions it took.

## 6.0.0

### Major Changes

- 6d0d7bc: A face loaded with `loadVizij`, or composed with `composeVizij`, speaks the vizij standard: its `standard/vizij/*` controls drive it through the bundle's `standard-adaptation`. The built-in ROS4HRI mapping composes when a page asks for it with `{ ros4hri: true }`, and brings the ROS4HRI keys (`standard/ros4hri/*`), an idle blink and the `look_at` skill's gaze. A face that embeds its own `standard::ros4hri` mapping runs it either way.

  A face's standard controls are declared as the `vizij-face` profile states them: `listKeys` reports each `standard/vizij/*` input as an `f32`, an eye position in `[-1, 1]` and a weight in `[0, 1]`, resting at the profile's rest, and the device takes a remote write of those keys only as an `f32` (a page's `setValue` with a number writes one).

## 5.1.0

### Minor Changes

- 0db97cd: A Vizij's rectangle is any rectangle of the canvas. `placeVizij` frames the Vizij on the whole rectangle and draws the part on the canvas, so a slot scrolled past the canvas's edge shows its Vizij cut off there, at the position and scale the whole slot gives it, where it used to be moved (past the top or left edge) or refitted into what was left (past the bottom or right edge). An empty rectangle (a width or height of 0 or less) or one wholly off the canvas draws nothing and takes no picks; a Vizij never placed still draws over the whole canvas. `safeArea` follows the whole rectangle, and reads `null` while it is empty. The canvas shows `mount`'s background wherever no Vizij draws, also when none does.

  `outputKeys(graph)` lists the store keys a run of `graph` writes and leaves holding when halted — its output nodes' paths, read as the device reads them, without the `task/…` outputs that write the run's own keys — what `invoke("reset_keys", { keys: { strs } })` takes to return a halted program's outputs to rest.

  `describe(glb).graphs` carries each graph's `id` beside its `kind` (`null` for an entry without one).

- e394ec0: The `vizij-face` profile declares the face's lipsync gain, `standard/vizij/lipsync/gain`, an input resting at 1 that the face's adaptation scales every viseme weight by. The ROS4HRI mapping relays the face's current viseme to `standard/ros4hri/speech/viseme`, which the `ros4hri` profile declares as an output.
- 7730edb: The `spawn` node takes an optional `when` input: a new value starts a run even when the arguments are unchanged. `play_viseme` takes an optional `duration`, and the ROS4HRI mapping plays each streamed viseme for its duration and starts a new run for each new time, so a repeated shape plays again.

## 5.0.0

### Major Changes

- ae39c50: The device runs on arora 12.2. A write from a client must be of the type the key's meta states: a value of another type is refused, with no conversion, and the update writes nothing. `describeMethods()` lists the functions every Arora device's HAL module exports: `models`, `model_glb` and `model_glbs`. In the browser the face's model is not stated, because the page already holds the face it loaded.

### Minor Changes

- d4b9ffc: A face's behavior evaluates its sources in the order they compose, and of two writers of one path the later wins: the face's own graphs, then the mappings, then the animations, then the playing program. A playing program now overrides a playing animation on a path both write; the sources used to evaluate in the order of their prefixes.
- 1d812ca: The `ros4hri` mapping plays the streamed viseme: each new `standard/ros4hri/viseme` code spawns a `play_viseme` run of its shape, which takes over from the run before it, and `sil` lets the lips settle at rest. The mapping writes no lip key itself, so a stream and the device's own `say` share the lips the way players do.

### Patch Changes

- e355a0b: The evaluation order is the graph's alone: where two outputs write one path, the later-listed one wins every time, instead of a winner that changed between loads. In a device, a task run writes after the main behavior and a later run after an earlier one, and grafting or pruning a run no longer re-plans the whole graph.
- b85594c: A face comes into view only once it is indexed, posed and shaded. A load or a reload no longer shows one frame of the GLB as it spawned, and during a reload the face being replaced is the only one drawn until the swap.

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
