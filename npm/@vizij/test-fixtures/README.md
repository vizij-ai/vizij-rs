# @vizij/test-fixtures

> **Browser/Node-friendly bundle of Vizij animation, graph, and orchestration fixtures.**

This package mirrors the Rust crate `vizij-test-fixtures`, repackaging the fixtures declared in `fixtures/manifest.json` so JavaScript tooling, demos, and automated tests can load the same assets as the Rust workspace.

---

## Table of Contents

1. [Overview](#overview)
2. [Exports](#exports)
3. [Usage](#usage)
4. [Development & Testing](#development--testing)
5. [Related Packages](#related-packages)

---

## Overview

- Emits ESM (and d.ts) modules that expose fixture helpers per domain: `animations`, `nodeGraphs`, `orchestrations`.
- Ships a copy of `fixtures/manifest.json` plus JSON payloads referenced by name.
- Provides filesystem helpers (`fixturesRoot`, `resolveFixturePath`) for Node environments that need absolute paths (e.g., bundling, CLI tools).
- Used heavily by the wasm npm packages (`@vizij/animation-wasm`, `@vizij/node-graph-wasm`, `@vizij/orchestrator-wasm`) to keep demos and tests deterministic.

---

## Exports

```ts
import {
  animations,
  nodeGraphs,
  orchestrations,
  fixturesRoot,
  manifest,
  resolveFixturePath,
} from "@vizij/test-fixtures";
```

Each domain module exposes helpers similar to the Rust crate:

| Module | Helpers | Notes |
|--------|---------|-------|
| `animations` | `animationNames()`, `animationJson()`, `animationFixture<T>()`, `animationPath()` | Raw JSON strings or parsed data for stored animations. |
| `nodeGraphs` | `graphNames()`, `graphSpecJson()`, `graphSpec<T>()`, `stageJson()`, `stage<T>()`, `graphSpecPath()` | Supports optional stage payloads for seeding graph inputs. |
| `orchestrations` | `orchestrationNames()`, `orchestrationJson()`, `orchestrationFixture<T>()`, `orchestrationPath()` | Mirrors orchestrator bundles used by demos. |
| Shared | `manifest()`, `fixturesRoot()`, `resolveFixturePath(rel)` | Inspect the manifest or compute absolute paths from relative entries. |

---

## Usage

```ts
import { animations, nodeGraphs, orchestrations } from "@vizij/test-fixtures";

const storedAnimation = animations.animationFixture("pose-quat-transform");
const graphSpec = nodeGraphs.graphSpec("simple-gain-offset");
const orchestrationBundle = orchestrations.orchestrationFixture("chain-sign-slew-pipeline");

for (const name of nodeGraphs.graphNames()) {
  console.log("available graph", name);
}
```

In Node environments you can read files directly:

```ts
import { readFile } from "node:fs/promises";
import { resolveFixturePath } from "@vizij/test-fixtures";

const path = resolveFixturePath("animations/pose-quat-transform.json");
const text = await readFile(path, "utf8");
```

---

## Distribution Layout

- ESM entry: `dist/index.js` with matching type definitions (`dist/index.d.ts`).
- CJS build: `dist/index.cjs` for legacy Node bundlers.
- Raw JSON assets live under `dist/fixtures/**`; helper functions resolve paths relative to that directory.

Import the scoped entry points (`@vizij/test-fixtures/animations`, etc.) when you only need a subset of fixtures to keep bundle size down.

### Versioning guidance

- Update this package and the Rust crate (`vizij-test-fixtures`) together so manifests stay in sync across languages.
- After editing `fixtures/manifest.json`, run `pnpm run build:shared` to regenerate the npm artefacts before publishing.

---

## Development & Testing

```bash
pnpm install
pnpm test
```

Vitest verifies manifest coverage, JSON parsing, and helper behaviour. When adding new fixtures:

1. Update `fixtures/manifest.json` in the Rust workspace.
2. Copy or generate the corresponding JSON within `src/`.
3. Regenerate type definitions (`pnpm run build`) and extend tests.

---

## Related Packages

- [`vizij-test-fixtures`](../../../crates/test-fixtures/vizij-test-fixtures/README.md) – Rust crate exposing the same fixture catalogue.
- [`@vizij/animation-wasm`](../animation-wasm/README.md) • [`@vizij/node-graph-wasm`](../node-graph-wasm/README.md) • [`@vizij/orchestrator-wasm`](../orchestrator-wasm/README.md) – wasm packages that re-export helpers from this bundle.

