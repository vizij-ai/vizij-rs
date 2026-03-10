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
│  │  ├─ vizij-graph-wasm          # wasm-bindgen binding
│  │  └─ vizij-graph-registry-export # Registry export utility used by npm tooling
│  ├─ orchestrator/
│  │  ├─ vizij-orchestrator-core   # Blackboard + pass scheduling runtime
│  │  └─ vizij-orchestrator-wasm   # wasm-bindgen binding
│  └─ test-fixtures/
│     └─ vizij-test-fixtures       # Loads JSON fixtures referenced across stacks
├─ npm/
│  ├─ @vizij/animation-wasm        # Stable ESM wrapper around `vizij-animation-wasm`
│  ├─ @vizij/node-graph-wasm       # Wrapper around `vizij-graph-wasm`
│  ├─ @vizij/orchestrator-wasm     # Wrapper around `vizij-orchestrator-wasm`
│  ├─ @vizij/test-fixtures         # Browser bundle of shared JSON fixtures
│  ├─ @vizij/value-json            # Shared JSON coercion helpers
│  └─ @vizij/wasm-loader           # Loader that enforces ABI compatibility
├─ fixtures/                       # Sample graphs, animations, orchestrations (+ manifest)
└─ scripts/                        # Build, watch, and release helpers
```

The major runtime crates and npm packages include dedicated READMEs with domain-specific guidance; the top-level README focuses on cross-cutting processes.

---

## Domain Stacks

| Stack          | Core Crate               | Bevy Adapter              | WASM Binding               | npm wrapper                  |
| -------------- | ------------------------ | ------------------------- | -------------------------- | ---------------------------- |
| Animation      | `vizij-animation-core`   | `bevy_vizij_animation`    | `vizij-animation-wasm`     | `@vizij/animation-wasm`      |
| Node graph     | `vizij-graph-core`       | `bevy_vizij_graph`        | `vizij-graph-wasm`         | `@vizij/node-graph-wasm`     |
| Orchestrator   | `vizij-orchestrator-core`| (planned)                 | `vizij-orchestrator-wasm`  | `@vizij/orchestrator-wasm`   |
| Test fixtures  | `vizij-test-fixtures`    | —                         | —                          | `@vizij/test-fixtures`       |

Shared API crates (`vizij-api-core`, `vizij-api-wasm`, `bevy_vizij_api`) provide the Value/Shape/TypedPath contract that keeps the stacks interoperable.
`vizij-test-fixtures` exposes the JSON assets defined under `fixtures/`, while `vizij-graph-registry-export` supports registry generation for the node-graph npm tooling.

Each WASM crate exposes a stable `abi_version()` (currently `2`); the npm wrappers verify this at runtime and instruct you to rebuild if versions drift.

### Support Packages

| Package | Purpose |
|---------|---------|
| `@vizij/value-json` | TypeScript helpers that normalise Value/Shape payloads to match `vizij-api-core`. |
| `@vizij/test-fixtures` | Workspace package that bundles the shared fixture manifest + JSON for browsers and Node tooling. |
| `@vizij/wasm-loader` | Shared loader that caches wasm-bindgen modules, resolves `file://` URLs, and enforces ABI checks. |

These packages build quickly (`pnpm run build:shared`) and should be rebuilt whenever fixtures or API contracts change.

---

## Toolchain & Requirements

