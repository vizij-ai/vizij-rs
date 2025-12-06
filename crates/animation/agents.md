# Animation Stack — Agent Notes

- **Scope**: Shared guidance for `vizij-animation-core`, `bevy_vizij_animation`, and `vizij-animation-wasm`.
- **Key files**: `vizij-animation-core/src/lib.rs`, `bevy_vizij_animation/src/`, `vizij-animation-wasm/src/lib.rs`, `scripts/build-animation-wasm.mjs`.
- **Primary commands**:
  - Rust: `cargo test -p vizij-animation-core`, `cargo test -p bevy_vizij_animation`.
  - WASM: `pnpm run build:wasm:animation`, `pnpm --filter @vizij/animation-wasm test`.
  - Watcher: `pnpm run watch:wasm:animation` (requires `cargo-watch`).
- **Fixture helpers**: Use `vizij_test_fixtures::animations` or `@vizij/test-fixtures/animations` when writing tests or demos.
- **Docs**: Revisit `crates/animation/README.md` and crate-specific sections before editing; keep ABI guard (`abi_version()`) in sync with npm packages.
- **Common traps**: Rebuild the wasm crate after touching `vizij-animation-core`; watch for Bevy feature flags when introducing new dependencies.
