# API Stack — Agent Notes

- **Scope**: `vizij-api-core`, `bevy_vizij_api`, `vizij-api-wasm`.
- **Purpose**: Shared Value/Shape/TypedPath contracts and Bevy/WASM helpers that keep stacks interoperable.
- **Key commands**: `cargo test -p vizij-api-core`, `cargo test -p bevy_vizij_api`, `cargo test -p vizij-api-wasm`, `pnpm --filter @vizij/value-json test`.
- **Dependencies**: Changes ripple into animation, graph, orchestrator stacks—coordinate via `ROADMAP.md`.
- **Doc hygiene**: Keep crate READMEs up to date when altering serialization, setter registries, or wasm exports.
- **Cross-repo**: Sync schema changes with `vizij-web` to avoid breaking browser clients.
