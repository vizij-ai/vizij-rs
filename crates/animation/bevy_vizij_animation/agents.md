# bevy_vizij_animation — Agent Notes

- **Purpose**: Bevy plugin embedding `vizij-animation-core` into ECS schedules (binding discovery, fixed-timestep updates).
- **Key modules**: `src/lib.rs`, `src/components.rs`, `src/resources.rs`, `src/systems.rs`.
- **Commands**: `cargo test -p bevy_vizij_animation`; rebuild wasm if you change bindings (`pnpm run build:wasm:animation`).
- **Integration tips**: Register setters via `bevy_vizij_api`; respect `FixedDt` resource when changing update cadence.
- **Docs**: Update the stack README when touching system ordering, binding resources, or custom setter guidance; cross-reference animation stack guidance.
- **Watch for**: Ensure new systems respect the plugin schedule and keep `WriterRegistry` integration accurate.
