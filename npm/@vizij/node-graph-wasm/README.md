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
class. Import the helpers from the ESM entry and call `init()` **once** before
constructing `Graph` instances.

```ts
import {
  init,
  Graph,
  type EvalResult,
  type GraphSpec,
  type ValueJSON,
  getNodeSchemas,
} from "@vizij/node-graph-wasm";

await init();

const graph = new Graph();
graph.loadGraph(spec); // spec: GraphSpec | JSON string

graph.setTime(0);
graph.step(1 / 60); // advance internal time in seconds

const { nodes, writes }: EvalResult = graph.evalAll();
// nodes: Record<NodeId, Record<PortId, { value: ValueJSON; shape: ShapeJSON }>>
// writes: Array<{ path: string; value: ValueJSON; shape: ShapeJSON }>

graph.setParam("nodeA", "value", { vec3: [1, 2, 3] });

const registry = await getNodeSchemas();

const normalized = await normalizeGraphSpec(spec);
```

`ValueJSON` mirrors the serialized shape produced by `vizij-api-core::Value`.
Primitive variants such as `{ float: number }` and `{ vec3: [...] }` are supported,
as well as composites:

- `{ record: { [fieldName]: ValueJSON } }`
- `{ array: ValueJSON[] }`
- `{ list: ValueJSON[] }`
- `{ tuple: ValueJSON[] }`

`EvalResult.writes` contains the typed output batch emitted by nodes that define a
`params.path`. Each entry mirrors the JSON contract exposed by
`vizij_api_core::WriteOp` and includes both the serialized value and its inferred
shape (`{ path: string, value: ValueJSON, shape: ShapeJSON }`). The per-node
output map follows the same `{ value, shape }` convention so UI layers can render
data using the same schema metadata returned by the Rust core.
