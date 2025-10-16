# @vizij/node-graph-wasm

> **Vizij’s node graph engine for JavaScript.**

This package ships the WebAssembly build of `vizij-graph-core` together with a TypeScript wrapper, schema helpers, and sample graphs. Load Vizij GraphSpecs, stage host inputs, evaluate outputs, and update node parameters from any modern JS runtime without compiling Rust.

---

## Table of Contents

1. [Overview](#overview)
2. [Key Concepts](#key-concepts)
3. [Installation](#installation)
4. [API](#api)
5. [Usage](#usage)
6. [Samples & Fixtures](#samples--fixtures)
7. [Development & Testing](#development--testing)
8. [Related Packages](#related-packages)

---

## Overview

- Built from `vizij-graph-core` with `wasm-bindgen`; the npm package is the canonical JavaScript distribution maintained by the Vizij team.
- Provides a high-level `Graph` class, low-level bindings, TypeScript definitions, and ready-to-use fixtures.
- Supports both browser and Node environments—`init()` chooses the right loader and validates the ABI (`abi_version() === 2`).
- Ships GraphSpec normalisers and schema inspection helpers so editors and tooling can speak the same language as Vizij runtimes.

---

## Key Concepts

- **GraphSpec** – Declarative JSON describing nodes, parameters, and the explicit `links` array that connects node outputs to inputs (selectors, output keys). The package normalises shorthand specs automatically.
- **Graph Runtime** – The `Graph` class owns a `GraphRuntime`, handling `loadGraph`, `stageInput`, `setParam`, `step`, and `evalAll`.
- **Staged Inputs** – Host-provided values keyed by `TypedPath`. They are latched until you replace or remove them.
- **Evaluation Result** – `evalAll()` returns per-node port snapshots plus a `WriteBatch` of sink writes (each with Value + Shape metadata).
- **Node Schema Registry** – `getNodeSchemas()` exposes the runtime-supported nodes, ideal for palettes/editors.
- **ABI Guard** – `abi_version()` ensures the JS glue and `.wasm` binary are compatible. Rebuild when versions change.

---

## Installation

```bash
npm install @vizij/node-graph-wasm
# or pnpm add @vizij/node-graph-wasm
```

For local development inside Vizij:

```bash
pnpm run build:wasm:graph
cd npm/@vizij/node-graph-wasm
pnpm install
pnpm run build
```

Link into `vizij-web` while iterating:

```bash
(cd npm/@vizij/node-graph-wasm && pnpm link --global)
(cd ../vizij-web && pnpm link @vizij/node-graph-wasm)
```

---

## API

```ts
async function init(input?: InitInput): Promise<void>;
function abi_version(): number;
async function normalizeGraphSpec(spec: GraphSpec | string): Promise<GraphSpec>;
async function getNodeSchemas(): Promise<Registry>;
async function logNodeSchemaDocs(nodeType?: NodeType | string): Promise<void>;
const graphSamples: Record<string, GraphSpec>;

class Graph {
  constructor();
  loadGraph(specOrJson: GraphSpec | string): void;
  unloadGraph(): void;
  stageInput(path: string, value: ValueInput, shape?: ShapeJSON, immediateEval?: boolean): void;
  clearStagedInputs(): void;
  applyStagedInputs(): void;
  evalAll(): EvalResult;
  setParam(nodeId: string, key: string, value: ValueInput): void;
  setTime(t: number): void;
  step(dt: number): void;
  getWrites(): WriteOpJSON[];
  clearWrites(): void;
  waitForGraphReady?(): Promise<void>; // only populated when used through React provider
}
```

### Normalization, Schema & Docs Helpers

- `normalizeGraphSpec(spec)` – round-trips any GraphSpec (object or JSON string) through the Rust normaliser so shorthand inputs/legacy `inputs` maps come back with explicit `links`, typed paths, and canonical casing.
- `getNodeSchemas()` – returns the runtime node registry (including node, port, and param documentation strings) for palette/editor usage.
- `logNodeSchemaDocs(nodeType?)` – pretty-prints the schema docs for every node or a specific `NodeType` right to the console (handy while prototyping editors).
- `graphSamples` – curated ready-to-load specs that already reflect the canonical `links` form and typed `path` parameters.

Types (`GraphSpec`, `EvalResult`, `ValueJSON`, `ShapeJSON`, etc.) are exported from `src/types`.

---

## Usage

```ts
import {
  init,
  Graph,
  normalizeGraphSpec,
  graphSamples,
  valueAsNumber,
} from "@vizij/node-graph-wasm";

await init();

const graph = new Graph();
const spec = await normalizeGraphSpec(graphSamples.vectorPlayground);
graph.loadGraph(spec);

graph.stageInput("demo/path", { float: 1 }, undefined, true);
const result = graph.evalAll();

const nodeValue =
  result.nodes["const"]?.out?.value ?? { type: "float", data: NaN };
console.log("Node value", valueAsNumber(nodeValue));

for (const write of result.writes) {
  console.log("Write", write.path, write.value);
}
```

Manual time control:

```ts
graph.setTime(0);
graph.step(1 / 60);
graph.evalAll();
```

Staging inputs lazily:

```ts
graph.stageInput("demo/path", { vec3: [0, 1, 0] });
graph.applyStagedInputs();
graph.evalAll();
```

---

## Samples & Fixtures

The package exports several ready-to-run specs:

- `graphSamples` map (e.g., `vectorPlayground`, `oscillatorBasics`, `logicGate`).
- Named exports (`oscillatorBasics`, `nestedTelemetry`, etc.) for convenience.
- Helpers:
  ```ts
  import { loadNodeGraphBundle } from "@vizij/node-graph-wasm";

  const { spec, stage } = await loadNodeGraphBundle("urdf-ik-position");
  graph.loadGraph(spec);
  if (stage) {
    for (const [path, payload] of Object.entries(stage)) {
      graph.stageInput(path, payload.value, payload.shape);
    }
  }
  ```

Fixtures originate from `@vizij/test-fixtures` so tests and demos share the same assets.

---

## Development & Testing

```bash
pnpm run build:wasm:graph          # regenerate pkg/
cd npm/@vizij/node-graph-wasm
pnpm test
```

The Vitest suite runs sample graphs through the wasm bridge, checking evaluation results and write batches. For Rust-side coverage, run `cargo test -p vizij-graph-wasm`.

---

## Related Packages

- [`vizij-graph-wasm`](../../crates/node-graph/vizij-graph-wasm/README.md) – Rust crate producing the wasm build.
- [`vizij-graph-core`](../../crates/node-graph/vizij-graph-core/README.md) – underlying evaluator.
- [`@vizij/node-graph-react`](../../../vizij-web/packages/@vizij/node-graph-react/README.md) – React integration built on this npm package.
- [`@vizij/value-json`](../value-json/README.md) – shared value helpers used during staging.

Need assistance or spot a bug? Open an issue—robust bindings keep Vizij graphs portable. 🧠
### Inspecting Schema Documentation

Each `NodeSignature` in the schema registry now ships with human-friendly descriptions for the node itself, its ports, and parameters.

```ts
import { getNodeSchemas, logNodeSchemaDocs } from "@vizij/node-graph-wasm";

await init();

// Fetch the registry and inspect docs programmatically.
const registry = await getNodeSchemas();
for (const node of registry.nodes) {
  console.log(node.name, node.doc);        // node.doc is a plain string
  for (const port of node.inputs) {
    console.log("  input:", port.label, port.doc);
  }
}

// Or print a nicely formatted summary for all nodes…
await logNodeSchemaDocs();

// …or just a single node type.
await logNodeSchemaDocs("remap");
```

The same documentation is embedded in the wasm JSON (`get_node_schemas_json`) so downstream tools can consume it without relying on these helpers.
