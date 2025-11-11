# @vizij/wasm-loader

## 0.1.4

### Patch Changes

- 9bd0189: Fix publishing to include dist again

## 0.1.3

### Patch Changes

- f6cba9e: Release process testing patch bump

## 0.1.2

### Patch Changes

- Add browser-specific entry point and shared loader core so bundlers choose a version without `fs/promises`, fixing Next.js builds while keeping the Node path intact.

## 0.1.1

### Patch Changes

- Replaced dynamic Node specifiers with literal imports and allow defaultWasmUrl() to return either a string or URL, preventing webpack’s RelativeURL fallback from exploding. Prefer a static ESM import (with fallback), cache the wasm path as a string, and document the bundler setup needed for async WebAssembly in Next.js/Webpack.

Release notes live in [Changesets](../../../.changeset/README.md). Queue changes with `pnpm changeset` before publishing.
