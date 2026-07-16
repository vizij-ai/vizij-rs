# Node Graph Stack — Agent Notes

- **Purpose**: Deterministic data-flow evaluation across Rust, Bevy, and wasm hosts.
- **Fixtures**: Use `vizij_test_fixtures::node_graphs` / `@vizij/test-fixtures` for specs + stage payloads.
- **Docs**: Consult the stack README before edits; capture selector/parameter guidance changes.
- **Special care**: Keep `urdf_ik` feature defaults aligned across crates; orchestrator depends on consistent behaviour.
