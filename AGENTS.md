# Vizij-RS Agent Guide

Welcome, Codex/Gemini/Claude teammates! Use this note to stay in sync with the
repo's current layout, workflows, and collaboration expectations. Revisit the
root `README.md` and crate-specific READMEs whenever behaviour drifts -- this
file summarises, it does not replace, those sources.

## Agent Workflow Checklist
- Scan `README.md` + any touched crate README before editing so you understand
  current stack boundaries and scripts.
- Skim `ROADMAP.md` for in-flight initiatives that might influence the work.
- Decide early whether the task needs the planning tool; when you use it,
  produce multi-step plans and keep them updated as work progresses.
- Prefer fast discovery commands: `rg`/`rg --files` for search and list, `cargo
  metadata --no-deps` for crate info, `just`-style helpers are not configured.
- Queue npm releases with Changesets: run `pnpm changeset` for publishable
  changes and commit the generated entry with your code.
- Always pass `workdir` to shell calls (the harness requires it) and watch for
  unexpected filesystem changes -- ask the user if something looks off.
- Install the local git hooks (`bash scripts/install-git-hooks.sh`) to wire the
  repo's `.githooks/pre-commit` and `.githooks/pre-push` scripts. Run those
  helpers directly (`./.githooks/pre-commit`, `./.githooks/pre-push`) whenever
  you need a one-off fmt/clippy/test sweep.
- Keep responses concise, note follow-up actions, and call out any steps you
  could not run.

## Workspace Snapshot
- **API stack**: the value/type/WriteBatch contract is arora-sdk's
  `arora-shape`; `crates/api` holds its wasm adapter, reused across stacks.
- **Node graph stack**: arora-sdk's `arora-dataflow-nodes` (the node functions,
  the importer onto arora-behavior-tree, `TreeRuntime`), `vizij-graph-wasm`,
  and npm `@vizij/node-graph`.
- **Interop (Arora) stack**: `crates/interop/*` adapts the Vizij stacks onto
  Arora runtime seams — `vizij-arora` (Value interop), `vizij-arora-store`,
  `vizij-arora-hal` and `vizij-arora-behavior`; a device's animations play
  through Arora's `arora-animation-module`, host-linked, and
  `vizij-animation-artifact` writes that module's directory, the guest
  `@vizij/runtime`'s tests load; `crates/vizij` is the view and the device on
  every target, and its `web` module is npm `@vizij/runtime`.
- **Test fixtures**: `vizij-test-fixtures` crate that exposes the shared JSON
  manifest, mirrored to npm `@vizij/test-fixtures` for browsers.
- **Support packages**: npm `@vizij/value-json`, `@vizij/wasm-loader`, and
  `@vizij/test-fixtures` build quickly via `pnpm run build:shared`; rebuild
  them whenever API contracts or fixtures change.
- **Scripts**: Build/link helpers in `scripts/` (see README "Setup" and
  "Usage"), git hooks installer, dry-run release script.
- **npm workspace**: Wrapper packages under `npm/@vizij` re-export wasm `pkg`
  outputs for the `vizij-web` repo.

## Command Reference
### Rust core tasks
| Task | Command |
|------|---------|
| Format everything | `cargo fmt --all` |
| Lint with warnings as errors | `cargo clippy --all-targets --all-features -- -D warnings` |
| Test full workspace | `cargo test --workspace` |
| Test a single crate | `cargo test -p vizij-arora-behavior` (replace crate name) |
| Build support crates | `pnpm run build:shared` |
| Run wasm/npm tests | `pnpm run test` |

### WASM builds and watchers
| Task | Command |
|------|---------|
| Build node-graph WASM pkg | `pnpm run build:wasm:graph` |
| Watch node-graph WASM | `pnpm run watch:wasm:graph` *(needs `cargo-watch`)* |

Install the watcher dependency once with `cargo install cargo-watch`.

