---
"@vizij/runtime": minor
---

A device loads a WebAssembly component as a guest: `startRuntime(graph, init, modules)` and `loadVizij`'s `options.modules` take a `{ headerJson, wasmBytes }` pair whose header names the `wasm-component` executor and whose bytes are a component of Arora's `arora:module` world, built for `wasm32-wasip2` (arora 13.7). Arora's guests, `arora-animation-module` 2 among them, are built that way. A `wasm32-wasip1` core module whose header names `wasm` loads as before.
