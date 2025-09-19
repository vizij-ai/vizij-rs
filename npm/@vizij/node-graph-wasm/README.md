# @vizij/node-graph-wasm (published from vizij-rs)

Wrapper around the wasm-pack output for the Vizij node-graph controller.

## Build (local dev)
```bash
# from vizij-rs/
wasm-pack build crates/node-graph/vizij-graph-wasm --target web --out-dir npm/@vizij/node-graph-wasm/pkg --release
cd npm/@vizij/node-graph-wasm
npm i && npm run build
```

## Link to vizij-web (local dev)
```bash
cd npm/@vizij/node-graph-wasm
npm link

# in vizij-web/
npm link @vizij/node-graph-wasm
```

## Runtime API

The published package exposes a thin TypeScript wrapper around the wasm `WasmGraph`
class. Import the helpers from the ESM entry and call `init()` once before
constructing `Graph` instances.

```ts
import {
  init,
  Graph,
  normalizeGraphSpec,
  getNodeSchemas,
  type EvalResult,
  type GraphSpec,
  type ValueJSON,
  type ShapeJSON,
} from "@vizij/node-graph-wasm";

await init();

const graph = new Graph();
graph.loadGraph(spec); // spec: GraphSpec | JSON string

// Time control
graph.setTime(0);
graph.step(1 / 60); // advance internal time in seconds

// Stage host inputs for next frame (epoch semantics)
const declared: ShapeJSON = { id: "Vec3" };
graph.stageInput("robot/Arm/ik_target", { vec3: [0.1, 0.2, 0.3] }, declared);

// Evaluate: returns per-node outputs and external writes
const { nodes, writes }: EvalResult = graph.evalAll();
// nodes: Record<NodeId, Record<PortId, { value: ValueJSON; shape: ShapeJSON }>>
// writes: Array<{ path: string; value: ValueJSON; shape: ShapeJSON }>

// Update node params (strict typing; non-floats for numeric params will throw)
graph.setParam("nodeA", "value", { vec3: [1, 2, 3] });

// Registry and normalization helpers
const registry = await getNodeSchemas();
const normalized = await normalizeGraphSpec(spec);
```

### ValueJSON

`ValueJSON` mirrors the serialized shape produced by `vizij-api-core::Value`.
Primitive variants such as `{ float: number }` and `{ vec3: [...] }` are supported,
as well as composites:

- `{ record: { [fieldName]: ValueJSON } }`
- `{ array: ValueJSON[] }`
- `{ list: ValueJSON[] }`
- `{ tuple: ValueJSON[] }`
- `{ enum: { tag: string, value: ValueJSON } }`
- `{ transform: { pos, rot, scale } }`
- `{ vector: number[] }`

Notes:
- When staging with `graph.stageInput`, raw JS arrays are encoded as `{ vector: [...] }` to avoid accidental `{ vec3: [...] }` promotion. If you intend a fixed-dimension vector, pass the explicit envelope (e.g., `{ vec3: [...] }`).
- Declared shapes on `Input` nodes still govern numeric coercion/null-of-shape behavior inside the core.

### Writes and shapes

`EvalResult.writes` contains the typed output batch emitted by explicit sink nodes (`type: "output"` with a configured `params.path`). Each entry mirrors the JSON contract exposed by `vizij_api_core::WriteOp`:

- `{ path: string, value: ValueJSON, shape: ShapeJSON }`

The wrapper now forwards the shape metadata produced by the Rust core (when present) instead of re-inferring it, preserving vector length and structured contracts for subscribers.

The per-node output map follows the same `{ value, shape }` convention so UI layers can render
data using the same schema metadata returned by the Rust core.

### Epoch staging guidance

- Stage inputs via `graph.stageInput(...)` before calling `graph.evalAll()` for the frame you want them visible.
- Each `evalAll()` advances the input epoch internally; inputs staged after `evalAll()` will be visible on the next call.
- This mirrors the core `GraphRuntime::set_input`/epoch behavior.

## Samples

This package ships with several ready-to-run graph samples that are compatible with the updated core (explicit `Output` nodes with `params.path`; some samples use `Input` nodes with declared shapes and defaults so they run without host staging):

```ts
import {
  graphSamples,
  oscillatorBasics,
  vectorPlayground,
  logicGate,
  tupleSpringDampSlew,
  init,
  Graph,
} from "@vizij/node-graph-wasm";

await init();

const g = new Graph();
g.loadGraph(vectorPlayground); // or oscillatorBasics, logicGate, tupleSpringDampSlew
g.setTime(0);
const res = g.evalAll();
console.log(res.writes);
```

Available samples:
- `oscillatorBasics`
- `vectorPlayground` (uses two `Input` nodes with declared `Vec3` shapes to define v1 and v2)
- `logicGate`
- `tupleSpringDampSlew` (tuple of two Vec3s is projected and processed with spring/damp/slew; output is a concatenated Vector as a stand-in for a tuple due to missing tuple-constructor node)

You can also access them as a map via `graphSamples`.

## Testing samples (local)

A minimal test script is included to load each sample, run the graph for two ticks, and assert that typed writes are produced.

Prerequisites:
- Build the wasm crate for web to generate `pkg/`:
  ```bash
  # from vizij-rs/
  wasm-pack build crates/node-graph/vizij-graph-wasm --target web --out-dir npm/@vizij/node-graph-wasm/pkg --release
  ```
- Install dev deps and build TypeScript:
  ```bash
  cd npm/@vizij/node-graph-wasm
  npm i
  npm run test
  ```
The test initializes the wasm module from the local `pkg/vizij_graph_wasm_bg.wasm`, evaluates each preset, and verifies that:
- `writes` is non-empty and each entry includes `{ path, value, shape }`
- `nodes` contains `{ value, shape }` snapshots for ports