### Tooling, release, and cross-repo
| Task | Command |
|------|---------|
| Install git hooks (fmt/clippy/test) | `bash scripts/install-git-hooks.sh` |
| Run hook jobs manually | `./.githooks/pre-commit` / `./.githooks/pre-push` |
| Create a Changeset entry | `pnpm changeset` |
| Apply pending changesets (what the Version Packages PR runs) | `pnpm ci:version` |
| List the npm versions CI would publish | `node scripts/ci-publish.mjs --dry-run` |
| Publish what npm is missing (what `publish-npm` runs) | `pnpm ci:publish` |
| Link npm packages for vizij-web | Build locally, then use temporary `link:` deps in `vizij-web` (see its README) |
| Rebuild after ABI bumps | `cargo build -p <wasm-crate> --target wasm32-unknown-unknown && pnpm run build:wasm:<stack>` |
| WASM perf harness (full) | `pnpm run perf:wasm` |
| WASM perf verify-only | `pnpm run perf:wasm:verify` |
| WASM perf smoke (CI) | `pnpm run perf:smoke` |

`pnpm ci:publish` temporarily resolves `workspace:` dependency ranges to real versions before publishing, then restores the manifests after the npm upload completes.

Prerequisite: add the wasm32 target with `rustup target add wasm32-unknown-unknown` before running the rebuild command above. Install `cargo-watch` (`cargo install cargo-watch --locked`) to use the `pnpm run watch:wasm:*` scripts.

`ROADMAP.md` aggregates documentation TODOs and engineering follow-ups pulled from crate READMEs—consult it when prioritising work or updating docs.

## Stack Briefs
- **API**: `TypedPath`, `ValueJSON` and WriteBatch tooling come from
  arora-sdk's `arora-shape`; read its README before changing serialization or
  shared types, which downstream stacks depend on.
- **Animation**: Arora's animation module (`arora-animation-module`, from
  crates.io) plays a device's animations, host-linked in `crates/vizij`
  (`modules::animation`); `module_animation` converts a face's authored
  animation into its clip format 2.0.
- **Node graph**: a graph runs on arora-sdk's behavior tree: `arora-dataflow-nodes`
  imports it as a tree's detached nodes and runs it (`TreeRuntime`).
  Features like `urdf_ik` are enabled by default and surfaced through the wasm
  build scripts. The wasm crate handles JSON normalisation for JS consumers.
  Until the arora crates are published, the workspace patches them to an
  arora-sdk branch (root `Cargo.toml`).
- **Test fixtures**: `vizij-test-fixtures` maps names from `fixtures/manifest.json`
  to on-disk JSON assets and offers helpers to load them in tests. The npm
  package ships pre-bundled fixture JSON for browser scenarios.

## Coding & Testing Expectations
- Keep solutions simple and aligned with existing patterns; prefer incremental
  changes over broad refactors unless explicitly requested.
- Return `Result`/`anyhow::Result` for fallible paths, use `thiserror` when
  introducing domain errors, and avoid panics in library code.
- Scope visibility with `pub(crate)` wherever possible and add Rustdoc comments
  for public APIs, including short usage snippets if the surface isn't obvious.
- Co-locate unit tests in `#[cfg(test)]` modules; use crate-level `tests/`
  folders for integration coverage (`crates/vizij/tests` is a good example).
  Add wasm-bindgen tests when adjusting wasm surfaces.
- Run fmt/clippy/tests (ideally via the git hooks) before shipping; call out any
  steps you skipped.

## Cross-Repo Workflow Notes
- Build the WASM stacks you need (`pnpm run build:wasm:<stack>`) before testing
  them in `vizij-web`.
- In the web repo, temporarily depend on those builds with
  `pnpm add @vizij/<pkg>@link:../vizij-rs/npm/@vizij/<pkg>`; revert the `link:`
  dependencies before committing so published packages remain the default.
- When introducing breaking changes to JSON schemas or ABI versions, update the
  corresponding npm wrapper README and ensure `vizij-web` has compatible code
  before publishing.

## Maintenance
- Update this file whenever new stacks, scripts, or workflows land. Use the root
  `README.md` and crate READMEs as the source of truth, and mirror their
  structure here so coding agents always have an accurate quickstart guide.
