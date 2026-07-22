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

Values cross the boundary in the normalized `ValueJSON` vocabulary from
[`@vizij/value-json`](https://www.npmjs.com/package/@vizij/value-json);
`setValue`/`writeValues` accept its `ValueInput` shorthands.

## Build

The `pkg/` wasm artifacts are produced by `wasm-pack` from the repository root:

```sh
pnpm run build:wasm:arora-web   # wasm-pack build -> npm/@vizij/runtime/pkg
pnpm --filter @vizij/runtime run build
```
