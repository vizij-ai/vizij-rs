# Evaluation Module Guide

This directory contains the modules that implement the `vizij-graph-core` evaluation pipeline. The goal is to keep the public entry points (`GraphRuntime`, `evaluate_all`) concise while domain-specific logic lives in focused submodules.

---

## Module Summary

| File | Responsibility |
|------|----------------|
| `mod.rs` | Re-exports the key types (`GraphRuntime`, `StagedInput`, `evaluate_all`) and wires submodules together. |
| `graph_runtime.rs` | Maintains per-node state, staged inputs, output caches, and frame timing (`t`/`dt`). |
| `value_layout.rs` | Flattens structured `Value` instances into contiguous numeric buffers, supports reconstruction, and handles scalar broadcasting. |
| `shape_helpers.rs` | Enforces declared output shapes and performs numeric coercions/null-of-shape fills. |
| `numeric.rs` / `variadic.rs` | Shared math utilities for unary/binary/variadic operations. |
| `eval_node.rs` | Dispatches every `NodeType` implementation, combining helpers from the other modules. |
| `urdfik.rs` | Optional robotics helpers gated behind the `urdf_ik` feature (URDF parsing, IK/FK solvers). |
| `tests.rs` | Behavioural coverage for evaluator paths (selectors, staging, blends, URDF flows). |
| `blend_tests.rs` | Focused regression tests for blend nodes. |

---

## Evaluation Flow

1. **Staging** – Hosts call `GraphRuntime::advance_epoch` and `GraphRuntime::set_input` to queue inputs for the upcoming frame.
2. **Topology** – `evaluate_all` computes a topological ordering, clears cached outputs, and retains node state for nodes still present in the spec.
3. **Per-node evaluation** – For each node ID, `eval_node.rs`:
   - Reads inputs via `graph_runtime.rs` staging helpers.
   - Uses `value_layout.rs` to flatten numeric content when needed.
   - Leverages `numeric.rs` / `variadic.rs` for math-heavy nodes.
   - Applies selectors and enforces output shapes through `shape_helpers.rs`.
   - Records `WriteOp`s for sink nodes (`Output`).
4. **Result aggregation** – Outputs and write batches are returned to the caller; persistent state remains in `GraphRuntime`.

---

## Development Notes

- Keep node-specific logic inside `eval_node.rs`. Shared helpers should live in `numeric.rs`, `variadic.rs`, or dedicated modules to avoid duplication.
- When adding new node types:
  1. Extend `NodeType` in `types.rs`.
  2. Implement evaluation logic in `eval_node.rs` (create helper functions to keep the main match concise).
  3. Update tests to cover success and failure paths (shape enforcement, selector errors, etc.).
- If a node requires persistent state, use the state helpers in `graph_runtime.rs` (see Spring/Damp/Slew implementations).
- URDF-specific code must stay behind the `urdf_ik` feature flag to keep the core dependency light.

---

## Testing

- Run `cargo test -p vizij-graph-core` to cover all modules, including the evaluation tests here.
- The `blend_tests.rs` file complements `tests.rs` with additional coverage for blending operations.
- When introducing new nodes or selector behaviour, add dedicated tests to keep regressions visible.

---

Need to touch this layout? Keep the table above up to date so contributors can navigate the module quickly. 🧭
