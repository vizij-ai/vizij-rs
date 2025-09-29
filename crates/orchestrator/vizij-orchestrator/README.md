# vizij-orchestrator

Orchestrator crate to coordinate vizij graphs and animations. This crate provides a small, deterministic runtime that composes multiple Graph controllers and Animation engines against a shared Blackboard. It demonstrates deterministic pass scheduling, last-writer-wins merging, conflict logging, and a small host API for stepping and wiring controllers.

This README documents the crate layout, public API, conventions (Blackboard/TyperPath mappings), examples, and how to run tests and examples.

---

## Overview

Purpose
- Provide a decoupled orchestrator that coordinates:
  - Graph evaluation (vizij-graph-core)
  - Animation engines (vizij-animation-core)
  - Shared data movement via `vizij-api-core` primitives (TypedPath, Value, Shape, WriteBatch)
- Support deterministic schedules:
  - `SinglePass`: Animations → Graphs
  - `TwoPass`: Graphs → Animations → Graphs
- Accumulate a per-frame merged WriteBatch and publish into a Blackboard with provenance and conflict logs.
- Allow per-graph Subscriptions to restrict which Blackboard paths are staged and which outputs are published.

Design choices
- Default merge policy: last-writer-wins inside a pass (deterministic by pass and controller order).
- Minimal conventions for mapping Blackboard → Animation Inputs (documented below).
- Graph-level optional Subscriptions to keep staging and publishing explicit and deterministic.

---

## Layout

- src/
  - lib.rs — public crate API (Orchestrator, OrchestratorFrame, exported helpers)
  - blackboard.rs — Blackboard implementation, BlackboardEntry, ConflictLog
  - scheduler.rs — run_single_pass / run_two_pass and per-frame diagnostics + merged_writes accumulation
  - controllers/
    - mod.rs
    - graph.rs — GraphController, Subscriptions, GraphControllerConfig
    - animation.rs — AnimationController (Engine wrapper), conservative mapping from Blackboard -> Inputs
  - diagnostics.rs — diagnostics placeholder
- tests/
  - integration_passes.rs — integration tests for SinglePass and TwoPass
- examples/
  - graph_only.rs — minimal graph-only example
  - single_pass.rs — minimal single-pass orchestrator (graph + animation controller)
  - two_pass.rs — minimal two-pass orchestrator example
- Cargo.toml — crate manifest (workspace member)

---

## Public API (high level)

The crate exposes these primary types through the crate root:

- Orchestrator
  - new(schedule: Schedule) -> Orchestrator
  - with_graph(self, cfg: GraphControllerConfig) -> Self
  - with_animation(self, cfg: AnimationControllerConfig) -> Self
  - set_input(&mut self, path: &str, value: serde_json::Value, shape: Option<serde_json::Value>)
  - step(&mut self, dt: f32) -> Result<OrchestratorFrame>

- OrchestratorFrame
  - epoch: u64
  - dt: f32
  - merged_writes: WriteBatch (deterministic merged writes for the frame)
  - conflicts: Vec<serde_json::Value> (diagnostic conflict logs)
  - timings_ms: HashMap<String, f32>
  - events: Vec<serde_json::Value>

- GraphControllerConfig / Subscriptions
  - Subscriptions { inputs: Vec<TypedPath>, outputs: Vec<TypedPath>, mirror_writes: bool }
  - GraphControllerConfig { id, spec, subs }

- AnimationControllerConfig
  - id: String
  - setup: serde_json::Value (future wiring for loading animation JSON / prebinds)

Notes:
- The orchestrator crate relies on `vizij-api-core` types (TypedPath, Value, WriteBatch) for all data movement. No duplicate definitions are introduced.
- Subscriptions let you declare what a graph expects to consume (inputs) and what it should publish (outputs). If `outputs` is empty, the graph's returned writes are fully published; otherwise only listed paths are published.

---

## Blackboard conventions / Animation mapping

Blackboard entries map `TypedPath` → `BlackboardEntry { value, shape?, epoch, source, priority }`

