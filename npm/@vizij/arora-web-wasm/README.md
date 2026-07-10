# @vizij/arora-web-wasm

Run a Vizij runtime in the browser **as an Arora device**.

The wasm module (built from
[`crates/interop/vizij-arora-web`](https://github.com/vizij-ai/vizij-rs/tree/main/crates/interop/vizij-arora-web))
assembles an [`arora-web`](https://crates.io/crates/arora-web) `BrowserRuntime`
over the Vizij interop seams — a blackboard store, a rig HAL, and your node
graph as the device's behavior. One device, one store, one step loop: the
graph reads its `input` nodes' paths from the store each tick and writes its
outputs back, and JS talks to the same store.

## Use

```ts
import { init, startDevice } from "@vizij/arora-web-wasm";

await init();
const device = await startDevice(graphSpec); // a Vizij graph spec (object or JSON)

// each animation frame (dt in ms, e.g. from requestAnimationFrame timestamps):
device.setValue("sensor/x", { f32: 0.75 });
device.step(dtMs);
const changes = device.drainChanges(); // path -> ValueJSON | null

device.dispose();
```

Values cross the boundary in the normalized `ValueJSON` vocabulary from
[`@vizij/value-json`](https://www.npmjs.com/package/@vizij/value-json);
`setValue`/`writeValues` accept its `ValueInput` shorthands.

## Build

The `pkg/` wasm artifacts are produced by `wasm-pack` from the repository root:

```sh
pnpm run build:wasm:arora-web   # wasm-pack build -> npm/@vizij/arora-web-wasm/pkg
pnpm --filter @vizij/arora-web-wasm run build
```
