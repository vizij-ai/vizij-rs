# Fixture Centralization Plan

## Objectives
- Consolidate animation, node graph, and orchestrator fixtures so Rust crates and npm packages share a single source of truth.
- Provide paired "simple" and "complex" scenarios for both animations and node graphs, covering the full span of value types and nodes used today.
- Eliminate duplicated inline fixtures in tests/examples by referencing the shared assets instead.
- Ensure orchestrator tests validate orchestration of the shared animation and node graph fixtures together rather than maintaining bespoke copies.
- Keep the test matrix additive: each test exercises a distinct combination of fixtures or value shapes to maximise overall coverage.

## Current Fixture Inventory (pre-change)
- Dispersed JSON fixtures under `crates/animation/test_fixtures/`, `npm/@vizij/node-graph-wasm/tests/fixtures/`, and `crates/orchestrator/vizij-orchestrator-core/fixtures/` with overlapping content.
- Inline TypeScript fixtures:
  - Animation smoke test constructs a `StoredAnimation` object inline and references `crates/animation/test_fixtures/ramp.json` directly.
  - Orchestrator smoke test defines graph specs + animation config inline for chained scenarios.
  - Node graph smoke test defines multiple graph specs + staged values inline despite overlapping with `src/samples.ts`.

### Step 1 Audit Notes
- **Animation coverage:**
  - `ramp.json` / inline TS ramp – scalar ramp track hitting float transitions.
  - `const.json` – vec3 constant track exercising vector values.
  - `loop_window.json` – scalar looping window variant (duration + loop semantics).
  - `new_format.json` – rich mix of vec3, Euler rotation, RGB color, vec2, scalar tracks (demonstrates settings metadata as well).
- **Node graph JSON fixtures:** weighted blend family exercises nested records/tuples/arrays, vector math, join outputs; lacks boolean/text predicate pathways.
- **Node graph inline specs:** oscillator/vector playground/logic gate/etc in `src/samples.ts` include booleans, conditionals, enums, vector transforms, URDF IK samples.
- **Orchestrator fixture:** `demo_single_pass.json` stitches scalar ramp animation with simple multiply/add graph.
- **Gaps to cover:** boolean toggles, enum values, transforms/quaternions, list/set shapes, tuple/record combos, multi-track animations synced with multi-output graphs for integrated orchestrator tests.

### Target Fixture Coverage (for future steps)
| Domain | Fixture Intent | Value / Node Coverage |
|--------|----------------|-----------------------|
| Animation | `simple-scalar-ramp` | scalar track + easing transitions |
| Animation | `vector-pose-combo` (repurpose `new_format.json`) | vec3 position, Euler rotation, vec2 offset, RGB color |
| Animation | `state-toggle` (new) | boolean or enum-style track to exercise discrete values |
| Node graph | `simple-gain-offset` | scalar multiply/add chain (mirrors orchestrator demo) |
| Node graph | `logic-gate` | boolean comparisons, conditional nodes, enum outputs |
| Node graph | `weighted-profile-blend` | nested tuple/record/vector/list manipulation |
| Node graph | `urdf-ik-position` | transform/quaternion outputs, lists of vec3 |
| Orchestrator | `scalar-ramp-pipeline` | simple animation + simple graph stitched |
| Orchestrator | `blend-pose-pipeline` | complex animation driving blend graph with varied value types |

> These targets will guide the migrations in Steps 3–5 so that each layer reuses the same shared assets while expanding coverage across value shapes.

### Step 2 Design Plan
- **Filesystem layout**
  - Create root `fixtures/` dir with subfolders:
    - `fixtures/animations/` – JSON fixtures named `simple-scalar-ramp.json`, `vector-pose-combo.json`, `state-toggle.json`.
    - `fixtures/node_graphs/` – JSON fixtures `simple-gain-offset.json`, `logic-gate.json`, `weighted-profile-blend.json`, `urdf-ik-position.json`.
    - `fixtures/orchestrations/` – JSON descriptors `scalar-ramp-pipeline.json`, `blend-pose-pipeline.json` referencing animation + graph fixture keys and any initial inputs/expectations.
  - Add `fixtures/manifest.json` to list fixture keys → file names per domain so Rust + TypeScript stay in sync without duplicating strings.
  - Manifest example:
    ```json
    {
      "animations": {
        "simple-scalar-ramp": "animations/simple-scalar-ramp.json",
        "vector-pose-combo": "animations/vector-pose-combo.json"
      },
      "node-graphs": {
        "simple-gain-offset": {
          "spec": "node_graphs/simple-gain-offset.json"
        }
      },
      "orchestrations": {
        "scalar-ramp-pipeline": {
          "animation": "simple-scalar-ramp",
          "graph": "simple-gain-offset",
          "inputs": {
            "demo/graph/gain": 1.5,
            "demo/graph/offset": 0.25
          }
        }
      }
    }
    ```
- **Rust access**
  - Introduce new crate `crates/test-fixtures/vizij-test-fixtures` (publish = false) exposing modules `animations`, `node_graphs`, `orchestrations`.
  - Each module loads the shared JSON via `include_str!` (using manifest for resolution) and returns typed helper structs / serde_json values for consumers.
  - Animation + graph crates update tests/examples to depend on this crate for fixture data.
- **TypeScript access**
  - Add workspace package `npm/@vizij/test-fixtures` (private) with `src/animations.ts`, `src/nodeGraphs.ts`, `src/orchestrations.ts`.
  - Package reads `fixtures/manifest.json` at runtime using `import.meta.url` and exposes helpers:
    - `animationFixture(name)` returning parsed `StoredAnimation`.
    - `nodeGraphFixture(name)` returning parsed `GraphSpec` + optional staged values.
    - `orchestratorFixture(name)` returning combined data (animation + graph + inputs + expected writes).
  - Export curated constants (e.g. `simpleScalarAnimation`, `simpleGainGraph`) for convenience in tests, plus file path getters when direct file I/O is needed.
