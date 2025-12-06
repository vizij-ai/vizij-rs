# vizij-graph-wasm — Agent Notes

- **Purpose**: wasm-bindgen bridge for `vizij-graph-core`, consumed by `@vizij/node-graph-wasm`.
- **Key files**: `src/lib.rs`, feature flags in `Cargo.toml`, generated `pkg/`.
- **Commands**: `pnpm run build:wasm:graph`, `cargo test -p vizij-graph-wasm`, `pnpm --filter @vizij/node-graph-wasm test`.
- **Doc reminders**: Keep the README current on bundler targets, output payload examples, and troubleshooting guidance.
- **Sync**: Keep `abi_version()` and normalisation helpers aligned with npm wrapper expectations.
- **Follow-ups**: wasm integration tests and incremental evaluation ideas tracked in `ROADMAP.md`.
