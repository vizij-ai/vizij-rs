---
"@vizij/orchestrator-wasm": patch
---

Publish a new orchestrator-wasm build that recognizes the newly-added node-graph noise node variants (`simplenoise`, `perlinnoise`, `simplexnoise`) when registering graph specs.

This resolves runtime graph registration failures when `@vizij/node-graph-wasm@0.6.x` graphs are passed through orchestrator.
