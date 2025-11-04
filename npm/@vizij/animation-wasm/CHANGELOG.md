# @vizij/animation-wasm

## 0.3.3

### Patch Changes

- Fix wasm entrypoints to resolve their generated JS shim from pkg/… when packaged. Adds ambient module shims so TypeScript accepts the path.

## 0.3.2

### Patch Changes

- Replaced dynamic Node specifiers with literal imports and allow defaultWasmUrl() to return either a string or URL, preventing webpack’s RelativeURL fallback from exploding. Prefer a static ESM import (with fallback), cache the wasm path as a string, and document the bundler setup needed for async WebAssembly in Next.js/Webpack.
- Updated dependencies
  - @vizij/wasm-loader@0.1.1

Release notes are generated via [Changesets](../../../.changeset/README.md). Use `pnpm changeset` to record updates before publishing.
