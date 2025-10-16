# @vizij/animation-wasm — Agent Notes

- **Purpose**: ESM wrapper around `vizij-animation-wasm` with high-level `Engine` API, fixtures, and ABI guards.
- **Key files**: `src/index.ts`, `src/engine.ts`, `src/types.ts`, `pkg/` (generated wasm outputs).
- **Commands**: `pnpm run build:wasm:animation`, `pnpm --filter @vizij/animation-wasm test`.
- **Docs**: Update the README when changing bundler guidance, fixture exports, or loader behaviour.
- **Integration**: Uses `@vizij/value-json`, `@vizij/test-fixtures`, and `@vizij/wasm-loader`; keep versions aligned.
- **Watch for**: Ensure `abi_version()` checks still match the wasm crate after rebuilds; re-export new fixture helpers as needed.
