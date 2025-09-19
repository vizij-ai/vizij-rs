# vizij-graph-wasm

`wasm-bindgen` adapter for `vizij-graph-core` that loads a GraphSpec, evaluates it each frame, and returns node outputs and external writes as JSON for web tooling.

The wrapper is a thin, transparent binding to the core evaluator while handling the basic JSON conversions necessary for web consumption.

## Current Capabilities

- Graph loading
  - `WasmGraph::load_graph(json: &str)` parses a JSON graph spec after applying shorthand normalization (see JSON normalization).
  - Accepts legacy `kind` (mapped to `type`) and lowercases node types to match core’s serde expectations.
  - Normalizes `output_shapes` entries that are strings to `{"id": "<shapeId>"}` objects.
- Evaluation lifecycle
  - Maintains a persistent `GraphRuntime` to preserve stateful node state across frames (spring, damp, slew).
  - `set_time(t: f64)` and `step(dt: f64)` manage time; `eval_all()` computes `dt` and forwards to the core runtime.
  - Each `eval_all()` calls core `evaluate_all`, which advances the input epoch, clears outputs/writes, retains node state for nodes that still exist, computes topo order, and evaluates nodes.
- Host input staging (NEW)
  - `stage_input(path: &str, value_json: &str, declared_shape_json: Option<String>)`
    - Normalizes `value_json` for staging (see Staging value normalization).
    - Parses `path` with `TypedPath::parse`.
    - Optionally parses `declared_shape_json` as a `vizij_api_core::Shape`.
    - Calls `GraphRuntime::set_input` so `Input` nodes can consume host data in the next `eval_all()`.
- Outputs and writes
  - `eval_all()` returns a JSON object: 
    ```
    {
      "nodes": { [nodeId]: { [portKey]: { "value": ValueJSON, "shape": ShapeJSON } } },
      "writes": [ { "path": string, "value": ValueJSON, "shape": ShapeJSON }, ... ]
    }
    ```
  - Node output `shape` is serialized from the core `PortValue.shape`.
  - Writes are surfaced for explicit `Output` nodes only (matching core behavior).
  - Shape fidelity (FIXED): The wrapper now emits the `shape` carried on `WriteOp` when present, falling back to inference only when absent.
- Parameter updates (now strict)
  - `set_param(node_id, key, json_value)` updates `NodeParams` keys at runtime.
  - For numeric keys, the wrapper validates the type; non-float values return an error instead of silently coercing to `0.0`.
  - Coverage includes:
    - Common: `value`, `frequency`, `phase`, `min`, `max`, `in_min`, `in_max`, `out_min`, `out_max`, `x`, `y`, `z`, `index`, `stiffness`, `damping`, `mass`, `half_life`, `max_rate`, `sizes`, `path`.
    - IK/URDF: `urdf_xml`, `root_link`, `tip_link`, `seed`, `weights`, `max_iters`, `tol_pos`, `tol_rot`, `joint_defaults`.
- Schema registry
  - `get_node_schemas_json()` returns the core schema registry for tooling.

## JSON normalization

Incoming graph JSON (GraphSpec) is normalized by `normalize_graph_spec_json` / `normalize_graph_spec_value` to support ergonomic shorthands:

- Node type
  - `kind` is accepted and rewritten to `type` (lowercased).
- Value shorthand
  - Primitive/structured values are accepted as plain JSON and normalized to the `{"type": "...", "data": ...}` envelope.
  - Examples:
    - Numbers → `{ "type": "Float", "data": 1.0 }`
    - Bools → `{ "type": "Bool", "data": true }`
    - Strings → `{ "type": "Text", "data": "hello" }`
    - Arrays of numbers:
      - length 2/3/4 → `Vec2`/`Vec3`/`Vec4`
      - other numeric lengths → `Vector`
    - Arrays with non-numeric entries → `List` of normalized items
  - Object aliases supported:
    - `float`, `bool`, `text`, `vec2`, `vec3`, `vec4`, `quat`, `color`, `vector`
    - `transform: { pos, rot, scale }`
    - `enum: { tag, value }`
    - `record: { ... }`
    - `array: [ ... ]`, `list: [ ... ]`, `tuple: [ ... ]`
- Params shorthands
  - `params.path` accepts `{ "path": "..." }` objects and is normalized to a string.
  - `params.sizes` accepts mixed numeric/string and normalizes to an array of numbers.
- Output shapes
  - `output_shapes: { "out": "Vec3" }` becomes `{ "out": { "id": "Vec3" } }`.

Note: If you intend to author non-numeric structured shapes (Array/List/Tuple/Record/Enum), prefer the explicit envelopes (`{"type":"Array","data":[...]}`, etc.) instead of relying on plain JSON arrays, which normalize to numeric vectors by default.

## Staging value normalization

When staging host inputs with `stage_input`, the wrapper uses a slightly different normalization policy to preserve intent without guessing:

