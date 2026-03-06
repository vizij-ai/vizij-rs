# npm Workspace — Agent Notes

- **Packages**: `@vizij/animation-wasm`, `@vizij/node-graph-wasm`, `@vizij/orchestrator-wasm`, `@vizij/value-json`, `@vizij/test-fixtures`, `@vizij/wasm-loader`.
- **Bootstrap**: `pnpm install` from repo root; rebuild shared packages with `pnpm run build:shared`.
- **Local links for `vizij-web`**: Build the stacks you need, then in `vizij-web` run `pnpm run wasm:link` (or scope with `WASM_PKGS="node-graph-wasm orchestrator-wasm"`). `pnpm run wasm:status` confirms whether the workspace points at `vizij-rs` or the registry. Use `pnpm run wasm:unlink` when you’re done.
- **Testing**: `pnpm run test` executes the workspace Rust, wasm-package, and shared-package checks. Run package-specific commands with `pnpm --filter @vizij/<pkg> test` where a package exposes a test script, or `build` for build-only packages.
- **Generated artefacts**: wasm packages write to `pkg/` and `dist/`. Do not edit generated files by hand; rerun the build scripts instead.
- **Docs**: Keep each package README + `agents.md` in sync with ABI changes, fixture additions, or loader updates.
