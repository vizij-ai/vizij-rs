# vizij-graph-core

Engine-agnostic evaluator for Vizij node graphs. The crate provides the data
model (`GraphSpec`/`NodeSpec`), execution primitives, and integration points for
hosts such as Bevy and WebAssembly.

## Key Concepts

- **Shapes & Values** — Node outputs are expressed with `vizij_api_core::Shape`
  and `Value`. Nodes may declare expected shapes via `NodeSpec::output_shapes`; the
  evaluator now enforces these declarations, surfacing mismatches as runtime
  errors while still inferring shapes for undeclared ports.
- **Typed paths & writes** — `NodeParams::path` stores a validated
  `vizij_api_core::TypedPath`. During evaluation each `Output` node emits
  `WriteOp`s into a `WriteBatch`, allowing hosts to forward graph results to the
  Vizij blackboard without re-parsing paths.
- **Vector semantics** — Numeric vectors use the dedicated `ShapeId::Vector`
  variant with optional length hints, keeping them distinct from heterogeneous
  lists and informing downstream tooling about expected cardinality.

## Evaluation Flow

```rust
use vizij_graph_core::{evaluate_all, GraphRuntime, GraphSpec};

let spec = GraphSpec { nodes: /* ... */ };
let mut runtime = GraphRuntime::default();
evaluate_all(&mut runtime, &spec)?;

// Per-node outputs with shapes inferred or enforced from declarations.
let outputs = &runtime.outputs;

// Batched writes produced by Output nodes with typed targets.
let writes = runtime.writes.iter().collect::<Vec<_>>();
```

`GraphRuntime` keeps the evaluation clock (`t`), a cache of per-node port
snapshots, and the most recent `WriteBatch` emitted by the graph. Hosts may reuse
or clear the runtime between frames depending on their scheduling model.

## Testing

Run the focused test suite with:

```bash
cargo test -p vizij-graph-core
```

## Roadmap Notes

- Enrich inverse kinematics nodes to support configurable joint chains in both
  2D and 3D.
- Introduce grouping/composition primitives so clusters of nodes can be saved
  and reused.
- Layer node/port labelling metadata (for UI rendering) on top of the core
  schema once the shape/type system settles.
- Expand high-level vector tooling (weighted blending, block loading) using the
  new `ShapeId::Vector` semantics.
