# @vizij/animation-module

The vizij animation engine ([`vizij-animation-core`](https://github.com/vizij-ai/vizij-rs/tree/main/crates/animation/vizij-animation-core)) packaged as an **Arora wasm module**, shipped as importable assets: the built `wasm32-wasip1` executable plus its Arora header (JSON). Any Arora runtime can load it; in the browser, pass it to `@vizij/runtime`'s `startDevice` so the device's engine hosts it and its functions (`load_animation`, `create_player`, `add_instance`, `step`, …) become callable.

```ts
import { startDevice } from "@vizij/runtime";
import { loadAnimationModule } from "@vizij/animation-module";

const device = await startDevice(graph, undefined, [await loadAnimationModule()]);
const result = JSON.parse(await device.call({ id: LOAD_ANIMATION_FN, args: [clipArg] }));
```

## Surface

- `loadAnimationModule(): Promise<{ headerJson, wasmBytes }>` — the artifact, read from the package (Node) or fetched (browser, via the `browser` export condition).
- `headerUrl` / `wasmUrl` — the artifact assets' URLs, for loaders that stream them themselves.

## Provenance

`artifact/` is produced by this package's build from the [`vizij-animation-module`](https://github.com/vizij-ai/vizij-rs/tree/main/crates/interop/vizij-animation-module) crate: `cargo build -p vizij-animation-module --target wasm32-wasip1 --release`, plus the crate's generated module header converted to JSON.
