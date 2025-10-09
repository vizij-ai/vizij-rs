# @vizij/orchestrator-wasm

> **Vizij’s orchestrator runtime for JavaScript.**

This package publishes the WebAssembly build of `vizij-orchestrator-core` together with a TypeScript wrapper, ABI guards, and orchestration fixtures. Register graph/animation controllers, stream merged writes, inspect conflict logs, and drive blackboard inputs from any modern JS runtime—no Rust toolchain required.

---

## Table of Contents

1. [Overview](#overview)
2. [Key Concepts](#key-concepts)
3. [Installation](#installation)
4. [API](#api)
5. [Usage](#usage)
6. [Fixtures](#fixtures)
7. [Development & Testing](#development--testing)
8. [Related Packages](#related-packages)

---

## Overview

- Compiled from the official Vizij orchestrator (`vizij-orchestrator-core`) using `wasm-bindgen`.
- Exposes a high-level `Orchestrator` class, low-level bindings, TypeScript types, and ready-to-run orchestration bundles.
- Works in browsers and Node—`init()` loads the `.wasm` binary and enforces ABI compatibility (`abi_version() === 2`).
- Designed and maintained by the Vizij team; this npm package is the sanctioned JavaScript entry point into the orchestrator.

---

## Key Concepts

- **Controllers** – Graph and animation controllers registered with IDs; each has its own configuration (`spec`, `subscriptions`, `setup`).
- **Blackboard** – Shared typed key-value store (`TypedPath`, `ValueJSON`, `ShapeJSON`) where controllers read/write.
- **Schedule** – `SinglePass`, `TwoPass`, or future `RateDecoupled` determine evaluation order.
- **Merged Writes** – Deterministic ordered batch of writes produced during a frame, suitable for UI or downstream consumers.
- **Conflicts** – Diagnostics when multiple controllers write to the same path; useful for debugging data races.
- **ABI Guard** – `abi_version()` ensures the JS glue matches the `.wasm` binary. Rebuild when versions change.

---

## Installation

```bash
npm install @vizij/orchestrator-wasm
# or pnpm add @vizij/orchestrator-wasm
```

Local development inside Vizij:

```bash
pnpm run build:wasm:orchestrator
cd npm/@vizij/orchestrator-wasm
pnpm install
pnpm run build
```

Link into `vizij-web` while iterating:

```bash
(cd npm/@vizij/orchestrator-wasm && pnpm link --global)
(cd ../vizij-web && pnpm link @vizij/orchestrator-wasm)
```

---

## API

```ts
async function init(input?: InitInput): Promise<void>;
function abi_version(): number;
async function createOrchestrator(opts?: CreateOrchOptions): Promise<Orchestrator>;
async function loadOrchestrationBundle(key: string): Promise<OrchestrationBundle>;
```

`Orchestrator` instance methods:

```ts
registerGraph(cfg: GraphRegistrationInput | string): string;
registerAnimation(cfg: AnimationRegistrationConfig): string;
prebind(resolver: (path: string) => string | number | null | undefined): void;
setInput(path: string, value: ValueJSON, shape?: ShapeJSON): void;
removeInput(path: string): boolean;
step(dtSeconds: number): OrchestratorFrame;
listControllers(): { graphs: string[]; anims: string[] };
removeGraph(id: string): boolean;
removeAnimation(id: string): boolean;
normalizeGraphSpec(spec: GraphSpec | string): Promise<GraphSpec>; // convenience passthrough
```

All types (`GraphSpec`, `ValueJSON`, `OrchestratorFrame`, etc.) are exported from `src/types`.

---

## Usage

```ts
import { init, createOrchestrator } from "@vizij/orchestrator-wasm";

await init();
const orchestrator = await createOrchestrator({ schedule: "SinglePass" });

const graphId = orchestrator.registerGraph({ spec: { nodes: [] } });
const animId = orchestrator.registerAnimation({ setup: {} });

orchestrator.prebind((path) => path.toUpperCase());
orchestrator.setInput("demo/input/value", { float: 1.0 });

const frame = orchestrator.step(1 / 60);
console.log(frame.merged_writes, frame.timings_ms);
```

Removing controllers:

```ts
const { graphs, anims } = orchestrator.listControllers();
graphs.forEach((id) => orchestrator.removeGraph(id));
anims.forEach((id) => orchestrator.removeAnimation(id));
```

---

## Fixtures

```ts
import { loadOrchestrationBundle } from "@vizij/orchestrator-wasm";

const bundle = await loadOrchestrationBundle("scalar-ramp-pipeline");
const orchestrator = await createOrchestrator();

if (bundle.graphSpec) {
  orchestrator.registerGraph({ id: "graph", spec: bundle.graphSpec });
}
if (bundle.animation) {
  orchestrator.registerAnimation({ id: "anim", setup: { animation: bundle.animation } });
}
if (bundle.graphStage) {
  for (const [path, payload] of Object.entries(bundle.graphStage)) {
    orchestrator.setInput(path, payload.value, payload.shape);
  }
}
```

Fixtures are sourced from `@vizij/test-fixtures` so demos/tests align with the Rust workspace.

---

## Development & Testing

```bash
pnpm run build:wasm:orchestrator     # regenerate pkg/
cd npm/@vizij/orchestrator-wasm
pnpm test
```

Vitest ensures `init()` loads correctly, the ABI guard fires, controller registration works, and bundled fixtures produce deterministic frames. Rust-side testing lives in `vizij-orchestrator-wasm` (`cargo test -p vizij-orchestrator-wasm`).

---

## Related Packages

- [`vizij-orchestrator-wasm`](../../crates/orchestrator/vizij-orchestrator-wasm/README.md) – Rust source of these bindings.
- [`vizij-orchestrator-core`](../../crates/orchestrator/vizij-orchestrator-core/README.md) – orchestrator runtime.
- [`@vizij/orchestrator-react`](../../../vizij-web/packages/@vizij/orchestrator-react/README.md) – React provider built on this package.
- [`@vizij/value-json`](../value-json/README.md) – shared value helpers used for staging.

Need help? Open an issue—stable orchestration bindings keep Vizij simulations consistent across platforms. 🔁
