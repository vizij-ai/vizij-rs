# vizij-orchestrator-core

> Deterministic scheduling for Vizij graphs, animations, and shared blackboard state.

`vizij-orchestrator-core` coordinates graph controllers and animation controllers against a shared blackboard. It stages inputs, runs configured passes, merges writes deterministically, and records conflicts for diagnostics. The crate is private to this workspace and is wrapped by `vizij-orchestrator-wasm`.

## Overview

- Shared blackboard built on `vizij-api-core` values, shapes, and typed paths.
- Graph and animation controllers with explicit IDs and subscriptions.
- Deterministic schedules: `SinglePass`, `TwoPass`, and `RateDecoupled` (currently aliases to `SinglePass`).
- `OrchestratorFrame` output containing merged writes, conflicts, timings, and animation events.

## Quick Start

```rust
use vizij_orchestrator::{
    Orchestrator, Schedule,
    controllers::{GraphControllerConfig, Subscriptions},
};
use vizij_graph_core::types::GraphSpec;

let graph_spec: GraphSpec =
    serde_json::from_str(include_str!("../../../../fixtures/node_graphs/simple-gain-offset.json"))?;

let graph_cfg = GraphControllerConfig {
    id: "graph:demo".into(),
    spec: graph_spec.with_cache(),
    subs: Subscriptions::default(),
};

let mut orchestrator = Orchestrator::new(Schedule::SinglePass).with_graph(graph_cfg);
orchestrator.set_input(
    "demo/input/value",
    serde_json::json!({ "type": "float", "data": 1.0 }),
    None,
)?;

let frame = orchestrator.step(1.0 / 60.0)?;
println!("epoch {} merged writes: {:?}", frame.epoch, frame.merged_writes);
```

## Key Concepts

### Blackboard

- Stores `BlackboardEntry { value, shape?, epoch, source, priority }`.
- `set`, `set_entry`, and `apply_writebatch` are the core mutation paths.
- Conflict logging is built into blackboard writes.

### Controllers

- Graph controllers wrap `GraphSpec` plus `Subscriptions`.
- Animation controllers wrap `vizij-animation-core` engines and translate blackboard paths into animation inputs.
- `GraphControllerConfig::merged` and `merged_with_options` let multiple graph specs run as one merged controller.

### Schedules

- `SinglePass`: animations then graphs.
- `TwoPass`: graphs, then animations, then graphs again.
- `RateDecoupled`: reserved for future work; today it behaves like `SinglePass`.

### Frames

Each `step(dt)` returns an `OrchestratorFrame` with:

- `merged_writes`
- `conflicts`
- `timings_ms`
- `events`

## Development And Testing

```bash
cargo test -p vizij-orchestrator-core
pnpm run build:wasm:orchestrator
```

Useful examples live under `examples/`:

```bash
cargo run -p vizij-orchestrator-core --example single_pass
```

## Related Packages

- [`vizij-graph-core`](../../node-graph/vizij-graph-core/README.md)
- [`vizij-animation-core`](../../animation/vizij-animation-core/README.md)
- [`vizij-orchestrator-wasm`](../vizij-orchestrator-wasm/README.md)
