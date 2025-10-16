# bevy_vizij_animation — Agent Notes

- **Purpose**: Bevy plugin embedding `vizij-animation-core` into ECS schedules (binding discovery, fixed-timestep updates).
- **Key modules**: `src/plugin.rs` (system ordering), `src/setters/`, `src/resources/`.
- **Commands**: `cargo test -p bevy_vizij_animation`; rebuild wasm if you change bindings (`pnpm run build:wasm:animation`).
- **Integration tips**: Register setters via `bevy_vizij_api`; respect `FixedDt` resource when changing update cadence.
- **Docs**: Update the stack README when touching system ordering or feature flags; cross-reference animation stack guidance.
- **Watch for**: Keep `urdf_ik` feature wiring consistent with `vizij-animation-core`; ensure new systems respect the plugin schedule.