Basic conventions used by AnimationController for v1:
- Player commands: TypedPath `anim/player/<player_id>/cmd/<action>`
  - Supported actions:
    - `play`, `pause`, `stop`
    - `set_speed` — value must be a Value::Float
    - `seek` — value must be a Value::Float
- Instance updates: TypedPath `anim/player/<player_id>/instance/<inst_id>/<field>`
  - Supported fields: `weight`, `time_scale`, `start_offset`, `enabled`
  - Values must match types (Float for numeric fields, Bool for enabled)

These are host-to-engine conventions used by the orchestrator's AnimationController to build `vizij_animation_core::Inputs`.

GraphController staging:
- Only TypedPaths listed in `Subscriptions.inputs` are staged into the `GraphRuntime` before evaluate. This keeps behavior deterministic and avoids staging unrelated Blackboard entries.

Graph output publishing:
- GraphController.evaluate returns a WriteBatch (combined: pre-populated writes + writes produced during evaluation).
- The scheduler publishes only the subset of returned writes that match `Subscriptions.outputs` (if non-empty). If `Subscriptions.outputs` is empty, all returned writes are published.

---

## Examples

Examples are in `crates/orchestrator/vizij-orchestrator/examples/`:

- graph_only.rs
  - Demonstrates injecting a pre-populated graph runtime write and stepping the orchestrator.
  - Shows merged_writes and Blackboard entries after a single step.

- single_pass.rs
  - Registers a graph and an animation controller and steps under `Schedule::SinglePass`.
  - Injects a graph write to demonstrate merged_writes.

- two_pass.rs
  - Registers two graphs and steps under `Schedule::TwoPass`.
  - Injects writes into each graph runtime to simulate multi-pass behavior and feedback.

Run an example:
- Build examples:
  cargo build --manifest-path crates/orchestrator/vizij-orchestrator/Cargo.toml --examples

- Run a single example:
  cargo run --manifest-path crates/orchestrator/vizij-orchestrator/Cargo.toml --example graph_only

Output from the examples (what you should expect)
- Examples print the OrchestratorFrame epoch, the `merged_writes` (JSON) and any Blackboard entries created by the scheduler. They are small smoke tests for the orchestrator flow.

---

## Tests

- Unit tests:
  - Blackboard behavior and ConflictLog semantics are covered in `src/blackboard.rs` tests.
  - AnimationController mapping unit test is in `src/controllers/animation.rs` (Blackboard→Inputs mapping).
- Integration tests:
  - `tests/integration_passes.rs` validates SinglePass and TwoPass merged_writes and Blackboard application.

Run tests for the orchestrator crate:
- Unit + integration:
  cargo test --manifest-path crates/orchestrator/vizij-orchestrator/Cargo.toml

Run just the integration tests:
- cargo test --manifest-path crates/orchestrator/vizij-orchestrator/Cargo.toml --test integration_passes

---

## Next steps (recommended)

If you want to extend this crate, consider:
- Add more unit tests for Subscriptions and mapping (edge cases and invalid paths).
- Improve the Blackboard→Inputs mapping or provide a configurable resolver for player/instance IDs.
- Wire example animations (load real animation JSON) into AnimationController via `setup`.
- Implement feature-gated `AnimationPlayer` within `vizij-graph-core` (planned next after stabilization).
- Add CI integration to run orchestrator crate tests and examples automatically.

---

## Notes for reviewers / maintainers

- The orchestrator intentionally keeps the cross-crate coupling minimal:
  - It depends on `vizij-graph-core` and `vizij-animation-core` for runtime logic.
  - Cross-crate features (like embedding an AnimationPlayer node in the graph) are intentionally feature-gated to avoid tight coupling.
- Merging semantics:
  - Writes are appended in deterministic order: pass order → controller order. Last-writer-wins within a pass (the scheduler applies controllers sequentially and records conflicts).
- Diagnostics:
  - `OrchestratorFrame` includes `timings_ms` and `conflicts` to help instruments and debug frames.

If you'd like, I can:
- Add more comprehensive examples loading real animation fixtures from `crates/animation/test_fixtures`.
- Prepare a PR branch and run the workspace CI (I can create the branch locally; you'll need to push/test the PR).
