# @vizij/orchestrator-wasm

## 0.2.5

### Patch Changes

- Fix wasm entrypoints to resolve their generated JS shim from pkg/… when packaged. Adds ambient module shims so TypeScript accepts the path.

## 0.2.4

### Patch Changes

- Replaced dynamic Node specifiers with literal imports and allow defaultWasmUrl() to return either a string or URL, preventing webpack’s RelativeURL fallback from exploding. Prefer a static ESM import (with fallback), cache the wasm path as a string, and document the bundler setup needed for async WebAssembly in Next.js/Webpack.
- Updated dependencies
  - @vizij/wasm-loader@0.1.1

Release notes are maintained with [Changesets](../../../.changeset/README.md). Run `pnpm changeset` for publishable updates.
