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
its device's pose every frame. The module is one artifact, 25 MB (7 MB
gzipped): a page that only runs a device or reads the registries fetches the
view with it.

## Use

```ts
import { init, mount, loadVizij, placeVizijIn, whenReady, unloadVizij, runStatus } from "@vizij/runtime";

await init();
await mount("#vizijs"); // the canvas; transparent wherever no Vizij draws

const glb = new Uint8Array(await (await fetch("/faces/Quori_Current_Extended.glb")).arrayBuffer());
const quori = await loadVizij("quori", glb); // composes its graphs, starts its device
placeVizijIn("quori", document.getElementById("quori-slot")!, canvas); // where it draws
await whenReady("quori"); // its scene is indexed; the pose shows from here on
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
  device holds, with their meta, and the methods it describes;
- `readValues`, `writeValues`, `setValue`, `snapshot` and `drainChanges`
  read and write its store;
- `loadGraph(spec)` and `applyGraphEdits(edits)` are the interpreter's LOAD
  and EDIT of a Vizij graph.

Each operation that reaches the device is enqueued before it returns and
applied at the device's next step, which its promise waits for: under
`run()` just `await` it; a page that drives the device with `step` steps
after issuing it. What the face does, it does through the methods its
modules and its interpreter declare: `say`, `look_at` and `play_viseme`
(the skills), `run_behavior` (programs), the animation module's functions,
and the rest module's `reset` — every key back to the value it rests at
(each free input of the face's graphs and programs to its authored default,
each input the bundle's neutral pose names to its neutral) — and
`reset_keys`, for the keys it names.

A Vizij's paths are its own: `quori.rigPrefix` is `rig/<faceId>/` (the
bundle's `faceId`) and `quori.path(relative)` builds one. `drainPicks()`
reports pointer presses as `{ vizijId, elementId }` — the slot and the
element id the GLB's RobotData declares, `null` for a press on no element;
`describe(glb)` reads what a GLB declares without loading it: its elements,
animatables, bounds and programs (each with its label and graph), and from
its bundle the poses and their groups, the rig's inputs with their ranges
and defaults, the animations, and the bundle's metadata as authored
(`speechConfig`, `activeMotionGraphId`, …) — everything a page builds its
controls from.

Vizijs share the page's canvas, not its framing:
`setView("quori", { bounds, fit, zoom, toneMapping })` frames and tone-maps
one Vizij over `mount`'s options, and `safeArea("quori")` says where its
framed bounds lie on the canvas, in CSS pixels, for a DOM overlay.

### Programs

A Vizij's programs are the motiongraphs its bundle carries
(`describe(glb).programs`), and any graph a page defines. A program runs
beside the face's graph as a task run of the device's interpreter: its
`run_behavior(name, behavior)` method, started and halted like any task run.

```ts
import { behaviorValue, runEdits, BEHAVIOR_RUNS, runStatus } from "@vizij/runtime";

const { programs } = await describe(glb);
const main = programs.find((p) => p.id === "authoring.motiongraph.main")!;
const live = await quori.invoke("run_behavior", { name: main.id, behavior: await behaviorValue(main.graph) });
await quori.applyGraphEdits(await runEdits(live.run, main.graph, editedGraph)); // in place
await quori.halt(live.run);                          // its outputs hold their last values
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
  returning its outputs to rest is the page's explicit `reset_keys` — the
  keys its graph's output nodes write.
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
wants an animation to let go of its keys gives it weight 0. Each call
applies at the device's next step, in the order the calls were issued.
`moduleAnimation(animation)` turns an animation in `describe`'s shape into
the module's `AnimationClip`, each track keyed by the store key its channel
names through this Vizij's rig, as the Vizij's own are loaded.

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

`profile(id, rigPrefix)` prefixes only a `face`-scoped profile; a `device`
profile such as `ros4hri` comes back unchanged. `mapping(id, rigPrefix)`
returns the mapping's node-graph with its written control paths prefixed for
the target face, ready to compose into a graph spec or embed into a GLB via the
bundle tool. `standardProfiles()` / `standardProfile()` remain as deprecated
aliases of `mappings()` / `mapping()`.

## Build

The `pkg/` wasm artifacts are produced by `wasm-pack` from the repository root
(one WebGL2 bundle of the `vizij` crate without its desktop half; a cold
build takes many minutes, Bevy is most of it):

```sh
pnpm run build:wasm:runtime     # wasm-pack build -> npm/@vizij/runtime/pkg
pnpm --filter @vizij/runtime run build
```
