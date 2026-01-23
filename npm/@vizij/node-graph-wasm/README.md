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
- Bakes the node registry (`metadata/registry.json`) straight from the Rust core so build-time tooling and authoring UIs stay in sync with the runtime.

---

## Key Concepts

- **GraphSpec** – Declarative JSON describing nodes, parameters, and the explicit `edges` array that connects node outputs to inputs (selectors, output keys). The package normalises shorthand specs automatically.
- **Graph Runtime** – The `Graph` class owns a `GraphRuntime`, handling `loadGraph`, `stageInput`, `setParam`, `step`, and `evalAll`. Structural parameter changes (e.g., `Split.sizes`) invalidate the wasm plan cache and the JS wrapper resets its delta baseline automatically so the next eval re-establishes a full snapshot before returning to deltas.
- **Plan caching & invalidation** – The wasm engine caches a compiled execution plan (topological order + port layouts + input bindings) for performance.
  - The cache is reused across frames when the graph layout is unchanged.
  - Only *structural* edits (changes that can affect port layouts or bindings) invalidate the plan. In practice today this includes `Split.sizes`; most other param changes are non-structural and do not force a plan rebuild.
  - `GraphSpec.specVersion` (and `fingerprint`) are treated as *plan-validity keys* (not a generic "state version"). The wrapper auto-fills them and bumps them only for structural changes, so ordinary value tweaks do not degrade steady-state performance.
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

## Bundler Configuration

Like the other Vizij wasm packages, this module now exports an ESM wrapper that first attempts a static import of the wasm-bindgen JS glue. Bundlers that support async WebAssembly (Webpack 5, Vite, etc.) should treat `pkg/vizij_graph_wasm_bg.wasm` as an emitted asset. For Next.js configure:

```js
// next.config.js
module.exports = {
  webpack: (config) => {
    config.experiments = { ...(config.experiments ?? {}), asyncWebAssembly: true };
    config.module.rules.push({
      test: /\.wasm$/,
      type: "asset/resource",
    });
    return config;
  },
};
```

If you host the wasm binary elsewhere, pass a string URL to `init()`:

```ts
await init("https://cdn.example.com/vizij/node_graph_wasm_bg.wasm");
```

Passing a string avoids Webpack’s `RelativeURL` helper, which previously attempted to call `.replace()` on a `URL` object.

---

## API

```ts
async function init(input?: InitInput): Promise<void>;
function abi_version(): number;
async function normalizeGraphSpec(spec: GraphSpec | string): Promise<GraphSpec>;
async function getNodeSchemas(): Promise<Registry>;
function getNodeRegistry(): Registry;
function findNodeSignature(typeId: NodeType | string): NodeSignature | undefined;
function requireNodeSignature(typeId: NodeType | string): NodeSignature;
function listNodeTypeIds(): NodeType[];
function groupNodeSignaturesByCategory(): Map<string, NodeSignature[]>;
const nodeRegistryVersion: string;
async function logNodeSchemaDocs(nodeType?: NodeType | string): Promise<void>;
const graphSamples: Record<string, GraphSpec>;

class Graph {
  constructor();
  loadGraph(
    specOrJson: GraphSpec | string,
    opts?: { hotPaths?: string[]; epsilon?: number; autoClearDroppedHotPaths?: boolean }
  ): void;
  unloadGraph(): void;
  stageInput(path: string, value: ValueInput, shape?: ShapeJSON, immediateEval?: boolean): void;
  setHotPaths(paths: string[], opts?: { epsilon?: number; autoClearDroppedHotPaths?: boolean }): void;
  stageInputs(paths: string[], values: Float32Array, shapes?: (ShapeJSON | null)[]): void; // routes through smart staging
  stageInputsBySlotDiff(indices: Uint32Array, values: Float32Array, epsilon?: number): void;
  clearSlot(idx: number): Promise<void>;
  clearInput(path: string): Promise<void>;
  clearStagedInputs(): void;
  applyStagedInputs(): void;
  evalAll(): EvalResult;
  evalAllFull(): EvalResult; // resets the delta baseline
  getOutputsDelta(sinceVersion?: number): EvalResult & { version: number };
  setParam(nodeId: string, key: string, value: ValueInput): void;
  setTime(t: number): void;
  step(dt: number): void;
  getWrites(): WriteOpJSON[];
  clearWrites(): void;
  waitForGraphReady?(): Promise<void>; // only populated when used through React provider
}
```

### Normalization, Schema & Docs Helpers

- `normalizeGraphSpec(spec)` – round-trips any GraphSpec (object or JSON string) through the Rust normaliser so shorthand inputs/legacy `inputs` maps come back with explicit `edges`, typed paths, and canonical casing.
- `getNodeSchemas()` / `getNodeRegistry()` – runtime and baked access to the node registry (including ports and params) for palette/editor usage.
- `findNodeSignature(typeId)` / `requireNodeSignature(typeId)` – quick lookups into the baked registry.
- `listNodeTypeIds()` / `groupNodeSignaturesByCategory()` – helpers for palettes or UI grouping.
- `logNodeSchemaDocs(nodeType?)` – pretty-prints the schema docs for every node or a specific `NodeType` right to the console (handy while prototyping editors).
- `graphSamples` – curated ready-to-load specs that already reflect the canonical `edges` form and typed `path` parameters.

Each registry entry exposes:

