# @vizij/orchestrator-wasm

> Vizij's orchestrator runtime for JavaScript.

This package ships the WebAssembly build of `vizij-orchestrator-core` together with a TypeScript wrapper, ABI checks, and orchestration fixtures. It is the primary JavaScript entry point for registering graph and animation controllers, staging blackboard inputs, and stepping merged writes.

## Overview

- Browser and Node compatible ESM package.
- Wrapper class: `Orchestrator`.
- Low-level init and ABI helpers: `init()`, `abi_version()`.
- Fixture helpers: `listOrchestrationFixtures`, `loadOrchestrationBundle`, `loadOrchestrationDescriptor`, `loadOrchestrationJson`.
- Built from the wasm package in `pkg/` plus TypeScript glue in `src/`.

## Installation

```bash
npm install @vizij/orchestrator-wasm
```

For local workspace development:

```bash
pnpm run build:wasm:orchestrator
pnpm --filter @vizij/orchestrator-wasm build
```

## API

Top-level exports:

```ts
async function init(input?: InitInput): Promise<void>;
function abi_version(): number;
async function createOrchestrator(opts?: CreateOrchOptions): Promise<Orchestrator>;
async function loadOrchestrationBundle(key: string): Promise<OrchestrationBundle>;
```

Notable `Orchestrator` methods:

```ts
registerGraph(cfg: GraphRegistrationInput | string): string;
replaceGraph(cfg: { id: string; spec: GraphSpec; subs?: GraphSubscriptions }): void;
registerMergedGraph(cfg: MergedGraphRegistrationConfig): string;
registerAnimation(cfg: AnimationRegistrationConfig): string;
exportGraph(id: string): GraphSpec;
prebind(resolver: (path: string) => string | number | null | undefined): void;
setInput(path: string, value: ValueJSON, shape?: ShapeJSON): void;
setHotInputs(paths: string[], opts?: { epsilon?: number }): void;
setInputsSmart(paths: string[], values: Float32Array, shapes?: (ShapeJSON | null)[]): void;
removeInput(path: string): boolean;
step(dtSeconds: number): OrchestratorFrame;
stepDelta(dtSeconds: number, sinceVersion?: number | bigint): OrchestratorFrame & { version: bigint };
listControllers(): { graphs: string[]; anims: string[] };
removeGraph(id: string): boolean;
removeAnimation(id: string): boolean;
setDebugLogging(enabled: boolean): void;
normalizeGraphSpec(spec: object | string): Promise<object>;
```

## Usage

```ts
import { init, createOrchestrator } from "@vizij/orchestrator-wasm";

await init();
const orchestrator = await createOrchestrator({ schedule: "SinglePass" });

const graphId = orchestrator.registerGraph({
  spec: { nodes: [], edges: [] },
});

orchestrator.setInput("demo/input/value", { float: 1.0 });
const frame = orchestrator.step(1 / 60);

console.log(graphId, frame.merged_writes, frame.timings_ms);
```

Use `replaceGraph` for structural graph edits. `stepDelta` is the incremental stepping API when a host wants versioned frame diffs instead of full snapshots.

## Fixtures

Fixture bundles are loaded from `@vizij/test-fixtures` at build time:

```ts
import { createOrchestrator, loadOrchestrationBundle } from "@vizij/orchestrator-wasm";

const bundle = await loadOrchestrationBundle("chain-sign-slew-pipeline");
const orchestrator = await createOrchestrator({
  schedule: bundle.descriptor.schedule ?? "SinglePass",
});
```

Available fixture keys today include:

- `scalar-ramp-pipeline`
- `blend-pose-pipeline`
- `chain-sign-slew-pipeline`
- `merged-blend-pipeline`

## Troubleshooting

- ABI mismatch: rebuild with `pnpm run build:wasm:orchestrator`.
- Graph registration errors: normalize specs first and check typed path strings.
- Empty `merged_writes`: confirm a controller actually emits output paths.

## Development And Testing

```bash
pnpm run build:wasm:orchestrator
pnpm --filter @vizij/orchestrator-wasm test
cargo test -p vizij-orchestrator-wasm
```

The package test script rebuilds the wrapper and runs the compiled Node test bundle from `dist/orchestrator-wasm/tests/all.test.js`.

## Related Packages

- [`vizij-orchestrator-wasm`](../../../crates/orchestrator/vizij-orchestrator-wasm/README.md)
- [`@vizij/node-graph-wasm`](../node-graph-wasm/README.md)
- [`@vizij/value-json`](../value-json/README.md)
