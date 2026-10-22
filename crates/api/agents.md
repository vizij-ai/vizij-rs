# API Stack — Agent Notes

- **Scope**: `vizij-api-wasm`, over arora-sdk's `arora-shape`.
- **Purpose**: Shared Value/Shape/TypedPath contracts and WASM helpers that keep stacks interoperable.
- **Key commands**: `cargo test -p vizij-api-wasm`, `pnpm --filter @vizij/value-json test`.
- **Dependencies**: Changes ripple into the graph stack and the interop crates—coordinate via `ROADMAP.md`.
- **Doc hygiene**: Keep crate READMEs up to date when altering serialization, setter registries, or wasm exports.
- **Cross-repo**: Sync schema changes with `vizij-web` to avoid breaking browser clients.
