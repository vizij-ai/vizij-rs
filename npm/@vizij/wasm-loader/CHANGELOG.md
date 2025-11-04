# @vizij/wasm-loader

## 0.1.1

### Patch Changes

- Replaced dynamic Node specifiers with literal imports and allow defaultWasmUrl() to return either a string or URL, preventing webpack’s RelativeURL fallback from exploding. Prefer a static ESM import (with fallback), cache the wasm path as a string, and document the bundler setup needed for async WebAssembly in Next.js/Webpack.

Release notes live in [Changesets](../../../.changeset/README.md). Queue changes with `pnpm changeset` before publishing.
