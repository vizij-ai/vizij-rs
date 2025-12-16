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
- **Graph merging** – `registerMergedGraph` rewires compatible graph specs into a single controller so shared paths become direct edges. Conflict strategies (`error`, `namespace`, `blend`, `add`, `default-blend`) are available through `MergeStrategyOptions`, letting you average, sum, or weight competing outputs.
- **Plan caching & invalidation** – Graph controllers internally use the same node-graph engine as `@vizij/node-graph-wasm`, including a cached execution plan (topological order + port layouts + input bindings).
  - The orchestrator normalizes and seeds cache keys when registering graphs.
  - Only *structural* edits (those that can affect port layouts or bindings, e.g. `Split.sizes`) require invalidating the cached plan; ordinary param/value tweaks should not force a plan rebuild.
  - `GraphSpec.specVersion`/`fingerprint` are treated as *plan-validity keys* and are managed by the wasm/Rust layer; most consumers should omit them and let the runtime handle it.
- **Blackboard** – Shared typed key-value store (`TypedPath`, `ValueJSON`, `ShapeJSON`) where controllers read/write.
- **Schedule** – `SinglePass`, `TwoPass`, or future `RateDecoupled` determine evaluation order.
- **Merged Writes** – Deterministic ordered batch of writes produced during a frame, suitable for UI or downstream consumers.
- **Conflicts** – Diagnostics when multiple controllers write to the same path; useful for debugging data races.
- **Subscriptions** – Graph controllers accept `subs.inputs`, `subs.outputs`, and `subs.mirrorWrites` to control which writes feed the blackboard vs. the merged frame.
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

## Bundler Configuration

This package now ships an ESM-friendly wrapper that prefers static imports so bundlers like Next.js/Webpack can emit plain asset URLs. Make sure your host enables async WebAssembly and treats the `.wasm` file as an emitted asset. For example, in Next.js:

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

When overriding the default wasm location (CDN or custom asset pipeline), pass a string URL to `init()`:

```ts
await init("https://cdn.example.com/vizij/vizij_orchestrator_wasm_bg.wasm");
```

The loader still accepts `URL`, `Response`, `ArrayBuffer`, or `WebAssembly.Module`, but providing a string keeps Webpack from wrapping the input in `RelativeURL`.

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
registerMergedGraph(cfg: MergedGraphRegistrationConfig): string; // merge multiple specs w/ conflict strategies
registerAnimation(cfg: AnimationRegistrationConfig): string;
exportGraph(id: string): GraphSpec; // inspect merged controller specs
prebind(resolver: (path: string) => string | number | null | undefined): void;
setInput(path: string, value: ValueJSON, shape?: ShapeJSON): void;
setHotInputs(paths: string[], opts?: { epsilon?: number }): void;
setInputsSmart(paths: string[], values: Float32Array, shapes?: (ShapeJSON | null)[]): void;
removeInput(path: string): boolean;
stepDelta(dtSeconds: number, sinceVersion?: number): OrchestratorFrame & { version: bigint };
step(dtSeconds: number): OrchestratorFrame;
listControllers(): { graphs: string[]; anims: string[] };
removeGraph(id: string): boolean;
removeAnimation(id: string): boolean;
normalizeGraphSpec(spec: GraphSpec | string): Promise<GraphSpec>; // convenience passthrough
setDebugLogging(enabled: boolean): void;
```

> Performance note: stay on the wrapper APIs (`stepDelta`, `setInputsSmart`, `setHotInputs`). Calling `inner.step` / legacy exports skips the diffed staging + delta path and will be slower.

All types (`GraphSpec`, `ValueJSON`, `OrchestratorFrame`, etc.) are exported from `src/types`.

### Registration payloads

- `registerGraph({ id?, spec, subs? })` expects a canonical `GraphSpec` object. Optional `subs.inputs`/`subs.outputs` arrays accept canonical TypedPath strings; invalid paths throw descriptive errors.
- `registerMergedGraph({ id?, graphs: GraphConfig[], strategy? })` mirrors the single-graph shape. `strategy.outputs`/`strategy.intermediate` accept:
  - `"error"` – fail merges when conflicts occur.
  - `"namespace"` – rename colliding final outputs to `graphId/original/path`.
  - `"blend"`, `"blend_equal"`, `"blend_equal_weights"`, or `"blend-equal-weights"` – average conflicts with equal weights.
  - `"add"`, `"sum"`, `"blend_sum"`, `"blend-sum"`, or `"additive"` – route conflicts through a variadic `add` node so consumers see the sum.
  - `"default-blend"`, `"default_blend"`, `"blend-default"`, `"blend_weights"`, `"blend-weights"`, or `"weights"` – inject a `default-blend` node where each contributor gets a dedicated weight input at `blend_weights/<path>/<graph>` (defaults to `1.0`).
- `registerAnimation({ id?, setup? })` forwards the payload to the Rust controller. Supply the animation JSON and optional player/instance overrides.
- All registration helpers auto-generate ids (`graph:0`, `anim:0`) when omitted.

---

## Usage

```ts
import { init, createOrchestrator } from "@vizij/orchestrator-wasm";

await init();
const orchestrator = await createOrchestrator({ schedule: "SinglePass" });

