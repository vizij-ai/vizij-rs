# vizij-orchestrator-wasm

WASM bindings for the vizij orchestrator core. This crate provides a JS-friendly wrapper around the Rust orchestrator that mirrors the ergonomics of `vizij-animation-wasm` and `vizij-graph-wasm`.

Features
- Construct an orchestrator instance with an optional schedule.
- Register graph and animation controllers from JS.
- Prebind targets with a JS resolver function (used by animation controllers).
- Set and remove blackboard inputs.
- Step the orchestrator and receive a JSON-friendly OrchestratorFrame.

> ABI guard: `abi_version()` returns `2`, and the npm wrapper enforces the same value during initialization to catch mismatched
> builds early.
- List and remove registered controllers.

API (high-level)
- constructor: `new(opts?: { schedule?: "SinglePass" | "TwoPass" | "RateDecoupled" })`
- `abi_version(): number`
- `register_graph(cfg: string | { id?: string, spec: object }): string` — returns controller id
- `register_animation(cfg: { id?: string, setup?: any }): string` — returns controller id
- `prebind(resolver: (path: string) => string | number | null | undefined): void`
- `set_input(path: string, valueJson: any, shapeJson?: any): void`
- `remove_input(path: string): boolean`
- `step(dt: number): OrchestratorFrame` — returns a JS object (serialized from OrchestratorFrame)
- `list_controllers(): { graphs: string[], anims: string[] }`
- `remove_graph(id: string): boolean`
- `remove_animation(id: string): boolean`

OrchestratorFrame JSON shape (example)
{
  epoch: number,
  dt: number,
  merged_writes: [ { path: string, value: ValueJSON, shape?: ShapeJSON }, ... ],
  conflicts: [ ... ], // serde_json values produced by conflict logs
  timings_ms: { animations_ms?: number, graphs_ms?: number, total_ms: number, ... },
  events: [ ... ],
}

Notes on value shapes
- The wrapper uses serde to serialize core types. For compatibility with older tooling, helper functions were added (in `utils.rs`) to convert core `Value` into legacy JSON shapes (e.g. `{ "vec3": [...] }`, `{ "float": 1.0 }`), if you need that representation for processed write batches.

JS usage example
```js
import init, { VizijOrchestrator, abi_version } from "@vizij/orchestrator-wasm";

await init(); // wasm module init, if using wasm-pack generated pkg

console.log("ABI:", abi_version());

const o = new VizijOrchestrator({ schedule: "SinglePass" });

// Register a graph from a JSON string (GraphSpec)
const graphId = o.register_graph(JSON.stringify({
  nodes: [],
  // ...
}));

// Register animation controller (simple)
const animId = o.register_animation({ setup: {} });

// Optional prebind resolver — used by animation controllers to resolve canonical paths
o.prebind((path) => {
  // return an opaque key (string or number) or null/undefined if unresolved
  return path.toUpperCase();
});

// Set a blackboard input
o.set_input("robot/arm/joint.angle", { float: 1.23 }, null);

// Step
const frame = o.step(1/60);
console.log("Frame:", frame);
```

Testing & build
- This crate is configured as a `cdylib` for wasm.
- To produce JS artifacts consistent with other wasm crates in the repo, run the repository's existing wasm build scripts (`scripts/build-graph-wasm.mjs` / `scripts/build-animation-wasm.mjs`) adapted for this crate, or use `wasm-pack` / `cargo +nightly build --target wasm32-unknown-unknown` followed by `wasm-bindgen` toolchain steps.

Next steps (if you want me to continue)
- Add wasm tests (wasm_bindgen_test) to validate the WASM API surface.
- Build the wasm package and generate the `npm/` wrapper (matching other wasm crates).
- Wire up GraphSpec normalization (copy existing logic from `node-graph` if you want to accept shorthand graph JSON in register_graph).
- Expand the wrapper to optionally return legacy-style write batches (utilities are already included).
