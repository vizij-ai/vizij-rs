# vizij-graph-wasm

`vizij-graph-wasm` wraps `vizij-graph-core` with `wasm-bindgen` so JavaScript/TypeScript tooling can load, evaluate, and interact
with Vizij node graphs. The crate builds to a `cdylib` that is republished to npm as `@vizij/node-graph-wasm`.

## Overview

* Provides a `WasmGraph` class that mirrors the Rust runtime (`load_graph`, `stage_input`, `step`, `eval_all`, `set_param`).
* Includes helpers to normalize graph JSON so authored specs can use ergonomic shorthands.
* Returns node outputs and external writes as JSON with explicit shape metadata.
* Supports staging host inputs with declared shapes and strict parameter updates (type-checked).
* Optional features: `urdf_ik` (default) and `console_error` (forward Rust panics to the browser console).

## Architecture

```
Rust (vizij-graph-core)  -->  wasm-bindgen (vizij-graph-wasm)  -->  npm/@vizij/node-graph-wasm
          ^                              |                                        |
          |                              |                                        +-- src/index.ts (wrapper, types, samples)
          |                              +-- wasm-pack output (pkg/)              +-- pkg/ (generated glue + .wasm)
          |
          +-- Shared schemas (GraphSpec, ValueJSON, ShapeJSON)
```

* `WasmGraph` owns a `GraphRuntime` and exposes methods to load specs, stage inputs, step time, and evaluate the graph.
* JSON normalization utilities convert ergonomic inputs (plain numbers/arrays, alias objects) into the tagged structures expected
  by the core crate.
* Outputs include per-node port values plus external writes, all annotated with shapes.

## Installation

Build the crate and npm wrapper from this repository:

```bash
# Build the WASM artifact
node scripts/build-graph-wasm.mjs

# Install npm dependencies and build the wrapper
cd npm/@vizij/node-graph-wasm
npm install
npm run build
```

In external projects install the published npm package:

```bash
npm install @vizij/node-graph-wasm
```

Enable the optional `console_error` feature if you want panic hooks in the browser:

```bash
wasm-pack build crates/node-graph/vizij-graph-wasm --target web --out-dir npm/@vizij/node-graph-wasm/pkg --release -- --features console_error
```

## Setup

1. Ensure `wasm-pack` and `wasm-bindgen-cli` are installed (`cargo install wasm-pack wasm-bindgen-cli`).
2. Run `node scripts/build-graph-wasm.mjs` to generate `pkg/` inside `npm/@vizij/node-graph-wasm/`.
3. For local development with `vizij-web`, run `npm link` inside the npm package and link it into the web workspace.
4. Call `await init()` in your JS/TS application before constructing `Graph` instances from the npm wrapper.

## Usage

### JavaScript / TypeScript API (npm wrapper)

```ts
import {
  init,
  Graph,
  normalizeGraphSpec,
  getNodeSchemas,
  type GraphSpec,
  type EvalResult,
  type ShapeJSON,
} from "@vizij/node-graph-wasm";

await init();

const graph = new Graph();
const normalized: GraphSpec = await normalizeGraphSpec(rawSpecJsonOrObject);
graph.loadGraph(normalized);

graph.setTime(0);
graph.step(1 / 60);

graph.stageInput("robot/Arm/ik_target", { vec3: [0.1, 0.2, 0.3] }, { id: "Vec3" });

const result: EvalResult = graph.evalAll();
console.log(result.nodes, result.writes);
```

### Core WASM bindings

* `normalize_graph_spec_json(json: &str) -> String` – Normalizes JSON to the explicit schema expected by `GraphSpec`.
* `class WasmGraph`:
  * `load_graph(json: &str)` – Parses and normalizes JSON before loading the spec.
  * `stage_input(path: &str, value_json: &str, declared_shape_json: Option<String>)` – Stage host inputs for the next evaluation.
  * `set_time(t: f64)` / `step(dt: f64)` – Manage internal time; `eval_all()` computes `dt` automatically if you only set time.
  * `eval_all() -> Result<String, JsValue>` – Evaluates the graph and returns JSON with `nodes` and `writes` arrays.
  * `set_param(node_id, key, json_value)` – Update node parameters at runtime with type validation.
  * `get_node_schemas_json()` – Returns the node schema registry for tooling.

## Key Details

### JSON normalization