const graphId = orchestrator.registerGraph({ spec: { nodes: [], edges: [] } });
const animId = orchestrator.registerAnimation({ setup: {} });

orchestrator.prebind((path) => path.toUpperCase());
orchestrator.setInput("demo/input/value", { float: 1.0 });

const frame = orchestrator.step(1 / 60);
console.log(frame.merged_writes, frame.timings_ms);

// Delta snapshot (first call with 0 returns full payload)
let version = 0n;
const deltaFrame = orchestrator.stepDelta(1 / 60, version);
version = deltaFrame.version;
console.log(deltaFrame.merged_writes);

// Hot-path inputs with diffed staging
orchestrator.setHotInputs(["rig/in_a", "rig/in_b"], { epsilon: 0 });
const paths = ["rig/in_a", "rig/in_b"];
const vals = new Float32Array([0, 0]);
orchestrator.setInputsSmart(paths, vals); // seeds; later frames only send changed values
orchestrator.step(1 / 60);

// Merge two graph specs into a single controller (auto-link shared output/input paths)
const mergedGraphId = orchestrator.registerMergedGraph({
  graphs: [
    {
      spec: {
        nodes: [
          { id: "source", type: "constant", params: { value: 1 } },
          { id: "publish", type: "output", params: { path: "shared/value" } },
        ],
        edges: [
          { from: { node_id: "source" }, to: { node_id: "publish", input: "in" } },
        ],
      },
    },
    {
      spec: {
        nodes: [
          { id: "input", type: "input", params: { path: "shared/value" } },
          {
            id: "double",
            type: "multiply",
            input_defaults: { rhs: { value: 2 } },
          },
          { id: "publish", type: "output", params: { path: "shared/doubled" } },
        ],
        edges: [
          { from: { node_id: "input" }, to: { node_id: "double", input: "lhs" } },
          { from: { node_id: "double" }, to: { node_id: "publish", input: "in" } },
        ],
      },
    },
  ],
  strategy: {
    outputs: "add",
    intermediate: "default-blend",
  },
});

// Inspect the merged spec (useful for tooling/debug)
const mergedSpec = orchestrator.exportGraph(mergedGraphId);
console.log(mergedSpec.nodes.length, "nodes in merged graph");
```

Removing controllers:

```ts
const { graphs, anims } = orchestrator.listControllers();
graphs.forEach((id) => orchestrator.removeGraph(id));
anims.forEach((id) => orchestrator.removeAnimation(id));
```

### Custom loader options

`init(input?: InitInput)` accepts any loader supported by `@vizij/wasm-loader` (`URL`, `Response`, `ArrayBuffer`, `Uint8Array`, or `WebAssembly.Module`). Use this when hosting the wasm binary on a CDN or inside desktop bundles.

---

## Fixtures

```ts
import {
  createOrchestrator,
  loadOrchestrationBundle,
} from "@vizij/orchestrator-wasm";
import { toValueJSON, type ValueInput } from "@vizij/value-json";

const bundle = await loadOrchestrationBundle("chain-sign-slew-pipeline");
const schedule = bundle.descriptor.schedule ?? "SinglePass";
const orchestrator = await createOrchestrator({ schedule });

for (const binding of bundle.graphs) {
  const config = { ...binding.config };
  if (binding.id) config.id = binding.id;
  orchestrator.registerGraph(config);
}

for (const merged of bundle.mergedGraphs) {
  const configs = merged.graphs.map((binding) => {
    const config = { ...binding.config };
    if (binding.id) config.id = binding.id;
    return config;
  });
  orchestrator.registerMergedGraph({ id: merged.id, graphs: configs, strategy: merged.strategy });
}

const primaryAnimation = bundle.animations[0];
if (primaryAnimation) {
  orchestrator.registerAnimation({
    id: primaryAnimation.id,
    setup: primaryAnimation.setup,
  });
}

for (const input of bundle.initialInputs) {
  orchestrator.setInput(
    input.path,
    toValueJSON(input.value as ValueInput),
    input.shape ? structuredClone(input.shape) : undefined,
  );
}
```

Available fixture keys today:

- `scalar-ramp-pipeline` – single graph + animation demonstrating gain/offset staging.
- `blend-pose-pipeline` – TwoPass orchestration mirroring the weighted pose blend demo.
- `chain-sign-slew-pipeline` – multi-graph example (sign → slew) showcasing chained controllers.
- `merged-blend-pipeline` – merged controller example that rewires shared outputs and applies blend strategies.

Fixtures are sourced from `@vizij/test-fixtures` so demos/tests align with the Rust workspace.

---

## Troubleshooting

- **ABI mismatch** – Re-run `pnpm run build:wasm:orchestrator` if `abi_version()` differs from the value expected by the wrapper.
- **Graph registration errors** – `registerGraph` and `registerMergedGraph` throw `JsError` when the payload is missing `spec` or contains invalid TypedPaths. Normalise specs first with `normalizeGraphSpec`.
- **Merge strategy failures** – The `strategy` object accepts the strings listed above (including aliases such as `"sum"` or `"weights"`). Any other string results in `merge strategy error`.
- **Empty writes** – Controllers must emit `Output` nodes with `params.path`; otherwise the orchestrator returns an empty `merged_writes` array.

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