| Field | Description |
|-------|-------------|
| `doc` / `short_doc` | Human-readable description for node palettes and tooltips. |
| `inputs` / `outputs` | Port metadata (`label`, `doc`, `shape` hints) useful for editors. |
| `params` | Parameter schema with expected value types and default values. |
| `categories` | Optional grouping tags for UI organisation. |

Types (`GraphSpec`, `EvalResult`, `ValueJSON`, `ShapeJSON`, etc.) are exported from `src/types`.

> Performance guardrails:
> - Structural param edits that change port layouts (e.g., `Split.sizes`) rebuild the plan; the wrapper drops its cached baseline and will emit a full snapshot on the next eval before returning to deltas.
> - The `Graph` wrapper automatically picks the optimized slots + delta path; calling `inner.eval_all_js` / other legacy exports bypasses these optimizations and is slower.

### Delta semantics (baseline resync)

`getOutputsDelta(sinceVersion?)` is designed for long-running loops where you want to transfer only the ports that changed since a version token.

- Pass `0` (or omit the argument) for the first call to establish a baseline snapshot.
- If the caller's `sinceVersion` does not match the runtime's cached baseline (including when the wasm runtime resets versions after `loadGraph`, `clear`, or a structural edit), the runtime returns a **full snapshot** flagged with `full: true`.
- When `full: true`, treat the payload as a replacement baseline (do not merge it with an older snapshot).

The `Graph` wrapper handles baseline management automatically for the common case; this section is mainly relevant if you call low-level bindings directly.

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

### Hot-path fast staging (unified)

```ts
const graph = new Graph();
graph.loadGraph(spec, { hotPaths: ["demo/a", "demo/b"], epsilon: 0, autoClearDroppedHotPaths: true });
// setHotPaths is still available; loadGraph can now register the hot list for you.

const paths = ["demo/a", "demo/b", "demo/rare"];
const values = new Float32Array([1, 2, 3]);

// Routes hot paths through slots (diffed after first call), others through path batch.
graph.stageInputs(paths, values);
const frame = graph.evalAll(); // first call returns full snapshot; subsequent calls use delta internally

// Debug visibility
graph.setDebugLogging(true);
console.log(graph.inspectStaging());
```

> Note: The hot-path fast lane currently targets numeric scalars. Non-scalar or explicitly shaped inputs in the hot list will fall back to path staging for that call (log in debug mode). Use `clearSlot` / `clearInput` to stop replaying cached values.

### Fast-path loop with deltas (slots-only eval)

```ts
let version = 0n;
const inputs = new Float32Array(paths.length);
const indices = graph.registerInputPaths(paths);
graph.prepareInputSlots(indices);

function tick(frame) {
  inputs.fill(frame);
  graph.stageInputsBySlotDiff(indices, inputs); // only sends changed slots
  graph.setTime(frame / 60);
  graph.step(1 / 60);
  const delta = graph.getOutputsDelta(Number(version));
  version = BigInt(delta.version);
  // delta.nodes contains only changed ports when the baseline matches.
  // When the baseline does NOT match (first call, after loadGraph, after structural edits),
  // the runtime will return a full snapshot and the wrapper will replace its cached baseline.
}
```

Manual time control (force a full baseline refresh when needed):

```ts
graph.setTime(0);
graph.step(1 / 60);
graph.evalAllFull(); // resets delta baseline
```

Staging inputs lazily:

```ts
graph.stageInput("demo/path", { vec3: [0, 1, 0] });
graph.applyStagedInputs();
graph.evalAll();
```

### Performance tips

- **Batch steady frames:** If inputs remain constant for N ticks, prefer `graph.evalSteps(N, dt)` to advance time and return only the final outputs/writes. This collapses N JS↔WASM calls into one. Don’t use it when you need to inject new inputs every frame.
- **Avoid JSON for numeric streams:** For hot numeric inputs, call the underlying wasm export `graph.inner.stage_input_f32(path, float32Array)` to skip JSON encode/decode. To read numeric outputs without JSON, use `graph.inner.get_output_f32(nodeId, outputKey)` to receive a `Float32Array`. Keep the JSON/ValueJSON path for mixed or non-numeric data.
- **One call per frame:** Even with per-frame inputs, batch all staging calls, then a single `evalAll` (or `evalSteps` when valid) per frame to minimize boundary crossings.

### Custom loader options

`init(input?: InitInput)` accepts any input supported by `@vizij/wasm-loader`:

```ts
import { init } from "@vizij/node-graph-wasm";
import { readFile } from "node:fs/promises";

// Host wasm from your CDN
await init(new URL("https://cdn.example.com/vizij/node_graph_wasm_bg.wasm"));

// Node / Electron / tests
const bytes = await readFile("dist/node_graph_wasm_bg.wasm");
await init(bytes);
```

This is useful for service workers, Electron, or any environment that needs explicit control over fetch behaviour.

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

## Troubleshooting

- **Selector mismatch** – Errors such as `selector index 5 out of bounds` mean the GraphSpec referenced an array element that does not exist. Normalise the spec and confirm upstream nodes emit the expected shape.
- **`set_param` validation** – Parameters enforce specific value types (`float`, `text`, tuple pairs). Coerce values with `normalizeValueJSON` before calling `setParam` to avoid runtime throws.
- **ABI mismatch** – Re-run `pnpm run build:wasm:graph` if `abi_version()` differs from the expected version logged by the package.
- **Missing fixtures** – `loadNodeGraphBundle` resolves names from `@vizij/test-fixtures`. Ensure `pnpm run build:shared` has been executed in local development.

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
