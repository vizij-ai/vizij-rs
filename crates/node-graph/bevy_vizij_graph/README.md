# bevy_vizij_graph

> **Bevy plugin for evaluating Vizij node graphs inside ECS schedules.**

`bevy_vizij_graph` wires `vizij-graph-core` into the Bevy game engine. It manages the current graph specification, stages host inputs, runs evaluations each frame, and exposes the results to gameplay systems.

---

## Table of Contents

1. [Overview](#overview)
2. [Installation](#installation)
3. [Quick Start](#quick-start)
4. [Key Concepts](#key-concepts)
5. [Configuration](#configuration)
6. [Development & Testing](#development--testing)
7. [Related Crates](#related-crates)

---

## Overview

- Provides `VizijGraphPlugin` which inserts the resources and systems needed to evaluate graphs.
- Persists `GraphRuntime` state between frames while allowing the `GraphSpec` resource to be hot-swapped.
- Offers `PendingInputs` and parameter update events for staging host data.
- Stores the latest outputs and writes in `EvalResultRes` for downstream systems.
- Optional `urdf_ik` feature (enabled by default) brings in robotics helpers from the core crate.

---

## Installation

```bash
cargo add bevy_vizij_graph
```

Disable the robotics helpers if you don’t need them:

```bash
cargo add bevy_vizij_graph --no-default-features
```

---

## Quick Start

```rust
use bevy::prelude::*;
use bevy_vizij_graph::{VizijGraphPlugin, GraphSpecRes, EvalResultRes};
use vizij_graph_core::types::GraphSpec;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(VizijGraphPlugin::default())
        .add_systems(Startup, load_graph)
        .add_systems(Update, inspect_outputs)
        .run();
}

fn load_graph(mut spec: ResMut<GraphSpecRes>) {
    let graph: GraphSpec =
        serde_json::from_str(include_str!("../../../fixtures/node_graphs/simple-gain-offset.json"))
            .expect("valid graph");
    *spec = GraphSpecRes::new(graph.with_cache());
}

fn inspect_outputs(result: Res<EvalResultRes>) {
    for (node, ports) in &result.nodes {
        info!(?node, ?ports, "graph output");
    }
    for write in &result.writes {
        info!("write {:?} => {:?}", write.path, write.value);
    }
}
```

Staging inputs via the provided resource:

```rust
use bevy_vizij_graph::{PendingInput, PendingInputs};
use vizij_api_core::{Shape, TypedPath, Value};

fn stage_inputs(mut pending: ResMut<PendingInputs>) {
    pending.push(PendingInput {
        path: TypedPath::parse("demo/input/value").unwrap(),
        value: Value::vec3([0.1, 0.2, 0.3]),
        declared: Some(Shape::vec3()),
    });
}
```

---

## Key Concepts

| Resource / System | Purpose |
|-------------------|---------|
| `GraphSpecRes` | Stores the loaded `GraphSpec`. Replace it to hot-swap graphs. |
| `GraphRuntimeRes` | Persistent runtime (node state, staged inputs, cached outputs). |
| `PendingInputs` | Queue of staged inputs to apply before the next evaluation. |
| `EvalResultRes` | Stores the latest node outputs and `WriteBatch`. |
| `stage_inputs_system` | Applies pending inputs to the runtime. |
| `evaluate_graph_system` | Runs `evaluate_all` each frame or fixed timestep. |

- When the spec changes, the runtime is reset to ensure deterministic results.
- Writes are stored in `EvalResultRes`; you can also consume them immediately by adding your own system after evaluation.
- Parameter updates are exposed via events/resources (see crate docs) mirroring the staging path.

---

## Configuration

- The plugin runs evaluation in the `Update` schedule by default. To drive graphs at a custom cadence, either mutate the public `GraphTime` resource before evaluation or wire the systems manually as shown below.
- Feature flag `urdf_ik` controls whether URDF IK/FK helpers are registered in the runtime. Disable it if you don’t need robotics nodes to reduce dependencies.
- Systems are ordered so staging happens before evaluation. Insert additional systems before/after to customise flow.

### Fixed timestep evaluation

The stock plugin samples `Time` each frame. For deterministic fixed stepping you can wire the core pieces yourself and schedule them in `FixedUpdate`:

```rust
use bevy::prelude::*;
use bevy_vizij_graph::{GraphOutputs, GraphResource, GraphRuntimeResource};
use vizij_graph_core::{evaluate_all, GraphRuntime};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(FixedTime::new_from_secs(1.0 / 120.0)))
        .insert_resource(GraphResource::default())
        .insert_resource(GraphRuntimeResource(GraphRuntime::default()))
        .insert_resource(GraphOutputs::default())
        .add_systems(FixedUpdate, step_graph_fixed)
        .run();
}

fn step_graph_fixed(
    mut runtime: ResMut<GraphRuntimeResource>,
    spec: Res<GraphResource>,
    mut outputs: ResMut<GraphOutputs>,
) {
    if let Err(err) = evaluate_all(&mut runtime.0, &spec.0) {
        bevy::log::error!("graph evaluation error: {err}");
        return;
    }
    outputs.0 = runtime.0.outputs.clone();
}
```

This approach gives you full control over cadence while reusing the same resources the plugin exposes. If you stick with `VizijGraphPlugin`, you can still override the timestep by mutating the public `GraphTime` resource before the plugin’s `system_eval` runs.

### Integrating with the orchestrator

- `EvalResultRes.writes` contains the `WriteBatch` produced by graph `Output` nodes. Forward it to `vizij-orchestrator-core` by calling `orchestrator.apply_writebatch(...)` or staging inputs on the orchestrator’s blackboard.
- When using merged controllers, allow the orchestrator to own graph evaluation and treat `bevy_vizij_graph` as a visualiser: read `EvalResultRes.nodes` to render inspector panels while letting the orchestrator apply writes.

### Logging & telemetry

- Evaluation errors are logged via `bevy::log::error!` inside the plugin. Hook your own diagnostics by adding a system after evaluation that inspects `EvalResultRes` and records metrics (`events.spawn(GraphEvalDiagnostics { frame, writes: result.writes.len() })`).
- `GraphTime` tracks the accumulated simulation time (`t`) and last delta (`dt`). Emit these values through your telemetry stack to correlate graph load with frame timing.

---

## Development & Testing

```bash
cargo test -p bevy_vizij_graph
```

Tests cover staging, evaluation, and write propagation using sample graphs. For live experimentation, run one of the demo apps in `vizij-web` after linking the WASM builds.

---

## Related Crates

- [`vizij-graph-core`](../vizij-graph-core/README.md) – core evaluator used by this plugin.
- [`vizij-orchestrator-core`](../../orchestrator/vizij-orchestrator-core/README.md) – can feed `WriteBatch` results into other controllers.
- [`bevy_vizij_animation`](../../animation/bevy_vizij_animation/README.md) – sister plugin for Vizij animations.

Need more bindings? Open an issue—well-documented ECS integration keeps graph evaluation straightforward. 🧩
