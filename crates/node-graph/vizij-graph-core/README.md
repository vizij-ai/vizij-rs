# vizij-graph-core

`vizij-graph-core` is Vizij’s deterministic data-flow graph evaluator. It interprets `GraphSpec` descriptions, stages typed
inputs from the host, evaluates nodes in topological order, and emits typed outputs and external write operations. The crate is
consumed by Bevy integrations, WebAssembly bindings, and tooling that authors or executes Vizij graphs.

## Overview

* Pure Rust crate (2021 edition) that depends on `vizij-api-core` for shared types (`Shape`, `Value`, `TypedPath`).
* Supports math, logic, vector, time, and stateful nodes (springs/damps/slews) with optional robotics/IK helpers (`urdf_ik`
  feature).
* Provides runtime types `GraphRuntime`, `evaluate_all`, and helpers for staging host inputs and reading node outputs.
* Emits external `WriteOp { path, value, shape }` batches for sinks so hosts can apply graph results to their own systems.

## Architecture

```
         +-------------------+
         | GraphSpec (JSON)  |
         +---------+---------+
                   |
                   v
         +---------+---------+
         | GraphRuntime      |
         |  - node state     |
         |  - staged inputs  |
         |  - output cache   |
         +---------+---------+
                   |
                   v
         +---------+---------+
         | evaluate_all()    |
         |  - topo ordering  |
         |  - selector eval  |
         |  - node dispatch  |
         |  - shape checks   |
         +---------+---------+
                   |
                   v
         +-------------------+
         | EvalResult        |
         |  nodes -> values  |
         |  writes -> sinks  |
         +-------------------+
```

* Graph specs describe nodes, connections, selectors, declared shapes, and sink paths.
* `GraphRuntime` retains per-node state across frames, staged host inputs keyed by `TypedPath`, and the last frame’s outputs.
* `evaluate_all` advances the input epoch, clears cached outputs, walks the graph in topological order, and dispatches each node’s
  evaluator. Declared shapes are enforced, selectors project structured values, and sinks enqueue writes.

## Installation

Add the crate to your Cargo project (replace the version with the published release):

```bash
cargo add vizij-graph-core
```

Features:

* `urdf_ik` *(default)* – Enables optional inverse-kinematics helpers (depends on `k` and `urdf-rs`). Disable with
  `--no-default-features` if you only need the math/logic/vector nodes.

## Setup

Typical host workflow:

1. **Load a graph** – Deserialize JSON into `GraphSpec` via `serde_json` or build the spec programmatically.
2. **Create a runtime** – `GraphRuntime::default()` stores node state, staged inputs, and cached outputs.
3. **Stage inputs** *(optional each frame)* – Call `runtime.advance_epoch()` and then `runtime.set_input(path, value, declared)`
   for each `TypedPath` you want visible on the next evaluation. `declared` is an optional `Shape` contract.
4. **Evaluate** – `let result = evaluate_all(&mut runtime, &spec)?;` returns node outputs and external writes.
5. **Consume outputs** – Inspect `result.nodes` for per-port values (with shapes) or iterate `result.writes` to apply side
   effects in your host application.

## Usage

```rust
use vizij_graph_core::{evaluate_all, GraphRuntime};
use vizij_graph_core::spec::GraphSpec;
use vizij_api_core::{TypedPath, Value, Shape};

let spec: GraphSpec = serde_json::from_str(include_str!("../../../../fixtures/node_graphs/simple-gain-offset.json"))?;
let mut runtime = GraphRuntime::default();

// Stage an optional input for the next tick
let path = TypedPath::parse("robot/Arm/ik_target")?;
runtime.advance_epoch();
runtime.set_input(path, Value::vec3([0.1, 0.2, 0.3]), Some(Shape::vec3()));

let result = evaluate_all(&mut runtime, &spec)?;
for (node_id, ports) in &result.nodes {
    println!("node {node_id}: {ports:?}");
}
for write in &result.writes {
    println!("write {:?} -> {:?} (shape {:?})", write.path, write.value, write.shape);
}
```

## Key Details

### Shapes & values

* Values are represented by `vizij_api_core::Value` with shape metadata via `Shape`/`ShapeId`.
* Nodes may declare expected output shapes (`NodeSpec::output_shapes`). When declared, evaluation validates that produced values
  match; mismatches return an error for the frame.
* Numeric vectors use the dedicated `ShapeId::Vector` variant (with optional length hints) to distinguish them from heterogeneous
  lists.

### Selectors on connections

* Each `InputConnection` may include a selector (sequence of `field`/`index` segments) that projects structured upstream values.
* Selectors leverage upstream shape metadata to make projections deterministic.
* If a selector cannot be resolved (missing field, out-of-bounds index), evaluation returns an error. Future work may provide
  numeric fallbacks when shapes are known.

### Typed host inputs

* Hosts publish values via `GraphRuntime::set_input(path, value, declared_shape)`.
* Inputs participate in epoch tracking so stale values do not leak across frames. Call `advance_epoch()` before staging for the
  upcoming evaluation.
* Declared numeric shapes allow the runtime to coerce or produce “null-of-shape” (NaN-filled) values when staging fails; non-numeric mismatches emit deterministic errors.

### External writes

* Sink nodes (currently the `Output` node type) enqueue `WriteOp`s into the runtime’s write batch.
* Each `WriteOp` carries `path: TypedPath`, the produced `Value`, and optional `Shape` metadata so hosts can deserialize without
  guessing.
* Setting `params.path` on non-sink nodes has no effect; explicit sink nodes control side effects.

### Error handling & migration notes

* Evaluation returns `anyhow::Error` with detailed context (selector failures, shape mismatches, missing staged inputs, etc.).
* Legacy graphs that relied on implicit splitters should migrate to edge selectors (e.g., `["pos", 1]` to select the Y component
  of a vector).
* Introduce `Input` nodes for host-provided data instead of mutating node params directly. Stage data each frame using
  `set_input`.
* Declare output shapes on critical ports to catch schema drift early and enable numeric coercions.

## Examples

* **Selector usage** – See `tests/selector_projection.rs` for examples of record/tuple/index selectors.
* **Staged inputs** – `tests/input_node.rs` covers declared shape behavior (numeric coercions, NaN fallbacks, deterministic
  errors).
* **External writes** – `tests/output_writes.rs` verifies write batches and shape serialization.
* **End-to-end graphs** – Fixtures such as `fixtures/node_graphs/blend-graph.json` demonstrate Input → math → Output flows.

## Testing

Run the crate’s test suite:

```bash
cargo test -p vizij-graph-core
```

Enable the `urdf_ik` feature explicitly if you disabled default features and need robotics coverage:

```bash
cargo test -p vizij-graph-core --features urdf_ik
```

## Additional Resources

* `src/eval/README.md` explains the internal module layout (runtime, value flattening, node dispatch, URDF support).
* The `vizij-graph-wasm` crate wraps this core for WebAssembly; its README documents JSON normalization shorthands and staging
  helpers for JS tooling.
* Example graphs and presets ship with the npm package `@vizij/node-graph-wasm` for quick experimentation.
