# @vizij/runtime

Run a Vizij runtime in the browser.

The wasm module (built from
[`crates/interop/vizij-arora-web`](https://github.com/vizij-ai/vizij-rs/tree/main/crates/interop/vizij-arora-web))
composes an [`arora`](https://crates.io/crates/arora) device over the Vizij
interop seams — a blackboard store, a rig HAL, and your node graph as its
behavior — and wraps it with
[`arora-web`](https://crates.io/crates/arora-web)'s browser JS surface. One
runtime, one store: the graph reads its `input` nodes' paths from the store
each tick and writes its outputs back, and JS talks to the same store.

## Use

```ts
import { init, startRuntime } from "@vizij/runtime";

await init();
const runtime = await startRuntime(graphSpec); // a Vizij graph spec (object or JSON)
runtime.run(); // the runtime paces itself from here on (the promise only ever rejects)

// any time — the store surface stays live while the runtime runs:
runtime.setValue("sensor/x", { f32: 0.75 });
const changes = runtime.drainChanges(); // path -> ValueJSON | null
// The FIRST drain returns the store's whole current state.

// swap the running graph in place (store, modules, runtime all survive):
await runtime.loadGraph(otherGraphSpec);
```

A host with its own clock skips `run()` and calls `runtime.step(dtMs)` per
frame instead (e.g. from `requestAnimationFrame` timestamps); `step()`
becomes unavailable once `run()` has taken the runtime.

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

The `pkg/` wasm artifacts are produced by `wasm-pack` from the repository root:

```sh
pnpm run build:wasm:arora-web   # wasm-pack build -> npm/@vizij/runtime/pkg
pnpm --filter @vizij/runtime run build
```
