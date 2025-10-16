# vizij-orchestrator-wasm

> **wasm-bindgen binding for Vizij’s orchestrator runtime – manage graphs, animations, and blackboard inputs from JavaScript.**

`vizij-orchestrator-wasm` exposes `vizij-orchestrator-core` to JavaScript/TypeScript environments. The npm package `@vizij/orchestrator-wasm` is built from this crate and provides the primary developer-facing API.

---

## Table of Contents

1. [Overview](#overview)
2. [Exports](#exports)
3. [Building](#building)
4. [Usage](#usage)
5. [OrchestratorFrame JSON](#orchestratorframe-json)
6. [Development & Testing](#development--testing)
7. [Related Packages](#related-packages)

---

## Overview

- Compiles to a `cdylib` with `wasm-bindgen`; `abi_version() == 2` guards the npm wrapper against mismatched builds.
- Wraps the Rust `Orchestrator` type in a `VizijOrchestrator` class for JavaScript consumers.
- Supports schedule configuration (`SinglePass`, `TwoPass`, future `RateDecoupled`), controller registration, input staging, stepping, and controller introspection.
- Provides optional helpers to convert core `Value` structures into legacy JSON envelopes when required.

---

## Exports

| Export | Description |
|--------|-------------|
| `class VizijOrchestrator` | Methods: constructor, `register_graph`, `register_merged_graph`, `register_animation`, `prebind`, `set_input`, `remove_input`, `step`, `list_controllers`, `remove_graph`, `remove_animation`. |
| `normalize_graph_spec_json(json: &str) -> String` | Normalises GraphSpec JSON (used internally and exposed for tooling). |
| `abi_version() -> u32` | Returns `2`; npm wrapper enforces this at init time. |
| `utils::value_to_legacy_json` et al. | Convert `Value`/`WriteBatch` into legacy `{ vec3: [...] }` style JSON (handy for older tooling). |

---

## Building

```bash
pnpm run build:wasm:orchestrator      # preferred script
```

Manual build:

```bash
wasm-pack build crates/orchestrator/vizij-orchestrator-wasm \
  --target bundler \
  --out-dir pkg \
  --release
```

The generated `pkg/` is republished via `npm/@vizij/orchestrator-wasm`.

---

## Usage

Via npm wrapper:

```ts
import init, { VizijOrchestrator, abi_version } from "@vizij/orchestrator-wasm";

await init();
console.log("ABI version", abi_version());

const orchestrator = new VizijOrchestrator({ schedule: "SinglePass" });

const graphId = orchestrator.register_graph({ spec: { nodes: [] } });
const mergedGraphId = orchestrator.register_merged_graph({
  graphs: [
    { spec: { nodes: [{ id: "source", type: "constant", params: { value: 1 } }] } },
    {
      spec: {
        nodes: [
          { id: "input", type: "input", params: { path: "shared/value" } },
          { id: "out", type: "output", params: { path: "shared/result" } }
        ],
        links: [
          { from: { node_id: "input" }, to: { node_id: "out", input: "in" } }
        ]
      }
    }
  ]
});
const animId = orchestrator.register_animation({ setup: {} });

// Optional: resolve animation targets
orchestrator.prebind((path) => path.toUpperCase());

orchestrator.set_input("demo/input/value", { float: 1.23 }, null);
const frame = orchestrator.step(1 / 60);
console.log(frame.merged_writes);
```

### Registration payloads

- `register_graph({ id?, spec, subs? })`
  - `spec` must be a canonical `GraphSpec` object (use `normalize_graph_spec_json` or the npm wrapper’s helper first).
  - `subs.inputs` / `subs.outputs` accept arrays of canonical `TypedPath` strings; invalid paths throw a `JsError`.
  - `subs.mirrorWrites` (boolean) mirrors every write from the controller back into the blackboard even when `outputs` filters public results.
- `register_merged_graph({ id?, graphs: GraphConfig[], strategy? })`
  - Each entry in `graphs` mirrors `register_graph`.
  - `strategy.outputs|intermediate` accept `"error"` (default), `"namespace"`, or `"blend"` to control how conflicting paths are handled during the merge.
- `register_animation({ id?, setup? })`
  - `setup` is forwarded to `AnimationControllerConfig::setup` and commonly contains `{ animation, player, instance }` blobs (see core README).

All registration helpers auto-generate an id when omitted (`graph:0`, `anim:0`, etc.).

### Custom `.wasm` location

```ts
import init from "@vizij/orchestrator-wasm";
import { readFile } from "node:fs/promises";

// CDN
await init(new URL("https://cdn.example.com/vizij/orchestrator_wasm_bg.wasm"));

// Desktop (Electron/Tauri)
const bytes = await readFile("dist/orchestrator_wasm_bg.wasm");
await init(bytes);
```

---

## OrchestratorFrame JSON

`step(dt)` returns a plain JS object:

```jsonc
{
  "epoch": 42,
  "dt": 0.016,
  "merged_writes": [
    { "path": "demo/output/value", "value": { "type": "float", "data": 1.0 }, "shape": { "id": "Scalar" } }
  ],
  "conflicts": [ /* conflict logs (serde_json::Value) */ ],
  "timings_ms": { "animations_ms": 1.2, "graphs_ms": 0.7, "total_ms": 1.9 },
  "events": [ /* animation events */ ]
}
```

Values use the same `{ type, data }` envelope as `vizij-api-core`. Shapes are included when available so the consumer can reason about numeric layouts.

### Error handling

- All registration and staging methods throw `JsError` on failure. Wrap calls in `try/catch` to surface actionable messages:
  ```ts
  try {
    orchestrator.register_graph({ spec: badSpec });
  } catch (err) {
    console.error("Graph registration failed:", err instanceof Error ? err.message : err);
  }
  ```
- ABI mismatches surface via `abi_version()`—if the returned value differs from the npm wrapper’s expected version, rebuild the wasm crate and retry initialisation.

---

## Troubleshooting

- **ABI mismatch**: Ensure both the `.wasm` binary and JS glue were rebuilt together (`pnpm run build:wasm:orchestrator`). The wrapper throws `ABI mismatch` with the expected/current values.
- **`graph cfg parse error`**: Indicates the registration payload lacked a `spec` or contained invalid JSON. Run it through `normalize_graph_spec_json` to canonicalise selectors and path strings.
- **`merge strategy error`**: Occurs when `strategy.outputs` / `strategy.intermediate` are not `"error"`, `"namespace"`, or `"blend"`.
- **Empty `merged_writes`**: Confirm at least one controller published to an output path (graphs without `Output` nodes only affect internal state).

---

## Development & Testing

```bash
pnpm run build:wasm:orchestrator     # ensure pkg output exists
cd npm/@vizij/orchestrator-wasm
pnpm test
```

Rust-side unit tests:

```bash
cargo test -p vizij-orchestrator-wasm
```

Consider adding `wasm_bindgen_test` cases if you expand the API surface.

---

## Related Packages

- [`vizij-orchestrator-core`](../../orchestrator/vizij-orchestrator-core/README.md) – underlying Rust orchestrator logic.
- [`npm/@vizij/orchestrator-wasm`](../../../npm/@vizij/orchestrator-wasm/README.md) – npm wrapper built from this crate.
- [`@vizij/orchestrator-react`](../../../vizij-web/packages/@vizij/orchestrator-react/README.md) – React provider built on the npm wrapper.

Found an issue or need a new helper? File an issue—reliable bindings keep orchestrations in sync across platforms. 🔄
