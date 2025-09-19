# Evaluation Module

This directory hosts the evaluation runtime for Vizij node graphs. Each module focuses on one
aspect of executing a `GraphSpec`:

- `mod.rs` wires the submodules together and exposes the primary entry points (`GraphRuntime` and
  `evaluate_all`).
- `graph_runtime.rs` keeps per-node state and staging buffers that persist across frames.
- `value_layout.rs` provides utilities for flattening structured values so math operators can work
  on raw `f32` buffers.
- `shape_helpers.rs` validates node outputs against declared shapes.
- `numeric.rs` and `variadic.rs` implement reusable math helpers for binary, unary, and variadic
  operations.
- `eval_node.rs` contains the dispatch logic for every `NodeType` variant.
- `urdfik.rs` is gated behind the `urdf_ik` feature and wraps kinematics helpers.
- `tests.rs` contains behavioural coverage for the evaluation path, including URDF-dependent
  scenarios behind the same feature gate.

The goal of this layout is to keep the high-level evaluation flow (`evaluate_all`) compact while the
submodules own domain-specific details like value coercion or IK solver setup.
