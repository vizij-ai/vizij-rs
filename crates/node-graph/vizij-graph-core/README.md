# vizij-graph-core

Engine-agnostic evaluator for Vizij node graphs. The crate provides the data
model (`GraphSpec`/`NodeSpec`), execution primitives, and integration points for
hosts such as Bevy and WebAssembly.

## Key Concepts

- Shapes & Values — Node outputs are expressed with `vizij_api_core::Shape`
  and `Value`. Nodes may declare expected shapes via `NodeSpec::output_shapes`; the
  evaluator enforces these declarations, surfacing mismatches as runtime
  errors while still inferring shapes for undeclared ports.
- Typed paths & writes — `NodeParams::path` stores a validated
  `vizij_api_core::TypedPath`. During evaluation sink nodes emit `WriteOp`s into
  a `WriteBatch`, carrying the produced `Shape` alongside the value so hosts can
  deserialize confidently without re-inferring types. See “External write
  behavior” below for details.
- Selectors on connections — An `InputConnection` may provide a selector
  (sequence of `field`/`index` segments) so downstream nodes can project
  structured values without intermediate splitter nodes. Selectors are resolved
  using upstream shape metadata to keep projections deterministic.
- Staged typed inputs — Hosts can push data into the graph each frame via
  `GraphRuntime::set_input`, and the `Input` node surfaces staged values as
  first-class ports. Declared numeric shapes fall back to NaN-filled placeholders
  when staging fails, while non-numeric shapes produce deterministic errors.
- Vector semantics — Numeric vectors use the dedicated `ShapeId::Vector`
  variant with optional length hints, keeping them distinct from heterogeneous
  lists and informing downstream tooling about expected cardinality.

## Engine Evaluation Flow (step-by-step)

This section documents how a typical frame (“tick”) runs end-to-end.

1) Host staging (optional, per frame)
   - If using typed inputs, stage values you want the graph to see using:
     - `GraphRuntime::set_input(path: TypedPath, value: Value, declared: Option<Shape>)`
   - Note: The runtime maintains an input epoch. See “Epochs for staged inputs”.

2) Evaluate the graph
   - Call `evaluate_all(&mut runtime, &graph_spec)`.
   - What evaluate_all does:
     - Clears `runtime.outputs` and resets `runtime.writes`.
     - Retains per-node state (springs/damps/slews) for nodes that still exist.
     - Computes a topological order and evaluates each node in sequence:
       a) For each input edge, fetch the upstream port value.
       b) If the edge has a selector (e.g., `["pos", 1]`), project the upstream value using its shape.
          - On success, the downstream input value and shape are the projected sub-structure/primitive.
          - On failure (no such field/index), evaluation returns an error (see “Selector error behavior”).
       c) Evaluate the node:
          - Stateless math/time/logic/vector nodes compute numeric outputs (supports broadcasting).
          - Stateful nodes (Spring/Damp/Slew) update and return integration state.
          - `Input` node reads the most recent staged value for its `TypedPath` and validates against any declared output shape (see “Choice B semantics” below).
          - `Output` node passes its input through and acts as a sink for external writes (see write behavior below).
       d) If the node declares an output shape on a port (`output_shapes`), the value must match that shape or evaluation fails.
       e) If the node is a sink with a configured path, enqueue a `WriteOp` containing the output value and its shape.
     - On success, `runtime.outputs` contains each node’s port values with shapes, and `runtime.writes` contains all external writes for the frame.

3) Consume results
   - Read `runtime.outputs` if you’re chaining or inspecting internal graph state.
   - Iterate `runtime.writes` to apply external effects (apply to a blackboard, engine, or UI).

### Epochs for staged inputs

- The runtime tracks an input “epoch” to avoid stale values leaking across frames.
- Typical convention:
  - Before staging new values for the upcoming frame, call `runtime.advance_epoch()`.
  - Then call `runtime.set_input(...)` for each channel you want to publish for the next evaluation.
- Note: Some hosts may choose a different cadence. The important behavioral guarantee is staged inputs are visible only for the epoch they’re tagged with.

## External write behavior

