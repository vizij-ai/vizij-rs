# npm Workspace — Agent Notes

- **Packages**: `@vizij/animation-wasm`, `@vizij/node-graph-wasm`, `@vizij/orchestrator-wasm`, `@vizij/value-json`, `@vizij/test-fixtures`, `@vizij/wasm-loader`.
- **Bootstrap**: `pnpm install` from repo root; rebuild shared packages with `pnpm run build:shared`.
- **Global linking**: Use `pnpm run link:wasm` (aggregates value-json + wasm packages) when testing with `vizij-web`. Remember to unlink before committing.
- **Testing**: `pnpm run test` executes wasm + shared package Vitest suites; run package-specific tests via `pnpm --filter @vizij/<pkg> test`.
- **Generated artefacts**: wasm packages write to `pkg/` and `dist/`. Do not edit generated files by hand; rerun the build scripts instead.
- **Docs**: Keep each package README + `agents.md` in sync with ABI changes, fixture additions, or loader updates.
