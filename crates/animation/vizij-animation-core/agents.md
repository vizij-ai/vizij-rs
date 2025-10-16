# vizij-animation-core — Agent Notes

- **Purpose**: Deterministic animation engine powering Bevy, wasm, and orchestrator integrations.
- **Hot spots**: `src/lib.rs` (engine API), `src/engine/`, `src/baking/`. Config tuning lives in `config.rs`.
- **Tests**: `cargo test -p vizij-animation-core` (unit + integration). Use fixtures via `vizij_test_fixtures::animations`.
- **Key dependencies**: `vizij-api-core` for values/paths; `vizij-test-fixtures` for sample assets.
- **When editing**:
  - Update the README sections on config/baking behaviour if you change APIs.
  - Run parity checks against wasm by rebuilding `pnpm run build:wasm:animation`.
  - Keep `Engine` and `InstanceCfg` constructors backwards compatible; orchestrator and wasm wrappers depend on them.
- **Follow-ups**: Roadmap tracks documentation gaps (EngineConfig tuning, derivative ordering) and potential profiling hooks—update items as you address them.
