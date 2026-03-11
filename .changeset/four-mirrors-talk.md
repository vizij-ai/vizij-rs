---
"@vizij/wasm-loader": patch
"@vizij/animation-wasm": patch
"@vizij/node-graph-wasm": patch
"@vizij/orchestrator-wasm": patch
---

Fix wasm init normalization so wrapper packages accept `{ module_or_path: ... }`
inputs without double-wrapping them, and make the shared loader normalize that
object form consistently in Node and browser contexts.
