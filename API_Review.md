# Cross-Stack API Alignment Review

## Overview
Vizij warehouses its Value, Shape, TypedPath, and WriteBatch primitives in `vizij-api-core`, but the surrounding stacks (animation, node-graph, orchestrator, and the npm wrappers) now maintain parallel JSON bridges, helpers, and fixtures. The duplication increases drift risk, forces consumers to learn different ergonomics per package, and makes future ABI/schema updates expensive to land. This document merges the two prior reviews into a single actionable plan that targets the highest-leverage fixes for keeping the stacks aligned.

## Themes & Findings

### 1. Shared Value/Shape serialization needs a single home
**Issues**
- `vizij-graph-wasm` and `vizij-orchestrator-wasm` ship near-identical `normalize_value_json` implementations (plus GraphSpec normalization glue) to coerce shorthands into `{ "type": ..., "data": ... }` payloads.【F:crates/node-graph/vizij-graph-wasm/src/lib.rs†L6-L153】【F:crates/orchestrator/vizij-orchestrator-wasm/src/normalize.rs†L1-L155】 The orchestrator copy already calls out that it mirrors the graph crate, and the graph crate maintains a staging variant; both copies must be kept in sync whenever Value variants change.
- Both wasm crates also duplicate the legacy serializer that rewrites `vizij_api_core::Value` (and WriteBatch data) back into `{ float: ... }`, `{ vec3: [...] }`, etc., for downstream tooling.【F:crates/node-graph/vizij-graph-wasm/src/lib.rs†L115-L154】【F:crates/orchestrator/vizij-orchestrator-wasm/src/utils.rs†L11-L84】
- The orchestrator wasm wrapper repeats JSON→Value→WriteBatch conversion logic that already exists in the core blackboard, leaving three different sites that parse arbitrary JSON into Value/Shape (`Blackboard::set`, wasm `set_input`, and ad-hoc tests).【F:crates/orchestrator/vizij-orchestrator-core/src/blackboard.rs†L66-L96】【F:crates/orchestrator/vizij-orchestrator-wasm/src/lib.rs†L249-L276】

**Impact**
- Every Value or Shape addition requires editing multiple Rust files plus TypeScript shims; missing a site causes runtime panics or silent mis-serialization.
- Test coverage has to be reimplemented in each crate, and debugging bugs demands diffing two “mirrored” versions of the same code.

**Actions**
1. Introduce a `vizij-api-core::json` (or small sibling crate such as `vizij-api-json`) module that houses:
   - Shorthand → normalized Value helpers.
   - GraphSpec normalization routines.
   - Legacy `{ float: ... }` / `{ type: ..., data: ... }` serializers for Value, WriteOp, and WriteBatch.
2. Re-export these helpers to wasm crates (and potentially to the core orchestrator) so the local copies disappear.
3. Migrate `Blackboard::set` and wasm `set_input` to call the shared JSON helpers, reducing the number of bespoke serde paths.
4. Add focused tests in the shared module that ensure all Value variants stay covered; wasm crates can lean on the shared tests instead of duplicating assertions.

### 2. WriteBatch and TypedPath bridging is reimplemented per controller
**Issues**
- The animation engine already returns a fully typed `WriteBatch` via `update_writebatch`, but the orchestrator controller re-parses change keys, rebuilding the exact same batch logic.【F:crates/animation/vizij-animation-core/src/engine.rs†L654-L666】【F:crates/orchestrator/vizij-orchestrator-core/src/controllers/animation.rs†L256-L288】
- `AnimationController::map_blackboard_to_inputs` converts `TypedPath` back to strings and manually splits segments to recover player/instance conventions instead of inspecting the structured fields on `TypedPath`.【F:crates/orchestrator/vizij-orchestrator-core/src/controllers/animation.rs†L148-L288】

**Impact**
- Manual string parsing is brittle; any convention tweak (e.g., extra namespaces) requires auditing every controller.
- Duplicate WriteBatch loops risk diverging behaviour (e.g., when shapes are attached, or when change filtering rules evolve).

**Actions**
1. Expose a reusable helper from the animation crate (e.g., `Outputs::into_writebatch()` or a thin wrapper around `update_writebatch`) and have controllers depend on it instead of duplicating the change loop.
2. Extend `TypedPath` with convenience accessors (namespaces, player/instance extraction, etc.) or supply domain-specific helpers inside `vizij-orchestrator-core` so controllers operate on structured data rather than string splits.
3. Add unit tests around the shared helper to ensure instance updates, command paths, and future fields stay correctly parsed.

### 3. JavaScript/TypeScript Value surfaces drift from Rust reality
**Issues**
- The npm packages document Value unions with capitalized discriminants (`"Float"`, `"Vec3"`, …) while Rust serializes with `rename_all = "lowercase"`; the orchestrator typings even export `NormalizedValue` with capitalized cases although the normalizer emits lowercase strings.【F:npm/@vizij/animation-wasm/src/types.d.ts†L103-L123】【F:npm/@vizij/orchestrator-wasm/src/types.ts†L30-L48】【F:crates/api/vizij-api-core/src/value.rs†L30-L80】
- The node-graph wrapper ships a friendly `toValueJSON` helper that accepts primitives, but orchestrator callers (and tests) rebuild small adapters like `floatVal` or `scalarFromValue` to bridge Value syntax.【F:npm/@vizij/node-graph-wasm/src/index.ts†L122-L133】【F:npm/@vizij/orchestrator-wasm/tests/all.test.ts†L168-L199】

**Impact**
- TypeScript consumers receive misleading typings; casing bugs slip through compile-time checks and only surface at runtime.
- Each package teaches a slightly different input surface, increasing friction when chaining controllers across stacks.

