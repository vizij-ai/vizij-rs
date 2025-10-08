# vizij-graph-wasm

> **wasm-bindgen bridge for Vizij node graphs – load specs, stage inputs, and evaluate graphs from JavaScript.**

`vizij-graph-wasm` compiles `vizij-graph-core` to WebAssembly and exposes a friendly API for TypeScript tooling. The npm wrapper `@vizij/node-graph-wasm` builds on this crate.

---

## Table of Contents

1. [Overview](#overview)
2. [Exports](#exports)
3. [Building](#building)
4. [Usage](#usage)
5. [Key Details](#key-details)
6. [Testing](#testing)
7. [Related Packages](#related-packages)

---

## Overview

- Compiles to a `cdylib` via `wasm-bindgen` with ABI guard `abi_version() == 2`.
- Wraps a `GraphRuntime` inside the `WasmGraph` class (load, stage, step, evaluate, set params).
- Provides JSON normalisation helpers so authored specs can use ergonomic shorthands.
- Emits evaluation results with explicit value/shape metadata to keep consumers schema-aware.
- Optional features:
  - `urdf_ik` (default) – include URDF chain helpers for robotics nodes.
  - `console_error` – enable `console_error_panic_hook` for clearer browser diagnostics.

---

## Exports

| Export | Description |
|--------|-------------|
| `normalize_graph_spec_json(json: &str) -> String` | Rewrites ergonomic JSON into the canonical `GraphSpec` envelope. |
| `get_node_schemas_json() -> String` | Returns the node schema registry as JSON. |
| `class WasmGraph` | Methods: `load_graph`, `stage_input`, `set_time`, `step`, `eval_all`, `set_param`, `clear`, `abi_version`. |
| `abi_version() -> u32` | Returns `2`; used by npm wrappers to enforce compatibility. |

---

## Building

From the repository root:

```bash
pnpm run build:wasm:graph      # recommended path (invokes scripts/build-graph-wasm.mjs)
```

Manual build:

```bash
wasm-pack build crates/node-graph/vizij-graph-wasm \
  --target bundler \
  --out-dir pkg \
  --release
```

The `pkg/` directory is consumed by `npm/@vizij/node-graph-wasm`.

---

## Usage

Using the npm wrapper (recommended):

```ts
import {
  init,
  Graph,
  normalizeGraphSpec,
  graphSamples,
  type GraphSpec,
  type EvalResult,
} from "@vizij/node-graph-wasm";

await init();

const graph = new Graph();
const spec: GraphSpec = await normalizeGraphSpec(graphSamples.vectorPlayground);
graph.loadGraph(spec);

graph.stageInput("nodes.inputA.inputs.in", { float: 1 }, undefined, true);
const result: EvalResult = graph.evalAll();
console.log(result.nodes, result.writes);
```

Direct WASM usage without the wrapper:

```ts
import initWasm, { WasmGraph, normalize_graph_spec_json } from "@vizij/node-graph-wasm/pkg";

await initWasm();
const raw = new WasmGraph();
const normalized = normalize_graph_spec_json(JSON.stringify(spec));
raw.load_graph(normalized);
raw.stage_input("demo/path", JSON.stringify({ float: 1 }), null);
const evalJson = raw.eval_all();
console.log(JSON.parse(evalJson));
```

---

## Key Details

- **Normalisation** – Accepts shorthands like `{ vec3: [...] }`, auto-lowers node type names, rewrites `kind` → `type`, and upgrades simple numbers/bools/arrays into tagged `ValueJSON`.
- **Staging** – `stage_input` mirrors `GraphRuntime::set_input`. Passing `undefined` (via the npm wrapper) removes staged entries safely.
- **Evaluation Result** – `eval_all` returns `{ nodes: { [id]: { port: { value, shape } } }, writes: WriteOpJSON[] }`. Shapes mirror `vizij_api_core::Shape`.
- **Parameter updates** – `set_param` validates types strictly (no silent coercion). All core node parameters plus robotics settings are supported.
- **Time management** – Call `set_time`/`step` as needed. `eval_all` computes `dt` based on internal time values when both are invoked.

---

## Testing

```bash
pnpm run build:wasm:graph      # ensure pkg/ is up to date
cd npm/@vizij/node-graph-wasm
pnpm test
```

The npm test suite runs a set of graph samples through the wasm binding. For Rust-only coverage run:

```bash
cargo test -p vizij-graph-wasm
```

---

## Related Packages

- [`vizij-graph-core`](../vizij-graph-core/README.md) – core evaluator used by this crate.
- [`npm/@vizij/node-graph-wasm`](../../../npm/@vizij/node-graph-wasm/README.md) – npm package built from this binding.
- [`@vizij/node-graph-react`](../../../vizij-web/packages/@vizij/node-graph-react/README.md) – React integration built on the npm wrapper.

Need assistance? Open an issue—predictable WASM bindings keep Vizij graphs portable. 🕸️
