# @vizij/animation-module

The vizij animation engine ([`vizij-animation-core`](https://github.com/vizij-ai/vizij-rs/tree/main/crates/animation/vizij-animation-core)) packaged as an **Arora wasm module**, shipped as importable assets: the built `wasm32-wasip1` executable, its Arora header (JSON) and its Arora records (YAML). Any Arora runtime can load it; in the browser, pass it to `@vizij/runtime`'s `startDevice` so the device's engine hosts it and its functions (`load_animation`, `create_player`, `add_instance`, `step`, …) become callable.

```ts
import { startDevice } from "@vizij/runtime";
import { loadAnimationModule } from "@vizij/animation-module";

const device = await startDevice(graph, undefined, [await loadAnimationModule()]);
const result = JSON.parse(await device.call({ id: LOAD_ANIMATION_FN, args: [clipArg] }));
```

## Surface

- `loadAnimationModule(): Promise<{ headerJson, wasmBytes, records }>` — the artifact, read from the package (Node) or fetched (browser, via the `browser` export condition).
- `records` — the module's Arora records: each record file's path under `records/` (`module/<id>@<version>.yaml`, `structure/<id>@<version>.yaml`, `folder/<id>.yaml`) mapped to its YAML, the map `arora-web`'s `withRecords` takes. A header names a structure by id alone, so a device loading the module from its header alone does not describe the exports taking or returning one (`step_values`, `player_states`, `output_keys`, `load_animation`, the bakes), and a behavior cannot call them; given these records, it describes every export. They are the module record and the records of the structures its functions name, in the `vizij` scope. The module is named `animation`, so its scoped name is `@vizij/animation` and its record path `vizij.animation`; that is the Arora module's name, not this npm package's.
- `headerUrl` / `wasmUrl` — the artifact assets' URLs, for loaders that stream them themselves.

## Provenance

`artifact/` is produced by this package's build from the [`vizij-animation-module`](https://github.com/vizij-ai/vizij-rs/tree/main/crates/interop/vizij-animation-module) crate: `cargo build -p vizij-animation-module --target wasm32-wasip1 --release`, plus the crate's generated module header converted to JSON (its `header` example) and its records written in a module directory's `records/` layout (its `records` example). A device's module directory takes `artifact/` as it is: `header.json`, the executable and `records/`.
