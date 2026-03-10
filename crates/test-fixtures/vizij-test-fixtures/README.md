# vizij-test-fixtures

> Shared JSON fixtures for Vizij animations, node graphs, and orchestrations.

`vizij-test-fixtures` reads `fixtures/manifest.json` and exposes a small Rust API for loading canonical fixture assets from tests, examples, and supporting tooling. The crate is private to this workspace (`publish = false`).

## Overview

- Loads the fixture manifest once and resolves logical keys to on-disk JSON.
- Exposes helpers for animations, node graphs, and orchestrations.
- Returns `anyhow::Result` with contextual errors for missing keys or files.
- Keeps Rust and npm fixture consumers aligned through the same manifest.

## Modules

| Module | Helpers |
|--------|---------|
| `animations` | `keys()`, `json()`, `load<T>()`, `path()` |
| `node_graphs` | `keys()`, `spec_json()`, `spec<T>()`, `stage_json()`, `stage<T>()`, `spec_path()`, `stage_path()` |
| `orchestrations` | `keys()`, `json()`, `load<T>()`, `path()` |

## Usage

```rust
use vizij_test_fixtures::{animations, node_graphs, orchestrations};

let stored: serde_json::Value = animations::load("pose-quat-transform")?;
let spec: serde_json::Value = node_graphs::spec("simple-gain-offset")?;
let stage = node_graphs::stage_json("simple-gain-offset")?;
let bundle: serde_json::Value = orchestrations::load("blend-pose-pipeline")?;
```

Use the `*_path()` helpers when a downstream tool needs the fixture file path instead of parsed JSON.

## Development And Testing

```bash
cargo test -p vizij-test-fixtures
```

When adding fixtures:

1. Register them in `fixtures/manifest.json`.
2. Add or update consuming tests.
3. Rebuild the npm fixture bundle with `pnpm run build:shared`.

## Related Packages

- [`@vizij/test-fixtures`](../../../npm/@vizij/test-fixtures/README.md)
- [`vizij-animation-core`](../../animation/vizij-animation-core/README.md)
- [`vizij-graph-core`](../../node-graph/vizij-graph-core/README.md)
- [`vizij-orchestrator-core`](../../orchestrator/vizij-orchestrator-core/README.md)