`normalize_graph_spec_json` accepts ergonomic shorthands and rewrites them into the explicit envelopes used by the core crate:

* Accepts `kind` and rewrites it to `type`, lowercasing the node type.
* Plain numbers, bools, strings, and arrays normalize to `{ "type": "float" }`, `{ "type": "bool" }`, etc. Numeric arrays of
  length 2/3/4 become `Vec2`/`Vec3`/`Vec4`; other numeric arrays become `Vector`.
* Object aliases such as `{ "vec3": [...] }`, `{ "quat": [...] }`, `{ "record": { ... } }`, `{ "enum": { "tag": "...", "value": ... } }`
  are supported.
* `output_shapes: { "out": "Vec3" }` normalizes to `{ "out": { "id": "Vec3" } }`.
* `params.path` accepts `{ "path": "..." }` objects and is normalized to strings.

### Staging inputs

* `stage_input` mirrors `GraphRuntime::set_input`. Values are normalized with a slightly different policy to avoid guessing
  vector dimensions (raw arrays default to `Vector` unless you use explicit envelopes like `{ "vec3": [...] }`).
* Declared shapes allow numeric coercions and NaN-filled “null-of-shape” fallbacks inside the core runtime.
* Each call participates in epoch tracking—values staged before `eval_all()` are consumed on that evaluation; later stages appear
  on the following run.

### Outputs and writes

* `eval_all()` returns JSON:
  ```json
  {
    "nodes": {
      "nodeId": {
        "port": { "value": ValueJSON, "shape": ShapeJSON }
      }
    },
    "writes": [
      { "path": "...", "value": ValueJSON, "shape": ShapeJSON }
    ]
  }
  ```
* `ValueJSON` mirrors `vizij_api_core::Value` (scalar, bool, vecN, quat, color, transform, vector, record, array, list, tuple,
  enum, text).
* Write entries forward the shape metadata from the core `WriteOp` when present so consumers retain vector length and structured
  contracts.

### Parameter updates

* `set_param` is strict: numeric fields reject non-numeric JSON instead of silently coercing to `0`.
* Coverage includes common parameters (`value`, `index`, `frequency`, `min/max`, `stiffness`, etc.) and robotics-specific fields
  (`urdf_xml`, `root_link`, `tip_link`, `seed`, `weights`, `max_iters`, etc.).

## Examples

### Loading built-in samples (npm wrapper)

```ts
import { init, Graph, graphSamples } from "@vizij/node-graph-wasm";

await init();

const g = new Graph();
g.loadGraph(graphSamples.vectorPlayground);
g.setTime(0);
const result = g.evalAll();
console.log(result.writes);
```

### Direct WASM usage (without wrapper)

```ts
import initWasm, { WasmGraph, normalize_graph_spec_json } from "@vizij/node-graph-wasm/pkg";

await initWasm();
const raw = new WasmGraph();
const normalized = normalize_graph_spec_json(JSON.stringify(rawGraphJson));
raw.load_graph(normalized);
raw.stage_input(
  "robot/Arm/ik_target",
  JSON.stringify({ vec3: [0.1, 0.2, 0.3] }),
  JSON.stringify({ id: "Vec3" })
);
raw.set_time(0);
raw.step(1 / 60);
const evalJson = raw.eval_all();
console.log(JSON.parse(evalJson));
```

## Testing

Run the crate’s tests via the npm wrapper:

```bash
node scripts/build-graph-wasm.mjs      # ensure pkg/ exists
cd npm/@vizij/node-graph-wasm
npm test
```

The included test script loads each bundled sample, runs a couple of evaluation ticks, and asserts that typed writes are emitted.
You can also run the Rust tests directly:

```bash
cargo test -p vizij-graph-wasm
```

## Troubleshooting

* **Graph fails to load** – Run the graph JSON through `normalize_graph_spec_json` to see the fully expanded form and ensure node
  types/values are valid.
* **Inputs ignored** – Remember that staged inputs apply to the *next* evaluation due to epoch semantics. Stage values before
  calling `eval_all()`.
* **Missing shape metadata on writes** – Ensure the producing graph nodes emit `WriteOp` entries with shapes (the core runtime
  attaches them when available). The wrapper forwards `WriteOp.shape` when present.
* **Strict parameter errors** – The wrapper rejects invalid types; update tooling to send correctly typed JSON (numbers for
  numeric params, arrays for vector params, etc.).
