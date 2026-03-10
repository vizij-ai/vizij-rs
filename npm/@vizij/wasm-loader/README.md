# @vizij/wasm-loader

> **Shared loader helpers for Vizij WebAssembly packages.**

`@vizij/wasm-loader` encapsulates the boilerplate required to initialise Vizij’s wasm-bindgen artefacts across browsers, Node, and Electron/Tauri hosts. The loader caches bindings, resolves file URLs, enforces ABI checks, and delegates to package-specific initialisers.

---

## Table of Contents

1. [Overview](#overview)
2. [API](#api)
3. [Usage](#usage)
4. [Development & Testing](#development--testing)
5. [Related Packages](#related-packages)

---

## Overview

- Provides a single `loadBindings` helper that wraps wasm-bindgen initialisation (`init(module, initArg)`).
- Normalises file URL handling by reading file:// URIs via Node’s `fs/promises` when running outside the browser.
- Caches the returned bindings so repeated `init()` calls reuse the same module.
- Enforces ABI compatibility when the caller supplies an `expectedAbi` and `getAbiVersion` function.

---

## API

```ts
import { loadBindings, type LoadBindingsOptions, type InitInput } from "@vizij/wasm-loader";

interface LoadBindingsOptions<TBindings> {
  cache: { current: TBindings | null };
  importModule: () => Promise<any>;
  defaultWasmUrl: () => URL | string;
  init: (module: any, initArg: unknown) => Promise<void>;
  getBindings?: (module: any) => TBindings;
  expectedAbi?: number;
  getAbiVersion?: (bindings: TBindings) => number;
}
```

- `cache` – Mutable holder for the currently loaded bindings (passed by reference from the consuming package).
- `importModule` – Dynamic import that resolves to the wasm-bindgen JS shim.
- `defaultWasmUrl` – Function returning the default `.wasm` location (`new URL("./pkg/package_bg.wasm", import.meta.url).toString()` in many packages).
- `init` – Async function that initialises the wasm module (`module.default(initArg)` for wasm-bindgen).
- `getBindings` – Optional extractor (defaults to returning the module itself).
- `expectedAbi` / `getAbiVersion` – Optional ABI guard to ensure wasm packages are rebuilt together.
- `InitInput` – Union covering strings, URLs, ArrayBuffers, `WebAssembly.Module`, and `Response`.

---

## Usage

Each wasm npm package wraps `loadBindings` with package-specific defaults:

```ts
// Inside a Vizij wasm wrapper (simplified)
const cache = { current: null };

export async function init(input?: InitInput): Promise<void> {
  await loadBindings({
    cache,
    importModule: () => import("../../pkg/package.js"),
    defaultWasmUrl: () =>
      new URL("../../pkg/package_bg.wasm", import.meta.url).toString(),
    init: (module, initArg) => module.default(initArg),
    getBindings: (module) => module,
    expectedAbi: 2,
    getAbiVersion: (bindings) => bindings.abi_version?.(),
  }, input);
}
```

Consumers can override the init argument when hosting assets elsewhere:

```ts
import { init } from "@vizij/animation-wasm";
import { readFile } from "node:fs/promises";

const bytes = await readFile("dist/animation_wasm_bg.wasm");
await init(bytes); // load from a buffer instead of fetch()
```

The loader automatically converts `file://` URLs into buffers when running under Node.

### Extending for multi-module packages

If you expose separate debug/release builds, call `loadBindings` with different caches and default URLs for each variant. This keeps cached bindings isolated while sharing the same loader logic.

### Error translation

Wrap initialisation in `try/catch` to present actionable messages (e.g. `Failed to load animation engine: ABI mismatch`). The loader surfaces meaningful strings—forward them to your UI or logs.

---

## Bundler Notes

- Tree-shakeable ESM build with explicit `browser` and `node` exports.
- Browser bundles never include Node-specific code paths because the `fs/promises` import is behind a dynamic `import()`.
- The browser entry (`@vizij/wasm-loader/browser`) skips the `file://` handling path entirely.

---

## Development & Testing

```bash
pnpm --filter @vizij/wasm-loader run build
```

`@vizij/wasm-loader` does not currently define a standalone test script; validate changes through the wrapper packages that consume it.

---

## Related Packages

- [`@vizij/animation-wasm`](../animation-wasm/README.md), [`@vizij/node-graph-wasm`](../node-graph-wasm/README.md), [`@vizij/orchestrator-wasm`](../orchestrator-wasm/README.md) – primary consumers of this loader.
- [`vizij-animation-wasm`](../../../crates/animation/vizij-animation-wasm/README.md) et al. – Rust crates that produce the wasm artefacts loaded through this helper.
