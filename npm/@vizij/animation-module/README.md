# @vizij/animation-module

Arora's animation module ([`arora-animation-module`](https://github.com/semio-ai/arora-sdk/tree/main/modules/animation)) packaged as importable assets: its module directory — the `wasm32-wasip1` guest, its Arora header (JSON) and its Arora records (YAML). Any Arora runtime can load it as a guest, and its functions (`load_clip`, `create_player`, `add_instance`, `step`, …) become callable. A Vizij device links the same module in natively and serves its id with it, so this package is for the other devices that load it as a guest.

```ts
import { startRuntime } from "@vizij/runtime";
import { loadAnimationModule } from "@vizij/animation-module";

const module = await loadAnimationModule();
const runtime = await startRuntime(graph, undefined, [module]);
const { ret } = await runtime.call({ module_id: MODULE_ID, id: LOAD_CLIP_FN, args: [clipArg] });
```

## Surface

- `loadAnimationModule(): Promise<{ headerJson, wasmBytes, records }>` — the artifact, read from the package (Node) or fetched (browser, via the `browser` export condition).
- `records` — the module's Arora records: each record file's path under `records/` (`module/<id>@<version>.yaml`, `structure/<id>@<version>.yaml`, `folder/<id>.yaml`) mapped to its YAML, the map `arora-web`'s `withRecords` takes. A header names a structure by id alone, so a device loading the module from its header alone does not describe the exports taking or returning one (`step_values`, `player_states`, `output_keys`, `load_clip`, the bakes), and a behavior cannot call them; given these records, it describes every export. They are the module record and the records of the structures its functions name, in the `arora` scope.
- `headerUrl` / `wasmUrl` — the artifact assets' URLs, for loaders that stream them themselves.

## Provenance

`artifact/` is produced by this package's build: `cargo build -p arora-animation-module --target wasm32-wasip1 --release` builds the guest, and the workspace's `vizij-animation-artifact` writes the module directory around it with `arora_animation_module::module_dir::write`. A device's module directory takes `artifact/` as it is: `header.json`, the guest (under the crate's `module_dir::GUEST_FILE` name, which `wasmUrl` names) and `records/`.
