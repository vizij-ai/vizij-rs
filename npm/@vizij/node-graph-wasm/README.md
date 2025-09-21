# @vizij/node-graph-wasm

`@vizij/node-graph-wasm` repackages the WebAssembly build of `vizij-graph-wasm` for JavaScript/TypeScript consumers. It ships the
wasm-bindgen output, a friendly wrapper, TypeScript definitions, and ready-to-run graph samples.

## Overview

* Wraps the `vizij-graph-wasm` Rust crate (built with `wasm-pack`).
* Exposes a high-level `Graph` class with helpers to normalize graph specs, stage typed inputs, evaluate graphs, and inspect
  outputs/writes.
* Bundles a library of sample graphs for demos and automated tests.
* Supports both browser and Node runtimes; `init()` auto-detects the environment to load the `.wasm` file.

## Architecture

```
vizij-graph-core (Rust) --wasm-bindgen--> vizij-graph-wasm (cdylib) --npm--> @vizij/node-graph-wasm
       ^                           |                                     |
       |                           |                                     +-- src/index.ts (wrapper, helpers, samples)
       |                           +-- pkg/ (wasm-pack output)           +-- pkg/ (distributed glue + .wasm)
       +-- Shared schemas (GraphSpec, ValueJSON, ShapeJSON)
```

## Installation

Install the published package:

```bash
npm install @vizij/node-graph-wasm
```

For local development inside `vizij-rs`:

```bash
# From repo root
node scripts/build-graph-wasm.mjs
cd npm/@vizij/node-graph-wasm
npm install
npm run build
```

Link into `vizij-web` during development:

```bash
(cd npm/@vizij/node-graph-wasm && npm link)
# in vizij-web/
npm link @vizij/node-graph-wasm
```

## Setup

1. `await init()` once in your application to load the WASM binary.
2. Create a `Graph` instance from the wrapper or use the raw bindings from the generated `pkg/` folder.
3. Normalize graph JSON with `normalizeGraphSpec` (recommended) before loading to ensure canonical shapes/values.
4. Stage host inputs before each evaluation using `graph.stageInput(path, value, declaredShape?)`.
5. Call `graph.evalAll()` every frame (optionally using `graph.step(dt)`/`graph.setTime(t)` to manage internal time).

## Usage

### Wrapper API

```ts
import {
  init,
  Graph,
  normalizeGraphSpec,
  getNodeSchemas,
  graphSamples,
  type EvalResult,
  type ShapeJSON,
} from "@vizij/node-graph-wasm";

await init();

const graph = new Graph();
const spec = await normalizeGraphSpec(graphSamples.vectorPlayground);

graph.loadGraph(spec);
graph.setTime(0);
graph.stageInput("robot/Arm/ik_target", { vec3: [0.1, 0.2, 0.3] }, { id: "Vec3" });

const result: EvalResult = graph.evalAll();
console.log(result.nodes, result.writes);
```

### Raw bindings

```ts
import initWasm, { WasmGraph, normalize_graph_spec_json } from "@vizij/node-graph-wasm/pkg";

await initWasm();
const raw = new WasmGraph();
const normalized = normalize_graph_spec_json(JSON.stringify(rawGraphJson));
raw.load_graph(normalized);
raw.set_time(0);
raw.stage_input("robot/Arm/ik_target", JSON.stringify({ vec3: [0.1, 0.2, 0.3] }), JSON.stringify({ id: "Vec3" }));
const resultJson = raw.eval_all();
console.log(JSON.parse(resultJson));
```

## Key Details

* **JSON normalization** – `normalizeGraphSpec` accepts plain JSON and rewrites shorthand values (numbers, bools, arrays, alias
  objects) into the explicit `ValueJSON` envelopes expected by the core crate. Output shapes expressed as strings are normalized
  to `{ id: "ShapeId" }` objects.
* **Typed inputs** – `graph.stageInput(path, value, declaredShape?)` mirrors `GraphRuntime::set_input`. Raw arrays default to the
  `Vector` shape unless you wrap them in explicit envelopes (e.g., `{ vec3: [...] }`). Declared shapes enable numeric coercions
  and NaN fallbacks.
* **Evaluation result** – `EvalResult` contains two fields:
  * `nodes: Record<NodeId, Record<PortId, { value: ValueJSON; shape: ShapeJSON }>>`
  * `writes: Array<{ path: string; value: ValueJSON; shape: ShapeJSON }>`
  Writes forward the shape metadata from the Rust core (`WriteOp.shape`).
* **Parameter updates** – `graph.setParam(nodeId, key, value)` validates types (numeric params must receive numeric JSON) and
  supports robotics-focused keys (`urdf_xml`, `root_link`, `tip_link`, `weights`, etc.).
* **Samples** – `graphSamples`, `oscillatorBasics`, `vectorPlayground`, `logicGate`, and `tupleSpringDampSlew` demonstrate modern
  graph patterns (selectors, Input nodes, typed writes).

## Examples

### Iterating writes

```ts
const { writes } = graph.evalAll();
for (const write of writes) {
  console.log(`apply ${write.path}`, write.value, write.shape);
}
```

### Inspecting node outputs

```ts
const { nodes } = graph.evalAll();
for (const [nodeId, ports] of Object.entries(nodes)) {
  for (const [port, snapshot] of Object.entries(ports)) {
    console.log(nodeId, port, snapshot.value, snapshot.shape);
  }
}
```

## Testing

Run the bundled tests to ensure the wasm build and samples work:

```bash
node scripts/build-graph-wasm.mjs
cd npm/@vizij/node-graph-wasm
npm test
```

The script initializes the wasm module, loads each sample graph, evaluates a couple of ticks, and asserts that typed writes are
produced. You can also run the underlying Rust tests:

```bash
cargo test -p vizij-graph-wasm
```

## Troubleshooting

* **Graph fails to load** – Normalize the JSON first and inspect the expanded structure for type mismatches or invalid node kinds.
* **Inputs seem stale** – Inputs staged after `graph.evalAll()` apply to the next frame. Stage values before calling `evalAll` to
  make them visible immediately.
* **Strict parameter errors** – Ensure numeric params receive numbers, vectors use arrays, and robotics params provide the
  expected JSON shapes (strings for URDF, arrays for weights, etc.).
* **Missing write shapes** – Graph specs must use explicit `Output` nodes. The wrapper forwards any shape metadata attached by the
  core runtime.
