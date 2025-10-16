# vizij-test-fixtures

> **Shared JSON fixtures for Vizij engines, graphs, and orchestrations.**

`vizij-test-fixtures` loads entries declared in `fixtures/manifest.json` and exposes helpers for animation, node-graph, and orchestration assets. Crates in this workspace (and downstream tooling) rely on it to keep regression tests and demos in sync with the canonical fixture set.

---

## Table of Contents

1. [Overview](#overview)
2. [Fixture Types](#fixture-types)
3. [Usage](#usage)
4. [Development & Testing](#development--testing)
5. [Related Packages](#related-packages)

---

## Overview

- Reads `fixtures/manifest.json` at compile time and maps logical names to on-disk JSON files.
- Provides modules per domain (`animations`, `node_graphs`, `orchestrations`) with helpers to list keys, load parsed JSON, and resolve file-system paths.
- Normalises error handling with `anyhow::Result` and contextual messages so missing fixtures are easy to diagnose.
- Powers integration tests across the workspace (animation, graph, orchestrator crates).

---

## Fixture Types

| Module | Helpers | Notes |
|--------|---------|-------|
| `animations` | `keys()`, `json()`, `load<T>()`, `path()` | Returns stored animation JSON compatible with `vizij-animation-core`. |
| `node_graphs` | `keys()`, `spec_json()`, `spec<T>()`, `stage_json()`, `stage<T>()`, `spec_path()`, `stage_path()` | Supports paired stage data for seeding graph inputs. |
| `orchestrations` | `keys()`, `json()`, `load<T>()`, `path()` | Resolves orchestrator bundle descriptors for npm and Rust demos. |

All helpers resolve paths relative to the repository’s `fixtures/` directory, ensuring test code can locate assets without `CARGO_MANIFEST_DIR` gymnastics.

---

## Usage

```rust
use vizij_test_fixtures::{animations, node_graphs, orchestrations};

// Load stored animation JSON
let stored: serde_json::Value = animations::load("pose-quat-transform")?;

// Pull a graph spec and optional stage payload
let spec: serde_json::Value = node_graphs::spec("simple-gain-offset")?;
let stage = node_graphs::stage_json("simple-gain-offset")?;

// Resolve an orchestration descriptor
let bundle: serde_json::Value = orchestrations::load("blend-pose-pipeline")?;
```

Use `*_path()` helpers when you need a filesystem path (e.g., to hand off to wasm-pack scripts or external tooling).

### Manifest snippet

Each entry in `fixtures/manifest.json` maps a logical key to a relative path. Node graphs may provide optional staged inputs:

```jsonc
{
  "node-graphs": {
    "simple-gain-offset": {
      "spec": "node_graphs/simple-gain-offset.json",
      "stage": "node_graphs/simple-gain-offset.stage.json"
    }
  }
}
```

Stage payloads mirror the `{ value, shape }` structure returned by the runtime, so consumers can prime inputs deterministically.

---

## Development & Testing

```bash
cargo test -p vizij-test-fixtures
```

Tests ensure every manifest entry deserialises, fixture paths exist, and orchestrations reference valid animation assets.

When adding new fixtures:

1. Register them in `fixtures/manifest.json` and include staged payloads when appropriate.
2. Include representative tests here or in the consuming crate.
3. Regenerate the npm bundle (`pnpm run build:shared`) so browser consumers stay aligned.
4. Bump both the Rust crate and npm package versions together to signal downstream updates.

Binary fixtures are discouraged; if you must add one, store it under `fixtures/bin/` and gate access through helper functions that stream from disk to avoid bloating crates.io packages.

---

## Related Packages

- [`npm/@vizij/test-fixtures`](../../../npm/@vizij/test-fixtures/README.md) – browser/Node distribution of the same manifest.
- [`vizij-animation-core`](../../animation/vizij-animation-core/README.md), [`vizij-graph-core`](../../node-graph/vizij-graph-core/README.md), [`vizij-orchestrator-core`](../../orchestrator/vizij-orchestrator-core/README.md) – primary consumers in Rust.