- Numeric arrays default to `Vector` (no automatic Vec2/Vec3/Vec4 promotion).
- Explicit aliases (e.g., `{"vec3":[...]}`, `{"quat":[...]}`) are honored.
- Structured types (`record`/`array`/`list`/`tuple`/`enum`/`transform`) are supported with the same envelopes as GraphSpec normalization.

Rationale: Host-provided values should not be implicitly converted to fixed-dimension vectors unless explicitly requested, keeping staging semantics predictable and allowing declared shapes on `Input` nodes to drive coercion when appropriate.

## Value JSON round-trip

For outputs and writes, values are serialized in a legacy-friendly structure:

- Scalar → `{ "float": f }`
- Bool → `{ "bool": b }`
- Vec2/3/4 → `{ "vecN": [...] }`
- Quat → `{ "quat": [...] }`
- Color → `{ "color": [...] }`
- Transform → `{ "transform": { "pos": ..., "rot": ..., "scale": ... } }`
- Vector → `{ "vector": [...] }`
- Text → `{ "text": "..." }`
- Record → `{ "record": { key: ValueJSON } }`
- Array → `{ "array": [ValueJSON] }`
- List → `{ "list": [ValueJSON] }`
- Tuple → `{ "tuple": [ValueJSON] }`
- Enum → `{ "enum": { "tag": "...", "value": ValueJSON } }`

Shapes are emitted via `serde_json::to_value(Shape)`; see `vizij-api-core` for exact `Shape`/`ShapeId` JSON form.

## Alignment with core design objectives

- Declared shapes as contracts
  - Core enforces declared `output_shapes` in `enforce_output_shapes`. Wrapper preserves this; node outputs include the declared shape in JSON.
- Edge-level selectors
  - `InputConnection.selector` is applied in core evaluation. The wrapper passes the GraphSpec through unchanged.
- Host-driven inputs via `TypedPath`
  - Wrapper now exposes `stage_input` to call core `GraphRuntime::set_input`.
  - Epoch semantics match core: `evaluate_all` advances the epoch; values staged before the call are visible for that evaluation.
- Numeric coercions and loud failures for non-numeric mismatches
  - Core implements coercion and null-on-failure for numeric-like declared shapes; non-numeric mismatches error.
  - Wrapper’s `set_param` is now strict for numeric fields and returns errors on type mismatches (no silent `0.0`).
- Shape metadata on writes
  - Core enqueues `WriteOp` with optional `shape`. Wrapper now forwards `op.shape` when present to preserve fidelity (e.g., vector length metadata).

## Remaining notes

- GraphSpec normalization still auto-promotes plain numeric arrays of length 2/3/4 to `Vec2`/`Vec3`/`Vec4`. This is retained for backward compatibility and ergonomics in authored graphs. If you need a quaternion, use the explicit `quat` alias or the explicit envelope.

## Public API (WASM)

- Utility
  - `normalize_graph_spec_json(json: &str) -> String`
- Graph instance
  - `new() -> WasmGraph`
  - `load_graph(json: &str) -> Result<(), JsValue>`
  - `stage_input(path: &str, value_json: &str, declared_shape_json: Option<String>) -> Result<(), JsValue>`
  - `set_time(t: f64)`
  - `step(dt: f64)`
  - `eval_all() -> Result<String, JsValue>` returns JSON with `nodes` and `writes`
  - `set_param(node_id: &str, key: &str, json_value: &str) -> Result<(), JsValue>`
- Schema
  - `get_node_schemas_json() -> String`

## Staging cadence and epochs

- Call `stage_input(...)` for each input you want visible in the next frame.
- Then call `eval_all()`; core will advance the epoch and consume values staged for the current epoch.
- If you stage after calling `eval_all()`, that data will be visible in the following call, not the current one.

## Example usage (TypeScript-like pseudocode)

```ts
import init, { WasmGraph, normalize_graph_spec_json } from '@vizij/node-graph-wasm';

await init();

const graph = new WasmGraph();
graph.load_graph(normalize_graph_spec_json(rawGraphJson));

// Frame 0
graph.set_time(0);
// Stage inputs for frame 1 (visible on next eval_all)
graph.stage_input("robot/Arm/ik_target", JSON.stringify({ type: "Vec3", data: [0.1, 0.2, 0.3] }), null);

// Frame 1
graph.step(1/60);
// Consume staged input; outputs/writes reflect it
const result = graph.eval_all();
const { nodes, writes } = JSON.parse(result);

// Example with declared shape
const declared = JSON.stringify({ id: "Vec3" });
graph.stage_input("robot/Arm/ik_target", JSON.stringify({ vector: [0.3, 0.2, 0.1] }), declared);
```

## Summary

- Transparent wrapping: selector semantics, declared shapes, evaluation order/state, and write emission all mirror `vizij-graph-core`.
- Gaps addressed:
  - Added staged input API (`stage_input`) with epoch semantics.
  - Preserved write shape metadata (`WriteOp.shape`) in JSON.
  - Made `set_param` strict, returning errors on type mismatches; expanded param coverage (including URDF/IK).
  - Staging normalization avoids auto-Vec2/3/4 guessing to keep host intent explicit.
