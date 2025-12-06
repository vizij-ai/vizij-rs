# vizij-graph-core — Agent Notes

- **Purpose**: Core evaluator for Vizij GraphSpec documents (topological execution, staging, write batching).
- **Hot spots**: `src/eval/`, `src/runtime/graph_runtime.rs`, `src/types/graph_spec.rs`.
- **Commands**: `cargo test -p vizij-graph-core`; run with `--features urdf_ik` when touching robotics nodes.
- **Docs**: Update the README when touching selectors, caching, or parameter APIs.
- **Diagnostics**: Profiling/tracing ideas are on the roadmap—coordinate changes to runtime metrics with orchestrator.
- **Release caution**: Schema changes require synchronising fixture normalisation and wasm wrappers.
