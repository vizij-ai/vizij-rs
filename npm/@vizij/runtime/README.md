# @vizij/runtime

Vizijs in the browser: the Bevy view and the Arora device in one wasm
module, built from the [`vizij`
crate](https://github.com/vizij-ai/vizij-rs/tree/main/crates/vizij) (its
`web` module). A Vizij is what the authoring app exports: a GLB carrying a
rig, its graphs and programs — a face, most often. A page mounts its canvas
once — one App for the page's lifetime — then loads as many Vizijs as it
shows. Each is a device of its own: an
[`arora`](https://crates.io/crates/arora) over a blackboard store, a rig HAL
and the Vizij's composed graphs, with the animation module and the skills
linked in, exposed through
[`arora-web`](https://crates.io/crates/arora-web)'s surface. The view draws
each Vizij into the rectangle of the canvas the page places it in, reading
its device's pose every frame. The module is one artifact, 24.3 MB (6.9
MB gzip, 4.3 MB brotli): a page that only runs a device or reads the
registries fetches the view with it.

## Use

```ts
import { init, mount, loadVizij, placeVizijIn, whenReady, unloadVizij, runStatus } from "@vizij/runtime";

await init();
await mount("#vizijs"); // the canvas; transparent wherever no Vizij draws

const glb = new Uint8Array(await (await fetch("/faces/Quori_Current_Extended.glb")).arrayBuffer());
const quori = await loadVizij("quori", glb); // composes its graphs, starts its device
placeVizijIn("quori", document.getElementById("quori-slot")!, canvas); // where it draws
await whenReady("quori"); // drawn on the GPU; the pose shows from here on
quori.run(); // the device paces itself (or call quori.step(dtMs) per frame)

// any time — the device's store stays live while it runs:
quori.setValue(quori.path("standard/vizij/expression/happy"), 1);
const run = await quori.invoke("say", { text: "Hello" }); // a task method: its run's RunHandle
runStatus(quori.readValues([run.status])[run.status]);  // "running" | "success" | "failure"
await quori.halt(run.run);

unloadVizij("quori"); // the scene, the camera, the GLB
quori.dispose();
```

A `Runtime` is a client of the Vizij's device with the surface any client of
an Arora device has — a page here, a remote on a bridge, Semio Studio:

- `call(call)` calls a module function by id; `invoke(method, args)` calls a
  method the device describes by name, its arguments by parameter name, and
  a task method (one returning a status) starts a run and resolves to its
  `RunHandle` (`{ run, status, feedback, result, update }`);
- `spawn(call)` and `halt(runId)` are the interpreter's SPAWN and HALT;
- `listKeys(prefix?)` and `describeMethods(prefix?)` list the keys the
  device holds, with their meta, and the methods it describes. A face's free
  input states its type, range and rest there. A number's range is its
  input node's `params.min` and `params.max` (optional, used when both are
  there and `max` exceeds `min`), else the `range` the rig declares for it in
  `metadata.vizij.inputs`, else `[0, 1]`; a key a shipped profile declares
  takes that profile's;
- `readValues`, `writeValues`, `setValue`, `snapshot` and `drainChanges`
  read and write its store;
