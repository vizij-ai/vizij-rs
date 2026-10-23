---
"@vizij/runtime": major
---

A device runs on arora 15, and loads a WebAssembly guest as a component only.

- `startRuntime(graph, init, modules)` and `loadVizij`'s `options.modules` take a `{ headerJson, wasmBytes }` pair. Its header names the `wasm-component` executor, and its bytes are a component of Arora's `arora:module` world built for `wasm32-wasip2`. Arora's guests, `arora-animation-module` 2 among them, are built that way.
- **Breaking:** a `wasm32-wasip1` core module, whose header names `wasm`, no longer loads. Rebuild such a guest for `wasm32-wasip2`.
- **Breaking:** a call to a guest whose arguments do not match the parameters its header declares fails, naming the parameter, and reaches no guest.
- A guest under the id of a module the device host-links, such as Arora's animation module, fails the build, naming both modules. A device loads one module per id; the host-linked module used to take the id in dispatch while the guest's functions stayed listed.
