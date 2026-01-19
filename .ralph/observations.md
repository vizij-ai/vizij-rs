
# Ralph Observations (write-only)

Use this file to capture out-of-scope findings such as:
- Code bugs
- Refactor ideas
- Missing tests
- Performance issues
- Future features

Do NOT implement these in the current loop.
- Observed many public APIs with sparse doc comments; consider batching a doc audit pass per crate to identify missing rustdoc examples.
- Node-graph public APIs (types, schema registry, plan cache) still lack runnable examples; consider adding minimal doctests in a future pass once inputs and fixtures are standardized.
# Observations (append-only)

- 2026-01-19: `vizij-graph-core` eval helpers like `read_inputs`/`materialize_outputs` remain undocumented in rustdoc; consider adding brief API notes or making them private if not intended for public use.
- 2026-01-19: `vizij-graph-core` eval helpers (`InputSlots`, `OutputSlots`, `read_inputs`) are public but still lack runnable examples; consider adding doctests or making them `pub(crate)` if external use is not intended.
- 2026-01-19: `vizij-graph-core` eval helpers still lack runnable doctests; consider adding minimal examples for `GraphRuntime::set_input` and `evaluate_all` once fixtures are stable.
- 2026-01-19: `vizij-graph-core` eval helpers now have improved rustdoc, but public API examples for `InputSlots`/`OutputSlots` remain absent; consider adding minimal doctests once a stable fixture or test harness is available.
- 2026-01-19: Orchestrator fixtures and scheduler APIs still lack runnable rustdoc examples; consider adding minimal doctests once fixture JSON stability is confirmed.
- 2026-01-19: Orchestrator fixtures still panic on invalid fixture data; consider adding fallible constructors for host-facing use if external consumers need them.
- 2026-01-19: Orchestrator animation controller docs mention path conventions but still lack runnable doctests; consider adding a minimal example once blackboard JSON setup is stable enough for doctest use.
- 2026-01-19: Scheduler rustdoc examples are now minimal smoke tests; consider wiring full controller examples once fixture JSON is stable enough for doctests.
- 2026-01-19: Animation core interpolation helpers (`interp/functions.rs`) now have runnable rustdoc examples; consider adding coverage for any remaining math helpers if additional functions are exposed.
- 2026-01-19: Bevy animation system fallbacks apply non-typed writes only to Transform properties; consider documenting or extending this for other component types if needed.
- 2026-01-19: `vizij-animation-core` sampling uses a fixed derivative epsilon outside of `BakingConfig` for runtime sampling; consider exposing this in runtime config if host apps need tuning.
- 2026-01-19: Git commits in this worktree fail because the git dir lives under `/home/chris/Code/Semio/vizij_ws/vizij-rs_workspace/.bare`, which is outside the writable roots in this environment.

- 2026-01-19: `bevy_vizij_animation::apply_outputs_system` returns early when `BindingIndex` or `PendingOutputs` are missing, despite docstring previously stating it panics; keep docs aligned with early-return behavior.
- 2026-01-19: Running `./.githooks/pre-commit` fails until doctests avoid `fn main`; keep Bevy plugin examples as `no_run` blocks without a `main` to satisfy clippy.
- 2026-01-19: `CoreEvent` variant fields now have rustdoc notes; remaining animation docs appear covered, but consider adding a single example of event handling if a stable fixture or minimal path exists.
- 2026-01-19: `vizij-animation-core::Config` scratch buffer fields are only capacity hints; consider adding perf guidance once scratch buffers are wired into runtime reuse.
- 2026-01-19: `bevy_vizij_graph` lacks usage examples in rustdoc; consider adding a short `no_run` snippet once Bevy plugin setup conventions are settled.
- 2026-01-19: `vizij-graph-core` eval helpers (`InputSlots`/`OutputSlots`/`read_inputs`) still have no runnable examples; consider adding minimal no_run doctests once a stable minimal spec exists.
- 2026-01-19: `pnpm changeset` failed with `Permission denied` due to missing/blocked `node_modules`; doc-only changes may still need a changeset once dependencies are installed.
- 2026-01-19: `bevy_vizij_graph` now has a minimal `VizijGraphPlugin` `no_run` example, but other Bevy adapters may still lack runnable snippets; consider aligning examples across Bevy crates.
- 2026-01-19: `vizij-graph-wasm` public wasm bindings still lacked rustdoc on several methods; added summaries but consider adding minimal JS usage examples if autogen docs need them.
- 2026-01-19: `vizij-graph-core` eval helpers still lack runnable examples for `InputSlots`/`OutputSlots` and `materialize_outputs`; consider adding minimal doctests once a stable micro-spec is defined.
- 2026-01-19: Doc tests for `bevy_vizij_api` require avoiding `anyhow` in examples; prefer `expect`-based fallbacks since the crate has no `anyhow` dependency.
- 2026-01-19: `vizij-graph-wasm` staging helpers (`stage_inputs_batch`, slot staging) only accept scalar floats, despite docs and API name suggesting vector support; consider adding vector batch APIs or clarifying naming for scalar-only staging.
- 2026-01-19: `bevy_vizij_graph` README referenced stale resource/event names (`GraphSpecRes`, `EvalResultRes`, `PendingInputs`); keep README aligned with actual exports when APIs shift.

- R-015
- Title: wasm binding docs missing JS examples for api/graph/orchestrator helpers
- Type: Docs
- Impact: Low
- Effort: S
- Evidence: Wasm bindings rely on JS usage but docs only mention inputs without examples
- Next action: Add JS examples to remaining wasm helpers if any new APIs are added
- Status

- 2026-01-18: `vizij-graph-core` URDF IK helpers (`eval/urdfik.rs`) still lack any rustdoc examples; consider adding minimal no_run snippets once a stable URDF fixture exists.

- 2026-01-19: Public eval helpers still need to be referenced via internal modules; examples were marked `ignore` to avoid referencing private `plan`/`value_layout` paths. Consider re-exporting minimal types or adding public constructors if you want runnable doctests.
- 2026-01-19: `vizij-test-fixtures` doctest examples now depend on fixture keys like `pose-quat-transform` and `simple-gain-offset`; if fixtures rename, update doctests to avoid brittle failures.
- 2026-01-19: `vizij-api-core` `ValueKind` and `ShapeId` enums now have variant-level docs; consider adding concise variant docs to any remaining public enums in other core crates.
- 2026-01-19: Placeholder wording in `vizij-animation-core` scratch/interp rustdoc replaced; remaining placeholder-like docs appear resolved for these modules.

- 2026-01-19: Orchestrator wasm `set_input` rustdoc previously showed non-canonical `{ kind, value }` JSON; updated example to match Value shorthand to avoid confusion in JS docs.
- 2026-01-19: Node-graph wasm docs could benefit from a brief note about `eval_all_slots`/`get_outputs_*` flow; consider adding a small JS usage snippet if docs still feel abstract.
- 2026-01-19: Bevy adapters now have corrected canonical TypedPath examples; ensure future changes keep `/Transform.*` suffixes consistent with TypedPath canonicalization to avoid drift in docs.
