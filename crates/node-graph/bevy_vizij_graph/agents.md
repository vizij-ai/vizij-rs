# bevy_vizij_graph — Agent Notes

- **Purpose**: Bevy plugin that owns a `GraphSpec`, advances a persistent `GraphRuntime`, and exposes output snapshots each frame.
- **Key files**: `src/lib.rs` (resources, event, and schedule wiring).
- **Commands**: `cargo test -p bevy_vizij_graph`; ensure orchestrator integration tests still pass if behaviour shifts.
- **Docs**: Keep the README sections on staging through `GraphRuntimeResource`, `SetNodeParam`, and `WriterRegistry` integration up to date.
- **Integration tips**: Stage host inputs with `GraphRuntimeResource.0.set_input(...)`, and replace `GraphResource.0` with a cached spec when swapping graphs.
- **Feature flags**: Keep `urdf_ik` default alignment with `vizij-graph-core`.
