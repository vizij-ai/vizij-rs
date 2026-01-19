# Ralph Primary Backlog — Docs (Rust docstrings -> autogen docs)

## Top 10 next
1. R-001 — Audit public Rust APIs for missing or placeholder docstrings
2. R-003 — Add runnable code examples for key public APIs
3. R-010 — Add usage snippets for frequently used types and helpers
4. R-004 — Standardize module-level docs and `//!` overviews
5. R-002 — Establish docstring conventions for examples, panics, safety, and errors
6. R-006 — Add cross-links between core crates and wasm wrappers
7. R-007 — Add doc tests or validate examples against current APIs
8. R-008 — Ensure docstrings mention JSON/ABI versioning contracts
9. R-005 — Document feature flags and conditional compilation behavior
10. R-009 — Document performance considerations and allocation hot spots

---

## Backlog items

### R-001 — Audit public Rust APIs for missing or placeholder docstrings
- Type: Docs
- Impact: High
- Effort: L
- Evidence: Autogen docs will expose public APIs that currently lack narrative or examples
- Next action: Audit remaining wasm/helper crates and any non-core crates for missing rustdoc (vizij-api-wasm, vizij-test-fixtures, bevy_vizij_graph/api, bevy_vizij_orchestrator once added).
- Status: In progress (iter-03 updated vizij-api-wasm + vizij-test-fixtures)

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
- Next action: Add minimal orchestrator examples once fixture JSON stability is confirmed (avoid brittle doctests); keep examples to light `no_run` for fixtures.
- Status: In progress

### R-004 — Standardize module-level docs and `//!` overviews
- Type: Docs
- Impact: Med
- Effort: M
- Evidence: Some modules lack context or usage overview
- Next action: Refresh `//!` blocks for remaining stacks (animation core/Bevy/wasm) and any module stubs; Bevy animation submodules now documented.
- Status: In progress

### R-005 — Document feature flags and conditional compilation behavior
- Type: Docs
- Impact: Med
- Effort: M
- Evidence: Feature-gated behavior affects API surface in docs
- Next action: Add doc notes for feature flags in relevant crates
- Status: Planned

### R-006 — Add cross-links between core crates and wasm wrappers
- Type: Docs
- Impact: Med
- Effort: S
- Evidence: Consumers need to understand Rust vs wasm API mapping
- Next action: Add `See also` links between core and wasm types
- Status: Planned

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
- Status: Planned

### R-009 — Document performance considerations and allocation hot spots
- Type: Docs
- Impact: Low
- Effort: M
- Evidence: Generated docs should note hot paths for integrators
- Next action: Add perf notes where relevant in engine loops and graph eval
- Status: Planned

### R-010 — Add usage snippets for frequently used types and helpers
- Type: Docs
- Impact: Med
- Effort: M
- Evidence: Users need examples beyond the top-level API; Value helpers, ValueKind links, and JSON tag notes need concise coverage (merged R-011/R-012/R-013). Eval/runtime docs now mention errors but still lack examples.
- Next action: Add short doc examples for any remaining public APIs that lack runnable snippets (likely outside animation stack). Avoid brittle doctests for fixture-heavy APIs.
- Status: In progress (iter-02 added clarifying notes for eval helpers; examples still pending)

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
