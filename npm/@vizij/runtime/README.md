# @vizij/runtime

Vizij faces in the browser: the Bevy view and the Arora device in one wasm
module, built from the [`vizij`
crate](https://github.com/vizij-ai/vizij-rs/tree/main/crates/vizij) (its
`web` module). A page mounts its canvas once — one App for the page's
lifetime — then loads as many faces as it shows. Each face is a device of
its own: an [`arora`](https://crates.io/crates/arora) over a blackboard
store, a rig HAL and the face's composed graphs, with the animation, gaze
and viseme modules linked in, exposed through
[`arora-web`](https://crates.io/crates/arora-web)'s surface. The view draws
each face into the rectangle of the canvas the page places it in, reading
its device's pose every frame.

## Use

```ts
import { init, mount, loadFace, placeFaceIn, whenReady, unloadFace, runStatus } from "@vizij/runtime";

await init();
await mount("#faces"); // the canvas; transparent wherever no face draws

const glb = new Uint8Array(await (await fetch("/faces/Quori_Current_Extended.glb")).arrayBuffer());
const face = await loadFace("quori", glb); // composes the face's graphs, starts its device
placeFaceIn("quori", document.getElementById("quori-slot")!, canvas); // where it draws
await whenReady("quori"); // its scene is indexed; the pose shows from here on
face.run(); // the device paces itself (or call face.step(dtMs) per frame)

// any time — the device's store stays live while it runs:
face.setValue(face.path("standard/vizij/expression/happy"), 1);
const run = await face.spawn({ id: SAY_ID, args: [{ id: SAY_TEXT_PARAM_ID, value: { str: "Hello" } }] });
runStatus(face.readValues([run.status])[run.status]); // "running" | "success" | "failure"
await face.halt(run);

unloadFace("quori"); // the scene, the camera, the GLB
face.dispose();
```

A face's paths are its own: `face.rigPrefix` is `rig/<faceId>/` and
`face.path(relative)` builds one. `drainPicks()` reports clicks on faces as
`{ faceId, elementId }` by the ids the GLB's RobotData declares;
`describe(glb)` reads a GLB's elements, animatables, bounds and programs
without loading it.

`startRuntime(graphSpec)` gives a device with no face — a graph on a store,
nothing drawn — for a bench or a graph run in Node; every `Device` method
works on it.

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
