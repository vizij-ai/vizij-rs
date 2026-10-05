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
const run = await quori.spawn({ id: SAY_ID, args: [{ id: SAY_TEXT_PARAM_ID, value: { str: "Hello" } }] });
runStatus(quori.readValues([run.status])[run.status]); // "running" | "success" | "failure"
await quori.halt(run);

unloadVizij("quori"); // the scene, the camera, the GLB
quori.dispose();
```

A Vizij's paths are its own: `quori.rigPrefix` is `rig/<faceId>/` (the
bundle's `faceId`) and `quori.path(relative)` builds one. `drainPicks()`
reports pointer presses as `{ vizijId, elementId }` — the slot and the
element id the GLB's RobotData declares, `null` for a press on no element;
`describe(glb)` reads what a GLB declares without loading it: its elements,
animatables, bounds and programs (with their labels), and from its bundle the
poses and their groups, the rig's inputs with their ranges and defaults, the
animations, and the bundle's metadata as authored (`speechConfig`,
`activeMotionGraphId`, …) — everything a page builds its controls from.

`quori.reset()` returns the face to rest: each free input of its graphs and
programs to its authored default, each input the bundle's neutral pose names
to its neutral; `quori.reset(keys)` returns only the keys it names. Vizijs
share the page's canvas, not its framing:
`setView("quori", { bounds, fit, zoom, toneMapping })` frames and tone-maps
one Vizij over `mount`'s options, and `safeArea("quori")` says where its
framed bounds lie on the canvas, in CSS pixels, for a DOM overlay.

### Programs

A Vizij's programs are the motiongraphs its bundle carries (`describe(glb)`
lists their ids), and any graph a page defines. A program runs beside the
face's graph as a task run of the device's interpreter — its
`run_behavior` method, spawned and halted like any task run — so every
client of the device starts, stops and watches programs the same way: a
page through this package, a remote through a bridge.

```ts
const live = await quori.spawnProgram("authoring.motiongraph.main"); // a TaskHandle; several run at once
const editor = await quori.spawnProgram(editorGraph, "editor");       // a page's graph, named
quori.programRuns();             // [{ handle, name, status }] — read off the device's store
await quori.editProgram(editor, editorGraph, editedGraph);           // in place
await quori.halt(live);          // its outputs hold their last values
await quori.reset(quori.programOutputs("authoring.motiongraph.main")); // back to rest, when the page says so
```

- `loadVizij`'s `program` option (the bundle's active program by default)
  names the program that runs from load; `programRuns()` finds its run by
  the program's id, to halt it like any other.
- A run's state is its status key: `running` until halted, `failure` once
  halted. Its name is a key of its own beside it, so any client tells which
  program a run runs.
- Halting removes the program's nodes and leaves the store as it is.
  Starting it again is a new run, its stateful nodes (springs, smoothing)
  starting afresh. A program that wants to act on its start does so itself;
  returning its outputs to rest is the page's explicit `reset(keys)`.
- `editProgram(handle, from, to)` takes the run from `from` — the program it
  runs — to `to`: the nodes `to` keeps keep their runtime state.
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
reachable by id from `runtime.call` and
from the graph's `ExternalFunction` nodes, like the host-linked modules'. A
guest under a host-linked module's id — the animation module's — is served
by the host-linked one.

### Animations

A device's animations live in its animation module, and every client loads,
plays and unloads them through the module's declared functions: the
`Runtime`, a client on a bridge, Semio Studio. Each animation plays on a
player of its own, named after the animation's id, so any client finds it in
the player states the animation source writes to `vizij/animations/players`
each step. A Vizij's own animations (its bundle's `animations`, what
`describe(glb).animations` lists) load when the face loads, unless
`loadVizij(id, glb, { animations: false })`; more load at any time. Each
track writes the rig input its channel names (`gaze/left_right` drives
`quori.path("gaze/left_right")`). An animation loads silent, stopped at its
start, looping at speed 1; it writes its keys while it plays, is paused or
has completed, and stops writing once stopped.

```ts
quori.animations();                                  // [{ id, duration }] — seconds; whoever loaded them
await quori.loadAnimation(animation);                // describe's shape → { id, name, duration }
await quori.playAnimation(id, { reset: true });      // from the start; { speed } too
quori.pauseAnimation(id);                            // holds the pose
quori.seekAnimation(id, 1.5);                        // seconds
quori.setAnimationLoop(id, false);                   // completes at its end instead of wrapping
quori.setAnimationSpeed(id, 2);
quori.animationState(id);                            // { time, duration, playing, loop, speed, completed }
quori.stopAnimation(id, { clearOutputs: true });     // back to its first frame, then silent
await quori.unloadAnimation(id);                     // its keys keep their last values
```

`loadAnimation` sends `load_animation`, `create_player`,
`add_instance_with_weight` (at 0) and `stop`; loading an id already loaded
replaces it in one step, keeping the playback the module reports for it —
playhead, loop mode, speed, weight, and whether it plays, is paused or is
stopped — whoever set it: the authoring timeline's live edit.
`unloadAnimation` sends `remove_player` and `unload_animation`. The transport
sends `set_weight`, `play`, `pause`, `stop`, `seek`, `set_speed` and
`set_loop`, and rejects for an id the device does not hold. Each call applies
at the device's next step, which its promise waits for; a load or unload waits
for the transport calls in flight on its id, and they for it. `animations` and
`animationState` read the player states.

The types are `Animation`, `AnimationTrack` and `AnimationKeyframe` (what
`describe` lists), `LoadedAnimation`, `AnimationState`,
`PlayAnimationOptions` and `StopAnimationOptions`. `Animation` is
`@vizij/runtime`'s: importing it shadows the DOM's Web Animations
`Animation` in that module.

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