- **Rust**: Stable toolchain via [rustup](https://rustup.rs/) plus the `wasm32-unknown-unknown` target.
- **Node.js**: v18 or newer (v20 recommended) with Corepack enabled (`corepack enable`).
- **pnpm**: v9.x (locked at `pnpm-lock.yaml`).
- **wasm-pack** & **wasm-bindgen-cli**: `cargo install wasm-pack wasm-bindgen-cli`
- Optional: `cargo-watch` (required for `pnpm run watch:wasm:*` scripts), `just`, or other workflow helpers.

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
cargo install cargo-watch --locked   # optional: required for pnpm run watch:wasm:*
pnpm run build:shared               # build support packages (@vizij/value-json, @vizij/test-fixtures, @vizij/wasm-loader)
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

This command runs the Rust workspace build, each WASM build, and the shared npm package builds (`@vizij/value-json`, `@vizij/wasm-loader`, `@vizij/test-fixtures`). Under the hood `build:rust` runs `cargo build --all-features --all-targets`, and the WASM scripts write their generated artifacts directly into `npm/@vizij/*/pkg/`.

### Build a specific WASM stack

```bash
pnpm run build:wasm:animation
pnpm run build:wasm:graph
pnpm run build:wasm:orchestrator
```

Each script invokes the corresponding Node helper in `scripts/`, which runs `wasm-pack build --target web --release` for the matching crate and writes the generated JS + `.wasm` artifacts into the npm package's `pkg/` directory.

### Continuous rebuilds during development

All WASM stacks expose watch scripts that rely on `cargo-watch`:

```bash
pnpm run watch:wasm:graph
pnpm run watch:wasm:animation
pnpm run watch:wasm:orchestrator
```

These scripts rebuild the WASM artefacts whenever source files change. For short-lived experiments you can still publish a global link via the `link:wasm:*` scripts, but the recommended flow is:

1. Build the desired stack(s) here (`pnpm run build:wasm:graph` etc.).
2. In `vizij-web`, run `pnpm run wasm:link` (or `WASM_PKGS="node-graph-wasm orchestrator-wasm" pnpm run wasm:link`) to point the workspace at these builds.
3. Use `pnpm run wasm:status` in `vizij-web` to confirm the resolution, and `pnpm run wasm:unlink` when you want to return to the published packages.

This keeps the published versions as the default source of truth while still allowing synchronous iteration when necessary.

### Shared npm packages

The support packages (`@vizij/value-json`, `@vizij/test-fixtures`, `@vizij/wasm-loader`) share the build flow, but only `@vizij/value-json` currently has a standalone test script:

```bash
pnpm run build:shared   # rebuild support packages after API/fixture changes
pnpm run test:shared    # runs the current shared-package test target (@vizij/value-json)
pnpm run link:value-json
```

Use `pnpm run link:value-json` (or the aggregate `pnpm run link:wasm`) when you need to exercise local builds inside `vizij-web`.

### Generate API docs locally

Build the publishable docs site locally with:

```bash
pnpm run docs:site
```

That command generates:

- Rust API docs in `target/docs-rust/doc`
- TypeDoc output in `target/docs-typedoc`
- a publishable combined site in `target/docs-site`

The GitHub Actions docs workflow publishes `target/docs-site` to GitHub Pages from `main` and uploads it as an artifact for pull requests.

---

## Fixture Catalog

- `fixtures/manifest.json` is the single source of truth for animation, node-graph, and orchestration fixture names.
- The Rust crate [`vizij-test-fixtures`](crates/test-fixtures/vizij-test-fixtures/README.md) exposes helpers to load fixtures by name (JSON strings, strongly typed values, filesystem paths).
- The npm package [`@vizij/test-fixtures`](npm/@vizij/test-fixtures/README.md) mirrors the same manifest for browser/Node consumers; rebuild it with `pnpm run build:shared`.
- Adding fixtures? Update the manifest, include representative tests, and bump both the Rust crate and npm package versions together.

---

## Testing

Rust tests are colocated with their crates. To run the full test suite:

```bash
pnpm run test:rust        # cargo test --workspace --all-features
pnpm run check:rust       # fmt --check, clippy, build, and test
pnpm run test             # rust tests + wasm wrapper tests + test:shared
pnpm run test:wasm        # wrapper package test suites
```

Per-crate runs are equally useful:

```bash
cargo test -p vizij-graph-core
cargo test -p vizij-animation-core
cargo test -p vizij-orchestrator-core
```

The wrapper packages exercise their generated bindings through package-level test scripts:

```bash
pnpm --filter "@vizij/node-graph-wasm" test
```

Fixtures live in `fixtures/` for repeatable scenario testing. Use them in integration tests or in downstream applications via `npm/@vizij/test-fixtures`.

---

### Performance baselines (Node/WASM)

- Canonical scenarios live in `fixtures/perf_scenarios` (hashes tracked in `fixtures/perf_scenarios/index.json`).
- Build wasm once: `pnpm run build:wasm`
- Full run (appends table rows in `../vizij_docs/current_documentation/perf_baselines.md` when that companion checkout exists): `pnpm run perf:wasm`
- Verify-only (no append, warns on signature/variance drift): `pnpm run perf:wasm:verify`
- CI smoke uses: `SMOKE=1 VERIFY_ONLY=1 pnpm run perf:smoke`
- Update goldens intentionally: `UPDATE_GOLDEN=1 pnpm run perf:wasm`

Scenarios cover: tiny smoke, defaults-only, kitchen mid (25 blocks), kitchen heavy (500 blocks), mixed animation (small/medium), and merged orchestrator blend. Each run records samples, warmup, steps, dt, ABI, build type, commit, and signatures.

---

## Quality Gates

Before landing changes, run the same checks that CI enforces:

- `./.githooks/pre-commit` – `cargo fmt --all` plus clippy.
- `./.githooks/pre-push` – fmt check, clippy, rust tests, and node registry verification.
- `pnpm run build` – workspace build, wasm bundles, shared packages.
- `pnpm run test` – rust tests, wasm wrapper tests, and the current shared-package test target.
- `pnpm run check:rust` – aligns with the full Rust CI pipeline.
- `pnpm run build:wasm:<stack>` – rebuild specific stacks touched by your changes.
- `bash scripts/dry-run-release.sh` – preflight before publishing crates/npm packages.

Document skipped steps in PR descriptions so reviewers have the right context.

---

### Verifying Hooks

To ensure your git hooks are correctly installed, run:

```bash
./scripts/doctor.sh
```

If the check fails, run `./scripts/install-git-hooks.sh` to fix it.

### Publishing & Versioning

Each domain stack keeps the Rust crate, WASM crate, and npm wrapper versions in lockstep. Publishing now flows through [Changesets](.changeset/README.md) plus the automated `publish-npm` workflow.

### Prerequisites

- `NPM_TOKEN` in repo secrets with publish rights for the `@vizij/*` scope.
- Each publishable package has `"private": false` and a `publishConfig.access` entry.

### How a release flows

1. Bump the Rust + WASM crate versions in their `Cargo.toml` files (npm wrappers stay on autopilot).
2. Run `pnpm changeset` and select the npm packages under `npm/@vizij/*` that changed. Commit the generated markdown under `.changeset/`.
3. Once those changes land on the branch you want to ship (e.g., `graph-refactor`), cut a tag named `npm-pub-<something>` (for example `npm-pub-graph-refactor-2025-11-11`) that points at that branch head, and push both the tag and branch to origin. You can also trigger the workflow manually with `workflow_dispatch`.
4. The `publish-npm` workflow finds the remote branch that contains the tagged commit, checks it out, runs `pnpm ci:version` (which deletes the processed changesets, bumps package versions, and commits `chore(release): version packages` onto that same branch), then runs `pnpm ci:publish`. That script temporarily rewrites any `workspace:` dependency ranges to real semver versions, rebuilds the wasm/shared packages, executes `changeset publish`, and restores the workspace protocol before the job pushes anything back. The workflow pushes the release commit and the generated package tags back to the branch, using `NPM_TOKEN` for provenance-enabled publishes.
5. After the workflow finishes, pull your feature branch so you have the auto-generated release commit locally.

Use `scripts/dry-run-release.sh` to sanity-check the end-to-end flow (builds, wasm bundling, npm pack contents) before pushing real releases.

---

## Development Tips

- **ABI mismatches**: If you see `ABI mismatch: expected 2` from a wasm wrapper, rebuild the Rust crate and rerun `pnpm run build:wasm:<stack>` to regenerate JS glue.
- **Time-dependent nodes**: `vizij-orchestrator-core` now advances `GraphRuntime.t`/`dt` on each evaluation. Ensure any custom integrations do the same when you embed `GraphController` elsewhere.
- **URDF features**: Enable the `urdf_ik` feature on `vizij-graph-core` if you need robotics nodes in native builds (`cargo build -p vizij-graph-core --features urdf_ik`). The WASM package ships with the feature enabled by default.
- **Diagnostics**: `vizij-orchestrator-core` captures conflict logs and per-pass timings inside `OrchestratorFrame`. Use these to debug controller order and merge behaviour before instrumenting downstream apps.
- **Fixtures**: The `fixtures/` directory includes ready-to-use JSON files for graphs, animations, and orchestrations. They are mirrored into `npm/@vizij/test-fixtures` for web consumers.

---

## Documentation Maintenance

- Keep `ROADMAP.md` aligned with the state of the codebase—move completed work out as soon as it lands.
- When adding new stacks or support packages, update this README, `AGENTS.md`, and the relevant per-crate `agents.md` files so coding agents stay in sync.
- Treat README updates as part of your definition of done: if behaviour changes, explain it where future contributors expect to find it.

---

## Reference Documentation

- Hosted API docs site (published from `main`): <https://vizij-ai.github.io/vizij-rs/>
- Rust API index: <https://vizij-ai.github.io/vizij-rs/rust/>
- npm API index: <https://vizij-ai.github.io/vizij-rs/npm/>

### Crate & Package Guides

- [vizij-animation-core/README](crates/animation/vizij-animation-core/README.md)
- [vizij-graph-core/README](crates/node-graph/vizij-graph-core/README.md)
- [vizij-orchestrator-core/README](crates/orchestrator/vizij-orchestrator-core/README.md)
- [vizij-api-core/README](crates/api/vizij-api-core/README.md)
- [vizij-test-fixtures/README](crates/test-fixtures/vizij-test-fixtures/README.md)
- [@vizij/node-graph-wasm/README](npm/@vizij/node-graph-wasm/README.md)
- [@vizij/orchestrator-wasm/README](npm/@vizij/orchestrator-wasm/README.md)
- [@vizij/animation-wasm/README](npm/@vizij/animation-wasm/README.md)
- [@vizij/value-json/README](npm/@vizij/value-json/README.md)
- [@vizij/test-fixtures/README](npm/@vizij/test-fixtures/README.md)
- [@vizij/wasm-loader/README](npm/@vizij/wasm-loader/README.md)

If you notice gaps or outdated instructions, please open an issue or ping the Vizij runtime team. High-quality documentation is as critical as the code it describes.

---

## Planning & Roadmap

High-level initiatives, open questions, and cross-stack follow-ups live in [ROADMAP.md](ROADMAP.md). Update it alongside feature work so contributors have an accurate view of what is in flight and what still needs owners.
