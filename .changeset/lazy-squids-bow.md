---
"@vizij/orchestrator-wasm": minor
"@vizij/node-graph-wasm": minor
---

Add performance-focused wrapper APIs for delta snapshots, hot-path staging, and structural edits.

- node-graph-wasm: add slot/hot-path staging helpers, delta-aware eval paths, and batch output APIs; expose specVersion/fingerprint in GraphSpec.

- orchestrator-wasm: add replaceGraph, stepDelta, and smart/hot input staging helpers; surface GraphSpec cache fields and GraphReplaceConfig.
