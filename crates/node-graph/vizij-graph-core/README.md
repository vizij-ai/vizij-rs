# vizij-graph-core

> **Deterministic data-flow evaluation for Vizij graphs – pure Rust with predictable staging, shapes, and side effects.**

`vizij-graph-core` turns declarative `GraphSpec` documents into structured values and typed write operations. The crate powers Vizij’s node graph tooling, Bevy integrations, and WebAssembly bindings.

---

## Table of Contents

1. [Overview](#overview)
2. [Features](#features)
3. [Installation](#installation)
4. [Quick Start](#quick-start)
5. [Usage](#usage)
6. [Key Concepts](#key-concepts)
7. [Development & Testing](#development--testing)
8. [Related Packages](#related-packages)

---

## Overview

- **Pure Rust runtime** that interprets `GraphSpec` documents using the shared `vizij-api-core` Value/Shape contract.
- **GraphRuntime** retains staged inputs, node-local state, and cached outputs across frames.
- **evaluate_all** walks the graph in topological order, performs selector projection, enforces declared shapes, and collects sink writes.
- **Extensible node library** covering math, logic, vector ops, time/transition nodes (Spring/Damp/Slew), blending, and (optionally) robotics IK/FK helpers behind the `urdf_ik` feature flag.

---

## Features

- Deterministic topological evaluation with cycle detection.
- Input staging model with epoch tracking to prevent stale data from leaking between frames.
- Shape-aware validators (`Shape`, `ShapeId`) for predictable coercions and helpful diagnostics.
- Selector support (`field`, `index`) on edges for structured projection.
- External `WriteBatch` accumulation from sink nodes for host-controlled side effects.
- Optional URDF IK/FK nodes compiled in via the `urdf_ik` feature (enabled by default).

---

## Installation

```bash
cargo add vizij-graph-core
```

Features:

| Feature   | Default | Description                                           |
|-----------|---------|-------------------------------------------------------|
| `urdf_ik` | ✔       | Enables URDF chain parsing + IK/FK nodes (depends on `k` and `urdf-rs`). |

Disable defaults with `--no-default-features` if you want a minimal build.

---

## Quick Start

```rust
use vizij_api_core::{TypedPath, Value, Shape};
use vizij_graph_core::{evaluate_all, GraphRuntime};
use vizij_graph_core::types::GraphSpec;

let spec: GraphSpec = serde_json::from_str(include_str!("../../../../fixtures/node_graphs/simple-gain-offset.json"))?;

let mut runtime = GraphRuntime::default();

// Stage an IK target for the next frame
let target = TypedPath::parse("robot/arm/ik_target")?;
runtime.advance_epoch();
runtime.set_input(target, Value::vec3([0.1, 0.2, 0.3]), Some(Shape::vec3()));

let result = evaluate_all(&mut runtime, &spec)?;

for (node_id, outputs) in &result.nodes {
    println!("node {node_id}: {outputs:#?}");
}
for write in &result.writes {
    println!("write {:?} -> {:?}", write.path, write.value);
}
```

---

## Usage

1. **Load or construct a `GraphSpec`.** Use `serde_json` to parse JSON, or build specs programmatically during tests.
2. **Create and reuse a `GraphRuntime`.** It retains node-local state (`Spring`, `Slew`, etc.), staged inputs, and cached outputs across frames.
3. **Stage host inputs.**
   ```rust
   runtime.advance_epoch();
   runtime.set_input(path, value, declared_shape);
   ```
   - Declared shapes help coerce numeric data or produce deterministically “null-of-shape” placeholders when staging fails.
4. **Evaluate the graph.**
   ```rust
   let eval = evaluate_all(&mut runtime, &spec)?;
   ```
5. **Consume results.**
   - `eval.nodes` – map of node → output key → `PortValue { value, shape }`.
   - `eval.writes` – `WriteBatch` of `WriteOp { path, value, shape }` emitted by `Output` nodes.
6. **Integrate with hosts.** Apply writes to your own blackboard, engine, or animation runtime as needed.

---

## Key Concepts

### GraphRuntime

- Holds `t`/`dt`, per-node persistent state, staged inputs (`HashMap<TypedPath, StagedInput>`), and cached outputs.
- `advance_epoch` bumps the staging epoch and evicts inputs not refreshed for the current frame.
- Evaluation now updates `t`/`dt` when used via `vizij-orchestrator-core`; if you embed the runtime directly, set them yourself before calling `evaluate_all` if time-based nodes are involved.

### Selectors

- Edges can include selectors composed of field/index segments.
- Example: `{"selector": ["outputs", "blend", 1]}` picks the second element of the `blend` array.
- Selector evaluation respects shapes; invalid paths throw descriptive errors rather than guessing.

### Shapes & Values

- Values use `vizij_api_core::Value` (scalar, vector, quat, record, array, tuple, text, bool, etc.).
- Shapes (`ShapeId`) describe numeric layouts and support inference.
- Declared shapes on node outputs guard against schema drift and provide better error messages.

### External Writes

- `Output` nodes enqueue writes automatically when `params.path` is set.
- Writes include optional shapes so hosts can validate or coerce downstream.
- Use hosts like `vizij-orchestrator-core` or your own glue to apply them.

### URDF Feature

- Enable `urdf_ik` to pull in robotics helpers (`UrdfIkPosition`, `UrdfIkPose`, `UrdfFk` nodes).
- The WASM build ships with the feature enabled; native builds can opt out to reduce dependencies.

---

## Development & Testing

Run the crate tests (unit + integration):

```bash
cargo test -p vizij-graph-core
```

Test the robotics nodes explicitly:

```bash
cargo test -p vizij-graph-core --features urdf_ik
```

Useful scripts from the workspace root:

```bash
pnpm run test:rust              # Checks the entire workspace (fmt, clippy, test)
pnpm run build:wasm:graph       # Rebuilds the WASM adapter that embeds this crate
pnpm run watch:wasm:graph       # Rebuilds on change (requires cargo-watch)
```

Benchmark ideas:

- Evaluate large graphs under different topologies to profile numeric performance.
- Leverage fixtures in `fixtures/node_graphs/` to validate schema migrations.

---

## Related Packages

- [`vizij-graph-wasm`](../../vizij-graph-wasm/README.md): wasm-bindgen binding that exposes JSON-friendly APIs plus normalization helpers.
- [`@vizij/node-graph-wasm`](../../../../npm/@vizij/node-graph-wasm/README.md): npm wrapper around the wasm build with ABI guards and utilities.
- [`bevy_vizij_graph`](../../bevy_vizij_graph/README.md): Bevy plugin that drives this runtime inside ECS worlds.
- [`vizij-orchestrator-core`](../../../orchestrator/vizij-orchestrator-core/README.md): Coordinates graphs, animations, and a blackboard.

Need help or spotted an inconsistency? Open an issue in the main Vizij repo or ping the runtime team—accurate docs keep our tooling reliable. 💡
