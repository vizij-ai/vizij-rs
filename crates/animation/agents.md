# Animation Stack — Agent Notes

- **Primary commands**:
  - WASM: `pnpm run build:wasm:animation`, `pnpm --filter @vizij/animation test`.
  - Watcher: `pnpm run watch:wasm:animation` (requires `cargo-watch`).
- **Fixture helpers**: Use `vizij_test_fixtures::animations` or `@vizij/test-fixtures/animations` when writing tests or demos.
- **Docs**: Revisit `crates/animation/README.md` and crate-specific sections before editing; keep ABI guard (`abi_version()`) in sync with npm packages.
- **Common traps**: Rebuild the wasm crate after touching `vizij-animation-core`; check wasm compatibility when introducing new dependencies.
