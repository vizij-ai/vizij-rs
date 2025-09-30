# @vizij/orchestrator-wasm

WASM bindings + TypeScript shim for the Vizij Orchestrator runtime.  
This package re-exports the wasm-pack output (pkg/) produced by the Rust crate at
`crates/orchestrator/vizij-orchestrator-wasm` and provides a small, ergonomic TypeScript wrapper
to initialize & use the orchestrator from ESM environments (browser, Node).

This README explains purpose, API surface, initialization, examples, and packaging notes for consumers.

Table of contents
- Purpose
- Key concepts
- Quickstart (browser / bundler)
- Quickstart (Node)
- API reference (high level)
- Returned frame shape
- Advanced: prebinding, GraphSpec normalization
- Building locally (contributor)
- Publishing / linking notes
- Troubleshooting

Purpose
--------
The orchestrator coordinates animation controllers and graph controllers, merges writes, applies last-writer-wins semantics
to a shared blackboard, and returns deterministic frame payloads that tooling and UI can consume. The WASM package exposes
this engine to JavaScript so frontends can run simulations and tooling without needing the native Rust toolchain at runtime.

Key concepts
------------
- Orchestrator: top-level runtime that owns blackboard, controllers, and schedule.
- Graph controllers: evaluate node graphs and produce writes.
- Animation controllers: advance animation players/instances and produce writes & events.
- WriteOp / WriteBatch: typed writes emitted by controllers with a `path`, `value`, and optional `shape`.
- OrchestratorFrame: single-step result containing merged writes, conflicts, timings, events and epoch.

Quickstart (ESM / browser bundler)
---------------------------------
1. Build the wasm pkg (maintainers):
   - From repository root:
     ```bash
     npm run build:wasm:orchestrator
     ```
   - This places the wasm pkg under `npm/@vizij/orchestrator-wasm/pkg`.

2. In your front-end project (or when using bundlers like Vite / Webpack):
   - Use the TypeScript wrapper entry:
     ```ts
     import { init, createOrchestrator } from "@vizij/orchestrator-wasm";

     // If using the local repo, ensure npm package is linked or use relative import to pkg .
     await init(); // loads the wasm glue
     const orchestrator = await createOrchestrator({ schedule: "SinglePass" });

     // register controllers
     const graphId = orchestrator.registerGraph({ spec: { nodes: [] } });
     const animId = orchestrator.registerAnimation({ setup: {} });

     // set a blackboard input
     orchestrator.setInput("robot/arm/joint.angle", { float: 1.23 });

     // step
     const frame = orchestrator.step(1 / 60);
     console.log(frame.merged_writes, frame.timings_ms);
     ```

Quickstart (Node)
-----------------
You can run in Node (>=18) by loading the pkg with file URL or letting the wrapper fetch the .wasm file.

Example:
```js
import { init, createOrchestrator } from "@vizij/orchestrator-wasm";

await init(); // optionally pass file:// URL to the .wasm if needed
const orchestrator = await createOrchestrator();

orchestrator.setInput("robot/x", { float: 0.5 });
const frame = orchestrator.step(1 / 60);
console.log(frame);
```

API reference (high level)
--------------------------
This TypeScript wrapper exposes an ergonomic class with the following primary functions:

- init(input?: InitInput): Promise<void>
  - Load and initialize the underlying wasm module. `InitInput` may be a URL, string, or Uint8Array (for Node file loads).

- createOrchestrator(opts?: object): Promise<Orchestrator>
  - Factory that returns a ready Orchestrator instance. `opts` supports:
    - schedule: "SinglePass" | "TwoPass" | "RateDecoupled"

Orchestrator instance methods:
- registerGraph(cfg: object | string): string
  - Register a graph controller. Accepts:
    - A GraphSpec object
    - A JSON string containing GraphSpec
    - An object `{ id?: string, spec: GraphSpec }`
  - Returns controller id.

- registerAnimation(cfg: object): string
  - Register an animation controller: `{ id?: string, setup?: any }`.

- prebind(resolver: (path: string) => string | number | null | undefined): void
  - Provide a resolver function used by animation controllers to resolve canonical target paths to opaque handles.

- setInput(path: string, value: any, shape?: any): void
  - Convenience for setting blackboard inputs. `value` must match the Value JSON shape or legacy shape.

- removeInput(path: string): boolean
  - Remove a blackboard entry.

- step(dt: number): OrchestratorFrame
  - Advance the orchestrator by dt (seconds) and return a frame object (see below).

- listControllers(): { graphs: string[], anims: string[] }
  - List registered controller ids.

- removeGraph(id: string): boolean
- removeAnimation(id: string): boolean

Returned frame shape
--------------------
The orchestrator returns a JSON-friendly frame object (serialized from Rust OrchestratorFrame). Main fields:

- epoch: number
- dt: number
- merged_writes: Array< { path: string, value: ValueJSON, shape?: ShapeJSON } >
- conflicts: Array<any> (diagnostic objects produced when writes overwrite existing blackboard entries)
- timings_ms: { animations_ms?: number, graphs_ms?: number, total_ms: number, ... }
- events: Array<any> (events forwarded by animation controllers)

ValueJSON and ShapeJSON mirror the core API shapes. The included TypeScript wrapper accepts flexible JS shapes (number, arrays, objects) and will pass through to wasm where serde deserializes into typed Value/Shape.

Advanced: prebinding & GraphSpec normalization
---------------------------------------------
- prebind(resolver) calls the engine-level prebind routine for registered animation controllers, allowing an external host to
  map canonical string paths -> opaque keys used by animation bindings.

- normalizeGraphSpec(spec) (available on the wrapper) will call the Rust normalizer to convert shorthand graph JSON (numbers, arrays, short path objects)
  into canonical GraphSpec shape. Useful for tooling that accepts many shorthand forms.

Building locally (contributor)
------------------------------
From repository root:

1. Ensure prerequisites installed:
```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack wasm-bindgen-cli
npm install
```

2. Build orchestrator wasm pkg and npm wrapper:
```bash
npm run build:wasm:orchestrator    # builds wasm pkg into npm/@vizij/orchestrator-wasm/pkg
cd npm/@vizij/orchestrator-wasm
npm install
npm run build                      # builds TS shim (dist/)
```

3. Link locally for consumption by another frontend:
```bash
cd npm/@vizij/orchestrator-wasm
npm run build
npm link
# in your frontend repo
npm link @vizij/orchestrator-wasm
```

Publishing & versioning
----------------------
- Versioning: Keep crate & npm versions in sync where appropriate. Publish core crates first, then WASM crate(s), then npm wrapper which bundles the pkg/.
- The repo contains `scripts/dry-run-release.sh` which builds all wasm artifacts and dry-runs cargo/npm publish. Use that to validate release order.

Troubleshooting
---------------
- Exec format / test runner errors:
  - Wasm unit tests compile to `.wasm` and require wasm-bindgen test harness to run in Node or a browser. Use `wasm-pack test --node` for Node-based tests or the wasm-bindgen test runner for browser tests.
- Missing .wasm errors in the browser:
  - Ensure the pkg/ folder is published or accessible. Use relative import to the generated `pkg/*.js` glue if not using npm packaging.
- Type issues:
  - The TS shim provides broad types (any) for convenience. If you need stronger typing for OrchestratorFrame, consider extending `src/index.ts` with a `types.d.ts` that mirrors the core shape.

Contact / Contributing
----------------------
- See the top-level repository README for contribution guidelines.
- To report bugs or propose improvements, create an issue on the repository: https://github.com/vizij-ai/vizij-rs/issues
