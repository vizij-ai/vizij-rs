# vizij-orchestrator-core

> **Deterministic multi-pass scheduling for Vizij – orchestrate graphs, animations, and blackboard state from Rust.**

`vizij-orchestrator-core` coordinates Vizij graph controllers and animation engines against a shared blackboard. It stages inputs, executes controllers in configurable passes, merges writes deterministically, and logs conflicts for diagnostics. The crate underpins the WebAssembly binding (`vizij-orchestrator-wasm`) and the React wrapper (`@vizij/orchestrator-react`).

---

## Table of Contents

1. [Overview](#overview)
2. [Features](#features)
3. [Installation](#installation)
4. [Quick Start](#quick-start)
5. [Workflow](#workflow)
6. [Key Concepts](#key-concepts)
7. [Development & Testing](#development--testing)
8. [Related Packages](#related-packages)

---

## Overview

- Manages a **shared blackboard** backed by `vizij-api-core` Value/Shape/TypedPath types.
- Hosts **GraphController** and **AnimationController** instances, each with user-defined IDs and subscription rules.
- Provides deterministic **scheduling strategies** (`SinglePass`, `TwoPass`, future `RateDecoupled`) with per-pass timings.
- Produces an `OrchestratorFrame` containing merged writes, conflict logs, timing metrics, and events.

---

## Features

- **Subscriptions** control which blackboard paths a graph consumes (`inputs`) and which writes are published (`outputs`) with optional mirroring for internal state.
- **Epoch-based staging** ensures graph runtimes only see current-frame inputs; stale entries are dropped automatically.
- **Conflict logging** records previous vs. new values whenever multiple controllers write to the same path.
- **Animation mapping** translates blackboard entries into `vizij-animation-core::Inputs` using a conservative naming convention.
- **Time propagation** – `GraphController::evaluate` advances `GraphRuntime.t`/`dt` before invoking `vizij-graph-core`, so time-based nodes respond to frame delta correctly.

---

## Installation

```bash
cargo add vizij-orchestrator-core
```

The crate depends on `vizij-graph-core`, `vizij-animation-core`, and `vizij-api-core`. Optional Bevy adapters can be layered on top (planned).

---

## Quick Start

```rust
use vizij_orchestrator_core::{
    Orchestrator, Schedule,
    controllers::{GraphControllerConfig, Subscriptions},
};
use vizij_graph_core::types::GraphSpec;

let graph_spec: GraphSpec = serde_json::from_str(include_str!("../../../../fixtures/node_graphs/simple-gain-offset.json"))?;

let graph_cfg = GraphControllerConfig {
    id: "graph:demo".into(),
    spec: graph_spec,
    subs: Subscriptions::default(), // stage all inputs, publish all outputs
};

let mut orchestrator = Orchestrator::new(Schedule::SinglePass)
    .with_graph(graph_cfg);

// Optional: inject staged inputs directly onto the blackboard
orchestrator.set_input(
    "demo/input/value",
    serde_json::json!({ "type": "float", "data": 1.0 }),
    None,
)?;

let frame = orchestrator.step(1.0 / 60.0)?;
println!("epoch {} merged writes: {:?}", frame.epoch, frame.merged_writes);
```

---

## Workflow

1. **Create an orchestrator** with a schedule (`SinglePass` or `TwoPass`).
2. **Register controllers.**
   - Graph controllers wrap `GraphSpec` + `Subscriptions`.
   - Animation controllers accept an ID and optional setup payload (`AnimationControllerConfig`).
3. **Stage host inputs** on the blackboard via `Orchestrator::set_input` (or directly through `blackboard.set` if you need typed control).
4. **Step the orchestrator** with a delta time.
   - `step(dt)` increments the epoch, runs the configured passes, applies writes with provenance, and returns an `OrchestratorFrame`.
5. **Consume results.**
   - Use `frame.merged_writes` as the external surface for downstream systems.
   - Inspect `frame.conflicts` for debugging.
   - Read `frame.events` (populated by animation controllers) for diagnostics.

---

## Key Concepts

### Blackboard

- `BlackboardEntry` stores `{ value, shape?, epoch, source, priority }`.
- `set` and `set_entry` insert values keyed by `TypedPath`.
- `apply_writebatch` merges controller outputs with last-writer-wins semantics and emits `ConflictLog` instances when existing entries are overwritten.

### Subscriptions

- `inputs` – list of `TypedPath`s to stage into a graph runtime before evaluation. Unlisted paths are ignored, guaranteeing deterministic input sets.
- `outputs` – optional filter determining which writes are published. Empty => publish all.
- `mirror_writes` – if true (default), the entire write batch updates the blackboard even when `outputs` filters the public result. Turn off to keep private state hidden.

### Schedules

- **SinglePass:** Animations → Graphs.
- **TwoPass:** Graphs → Animations → Graphs (supports feedback loops where animations need graph output and vice versa).
- `Schedule::RateDecoupled` is reserved for future work; it currently aliases to `SinglePass`.

### Animation Controller Path Conventions

Blackboard paths are parsed to build `vizij_animation_core::Inputs`:

- `anim/player/<player_id>/cmd/play|pause|stop|set_speed|seek`
- `anim/player/<player_id>/instance/<inst_id>/weight|time_scale|start_offset|enabled`

Unrecognised paths are ignored to keep the mapping conservative.

### Time Propagation

`GraphController::evaluate` updates `GraphRuntime.dt` with a clamped, finite delta and increments `GraphRuntime.t`. Time-based nodes (`Time`, `Spring`, `Damp`, `Slew`, oscillators) therefore respond correctly when orchestrator hosts call `step(dt)`. If you embed `GraphController` elsewhere, mimic this behaviour.

---

## Development & Testing

Run unit and integration tests:

```bash
cargo test -p vizij-orchestrator-core
```

Notable tests:

- `src/blackboard.rs` – entry storage and conflict logging semantics.
- `src/controllers/animation.rs` – blackboard → animation input mapping.
- `tests/integration_passes.rs` – end-to-end coverage for `SinglePass` and `TwoPass` schedules.

Helpful workspace scripts:

```bash
pnpm run test:rust                 # fmt, clippy, tests across the workspace
pnpm run build:wasm:orchestrator   # rebuilds the WASM adapter that embeds this crate
pnpm run watch:wasm:orchestrator   # rebuild continuously (requires cargo-watch)
```

Examples (under `examples/`) demonstrate minimal graph-only, single-pass, and two-pass orchestrations you can run with:

```bash
cargo run -p vizij-orchestrator-core --example single_pass
```

---

## Related Packages

- [`vizij-graph-core`](../../node-graph/vizij-graph-core/README.md) – graph runtime used by graph controllers.
- [`vizij-animation-core`](../../animation/vizij-animation-core/README.md) – animation engine wrapped by animation controllers.
- [`vizij-orchestrator-wasm`](../vizij-orchestrator-wasm/README.md) – wasm-bindgen binding replicating the host API for JavaScript environments.
- [`@vizij/orchestrator-react`](../../../vizij-web/packages/@vizij/orchestrator-react/README.md) – React provider and hooks built on the wasm binding.

Need help or spotted an inconsistency? Open an issue in the Vizij repository—predictable orchestrations keep downstream toolchains healthy. 🔁
