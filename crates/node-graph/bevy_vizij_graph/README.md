# bevy_vizij_graph

> Bevy plugin for evaluating Vizij node graphs inside ECS schedules.

`bevy_vizij_graph` wires `vizij-graph-core` into Bevy. It owns the current `GraphSpec`, keeps a persistent `GraphRuntime`, advances graph time every frame, applies parameter updates from Bevy events, and exposes the latest output snapshot for inspection.

## Overview

- Adds `VizijGraphPlugin`.
- Inserts `GraphResource(pub GraphSpec)`.
- Inserts `GraphRuntimeResource(pub GraphRuntime)`.
- Inserts `GraphOutputs(pub HashMap<NodeId, HashMap<String, PortValue>>)`.
- Inserts `GraphTime { t, dt }`.
- Registers the `SetNodeParam` event.
- Runs `system_time`, `system_set_params`, and `system_eval` in `Update`.

If a `bevy_vizij_api::WriterRegistry` resource is present, `system_eval` also applies the graph's `WriteBatch` to the world.

## Installation

```bash
cargo add bevy_vizij_graph
```

Disable the default robotics helpers if you do not need them:

```bash
cargo add bevy_vizij_graph --no-default-features
```

## Quick Start

```rust
use bevy::prelude::*;
use bevy_vizij_graph::{GraphOutputs, GraphResource, VizijGraphPlugin};
use vizij_graph_core::types::GraphSpec;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(VizijGraphPlugin)
        .add_systems(Startup, load_graph)
        .add_systems(Update, inspect_outputs)
        .run();
}

fn load_graph(mut spec: ResMut<GraphResource>) {
    let graph: GraphSpec =
        serde_json::from_str(include_str!("../../../fixtures/node_graphs/simple-gain-offset.json"))
            .expect("valid graph");
    spec.0 = graph.with_cache();
}

fn inspect_outputs(outputs: Res<GraphOutputs>) {
    for (node, ports) in &outputs.0 {
        info!(?node, ?ports, "graph output");
    }
}
```

## Staging Inputs

Stage host inputs directly on `GraphRuntimeResource`. They become visible on the next evaluation because `evaluate_all` advances the runtime epoch before reading staged inputs.

```rust
use bevy::prelude::*;
use bevy_vizij_graph::GraphRuntimeResource;
use vizij_api_core::{TypedPath, Value};

fn stage_input(mut runtime: ResMut<GraphRuntimeResource>) {
    runtime.0.set_input(
        TypedPath::parse("demo/input/value").unwrap(),
        Value::Float(0.5),
        None,
    );
}
```

## Updating Node Parameters

Use the `SetNodeParam` event to mutate supported node params in the loaded `GraphSpec`.

```rust
use bevy::prelude::*;
use bevy_vizij_graph::SetNodeParam;
use vizij_api_core::Value;

fn tweak_gain(mut events: EventWriter<SetNodeParam>) {
    events.write(SetNodeParam {
        node: "gain".into(),
        key: "value".into(),
        value: Value::Float(2.0),
    });
}
```

Unknown keys are ignored; supported keys mirror the parameter branches in `system_set_params`.

## Key Concepts

| Item | Purpose |
|------|---------|
| `GraphResource` | Current `GraphSpec`. Replace `graph_resource.0` to hot-swap the active graph. |
| `GraphRuntimeResource` | Persistent runtime state, staged inputs, outputs, writes, and per-node integration state. |
| `GraphOutputs` | Snapshot of the most recent per-node outputs, copied from the runtime after evaluation. |
| `GraphTime` | Public `t` and `dt` values updated from Bevy `Time` each frame. |
| `SetNodeParam` | Event for mutating known scalar/value parameters on the active graph spec. |

## Configuration

- The plugin evaluates in `Update` every frame.
- `GraphTime` is public, so custom systems can inspect or mutate it before `system_eval`.
- To apply writes automatically, insert `bevy_vizij_api::WriterRegistry` and register setters for the paths your graph emits.
- The crate's `urdf_ik` feature is enabled by default and forwards to `vizij-graph-core/urdf_ik`.

For full manual control, reuse the same resources and call `vizij_graph_core::evaluate_all(&mut runtime.0, &spec.0)` in your own system instead of the plugin.

## Development And Testing

```bash
cargo test -p bevy_vizij_graph
```

Tests cover staged inputs, evaluation, parameter updates, and write propagation.

## Related Crates

- [`vizij-graph-core`](../vizij-graph-core/README.md)
- [`bevy_vizij_api`](../../api/bevy_vizij_api/README.md)
- [`vizij-orchestrator-core`](../../orchestrator/vizij-orchestrator-core/README.md)
