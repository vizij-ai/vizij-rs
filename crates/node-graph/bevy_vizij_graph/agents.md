# bevy_vizij_graph — Agent Notes

- **Purpose**: Bevy plugin that stages inputs, evaluates graphs, and exposes outputs/writes each frame.
- **Key files**: `src/plugin.rs` (schedule setup), `src/resources.rs`, `src/systems/`.
- **Commands**: `cargo test -p bevy_vizij_graph`; ensure orchestrator integration tests still pass if behaviour shifts.
- **Docs**: Keep the README sections on fixed timestep configuration, orchestrator wiring, and logging up to date.
- **Integration tips**: Respect `PendingInputs` ordering and runtime resets when swapping `GraphSpecRes`.
- **Feature flags**: Keep `urdf_ik` default alignment with `vizij-graph-core`.
