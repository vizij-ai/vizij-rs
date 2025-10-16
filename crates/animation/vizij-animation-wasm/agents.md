# vizij-animation-wasm — Agent Notes

- **Purpose**: wasm-bindgen bridge for the animation engine, consumed by `@vizij/animation-wasm`.
- **Key files**: `src/lib.rs`, `Cargo.toml` (feature flags), `pkg/` output (generated).
- **Build/test**:
  - Build: `pnpm run build:wasm:animation` (invokes `scripts/build-animation-wasm.mjs`).
  - Rust tests: `cargo test -p vizij-animation-wasm`.
  - JS tests: `pnpm --filter @vizij/animation-wasm test`.
- **Doc hooks**: Maintain README notes on bundler targets and troubleshooting; update ABI version guard (`abi_version()`).
- **Common pitfalls**: Regenerate bindings after touching `vizij-animation-core`; keep `wasm-bindgen` signatures aligned with npm wrapper expectations.
