# @vizij/wasm-loader — Agent Notes

- **Purpose**: Shared helper that loads wasm-bindgen modules, resolves `file://` URLs, caches bindings, and enforces ABI checks.
- **Key files**: `src/index.ts`, TypeScript declarations.
- **Commands**: `pnpm --filter @vizij/wasm-loader test`; rebuild via `pnpm run build:shared`.
- **Integration**: Consumed by all wasm npm packages—coordinate breaking changes carefully and update their loaders simultaneously.
- **Docs**: Maintain README coverage for multi-module extensions, error translation, and bundler configuration tips.
- **Future work**: Loader builder utilities, telemetry, and sync init pathways are tracked in `ROADMAP.md`.
- **Release**: Log updates with `pnpm changeset`, apply them via `pnpm version:packages`, and publish using `pnpm release`.
