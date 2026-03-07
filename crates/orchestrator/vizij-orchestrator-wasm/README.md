# vizij-orchestrator-wasm

> `wasm-bindgen` bridge for Vizij's orchestrator runtime.

`vizij-orchestrator-wasm` exposes `vizij-orchestrator-core` to JavaScript environments. The crate itself is the low-level binding; most consumers should use the npm wrapper in [`npm/@vizij/orchestrator-wasm`](../../../npm/@vizij/orchestrator-wasm/README.md).

## Overview

- Builds a `VizijOrchestrator` class around the Rust `Orchestrator`.
- Supports controller registration, graph replacement/export, blackboard input staging, stepping, and controller listing/removal.
- Exposes `normalize_graph_spec_json` for GraphSpec normalization.
- Exposes `abi_version()` for wrapper compatibility checks.

## Exported Surface

### `VizijOrchestrator`

Methods exported by the wasm class:

- `new({ schedule? })`
- `register_graph(cfg)`
- `replace_graph({ id, spec, subs? })`
- `export_graph(id)`
- `register_merged_graph(cfg)`
- `register_animation(cfg)`
- `prebind(resolver)`
- `set_input(path, value, shape?)`
- `remove_input(path)`
- `step(dt)`
- `step_delta(dt, sinceVersion?)`
- `list_controllers()`
- `remove_graph(id)`
- `remove_animation(id)`

### Top-level functions

- `normalize_graph_spec_json(json)` returns canonical GraphSpec JSON as a JS string.
- `abi_version()` returns the current binding ABI (`2`).

## Build

Preferred command from the repository root:

```bash
pnpm run build:wasm:orchestrator
```

Manual equivalent:

```bash
wasm-pack build crates/orchestrator/vizij-orchestrator-wasm \
  --target web \
  --out-dir npm/@vizij/orchestrator-wasm/pkg \
  --release \
  --features urdf_ik
```

## Usage

```ts
import init, { VizijOrchestrator, abi_version } from "@vizij/orchestrator-wasm";

await init();
console.log("ABI version", abi_version());

const orchestrator = new VizijOrchestrator({ schedule: "SinglePass" });
const graphId = orchestrator.register_graph({ spec: { nodes: [], edges: [] } });

orchestrator.set_input("demo/input/value", { float: 1.0 }, null);
const frame = orchestrator.step(1 / 60);
console.log(graphId, frame.merged_writes);
```

Use `replace_graph` when the running graph changes structurally. `step_delta` is the incremental stepping variant used by the npm wrapper's diffed frame API.

## Development And Testing

```bash
cargo test -p vizij-orchestrator-wasm
pnpm --filter @vizij/orchestrator-wasm test
```

The npm package test builds the wrapper and runs the compiled Node test bundle against the generated wasm package.

## Related Packages

- [`vizij-orchestrator-core`](../vizij-orchestrator-core/README.md)
- [`npm/@vizij/orchestrator-wasm`](../../../npm/@vizij/orchestrator-wasm/README.md)
