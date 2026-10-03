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
element id the GLB's RobotData declares;
`describe(glb)` reads what a GLB declares without loading it: its elements,
animatables, bounds and programs (with their labels), and from its bundle the
poses and their groups, the rig's inputs with their ranges and defaults, the
animation clips, and the bundle's metadata as authored (`speechConfig`,
`activeMotionGraphId`, …) — everything a page builds its controls from.

### Programs

A Vizij's programs are the motiongraphs its bundle carries (`describe(glb)`
lists their ids). The one `loadVizij`'s `program` option selects — the
bundle's active program by default — plays from load; any of them plays,
pauses and stops while the device runs, several at once, and a page can define
its own:

```ts
await quori.startProgram("authoring.motiongraph.main"); // beside the one already playing
quori.programState("authoring.motiongraph.main");        // "playing" | "paused" | "stopped"
await quori.pauseProgram("authoring.motiongraph.main");  // outputs hold their last values
await quori.stopProgram("authoring.motiongraph.program.1", { resetOutputs: true }); // outputs back to rest

await quori.setProgram("editor", graphSpec); // define a program, or replace a program's graph
await quori.startProgram("editor");
await quori.setProgram("editor", editedSpec); // a playing program changes in place
```

A playing program's nodes are part of the device's one running graph, beside
the base composition; starting, pausing, stopping and replacing edit that
graph in place, so the face is not reloaded and the store keeps every other
value. A replaced program keeps the runtime state of the nodes its new graph
keeps; a paused program plays again from fresh node state. `resetOutputs`
returns each key the program writes to the value the face staged for it at
load (its neutral pose), or clears it, so every input that reads it falls back
to its own default. Each call resolves once its change has landed, at the next
step.

`quori.reset()` returns the face to rest: each free input of its graphs to
its authored default, each input the bundle's neutral pose names to its
neutral. Vizijs share the page's canvas, not its framing:
`setView("quori", { bounds, fit, zoom, toneMapping })` frames and tone-maps
one Vizij over `mount`'s options, and `safeArea("quori")` says where its
framed bounds lie on the canvas, in CSS pixels, for a DOM overlay.

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

### Clips

A Vizij's animation clips (its bundle's `animations`, what `describe(glb).clips`
lists) load into its device's animation module with the device, each track
writing the rig input its channel names (`gaze/left_right` drives
`quori.path("gaze/left_right")`). A clip loads silent, stopped at its start,
looping at speed 1; it writes its keys while it plays, is paused or has
completed, and stops writing once stopped.

```ts
quori.clips();                                  // [{ id, name, duration }] — seconds
await quori.playClip(id, { reset: true });      // from the start; { speed } too
quori.pauseClip(id);                            // holds the pose
quori.seekClip(id, 1.5);                        // seconds
quori.setClipLoop(id, false);                   // completes at its end instead of wrapping
quori.setClipSpeed(id, 2);
quori.clipState(id);                            // { time, duration, playing, loop, speed, completed }
quori.stopClip(id, { clearOutputs: true });     // back to its first frame, then silent
quori.setClip(clip);                            // add, or replace live by id (describe's shape)
quori.removeClip(id);
```

Transport calls are the module's declared functions through `runtime.call`:
each applies at the device's next step, which its promise waits for.
`clipState` reads the player states the animation source writes to
`vizij/animations/players` each step.

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
