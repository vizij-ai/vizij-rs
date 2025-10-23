# @vizij/orchestrator-wasm — Agent Notes

- **Purpose**: TypeScript wrapper for `vizij-orchestrator-wasm` providing orchestration bundles, loaders, and ABI checks.
- **Key files**: `src/index.ts`, `src/orchestrator.ts`, `src/types.ts`, `pkg/`.
- **Commands**: `pnpm run build:wasm:orchestrator`, `pnpm --filter @vizij/orchestrator-wasm test`.
- **Docs**: Update the README when adjusting bundle structures, ABI handling, or wasm-loader integration guidance.
- **Integration**: Consumes fixtures from `@vizij/test-fixtures` and loader utilities from `@vizij/wasm-loader`.
- **Roadmap**: Snapshot tests, event streams, and builder helpers are open initiatives—reference `ROADMAP.md`.
- **Release**: Use `pnpm changeset`, `pnpm version:packages`, and `pnpm release` (from repo root) after verifying the wasm build.
