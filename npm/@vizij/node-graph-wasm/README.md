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

## Use
```ts
import init, { NodeGraph } from "@vizij/node-graph-wasm";
await init();
const graph = new NodeGraph({ frequency_hz: 1.0 });
const out = graph.update(0.016, {});
```
