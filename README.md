# Vizij RS Workspace

> **Rust cores, Bevy plugins, and WASM bridges for Vizij’s real‑time animation platform.**

This repository contains the Rust source for Vizij’s animation, node graph, and orchestration stacks together with the tooling needed to surface them in web applications. Each domain ships as a trio of crates:

- a **core crate** with deterministic runtime logic,
- an optional **Bevy integration**,
- and a **WASM binding** that is re-exported through `npm/@vizij/*-wasm`.

What you read here should give you everything you need to build, test, and publish those artifacts.

---

## Table of Contents

1. [Workspace Layout](#workspace-layout)
2. [Domain Stacks](#domain-stacks)
3. [Toolchain & Requirements](#toolchain--requirements)
4. [Setup](#setup)
5. [Common Workflows](#common-workflows)
6. [Testing](#testing)
7. [Publishing & Versioning](#publishing--versioning)
8. [Development Tips](#development-tips)
9. [Reference Documentation](#reference-documentation)

---

## Workspace Layout

```
vizij-rs/
├─ crates/
│  ├─ api/
│  │  ├─ vizij-api-core            # Shared Value/Shape/TypedPath types
│  │  ├─ vizij-api-wasm            # wasm-bindgen helpers for Value/WriteBatch JSON
│  │  └─ bevy_vizij_api            # Bevy utilities for applying WriteOps
│  ├─ animation/
│  │  ├─ vizij-animation-core      # Deterministic animation engine
│  │  ├─ bevy_vizij_animation      # Bevy plugin wrapping the engine
│  │  └─ vizij-animation-wasm      # wasm-bindgen binding
│  ├─ node-graph/
│  │  ├─ vizij-graph-core          # Data-flow node graph evaluator
│  │  ├─ bevy_vizij_graph          # Bevy plugin
│  │  └─ vizij-graph-wasm          # wasm-bindgen binding
│  └─ orchestrator/
│     ├─ vizij-orchestrator-core   # Blackboard + pass scheduling runtime
│     └─ vizij-orchestrator-wasm   # wasm-bindgen binding
├─ npm/
│  ├─ @vizij/animation-wasm        # Stable ESM wrapper around `vizij-animation-wasm`
│  ├─ @vizij/node-graph-wasm       # Wrapper around `vizij-graph-wasm`
│  ├─ @vizij/orchestrator-wasm     # Wrapper around `vizij-orchestrator-wasm`
│  ├─ @vizij/value-json            # Shared JSON coercion helpers
│  └─ @vizij/wasm-loader           # Loader that enforces ABI compatibility
├─ fixtures/                       # Sample graphs, animations, orchestrations
└─ scripts/                        # Build, watch, and release helpers
```

Every crate includes a dedicated README with domain-specific guidance; the top-level README focuses on cross-cutting processes.

---

## Domain Stacks

| Stack          | Core Crate               | Bevy Adapter              | WASM Binding               | npm wrapper                  |
| -------------- | ------------------------ | ------------------------- | -------------------------- | ---------------------------- |
| Animation      | `vizij-animation-core`   | `bevy_vizij_animation`    | `vizij-animation-wasm`     | `@vizij/animation-wasm`      |
| Node graph     | `vizij-graph-core`       | `bevy_vizij_graph`        | `vizij-graph-wasm`         | `@vizij/node-graph-wasm`     |
| Orchestrator   | `vizij-orchestrator-core`| (planned)                 | `vizij-orchestrator-wasm`  | `@vizij/orchestrator-wasm`   |

Shared API crates (`vizij-api-core`, `vizij-api-wasm`, `bevy_vizij_api`) provide the Value/Shape/TyperPath contract that keeps the stacks interoperable.

Each WASM crate exposes a stable `abi_version()` (currently `2`); the npm wrappers verify this at runtime and instruct you to rebuild if versions drift.

---

## Toolchain & Requirements

- **Rust**: Stable toolchain via [rustup](https://rustup.rs/) plus the `wasm32-unknown-unknown` target.
- **Node.js**: v18 or newer (v20 recommended) with Corepack enabled (`corepack enable`).
- **pnpm**: v9.x (locked at `pnpm-lock.yaml`).
- **wasm-pack** & **wasm-bindgen-cli**: `cargo install wasm-pack wasm-bindgen-cli`
- Optional: `cargo-watch`, `just`, or other workflow helpers.

Ensure Git LFS is configured if you pull large fixture assets.

---

## Setup

Clone the repo and install dependencies the first time you work with the workspace:

```bash
git clone https://github.com/vizij-ai/vizij-rs.git
cd vizij-rs

corepack enable
pnpm install               # install npm workspace deps (wasm wrappers, scripts)
cargo fetch                # prefetch Rust dependencies (optional but speeds CI)
rustup target add wasm32-unknown-unknown
```

Install the shared git hooks (formatting, clippy, tests):

```bash
bash scripts/install-git-hooks.sh
```

Hooks can be bypassed with `SKIP_GIT_HOOKS=1` or extended using the `HOOK_RUN_*` environment variables documented in `scripts/hook-tasks.sh`.

---

## Common Workflows

### Build all crates and wrappers

```bash
# remember to install first
pnpm run build
```

This command runs the Rust checks followed by each npm wrapper build. The Rust build uses the fast `cargo check --workspace` path; WASM bundles land in `crates/*/*/pkg/` and are copied into `npm/@vizij/*/pkg/`.

### Build a specific WASM stack

```bash
pnpm run build:wasm:animation
pnpm run build:wasm:graph
pnpm run build:wasm:orchestrator
```

Each script invokes the corresponding Node helper in `scripts/` which runs `cargo build --target wasm32-unknown-unknown`, `wasm-bindgen`, and copies the generated JS+wasm artefacts into the npm package.

### Continuous rebuilds during development

All WASM stacks expose watch scripts that rely on `cargo-watch`:

```bash
pnpm run watch:wasm:graph
pnpm run watch:wasm:animation
pnpm run watch:wasm:orchestrator
```

These scripts rebuild the WASM artefacts whenever source files change. For short-lived experiments you can still publish a global link via the `link:wasm:*` scripts, but the recommended flow is:

1. Build the desired stack(s) here (`pnpm run build:wasm:graph` etc.).
2. In `vizij-web`, temporarily depend on those builds using `pnpm add @vizij/<pkg>@link:../vizij-rs/npm/@vizij/<pkg>`.
3. Revert those `link:` dependencies before committing to return to the published packages.

This keeps the published versions as the default source of truth while still allowing synchronous iteration when necessary.

---

## Testing

Rust tests are colocated with their crates. To run the full test suite:

```bash
pnpm run test:rust        # cargo fmt --check, clippy, test
pnpm run check:rust       # adds workspace build to the test suite
```

Per-crate runs are equally useful:

```bash
cargo test -p vizij-graph-core
cargo test -p vizij-animation-core
cargo test -p vizij-orchestrator-core
```

Many WASM crates include wasm-bindgen integration tests under `tests/` that execute with `wasm-pack test --node`. Trigger them via the package script:

```bash
pnpm --filter "@vizij/node-graph-wasm" test
```

Fixtures live in `fixtures/` for repeatable scenario testing. Use them in integration tests or in downstream applications via `npm/@vizij/test-fixtures`.

---

## Publishing & Versioning

Each domain stack keeps crate, WASM crate, and npm wrapper versions in lockstep. When you publish:

1. Bump the version in all three manifests (`Cargo.toml`, `package.json`).
2. Use the appropriate tags on the main branch to trigger the workflow. Use `git push && git push --tags` after tagging them appropriately.

`scripts/dry-run-release.sh` runs through the entire sequence without pushing artefacts; use it to confirm that crates build, WASM bundling succeeds, and npm tarballs contain the correct files.

---

## Development Tips

- **ABI mismatches**: If you see `ABI mismatch: expected 2` from a wasm wrapper, rebuild the Rust crate and rerun `pnpm run build:wasm:<stack>` to regenerate JS glue.
- **Time-dependent nodes**: `vizij-orchestrator-core` now advances `GraphRuntime.t`/`dt` on each evaluation. Ensure any custom integrations do the same when you embed `GraphController` elsewhere.
- **URDF features**: Enable the `urdf_ik` feature on `vizij-graph-core` if you need robotics nodes in native builds (`cargo build -p vizij-graph-core --features urdf_ik`). The WASM package ships with the feature enabled by default.
- **Diagnostics**: `vizij-orchestrator-core` captures conflict logs and per-pass timings inside `OrchestratorFrame`. Use these to debug controller order and merge behaviour before instrumenting downstream apps.
- **Fixtures**: The `fixtures/` directory includes ready-to-use JSON files for graphs, animations, and orchestrations. They are mirrored into `npm/@vizij/test-fixtures` for web consumers.

---

## Reference Documentation

- [vizij-animation-core/README](crates/animation/vizij-animation-core/README.md)
- [vizij-graph-core/README](crates/node-graph/vizij-graph-core/README.md)
- [vizij-orchestrator-core/README](crates/orchestrator/vizij-orchestrator-core/README.md)
- [vizij-api-core/README](crates/api/vizij-api-core/README.md)
- [@vizij/node-graph-wasm/README](npm/@vizij/node-graph-wasm/README.md)
- [@vizij/orchestrator-wasm/README](npm/@vizij/orchestrator-wasm/README.md)

If you notice gaps or outdated instructions, please open an issue or ping the Vizij runtime team. High-quality documentation is as critical as the code it describes.