**Actions**
1. Regenerate or hand-edit the published typings so discriminants match the actual serde casing (lowercase). Consider generating the union from the shared JSON module above to keep Rust and TS in sync.
2. Publish a shared TS helper (e.g., `@vizij/value-json`) or re-export the same utility from each package so all wasm wrappers accept primitives consistently.
3. Update orchestrator tests and samples to consume the shared helper, demonstrating the canonical Value ergonomics.

### 4. Example and test fixtures are copy-pasted
**Issues**
- The orchestrator ramp demo (graph spec + animation setup) appears in Rust examples and JS integration tests with only formatting differences.【F:crates/orchestrator/vizij-orchestrator-core/examples/repro.rs†L10-L128】【F:npm/@vizij/orchestrator-wasm/tests/all.test.ts†L187-L220】
- Additional examples (`single_pass.rs`, `graph_only.rs`, etc.) manually rebuild the same structures that npm fixtures also define.

**Impact**
- Any tweak to the canonical demo requires editing multiple files across Rust and TypeScript, risking subtle drift.
- Maintaining parallel fixtures wastes effort and complicates regression testing.

**Actions**
1. Move shared fixtures into a single source of truth (e.g., `crates/orchestrator/.../fixtures/` plus a small npm `samples` module) and load them from both Rust and JS tests/examples.
2. Provide helper builders/utilities for constructing common orchestrator scenarios so tests can evolve together across stacks.
3. Add snapshot-style tests that validate the shared fixture against the orchestrator normalizer to catch accidental schema drift.

### 5. Wasm loader scaffolding is redundantly maintained
**Issues**
- Each npm package copies the same logic: caching `_bindings`, resolving `pkg/*.wasm`, handling Node `file://` URLs, and enforcing ABI checks.【F:npm/@vizij/orchestrator-wasm/src/index.ts†L40-L180】【F:npm/@vizij/node-graph-wasm/src/index.ts†L1-L140】【F:npm/@vizij/animation-wasm/src/index.ts†L61-L156】

**Impact**
- Fixes to loader behaviour (e.g., better Node detection, ABI guards) must be ported manually to three packages.
- Minor deviations have already crept in (different comments, slightly different ABI handling), making it harder to reason about the canonical loader behaviour.

**Actions**
1. Extract a shared loader (module or small package, e.g., `@vizij/wasm-loader`) that exports `initWasm`, caching, and `file://` support.
2. Have each wasm package delegate to the shared loader, only injecting package-specific parameters (expected ABI version, binding names).
3. Centralize ABI mismatch messaging so consumers receive consistent guidance regardless of the package they import.

## Immediate Next Steps
1. Prototype the `vizij-api-core` JSON helper module and migrate one wasm crate to exercise the shared path; expand once stable.
2. Wire the animation controller to consume the engine’s WriteBatch helper and add TypedPath accessors to eliminate manual parsing.
3. Update npm typings and publish a shared `toValueJSON` helper, then refactor orchestrator tests/examples to use it.
4. Create a shared fixture package for orchestrator demos and point both Rust and JS tests at it.
5. Draft the shared wasm loader utility and swap one package to verify the integration before roll-out to the others.

## Longer-Term Follow-Ups
- Add CI checks (Rust + TS) that exercise the shared JSON helpers against real wasm bindings to ensure serialization changes fail fast.
- Document the canonical Value/Shape/TypedPath flows in `vizij-api-core` so new contributors know where to add future variants.
- Evaluate whether additional stacks (Bevy adapters, web orchestrator) can reuse the shared fixtures and loader utilities once extracted.

## Progress Log
- **Shared JSON helpers**: Created `vizij_api_core::json` module and re-exported normalization/legacy conversion utilities. Removed duplicated logic from orchestrator/node-graph wasm crates and pointed `Blackboard::set` at the new helpers.
- **Animation write batching**: Added `Outputs::to_writebatch()` in `vizij-animation-core`, updated `Engine::update_writebatch`, and refactored the orchestrator animation controller to rely on the shared helper instead of manual loops.
- **TypedPath ergonomics**: Extended `TypedPath` with namespace/target accessor methods and reworked `AnimationController::map_blackboard_to_inputs` to use structured parsing via a new `AnimationPathKind` helper.
- **Value JSON TS alignment**: Introduced `@vizij/value-json` package with shared helpers/types, updated npm wrappers to depend on it, and switched orchestrator tests to the helper while correcting discriminant casing.
- **Shared orchestrator fixture**: Added `fixtures/demo_single_pass.json` plus `vizij_orchestrator::fixtures` loader, and updated the wasm integration test to consume the shared data instead of inlined specs.
- **Wasm loader utilities**: Created `@vizij/wasm-loader` and migrated the orchestrator wrapper to the shared loader to deduplicate Node file URL handling and ABI checks.
- **Loader rollout**: Updated the node-graph and animation npm wrappers to import `@vizij/wasm-loader`, removing bespoke `init()`/`file://` handling and wiring ABI guards where available.
- **Workspace tooling**: Switched the repo to pnpm workspaces (`pnpm-workspace.yaml`, `pnpm-lock.yaml`) so `workspace:*` dependencies install cleanly; refreshed the README/setup docs and package scripts accordingly.
- **CI coverage**: Extended `.github/workflows/ci.yml` to build all wasm crates, install pnpm, and run TypeScript builds/tests for the shared helpers and wasm wrappers.
- **Wasm normalize cleanup**: Dropped the redundant orchestrator `normalize_value_json` re-export and unused GraphSpec value helper now that the crate consumes the shared `vizij_api_core::json` module directly, silencing the remaining build warnings.