- Writes are enqueued only for explicit sink nodes (currently the `Output` node).
- Setting `params.path` on non-sink nodes has no effect on the `WriteBatch`.
- Note: Earlier iterations allowed any node with `params.path` to write; this has been removed to prevent unintended side effects and to make sinks explicit.

## Selector error behavior and future fallback

- Current behavior: If a selector on an edge cannot be applied (missing field, out-of-bounds index, unsupported structure), evaluation returns an error for the frame. This favors safety and clear diagnostics.
- Future behavior (planned): When the expected input shape is known and numeric-like, selector failures may fall back to “null-of-shape” values (NaN-filled composites) rather than erroring immediately. This makes graphs more resilient to transient data shape changes (e.g., sensors).

## Compile-time validation (future)

- Currently, selector and shape validation happens at runtime during evaluation.
- Future plans include a compile step that:
  - Validates selectors against declared/upstream shapes.
  - Pre-resolves projection metadata to reduce runtime overhead.
  - Enables more graceful fallback behavior (e.g., null-of-shape on input mismatch) because the target contracts are known in advance.

## Migration guide for existing graphs

The new system allows authors to express slicing logic (e.g., extracting `pos.y` from a structured sensor) directly on edges, and to treat host-supplied data as first-class, typed inputs. This section outlines typical changes needed to adapt older graphs.

1) Replace splitter chains with selectors
   - Before:
     - Sensor → Split → VectorIndex → Math
   - After:
     - Sensor →(selector=["pos", 1])→ Math
   - Benefit: Fewer nodes, clear structural intent, earlier validation if shape changes.

2) Introduce Input nodes for host data
   - Before:
     - Hosts mutated node params directly to update targets (e.g., IK target).
   - After:
     - Hosts publish typed values each frame via `set_input("robot1/Arm/ik_target", value, declared_shape?)`.
     - Graph contains an `Input` node bound to that `TypedPath`. Downstream nodes consume the staged value.
   - Optional: Declare the `Input` node’s output shape to get resilient numeric null fallbacks (NaN-filled) on mismatch or missing data.

3) Declare shapes where stability matters
   - For ports consumed by external systems (or critical internal boundaries), add `output_shapes` to the producing node:
     - Example:
       - `output_shapes["out"] = Shape::new(ShapeId::Vec3)` on a position feed.
     - On mismatch, evaluation fails with a clear message, helping catch drift early.

4) Route external writes through Output nodes
   - Before:
     - Some graphs relied on side effects from intermediate nodes.
   - After:
     - Add an `Output` node with a configured `path`. Wire the signal you want to publish into its `in` port.
     - The runtime emits a `WriteOp` containing both value and shape for robust subscribers.

5) Notes on epochs and staging
   - If you use `Input` nodes, ensure your host’s frame loop advances the epoch and re-stages values for each tick you want the graph to see updated data:
     - `runtime.advance_epoch();`
     - `runtime.set_input(path, value, declared_shape?);`
     - `evaluate_all(&mut runtime, &spec);`

### Example: Adapting a sensor feed

- Legacy:
  - Node A (sensor) outputs `{ pos: vec3, rot: quat }`
  - Node B needs `pos.y` as a scalar
  - Graph: A → Split → VectorIndex → B
- New:
  - Edge from A’s out port to B’s input has selector `["pos", 1]`
  - If A’s shape changes (e.g., field renamed), evaluation errors with a precise message during wiring or the next run.

## Testing

Run the focused test suite with:

```bash
cargo test -p vizij-graph-core
```

Graph-level tests include:
- Selector projection success/failure (record/transform, nested indices, bounds).
- Input node behavior with declared numeric-like shapes (match, coercion, missing → NaN composite) and non-numeric mismatch (error).
- Output `WriteOp` value+shape JSON roundtrip.
- End-to-end example: `Input → selector to scalar → math → Output` with scalar shape write.

## Roadmap Notes

- Enrich inverse kinematics nodes to support configurable joint chains in both
  2D and 3D.
- Introduce grouping/composition primitives so clusters of nodes can be saved
  and reused.
- Layer node/port labelling metadata (for UI rendering) on top of the core
  schema once the shape/type system settles.
- Expand high-level vector tooling (weighted blending, block loading) using the
  new `ShapeId::Vector` semantics.
