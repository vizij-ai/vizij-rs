# Ralph Decision Log

- 2026-01-18: Picked R-010 (usage snippets) and added rustdoc improvements in `vizij-api-core` Value docs plus a runnable example for `Value::transform`.
- 2026-01-18: Scope `vizij-api-core` docs pass for iter-01; expanded module-level summaries and docstrings for shapes, typed paths, write ops, coercion, blend, and JSON normalization. Assumed ASCII-only edits for doc output.
- 2026-01-18: Scope `vizij-api-core` docs pass for iter-02; added rustdoc examples for `WriteOp`/`WriteBatch`, normalized write op JSON example, and added focused notes to coercion, json normalization, blend, and typed path docs. Assumed no new feature flags or ABI changes.
- 2026-01-18: Scope `vizij-api-core` docs pass for iter-03; added runnable examples for blend, coercion, and JSON normalization helpers and noted blend extrapolation behavior. Assumed doc-only edits and no feature flag changes.
- 2026-01-18: Scope `vizij-api-core` docs pass for iter-04; added rustdoc examples and error notes for JSON helpers plus iterator docs for `TypedPath` and `WriteBatch`. Verified doc tests for `vizij-api-core`. Assumed docstring-only edits.
- 2026-01-19: Scope `vizij-graph-core` public API docs pass for iter-01; added module-level docs and concise rustdoc on graph types, runtime, and schema registry. Assumed doc-only edits and kept changes within node-graph core.
- 2026-01-19: Scope `vizij-graph-core` eval/types/schema docs pass for iter-02; added field-level rustdoc for node graph schema/types plus clarifying notes in eval helpers and runtime staging. Assumed doc-only edits, no feature flag changes.
