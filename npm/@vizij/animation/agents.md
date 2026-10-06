# @vizij/animation — Agent Notes

- **Purpose**: ESM wrapper around `vizij-animation-wasm` with high-level `Engine` API, fixtures, and ABI guards.
- **Key files**: `src/index.ts`, `src/engine.ts`, `src/types.ts`, `pkg/` (generated wasm outputs).
- **Commands**: `pnpm run build:wasm:animation`, `pnpm --filter @vizij/animation test`.
- **Docs**: Update the README when changing bundler guidance, fixture exports, or loader behaviour.
- **Integration**: Uses `@vizij/value-json`, `@vizij/test-fixtures`, and `@vizij/wasm-loader`; keep versions aligned.
- **Watch for**: Ensure `abi_version()` checks still match the wasm crate after rebuilds; re-export new fixture helpers as needed.
- **Release**: Add a changeset (`pnpm changeset`) in your PR. Once it merges, CI opens or updates the Version Packages PR; merging that publishes the package.
