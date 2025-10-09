# Vizij-RS Agent Guide

Welcome, Codex/Gemini/Claude teammates! Use this note to stay in sync with the
repo's current layout, workflows, and collaboration expectations. Revisit the
root `README.md` and crate-specific READMEs whenever behaviour drifts -- this
file summarises, it does not replace, those sources.

## Agent Workflow Checklist
- Scan `README.md` + any touched crate README before editing so you understand
  current stack boundaries and scripts.
- Decide early whether the task needs the planning tool; when you use it,
  produce multi-step plans and keep them updated as work progresses.
- Prefer fast discovery commands: `rg`/`rg --files` for search and list, `cargo
  metadata --no-deps` for crate info, `just`-style helpers are not configured.
- Always pass `workdir` to shell calls (the harness requires it) and watch for
  unexpected filesystem changes -- ask the user if something looks off.
- Install the local git hooks (`bash scripts/install-git-hooks.sh`) to wire the
  repo's `.githooks/pre-commit` and `.githooks/pre-push` scripts. Run those
  helpers directly (`./.githooks/pre-commit`, `./.githooks/pre-push`) whenever
  you need a one-off fmt/clippy/test sweep.
- Keep responses concise, note follow-up actions, and call out any steps you
  could not run.

## Workspace Snapshot
- **API stack**: Shared value/type/WriteBatch contracts (`crates/api`), plus
  Bevy + wasm adapters reused across other stacks.
- **Animation stack**: `vizij-animation-core`, `bevy_vizij_animation`,
  `vizij-animation-wasm`, and npm `@vizij/animation-wasm`.
- **Node graph stack**: `vizij-graph-core`, `bevy_vizij_graph`,
  `vizij-graph-wasm`, and npm `@vizij/node-graph-wasm`.
- **Orchestrator stack**: `vizij-orchestrator-core` runtime coordinating
  graphs/animations, `vizij-orchestrator-wasm`, and npm `@vizij/orchestrator-wasm`.
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
| Test a single crate | `cargo test -p vizij-orchestrator-core` (replace crate name) |
| Build orchestrator examples | `cargo build --manifest-path crates/orchestrator/vizij-orchestrator-core/Cargo.toml --examples` |

### WASM builds and watchers
| Task | Command |
|------|---------|
| Build animation WASM pkg | `npm run build:wasm:animation` |
| Build node-graph WASM pkg | `npm run build:wasm:graph` |
| Build orchestrator WASM pkg | `npm run build:wasm:orchestrator` |
| Watch animation WASM | `npm run watch:wasm:animation` *(needs `cargo-watch`)* |
| Watch node-graph WASM | `npm run watch:wasm:graph` *(needs `cargo-watch`)* |
| Watch orchestrator WASM | `npm run watch:wasm:orchestrator` *(needs `cargo-watch`)* |

Install the watcher dependency once with `cargo install cargo-watch`.

### Tooling, release, and cross-repo
| Task | Command |
|------|---------|
| Install git hooks (fmt/clippy/test) | `bash scripts/install-git-hooks.sh` |
| Run hook jobs manually | `./.githooks/pre-commit` / `./.githooks/pre-push` |
| Dry-run crates + npm release | `bash scripts/dry-run-release.sh` |
| Link npm packages for vizij-web | `npm run link:wasm` (then run the companion script in `vizij-web`) |
| Rebuild after ABI bumps | `cargo build -p <wasm-crate> --target wasm32-unknown-unknown && npm run build:wasm:<stack>` |

Prerequisite: add the wasm32 target with `rustup target add wasm32-unknown-unknown` before running the rebuild command above.

## Stack Briefs
- **API**: Hub for `TypedPath`, `ValueJSON`, and WriteBatch tooling. Read
  `crates/api/vizij-api-core/README.md` before changing serialization or shared
  types; downstream stacks depend on version alignment here.
- **Animation**: `crates/animation/vizij-animation-core/src/lib.rs` exports
  `Engine`/`StoredAnimation` APIs. WASM (`vizij-animation-wasm`) normalises JSON
  inputs/outputs and enforces ABI version checks mirrored in the npm wrapper.
- **Node graph**: `vizij-graph-core` evaluates deterministic data-flow graphs.
  Features like `urdf_ik` are enabled by default and surfaced through the wasm
  build scripts. The Bevy adapter and wasm crate share JSON normalisation logic.
- **Orchestrator**: `vizij-orchestrator-core` (`src/lib.rs`, `controllers/*`)
  coordinates animation engines and graph controllers. Check the crate README
  for scheduler semantics, blackboard conventions, and example entry points.
  The wasm crate mirrors the Rust API and serialises frames for JS consumers.

## Coding & Testing Expectations
- Keep solutions simple and aligned with existing patterns; prefer incremental
  changes over broad refactors unless explicitly requested.
- Return `Result`/`anyhow::Result` for fallible paths, use `thiserror` when
  introducing domain errors, and avoid panics in library code.
- Scope visibility with `pub(crate)` wherever possible and add Rustdoc comments
  for public APIs, including short usage snippets if the surface isn't obvious.
- Co-locate unit tests in `#[cfg(test)]` modules; use crate-level `tests/`
  folders for integration coverage (`vizij-orchestrator-core/tests` is a good
  example). Add wasm-bindgen tests when adjusting wasm surfaces.
- Run fmt/clippy/tests (ideally via the git hooks) before shipping; call out any
  steps you skipped.

## Cross-Repo Workflow Notes
- During local development, link the wasm npm packages into `vizij-web` using
  `npm run link:wasm` here followed by the matching script in the web repo. The
  Vite dev server reloads automatically when the linked `pkg/` outputs change.
- When introducing breaking changes to JSON schemas or ABI versions, update the
  corresponding npm wrapper README and ensure `vizij-web` has compatible code
  before publishing.

## Maintenance
- Update this file whenever new stacks, scripts, or workflows land. Use the root
  `README.md` and crate READMEs as the source of truth, and mirror their
  structure here so coding agents always have an accurate quickstart guide.
