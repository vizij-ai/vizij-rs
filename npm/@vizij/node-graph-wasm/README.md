# @vizij/node-graph-wasm

> Vizij's node graph engine for JavaScript.

This package ships the WebAssembly build of `vizij-graph-core` together with a TypeScript wrapper, schema helpers, node registry metadata, and fixture bundles.

## Overview

- Browser and Node compatible ESM package.
- Main runtime wrapper: `Graph`.
- Schema helpers: `normalizeGraphSpec`, `getNodeSchemas`, `getNodeRegistry`, `findNodeSignature`, `requireNodeSignature`, `listNodeTypeIds`, `groupNodeSignaturesByCategory`, `logNodeSchemaDocs`.
- Fixture helpers: `listNodeGraphFixtures`, `loadNodeGraphBundle`, `loadNodeGraphSpec`, `loadNodeGraphSpecJson`, `loadNodeGraphStage`.
- Sample exports via `graphSamples`.

## Installation

```bash
npm install @vizij/node-graph-wasm
```

For local workspace development:

```bash
pnpm run build:wasm:graph
pnpm --filter @vizij/node-graph-wasm build
```

## Core API

Top-level exports:

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
async function logNodeSchemaDocs(nodeType?: NodeType | string): Promise<void>;
const graphSamples: Record<string, GraphSpec>;
```

Notable `Graph` methods:

```ts
loadGraph(
  spec: GraphSpec | string,
  opts?: { hotPaths?: string[]; epsilon?: number; autoClearDroppedHotPaths?: boolean }
): void;
stageInput(path: string, value: ValueInput, declaredShape?: ShapeJSON): void;
stageInputs(paths: string[], values: Float32Array, shapes?: (ShapeJSON | null)[]): void;
registerInputPaths(paths: string[]): Uint32Array;
prepareInputSlots(indices: Uint32Array, declaredShapes?: (ShapeJSON | null)[]): void;
stageInputsByIndex(indices: Uint32Array, values: Float32Array): void;
stageInputsBySlot(indices: Uint32Array, values: Float32Array): void;
stageInputsBySlotDiff(indices: Uint32Array, values: Float32Array, epsilon?: number): void;
setHotPaths(paths: string[], opts?: { epsilon?: number; autoClearDroppedHotPaths?: boolean }): void;
setParam(nodeId: string, key: string, value: ValueInput): void;
setTime(t: number): void;
step(dt: number): void;
evalAll(): EvalResult;
evalAllFull(): EvalResult;
getOutputsDelta(sinceVersion?: number): EvalResult & { version: number };
getOutputsBatch(nodeIds: string[]): Float32Array;
evalSteps(steps: number, dt: number): EvalResult;
clearSlot(slotIdx: number): Promise<void>;
clearInput(path: string): Promise<void>;
setDebugLogging(enabled: boolean): void;
inspectStaging(): {
  hotPaths: string[];
  epsilon: number;
  autoClearDroppedHotPaths: boolean;
  slotDiffWarm: boolean;
  lastOutputVersion: bigint;
  debugLogging: boolean;
};
```

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

graph.stageInput("demo/path", { float: 1 });
const result = graph.evalAll();

const nodeValue =
  result.nodes["const"]?.out?.value ?? { type: "float", data: NaN };
console.log("Node value", valueAsNumber(nodeValue));
```

### Hot-path staging

```ts
graph.loadGraph(spec, {
  hotPaths: ["demo/a", "demo/b"],
  epsilon: 0,
  autoClearDroppedHotPaths: true,
});

const paths = ["demo/a", "demo/b", "demo/rare"];
const values = new Float32Array([1, 2, 3]);
graph.stageInputs(paths, values);

graph.setDebugLogging(true);
console.log(graph.inspectStaging());
```

Use `registerInputPaths` plus `prepareInputSlots` when you want to reuse slot indices across many frames and minimize path parsing overhead.

## Fixtures And Samples

The package exports named graph samples and fixture loaders:

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

## Troubleshooting

- Selector mismatch errors usually mean the spec references an element the upstream node does not emit.
- `setParam` enforces the same value-shape rules as the Rust runtime.
- ABI mismatch means the wasm bindings need to be rebuilt with `pnpm run build:wasm:graph`.

## Development And Testing

```bash
pnpm run build:wasm:graph
pnpm --filter @vizij/node-graph-wasm test
cargo test -p vizij-graph-wasm
```

The package test script rebuilds the wrapper and runs the compiled Node test bundle from `dist/node-graph-wasm/tests/all.test.js`.

## Related Packages

- [`vizij-graph-wasm`](../../../crates/node-graph/vizij-graph-wasm/README.md)
- [`@vizij/value-json`](../value-json/README.md)
- [`@vizij/wasm-loader`](../wasm-loader/README.md)
