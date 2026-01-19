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
- Offers parameter update events for staging host data.
- Stores the latest outputs in `GraphOutputs` for downstream systems.
- Optional `urdf_ik` feature (enabled by default) brings in robotics helpers from the core crate.
- `WriteBatch` outputs are applied automatically when a `bevy_vizij_api::WriterRegistry` resource exists.

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
use bevy_vizij_graph::{GraphOutputs, GraphResource, VizijGraphPlugin};
use vizij_graph_core::GraphSpec;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(VizijGraphPlugin::default())
        .add_systems(Startup, load_graph)
        .add_systems(Update, inspect_outputs)
        .run();
}

fn load_graph(mut spec: ResMut<GraphResource>) {
    let graph: GraphSpec =
        serde_json::from_str(include_str!("../../../fixtures/node_graphs/simple-gain-offset.json"))
            .expect("valid graph");
    *spec = GraphResource(graph.with_cache());
}

fn inspect_outputs(outputs: Res<GraphOutputs>) {
    for (node, ports) in &outputs.0 {
        info!(?node, ?ports, "graph output");
    }
}
```

Staging inputs via the provided resource:

```rust
use bevy_vizij_graph::GraphResource;
use vizij_api_core::Value;

fn set_param(mut spec: ResMut<GraphResource>) {
    if let Some(node) = spec.0.nodes.iter_mut().find(|node| node.id == "gain") {
        node.params.value = Some(Value::Float(2.0));
    }
}
```

---

## Key Concepts

| Resource / System | Purpose |
|-------------------|---------|
| `GraphResource` | Stores the loaded `GraphSpec`. Replace it to hot-swap graphs. |
| `GraphRuntimeResource` | Persistent runtime (node state, staged inputs, cached outputs). |
| `GraphOutputs` | Stores the latest node outputs. |
| `SetNodeParam` | Event to update node params by key. |
| `system_eval` | Runs `evaluate_all` each frame (and applies `WriteBatch` when possible). |

- When the spec changes, the runtime is reset to ensure deterministic results.
- Writes are applied via `bevy_vizij_api::WriterRegistry` when present; otherwise collect them by extending `system_eval`.
- Parameter updates are exposed via [`SetNodeParam`] events or by mutating [`GraphResource`] directly.

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

- `bevy_vizij_api::WriterRegistry` can apply writes directly to the Bevy world. If you need to pass writes to the orchestrator, extend `system_eval` to capture the runtime `WriteBatch`.
- When using merged controllers, allow the orchestrator to own graph evaluation and treat `bevy_vizij_graph` as a visualiser: read `GraphOutputs` to render inspector panels while letting the orchestrator apply writes.

### Logging & telemetry

- Evaluation errors are logged via `bevy::log::error!` inside the plugin. Hook your own diagnostics by adding a system after evaluation that inspects `GraphOutputs` and records metrics.
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