- **Naming conventions**
  - Use kebab-case for fixture keys and file names (e.g. `simple-scalar-ramp`). Map to camelCase constants in TypeScript.
  - Keep orchestrator fixtures referencing animation/node graph keys to prevent drift.
- **Documentation**
  - Update `fixtures_for_tests.md` after scaffolding to note manifest format and helper APIs.

### 2025-10 Additions
- Added `fixtures/animations/pose-quat-transform.json` exercising quaternion + transform tracks alongside the existing scalar/vector fixtures.
- Captured boolean + URDF IK graph coverage via `fixtures/node_graphs/logic-gate.json` and `fixtures/node_graphs/urdf-ik-position.json` (with default stage data).
- Introduced `fixtures/orchestrations/blend-pose-pipeline.json`, pairing the new animation and weighted-profile graph so orchestrator tests can assert complex value writes.
- Rust coverage:
  - `vizij-test-fixtures` now smoke tests the new manifest entries.
  - `vizij-animation-core/tests/stored_animation_loader.rs` verifies quaternion/transform parsing.
  - `vizij-orchestrator-core/tests/integration_passes.rs` asserts blended pipeline writes against `ValueJSON` expectations.
- npm coverage:
  - `@vizij/node-graph-wasm` and `@vizij/orchestrator-wasm` smoke tests load shared fixtures (falling back gracefully when local wasm builds lag).
  - `@vizij/animation-wasm` validates vector/quaternion/transform outputs or confirms fixture structure if the packaged wasm lacks the new parser.

### Step 3 Execution Notes (ongoing)
- Created `fixtures/` tree with `animations/`, `node_graphs/`, `orchestrations/` and moved existing JSON assets there (renamed where useful).
- Authored `fixtures/manifest.json` tracking fixture keys to file locations and stage data.
- Added new Rust crate `vizij-test-fixtures` exposing loader APIs for animations, node graphs, and orchestrations.
- Updated animation + orchestrator Rust tests/examples to consume shared fixtures via the new crate or direct includes.

### Step 4 Execution Notes (ongoing)
- Scaffolded npm workspace package `@vizij/test-fixtures` with helper APIs mirroring the shared manifest.
- Migrated npm animation/node-graph/orchestrator smoke tests to load fixtures via the shared package (covering scalar, vector, boolean/text, chained graph, and graph-driven animation scenarios).
- Added new shared fixtures for chain ramp animations and sign/slew/sine driver graphs to support orchestrator coverage.

### Step 5 Execution Notes (ongoing)
- Updated orchestrator core tests to consume shared fixtures for scalar ramp, chained sign/slew, and sine-driven scenarios.

## High-Level Strategy
1. **Design shared fixture layout** under a new top-level `fixtures/` directory with sub-folders (e.g. `animations/`, `node_graphs/`, `orchestrations/`) and corresponding TypeScript helpers in a workspace package `@vizij/test-fixtures` for typed access.
2. **Migrate existing JSON assets** into the shared structure, extending coverage with clearly named simple vs complex fixtures for both animations and node graphs.
3. **Expose fixtures to Rust** via helper modules (e.g. `crates/fixtures/animation.rs`) using `include_str!` paths relative to the new directory, and update crates/tests/examples to use these helpers.
4. **Expose fixtures to npm packages** by authoring TypeScript exports in `@vizij/test-fixtures` that wrap the shared JSON (covering both parsed objects and file paths) so tests can import shared data instead of re-declaring it.
5. **Refactor orchestrator tests** (Rust + npm) to depend on the shared animation/node graph fixtures, adding combined scenarios that demonstrate orchestration of different value shapes without duplicating definitions.
6. **Verify** with fmt/clippy/tests for Rust workspace and build/test for npm packages, updating this log as steps complete.

## Work Breakdown
- [X] **Step 1 – Audit & gap analysis**: Catalogue required fixture scenarios (value types, nodes) and map them to shared assets to confirm coverage needs.
- [X] **Step 2 – Shared layout & package scaffolding**: Create `fixtures/` directory tree and the `@vizij/test-fixtures` workspace package with README + TypeScript entrypoints mirroring the JSON assets.
- [X] **Step 3 – Migrate JSON fixtures**: Move/rename current JSON files into the shared tree, add missing simple/complex examples for animations & node graphs, and adjust Rust code to load them via helper modules.
- [X] **Step 4 – Update npm consumers**: Switch animation/node-graph/orchestrator tests to import from `@vizij/test-fixtures`, remove inline specs, and ensure each test exercises a distinct scenario.
- [X] **Step 5 – Orchestrator integration**: Ensure orchestrator tests (Rust + npm) use the shared fixtures for both individual component validation and combined execution.
- [X] **Step 6 – Final verification**: Run `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --workspace`, and the npm package builds/tests; document outcomes here.

## Progress Log
- _Done_ – Step 1: Audited existing fixtures and documented coverage gaps / targets.
- _Done_ – Step 2: Documented shared fixture layout & helper package scaffolding.
- _Done_ – Step 3: Migrated JSON fixtures and wired Rust helpers via vizij-test-fixtures.
- _Done_ – Step 4: npm packages now load shared fixtures via @vizij/test-fixtures with additive coverage.
- _Done_ – Step 5: Orchestrator integration with shared fixtures.
- _Done_ – Step 6: Final verification commands.
