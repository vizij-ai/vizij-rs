# Node Graph Stack — Agent Notes

- **Scope**: `vizij-graph-core`, `bevy_vizij_graph`, `vizij-graph-wasm`.
- **Purpose**: Deterministic data-flow evaluation across Rust, Bevy, and wasm hosts.
- **Key commands**: `cargo test -p vizij-graph-core`, `cargo test -p bevy_vizij_graph`, `pnpm run build:wasm:graph`, `pnpm --filter @vizij/node-graph-wasm test`.
- **Fixtures**: Use `vizij_test_fixtures::node_graphs` / `@vizij/test-fixtures` for specs + stage payloads.
- **Docs**: Consult the stack README before edits; capture selector/parameter guidance changes.
- **Special care**: Keep `urdf_ik` feature defaults aligned across crates; orchestrator depends on consistent behaviour.
