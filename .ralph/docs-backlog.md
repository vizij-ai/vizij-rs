# Ralph Primary Backlog — Docs (Rust docstrings -> autogen docs)

## Top 10 next
1. R-001 — Audit public Rust APIs for missing or placeholder docstrings
2. R-003 — Add runnable code examples for key public APIs
3. R-010 — Add usage snippets for frequently used types and helpers
4. R-004 — Standardize module-level docs and `//!` overviews
5. R-002 — Establish docstring conventions for examples, panics, safety, and errors
6. R-006 — Add cross-links between core crates and wasm wrappers
7. R-007 — Add doc tests or validate examples against current APIs
8. R-016 — Add JSDoc coverage for npm packages used in JS autogen docs
9. R-008 — Ensure docstrings mention JSON/ABI versioning contracts
10. R-005 — Document feature flags and conditional compilation behavior

---

## Backlog items

### R-001 — Audit public Rust APIs for missing or placeholder docstrings
- Type: Docs
- Impact: High
- Effort: L
- Evidence: Autogen docs will expose public APIs that currently lack narrative or examples
- Next action: Audit remaining wasm/helper crates and any non-core crates for missing rustdoc (bevy_vizij_orchestrator once added).
- Status: In progress (iter-07 added enum rustdoc examples for core crates; iter-06 updated Bevy adapters: corrected TypedPath examples, clarified Transform alias registration, added small resource snippets)

### R-002 — Establish docstring conventions for examples, panics, safety, and errors
- Type: Docs
- Impact: High
- Effort: M
- Evidence: Consistency needed for `cargo doc` output and downstream JS consumers
- Next action: Draft a short conventions section in a shared doc (or top-level crate docs)
- Status: Planned

### R-003 — Add runnable code examples for key public APIs
- Type: Docs
- Impact: High
- Effort: L
- Evidence: Autogen docs are less useful without concrete usage examples; added examples across animation core engine, baking helpers, and interpolation utilities.
- Next action: Add minimal orchestrator examples once fixture JSON stability is confirmed (avoid brittle doctests); keep examples to light `no_run` for fixtures; revisit node-graph eval helpers if public re-exports are added (current examples are `ignore`); add JS examples for wasm graph slot staging helpers.
- Status: In progress (iter-06 added Bevy adapter resource examples; continue with orchestrator + wasm JS usage)

### R-004 — Standardize module-level docs and `//!` overviews
- Type: Docs
- Impact: Med
- Effort: M
- Evidence: Some modules lack context or usage overview
- Next action: Refresh `//!` blocks for remaining stacks (animation core/Bevy/wasm) and any module stubs; add module-level docs to remaining wasm and helper crates.
- Status: In progress (iter-11 added module docs for registry export CLI)

### R-005 — Document feature flags and conditional compilation behavior
- Type: Docs
- Impact: Med
- Effort: M
- Evidence: Feature-gated behavior affects API surface in docs
- Next action: Add doc notes for feature flags in relevant crates
- Status: In progress (iter-12 added urdf_ik notes in vizij-graph-core)

### R-006 — Add cross-links between core crates and wasm wrappers
- Type: Docs
- Impact: Med
- Effort: S
- Evidence: Consumers need to understand Rust vs wasm API mapping
- Next action: Add `See also` links between core and wasm types
- Status: In progress (iter-05 clarified api-wasm rustdoc examples with wasm init usage)

### R-007 — Add doc tests or validate examples against current APIs
- Type: Docs
- Impact: Med
- Effort: M
- Evidence: Examples can drift without doc tests
- Next action: Convert key examples into `rustdoc` tests or verify manually; iter-14 ran `cargo test -p vizij-animation-core --doc`.
- Status: In progress

### R-008 — Ensure docstrings mention JSON/ABI versioning contracts
- Type: Docs
- Impact: Med
- Effort: S
- Evidence: ABI changes must be visible in generated docs
- Next action: Add versioning notes to public APIs that serialize/deserialize JSON
- Status: In progress (iter-10 added ABI notes to wasm bindings)

### R-009 — Document performance considerations and allocation hot spots
- Type: Docs
- Impact: Low
- Effort: M
- Evidence: Generated docs should note hot paths for integrators
- Next action: Add perf notes where relevant in engine loops and graph eval
- Status: Planned

### R-016 — Add JSDoc coverage for npm packages used in JS autogen docs
- Type: Docs
- Impact: Med
- Effort: M
- Evidence: JS/TS exports in `npm/@vizij/*` lack JSDoc summaries, hurting JS doc autogen.
- Next action: Add concise JSDoc for remaining public exports in wasm wrappers and any TS declaration files that still lack summaries/examples.
- Status: In progress (iter-01 added JSDoc for wasm-loader/value-json/test-fixtures and wasm wrapper entry points; iter-03 tightened fixtures and node-graph metadata helper docs; iter-04 added JSDoc examples for node-graph/orchestrator wasm wrappers and fixtures; iter-05 filled browser test-fixtures and orchestrator-wasm types JSDoc gaps; iter-06 added JSDoc summaries to animation-wasm + node-graph-wasm type exports; iter-07 added field-level JSDoc for orchestrator-wasm types; iter-08 added usage examples to test-fixtures JS helpers; iter-09 added field-level JSDoc for animation-wasm types and documented node-graph sample maps)

### R-010 — Add usage snippets for frequently used types and helpers
- Type: Docs
- Impact: Med
- Effort: M
- Evidence: Users need examples beyond the top-level API; Value helpers, ValueKind links, and JSON tag notes need concise coverage (merged R-011/R-012/R-013). Eval/runtime docs now mention errors but still lack examples.
- Next action: Add short doc examples for any remaining public APIs that lack runnable snippets (likely outside animation stack). Avoid brittle doctests for fixture-heavy APIs.
- Status: In progress (iter-06 added Bevy adapter resource examples; continue with remaining public helpers)

### R-011 — Add docstrings for `Value` enum variants needing clarity
- Type: Docs
- Impact: Med
- Effort: S
- Evidence: Several `Value` variants use brief comments (e.g., List vs Array) that could be clearer for docs
- Next action: Expand variant docs with concise distinctions where needed
- Status: Completed

### R-012 — Add rustdoc examples for `WriteBatch` construction patterns
- Type: Docs
- Impact: Med
- Effort: S
- Evidence: `WriteBatch` is common in orchestrator/animation outputs but has no examples
- Next action: Add follow-up examples if additional patterns emerge
- Status: Completed

### R-013 — Document `ShapeId` and `Shape` JSON expectations
- Type: Docs
- Impact: Low
- Effort: S
- Evidence: Shape structs serialize across crates but docstrings omit JSON shape expectations
- Next action: Add brief notes in `shape.rs` about JSON fields and intended use
- Status: Completed

### R-014 — Clarify wasm batch staging semantics for scalar-only inputs
- Type: Docs
- Impact: Low
- Effort: S
- Evidence: `vizij-graph-wasm` batch staging helpers only accept scalars but docs implied vectors.
- Next action: Ensure rustdoc and README describe scalar-only staging, or add vector batch API if intended.
- Status: Completed (iter-10 clarified scalar-only staging in graph wasm docs)
