# vizij-orchestrator-wasm — Agent Notes

- **Purpose**: wasm-bindgen bindings exposing `vizij-orchestrator-core` APIs to JavaScript.
- **Key files**: `src/lib.rs`, `Cargo.toml` (feature flags), generated `pkg/`.
- **Commands**: `pnpm run build:wasm:orchestrator`, `cargo test -p vizij-orchestrator-wasm`, `pnpm --filter @vizij/orchestrator-wasm test`.
- **Docs**: Keep README entries on JSON payload structures, ABI mismatches, and custom wasm hosting up to date.
- **Sync**: Ensure `abi_version()` stays aligned with npm wrapper; rebuild after orchestrator core changes.
- **Future work**: wasm integration tests, incremental tick exposure, and bundled fixtures are tracked in `ROADMAP.md`.