- `loadGraph(spec)` and `applyGraphEdits(edits)` are the interpreter's LOAD
  and EDIT of a Vizij graph. A graph that would fail every evaluation is
  refused, and the promise rejects naming the node at fault: a node leaving
  a `required` input (as `@vizij/node-graph`'s registry marks it) with
  neither an edge nor an inline default, an edge to a missing node, two
  edges into one input, two nodes sharing an id, or a cycle. The running
  graph and its runs go on as they were.

Each operation that changes the device (`call`, `invoke`, `spawn`, `halt`,
`loadGraph`, `applyGraphEdits`) is queued before it returns, and its promise
resolves once a step has applied it. Just `await` it:
- **No `run()` going:** a step with no time in it, a `step(0)`, applies it in
  the next microtask. A device nothing steps settles every operation, so
  `await quori.loadGraph(spec)` resolves with the new graph already run once.
  The operations and the `setValue`/`writeValues` of one turn share that
  step, applied in the order they were issued. A page that calls `step`
  in the same turn applies them with its own step instead.
- **Under `run()`:** the run's next step applies it, and the page adds no
  step.
- **`listKeys` and `describeMethods`:** these never step. They answer at
  once from the device as it is, and don't see an operation still queued.

A call issued from inside a step (from a page callback the device calls,
such as the `audio` hook's playhead) is an ordinary call. A step can't nest
another, so the next step applies it: a `step(0)` once the current step has
ended, or under `run()` the run's next step. A write from inside a step is
carried by the current step, or by the next one if it lands after the
current step has written out its changes.

A `step(0)` advances no time: it carries changes through the
Vizij's graphs and runs and moves nothing that time drives —
clocks, oscillators, noise, springs, dampers, slews and animations hold — so
a paused Vizij shows each edit as it is made, wherever the face computes
from it without time. An edit whose way to the rig crosses a spring, a
damper or a slew becomes that smoother's new target, which shows once time
passes. A value crosses one store key
per step on its way from one of the Vizij's graphs to the next, so an edit
can take a few zero steps to reach the rig. A `say` run follows its audio,
which plays in real time. What the face does, it does through the methods its
modules and its interpreter declare: `say`, `look_at` (ROS4HRI's gaze, see
[Profiles and mappings](#profiles-and-mappings)) and `play_viseme` (the
skills), `run_behavior` (programs), the animation module's functions,
and the rest module's `reset` — every key back to the value it rests at
(each free input of the face's graphs and programs to its authored default,
each input the bundle's neutral pose names to its neutral) — and
`reset_keys`, for the keys it names.

A Vizij's paths are its own: `quori.rigPrefix` is `rig/<faceId>/` (the
bundle's `faceId`) and `quori.path(relative)` builds one. `drainPicks()`
reports pointer presses as `{ vizijId, elementId }` — the slot and the
element id the GLB's RobotData declares, `null` for a press on no element;
`describe(glb)` reads what a GLB declares without loading it: its elements,
animatables, bounds, graphs (each with its id and kind) and programs (each
with its label and graph), and from
its bundle the poses and their groups, the rig's inputs with their ranges
and defaults, the animations, and the bundle's metadata as authored
(`speechConfig`, `activeMotionGraphId`, …) — everything a page builds its
controls from.

Vizijs share the page's canvas, not its framing:
`setView("quori", { bounds, fit, zoom, toneMapping })` frames and tone-maps
one Vizij over `mount`'s options, and `safeArea("quori")` says where its
framed bounds lie on the canvas, in CSS pixels, for a DOM overlay.

Where a Vizij draws is its rectangle of the canvas:

- `placeVizij(id, { x, y, width, height })` (or `placeVizijIn(id, element,
  canvas)`, called again as the page scrolls or its layout changes) takes any
  rectangle. The Vizij is framed on the whole rectangle, and the part on the
  canvas draws: a slot scrolled half past the canvas's edge shows half its
  Vizij, at the position and scale the whole slot gives it.
- An empty rectangle (a width or height of 0 or less) or one wholly off the
  canvas draws nothing, and its Vizij takes no picks.
- A Vizij never placed, or given back with `fillCanvas(id)`, draws over the
  whole canvas — the single face of a page.
- Wherever no Vizij draws, the canvas shows `mount`'s `background`, and is
  transparent without one.

### Programs

A Vizij's programs are the motiongraphs its bundle carries
(`describe(glb).programs`), and any graph a page defines. A program runs
beside the face's graph as a task run of the device's interpreter: its
`run_behavior(name, behavior)` method, started and halted like any task run.

```ts
import { behaviorValue, outputKeys, runEdits, BEHAVIOR_RUNS, runStatus } from "@vizij/runtime";

const { programs } = await describe(glb);
const main = programs.find((p) => p.id === "authoring.motiongraph.main")!;
const live = await quori.invoke("run_behavior", { name: main.id, behavior: await behaviorValue(main.graph) });
await quori.applyGraphEdits(await runEdits(live.run, main.graph, editedGraph)); // in place
await quori.halt(live.run);                          // its outputs hold their last values
const outputs = await outputKeys(main.graph);       // the keys its run leaves holding
await quori.invoke("reset_keys", { keys: { strs: outputs } }); // back to rest, when the page says so

const runs = await quori.listKeys(BEHAVIOR_RUNS);   // every run: <run id>/status and <run id>/name
```

- `loadVizij`'s `program` option (the bundle's active program by default)
  names the program that runs from load, as a run named after the program.
- A run's state is its status key: `running` until halted, `failure` once
  halted. Its name is a key of its own beside it, so any client tells which
  program a run runs. An ended run's keys stay.
- Halting removes the program's nodes and leaves the store as it is.
  Starting it again is a new run, its stateful nodes (springs, smoothing)
  starting afresh. A program that wants to act on its start does so itself;
  returning its outputs to rest is the page's explicit `reset_keys` of
  `outputKeys(graph)`: the keys its graph's output nodes write, read as the
  device reads them — any form the spec normalizer accepts — and without the
  outputs on `task/…` paths, which write the run's own keys. Any graph a
  page runs has them, the bundle's programs and its own alike.
- `runEdits(runId, from, to)` builds the edits that take the run from
  `from` — the graph it runs — to `to`: a run's nodes live in the device's
  graph under ids the interpreter derives from the run, and the edits name
  them, so the nodes `to` keeps keep their runtime state.
- Two programs that write one key both write it each step; which value
  stands is not defined yet.

### Authoring

An authoring viewport edits what the view shows while the device runs:

```ts
import { drainPicks, setSelection, setStaticFeature, loadVizij } from "@vizij/runtime";

for (const { vizijId, elementId } of drainPicks()) {
  setSelection(vizijId, elementId ? [elementId] : []); // a miss clears the selection
}
quori.hold([mouthOpenId, `${eyeOffsetId}:x`]); // keep what shows while the device writes on
quori.release();                               // follow the device again
const weight = quori.path("poses/pose_d_happy_d.weight");
quori.holdKeys([weight]);   // the face's behavior no longer writes it…
quori.setValue(weight, 0.8); // …so the edit stands, and drives the pose
quori.releaseKeys();         // its writers write it again from the next step
setStaticFeature("quori", plateId, "color", { r: 0.2, g: 0.4, b: 0.8 }); // in place, no reload

// A structural edit: the authoring world exported to GLB bytes, reloaded.
const next = await loadVizij("quori", exportedGlb);
quori.dispose(); // the previous scene drew until the new one was ready
```

- A pick is found over each element's mesh as drawn, its morph targets
  applied, the nearest along the pointer's ray; a press in a Vizij's
  rectangle that meets no element is that Vizij's miss.
- The selection glow outlines each selected element's mesh along its
  feature edges (its boundary and its creases), drawn over the face in the
  selection red; a selected group glows every element mesh under it.
- A hold is the view declining the device's writes to an output, whole
  (`<animatableId>`) or per component (`<animatableId>:x` … `:z`, `:r` …
  `:b`); the device and its store run on untouched.
- Selection and holds belong to the Vizij's slot: they carry over a reload
  and go with `unloadVizij`.
- `holdKeys(keys)` holds store keys — rig inputs, pose weights, a program's
  inputs, any key the face's behavior writes — at the values clients write:
  from the next step until `releaseKeys(keys?)`, the face's graphs, its
  program and skill runs and its animations leave them alone, while
  `setValue`, `writeValues` and the rest module's `reset` still write them.
  What the graphs compute from a held key follows the client's value, and a
  released key the behavior writes every step takes its writer's value at
  the next step, which the release asks for as an operation does (a
  `step(0)` on a device nothing steps, the run's next step under `run()`). Holding a key the behavior does not write changes nothing.
  Keys are full store paths (`quori.path(…)`). Key holds are the device's,
  one set whoever holds: a reloaded Vizij starts with none.

`startRuntime(graphSpec)` gives a device with no Vizij — a graph on a store,
nothing drawn — for a bench or a graph run in Node; every `Runtime` method
works on it.

Arora wasm modules load into a device as guests: `startRuntime(graph, init,
modules)` and `loadVizij`'s `options.modules` take `AroraModule`s —
`{ headerJson, wasmBytes }`, what `@vizij/animation-module`'s
`loadAnimationModule()` returns for its artifact; their functions are
reachable from `runtime.call` and `runtime.invoke` and
from the graph's `ExternalFunction` nodes, like the host-linked modules'. A
guest under a host-linked module's id — the animation module's — is served
by the host-linked one.

### Animations

A device's animations live in its animation module, and every client loads,
plays and unloads them through the module's declared functions, by name. A
Vizij's own animations (its bundle's `animations`, what
`describe(glb).animations` lists) load when the face loads, unless
`loadVizij(id, glb, { animations: false })`: each on a player named after the
animation's id, its instance at weight 0, stopped — silent until a client
gives it a weight and plays it. The player states the animation source writes
to `vizij/animations/players` each step say which player plays what and
where each stands; `decodePlayerStates` reads them. Each track writes the rig
input its channel names (`gaze/left_right` drives
`quori.path("gaze/left_right")`).

```ts
import { ANIMATION_PLAYERS_PATH, decodePlayerStates } from "@vizij/runtime";

const players = () => decodePlayerStates(quori.readValues([ANIMATION_PLAYERS_PATH])[ANIMATION_PLAYERS_PATH]);
const { player, instances } = players().find((p) => p.name === "wave")!;
const u32 = (n: number) => ({ u32: n });

await quori.invoke("set_weight", { player: u32(player), instance: u32(instances[0].instance), weight: { f32: 1 } });
await quori.invoke("play", { player: u32(player) });
await quori.invoke("set_speed", { player: u32(player), speed: { f32: 2 } }); // kept across pause and play
await quori.invoke("pause", { player: u32(player) });                         // holds the pose
await quori.invoke("seek", { player: u32(player), time_ns: { u64: 1.5e9 } });
await quori.invoke("set_loop", { player: u32(player), mode: "once" });        // "loop", "ping_pong"
await quori.invoke("stop", { player: u32(player) });                          // back to its first frame
await quori.invoke("set_weight", { player: u32(player), instance: u32(instances[0].instance), weight: { f32: 0 } }); // silent

// a page's own animation, in describe's shape:
const { moduleAnimation } = quori.moduleAnimation(animation);
const anim = await quori.invoke("load_animation", { clip: moduleAnimation });   // { u32 }
const created = await quori.invoke("create_player", { name: animation.id });   // { u32 }
await quori.invoke("add_instance_with_weight", { player: created, anim, weight: { f32: 0 } });
await quori.invoke("reload_animation", { anim, clip: edited });               // in place: the playback stays
await quori.invoke("remove_player", { player: created });
await quori.invoke("unload_animation", { anim });                              // its keys keep their last values
```

A stopped player still writes its first frame at its weight: a client that
wants an animation to let go of its keys gives it weight 0. The calls apply
in the order they were issued.
`moduleAnimation(animation)` turns an animation in `describe`'s shape into
the module's `AnimationClip`, each track keyed by the store key its channel
names through this Vizij's rig, as the Vizij's own are loaded.

`channelKeys(gltf, channels)` names those keys from a Vizij's glTF JSON
document, without loading it — its `VIZIJ_bundle` read as `composeVizij`
reads it: one key per channel, in order, the key `moduleAnimation` gives the
track once the Vizij is loaded. It is what a page drives on a `startRuntime`
device over `composeVizij`'s graph, to move it as the Vizij's animations
move the loaded Vizij; that device has no Vizij, so its own
`moduleAnimation` keys a channel as is.

```ts
import { channelKeys, composeVizij, startRuntime } from "@vizij/runtime";

const bench = await startRuntime(await composeVizij(gltf, { program: "none" }));
const keys = await channelKeys(gltf, animation.tracks.map((track) => track.channel));
// ["rig/quori_latest/gaze/left_right", …]: write each track's samples there
```

A channel resolves through the bundle's rig, to one key:

- itself, when it is a rig input's path;
- the rig input at `rig/<faceId>/<channel>`;
- the input the rig names it by: its input node's id without `input_`, or
  that id's `direct_` or `pose_control_` alias (`gaze_left_right` names
  `input_direct_gaze_left_right`);
- else `rig/<faceId>/<channel>` regardless, a key some other graph may read
  (the bare channel when the bundle names no `faceId`).

A channel under `rig/` is never prefixed again. `channelKeys` rejects a
document that carries no `VIZIJ_bundle`.

The types are `Animation`, `AnimationTrack` and `AnimationKeyframe` (what
`describe` lists), `ModuleAnimation`, `PlayerState` and `InstanceState`.
`Animation` is `@vizij/runtime`'s: importing it shadows the DOM's Web
Animations `Animation` in that module.

### Profiles and mappings

A **profile** is an interface: the set of store paths one party exposes to
another, each with its type, range, and default. A **mapping** is a graph that
implements one profile in terms of another — Vizij ships one, the [ROS4HRI
mapping](https://github.com/vizij-ai/vizij-rs/blob/main/docs/ros4hri.md),
which consumes the `ros4hri` profile and produces the `vizij-face` profile.
The model is laid out in
[profiles and mappings](https://github.com/vizij-ai/vizij-rs/blob/main/docs/profiles-and-mappings.md).

```ts
import { profiles, profile, mappings, mapping } from "@vizij/runtime";

profiles();                               // [{ id, version, title, description, scope, keys }]
const face = profile("vizij-face", "rig/<faceId>/"); // every path, typed, addressed to the face
const ros = profile("ros4hri");           // device-scoped: absolute paths, no prefix

mappings();                               // [{ id, title, description }] — the opt-in menu
const graph = mapping("ros4hri", "rig/<faceId>/"); // the mapping graph as an object
```

A loaded Vizij speaks the vizij standard: its `standard/vizij/*` controls,
through the bundle's `standard-adaptation`. Its program and its clients
blink it through `standard/vizij/blink` and move its eyes through
`standard/vizij/{left,right}_eye/pos/{x,y}`.

The ROS4HRI mapping is for a page that drives the face as a ROS robot does.
It composes when asked for — `loadVizij(id, glb, { ros4hri: true })` (or
`composeVizij`'s option of the same name) — and the device-scoped
`standard/ros4hri/*` keys then drive the face too. The mapping brings two
behaviours of its own: an idle blink, and the gaze of the `look_at` skill,
whose ROS4HRI gaze target it turns into eye positions. A face that embeds its
own `standard::ros4hri` copy runs that copy either way.

`profile(id, rigPrefix)` prefixes only a `face`-scoped profile; a `device`
profile such as `ros4hri` comes back unchanged. `mapping(id, rigPrefix)`
returns the mapping's node-graph with its written control paths prefixed for
the target face, ready to compose into a graph spec or embed into a GLB via the
bundle tool. `standardProfiles()` / `standardProfile()` remain as deprecated
aliases of `mappings()` / `mapping()`.

## Loading

`init()` fetches and compiles the wasm module; `onInitProgress` follows it,
for a page that shows its own loading state:

```ts
import { init, onInitProgress } from "@vizij/runtime";

const stop = onInitProgress((p) => {
  if (p.stage === "download") bar.value = p.total ? p.loaded / p.total : NaN; // NaN: indeterminate
  else label.textContent = p.stage; // "compile", "start", "ready"
});
await init();
stop();
```

The listener is called with the current stage at once, then with each
change: `download` with the bytes received (`total` is the response's
`Content-Length`, `null` when it is unknown or counts compressed bytes), at
most every 50 ms and once more with the final count; `compile` once the
last byte is in, while the module compiles and instantiates; `start` once
it is instantiated and its own start has run; `ready` once `init()` has
resolved. On a visible page each of the last three stays for one painted
frame, which is what that costs `init()`. Building the page's App is
`mount()`'s work and showing a Vizij is `loadVizij()`'s, both after `ready`
— and so is most of the compile: V8 compiles a function the first time it
is called. A module passed to `init` as bytes or as a compiled
`WebAssembly.Module` has no download.

The module compiles while it downloads (`WebAssembly.instantiateStreaming`
on the fetched response), and Chrome keeps the compiled code with the HTTP
cache entry of the module's URL: the next load of an unchanged module
deserializes it instead of compiling it again. What a host does to get both:

- **A content-hashed URL.** Bundlers emit one for
  `new URL("…/vizij_bg.wasm", import.meta.url)`, which is how the package
  finds its module: a new version is a new URL, and the old one never
  changes under its cache entry.
- **`Cache-Control: public, max-age=31536000, immutable`** on that URL, so
  the response comes from the HTTP cache, which holds the compiled code.
- **`Content-Type: application/wasm`.** Under another type the browser
  refuses to compile while streaming; the module still loads, from the
  whole response once it has arrived.
- **Precompressed brotli (`.br`)**, served with `Content-Encoding: br`: the
  module is 24.3 MB raw, 6.9 MB gzip at level 9 and 4.3 MB brotli at level
  11. A server compressing on the fly uses a much lower level than these.

## Build

The `pkg/` wasm artifacts are produced by `wasm-pack` from the repository root
(one WebGL2 bundle of the `vizij` crate without its desktop half, linked by
the `vizij-wasm` crate with fat LTO; a cold build takes many minutes, Bevy and
the LTO link are most of it):

```sh
pnpm run build:wasm:runtime     # wasm-pack build -> npm/@vizij/runtime/pkg
pnpm --filter @vizij/runtime run build
```
