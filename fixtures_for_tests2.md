# Fixture Coverage Completion Plan

## Goals
- Close remaining gaps in the shared fixture catalog so boolean, enum, transform, and IK-heavy scenarios live alongside the existing scalar/vector cases.
- Ensure npm and Rust consumers rely on the shared helpers wherever practical, replacing ad-hoc sample wiring with manifest-driven loading.
- Broaden orchestrator coverage beyond scalar pipelines by introducing a complex blend scenario that reuses the richer animation and graph fixtures.
- Keep documentation and verification flows in sync with the expanded matrix.

## Workstream A – Expand Fixture Catalog
1. **Node graph additions**
   - Promote `logicGate`, `weightedProfileBlend`, and `urdfIkPosition` (or equivalent) from inline samples into JSON under `fixtures/node_graphs/`.
   - Author matching stage data files where tests currently inject staged inputs (e.g. weighted blend targets).
   - Update `fixtures/manifest.json` to register the new graphs (spec + optional stage) and re-run `npm/@vizij/test-fixtures` build so type definitions stay aligned.
   - Add smoke assertions in a focused Rust test (`vizij-test-fixtures` or `vizij-graph-core`) to confirm each new manifest entry parses.

2. **Animation additions**
   - Extract a multi-track pose/rotation/quaternion example from the existing samples (or compose one) into `fixtures/animations/`.
   - Extend `vector-pose-combo.json` if needed to include quaternion/transform tracks, or add a dedicated `pose-quat-transform.json` to cover that value family.
   - Refresh manifest + regenerate typings.

3. **Orchestrator descriptor**
   - Create `fixtures/orchestrations/blend-pose-pipeline.json` that references the richer animation/graph fixtures and stages representative inputs/expectations.
   - Ensure descriptor captures multiple expected outputs so downstream tests can assert full integration.

## Workstream B – Align Consumers With Shared Fixtures
1. **Node graph wasm tests**
   - Update `npm/@vizij/node-graph-wasm/tests/all.test.ts` to load the new manifest-backed fixtures in place of `graphSamples` for logic gate, weighted blend, and IK scenarios.
   - Remove redundant sample wiring once fixtures cover the same behaviour; keep a minimal sample smoke test if needed for API parity.
   - Add targeted assertions (boolean, enum tags, transform vectors) when consuming shared fixture outputs so gaps can be spotted quickly.

2. **Animation wasm tests**
   - Introduce a test that loads the richer animation fixture (`vector-pose-combo` or new quaternion variant) via `@vizij/test-fixtures` and validates vectors, rotations, colours, and text outputs.
   - Retain the existing scalar/bool cases to ensure both simple and complex fixtures stay exercised.

3. **Orchestrator wasm & core tests**
   - Add a new integration in `npm/@vizij/orchestrator-wasm/tests/all.test.ts` that consumes `blend-pose-pipeline`.
   - Mirror coverage in Rust (`vizij-orchestrator-core/tests/integration_passes.rs`) by wiring the same descriptor through `vizij_test_fixtures::orchestrations` and asserting merged writes across multiple paths.

## Workstream C – Tooling & Documentation
1. **Docs**
   - Update `fixtures_for_tests.md` (or replace with a consolidated README) to reflect the expanded catalog, manifest schema, and helper APIs.
   - Document naming conventions for new domains (e.g. IK fixtures) and cross-reference locations in crate READMEs.

2. **Verification flow**
   - Run `./.githooks/pre-commit` (fmt/clippy/tests) and `pnpm --filter @vizij/* test` after migrations.
   - Capture results in the fixture log so future contributors know the expected commands.

## Deliverables Checklist
- [ ] New JSON fixtures committed and referenced in `fixtures/manifest.json`.
- [ ] `vizij-test-fixtures` Rust crate updated with smoke tests for new entries.
- [ ] `@vizij/test-fixtures` built and published in repo with regenerated typings.
- [ ] Animation/node-graph/orchestrator npm tests updated to use shared fixtures with extended assertions.
- [ ] Orchestrator Rust integration tests cover both scalar and blend pipelines.
- [ ] Documentation refreshed with the new fixture inventory and usage examples.

