# bevy_vizij_graph

`bevy_vizij_graph` integrates `vizij-graph-core` into the Bevy ECS. It keeps a shared graph runtime, evaluates graphs each frame,
and exposes typed outputs/writes to other systems.

## Overview

* Provides a Bevy plugin (`VizijGraphPlugin`) that owns the core `GraphRuntime` and current `GraphSpec`.
* Evaluates graphs during `Update` or `FixedUpdate` (configurable) and stores results in resources/events for downstream systems.
* Supports host-driven parameter updates and typed input staging through ECS events/resources.
* Optional `urdf_ik` feature (enabled by default) pulls in robotics helpers from the core crate.

## Architecture

```
+-----------------------------+
| VizijGraphPlugin            |
|  - Inserts resources        |
|  - Registers systems        |
+-----------------------------+
            |
            v
+-----------------------------+
| Resources                   |
|  GraphSpecRes(GraphSpec)    |
|  GraphRuntimeRes(GraphRuntime)
|  PendingInputs              |
|  EvalResultRes              |
+-----------------------------+
            |
            v
+-----------------------------+
| Systems                     |
|  stage_inputs_system        |
|  evaluate_graph_system      |
|  apply_writes_system (opt)  |
+-----------------------------+
            |
            v
+-----------------------------+
| ECS consumers               |
|  - Read EvalResultRes       |
|  - Listen for Write events  |
+-----------------------------+
```

* `GraphSpecRes` holds the currently loaded spec (hot-swappable if you mutate the resource).
* `GraphRuntimeRes` persists node state and staged inputs across frames.
* `EvalResultRes` exposes the latest evaluation output so gameplay systems can read node values or external writes.
* Input staging can be driven by events or direct resource access to mirror `GraphRuntime::set_input`.

## Installation

```bash
cargo add bevy_vizij_graph
```

Features:

* `urdf_ik` *(default)* – Enables URDF/IK helpers from the core crate. Disable with `--no-default-features` if not needed.

## Setup

1. Add the plugin to your Bevy app:
   ```rust
   use bevy::prelude::*;
   use bevy_vizij_graph::VizijGraphPlugin;

   App::new()
       .add_plugins(DefaultPlugins)
       .add_plugins(VizijGraphPlugin::default())
       .run();
   ```
2. Load or construct a `GraphSpec` and insert it into the `GraphSpecRes` resource (e.g., during startup).
3. Stage inputs by writing to the provided resources/events (`PendingInputs`) before the evaluation system runs.
4. Read `EvalResultRes` after evaluation to inspect per-node outputs or handle external writes.

## Usage

```rust
use bevy::prelude::*;
use bevy_vizij_graph::{VizijGraphPlugin, GraphSpecRes, GraphRuntimeRes, EvalResultRes};
use vizij_graph_core::spec::GraphSpec;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(VizijGraphPlugin::default())
        .add_systems(Startup, load_graph)
        .add_systems(Update, inspect_outputs)
        .run();
}

fn load_graph(mut spec_res: ResMut<GraphSpecRes>) {
    let spec: GraphSpec = serde_json::from_str(include_str!("../../../fixtures/node_graphs/simple-gain-offset.json")).unwrap();
    *spec_res = GraphSpecRes::new(spec);
}

fn inspect_outputs(result: Res<EvalResultRes>) {
    for (node, ports) in &result.nodes {
        info!(?node, ?ports, "graph output");
    }
    for write in &result.writes {
        info!("write {:?} -> {:?}", write.path, write.value);
    }
}
```

### Staging host inputs

The plugin exposes `PendingInputs` so other systems can push values that mimic `GraphRuntime::set_input`.

```rust
use bevy_vizij_graph::{PendingInputs, PendingInput};
use vizij_api_core::{TypedPath, Value, Shape};

fn stage(mut pending: ResMut<PendingInputs>) {
    let path = TypedPath::parse("robot/Arm/ik_target").unwrap();
    pending.push(PendingInput {
        path,
        value: Value::vec3([0.1, 0.2, 0.3]),
        declared: Some(Shape::vec3()),
    });
}
```

## Key Details

* **Resource ownership** – `GraphSpecRes` and `GraphRuntimeRes` are separate so you can hot-swap specs while retaining runtime
  state where appropriate. Replacing the spec resets outputs and writes to avoid stale data.
* **Scheduling** – Evaluation runs after input staging systems. Customize the system ordering or schedule (e.g., run evaluation
  in `FixedUpdate`) by configuring the plugin.
* **Write handling** – By default the plugin stores writes in `EvalResultRes`. You can also register listener systems to react to
  writes immediately.
* **Parameter updates** – Use provided events/resources to change node parameters at runtime (see crate docs for exact types).
* **Debugging** – `EvalResultRes` includes both node outputs and writes along with shape metadata, making it easy to visualize
  graph state in editor overlays or logs.

## Testing

Run the crate’s tests:

```bash
cargo test -p bevy_vizij_graph
```

Tests spin up a Bevy `App`, load fixtures, stage inputs, and assert that evaluation results and writes match expectations.

## Troubleshooting

* **Graph doesn’t evaluate** – Ensure `GraphSpecRes` contains a valid spec and that the evaluation system is scheduled (plugin
  adds it by default). Check logs for selector/shape errors propagated from the core crate.
* **No writes produced** – Verify that your graph includes explicit `Output` nodes with `params.path` configured. Non-sink nodes
  no longer emit writes.
* **Inputs not visible** – Confirm that staging happens before the evaluation system (same frame) and that you call `PendingInputs::clear()` only after evaluation if you manage the buffer manually.