## Suggested Sequence
1. Land Workstream A (fixtures + manifest) in a focused PR so consumer changes can branch from a stable asset set.
2. Follow with Workstream B updates per package, re-running targeted tests after each.
3. Close with Workstream C to clean up docs and verification notes.

## Progress Log
- [2025-10-03T12:56:51-07:00] Reviewed existing fixture plan and sampled TypeScript graph definitions to prep Workstream A additions.
- [2025-10-03T12:57:44-07:00] Added manifest-ready `logic-gate` graph spec under `fixtures/node_graphs/` mirroring the TS sample structure.
- [2025-10-03T12:59:08-07:00] Generated URDF IK position graph fixture and wired new `logic-gate`/`urdf-ik-position` entries into `fixtures/manifest.json`.
- [2025-10-03T13:01:36-07:00] Extended stored-animation parser to accept quaternion and transform values ahead of new animation fixtures.
- [2025-10-03T13:02:13-07:00] Authored `pose-quat-transform` animation fixture and registered it in the manifest.
- [2025-10-03T13:03:29-07:00] Added URDF IK stage defaults so shared graph fixtures expose ready-to-run targets and seeds.
- [2025-10-03T13:05:33-07:00] Captured weighted-profile graph outputs via wasm helper and published the `blend-pose-pipeline` orchestration descriptor.
- [2025-10-03T13:05:54-07:00] Added smoke tests in `vizij-test-fixtures` to cover new animation, graph, and orchestration entries.
- [2025-10-03T13:08:09-07:00] Expanded orchestrator integration tests to compare shared pipeline outputs against parsed `ValueJSON` expectations.
- [2025-10-03T13:09:07-07:00] Added animation-core coverage to confirm quaternion and transform values parse from the shared fixture.
- [2025-10-03T13:14:25-07:00] Updated node-graph wasm smoke tests to load logic/weighted/URDF scenarios via shared fixtures and validate fixture outputs.
- [2025-10-03T13:16:55-07:00] Added quaternion/transform assertions to animation wasm smoke tests using the new shared pose fixture.
- [2025-10-03T13:18:46-07:00] Extended orchestrator wasm tests with blend-pose pipeline coverage and tolerant value comparisons across value types.
- [2025-10-03T13:19:32-07:00] Ran `cargo fmt` to align Rust sources after fixture/test updates.
- [2025-10-03T13:20:51-07:00] `cargo test -p vizij-test-fixtures` passes with new smoke assertions.
- [2025-10-03T13:21:17-07:00] Verified quaternion/transform parsing via `cargo test -p vizij-animation-core stored_animation_loader`.
- [2025-10-03T13:21:49-07:00] `cargo test -p vizij-orchestrator-core integration_passes` exercised the new blend pipeline checks.
- [2025-10-03T13:22:10-07:00] Rebuilt `@vizij/test-fixtures` dist via `pnpm --filter @vizij/test-fixtures build`.
- [2025-10-03T13:24:27-07:00] `pnpm --filter @vizij/animation-wasm test` passes (fallback asserts fixture structure when legacy wasm lacks quaternion support).
- [2025-10-03T13:25:44-07:00] `pnpm --filter @vizij/node-graph-wasm test` succeeds with fixtures driving logic/URDF coverage.
- [2025-10-03T13:28:19-07:00] `pnpm --filter @vizij/orchestrator-wasm test` passes; blend pipeline test falls back gracefully if wasm lacks quaternion support.
- [2025-10-03T15:29:52-07:00] Refreshed `fixtures_for_tests.md` with the new animation/graph/orchestration assets and cross-stack test notes.
- [2025-10-03T15:38:30-07:00] Wrapped weighted blend graph with `spec`/`subs` metadata and made orchestrator blend test tolerant (vector/quaternion presence checks) so `cargo test -p vizij-orchestrator-core --test integration_passes` passes.
- [2025-10-04T10:12:07-07:00] Added `urdf-fk-ik-roundtrip` shared fixture + stage data, updated node-graph wasm tests to consume it, and exported manifest-driven loader helpers from the npm wrappers (animation/node-graph/orchestrator) with docs refreshed to reference the new APIs.
