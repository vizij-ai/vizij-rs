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
  you need a one-off fmt/clippy/test sweep. Never bypass them
  (`SKIP_GIT_HOOKS=1`, `--no-verify`): a hook failing on code you did not
  touch points at a dirty checkout or stale generated files (the node-graph
  registry, the animation module's sources), and that is what to fix. The
  hooks build `vizij` with `ros2-dds` only; `studio` and `tts-piper` are
  checked in CI, so a break there is fixed there.
- Keep responses concise, note follow-up actions, and call out any steps you
  could not run.

## Workspace Snapshot
- **API stack**: Shared value/type/WriteBatch contracts (`crates/api`), plus
  wasm adapters reused across other stacks.
- **Node graph stack**: `vizij-graph-core` (the graph format, the importer onto
  arora-behavior-tree, `TreeRuntime`, over arora-sdk's `arora-dataflow-nodes`,
  the node functions), `vizij-graph-wasm`, and npm `@vizij/node-graph`.
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
| Test a single crate | `cargo test -p vizij-graph-core` (replace crate name) |
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
- **API**: Hub for `TypedPath`, `ValueJSON`, and WriteBatch tooling. Read
  `crates/api/vizij-api-core/README.md` before changing serialization or shared
  types; downstream stacks depend on version alignment here.
- **Animation**: Arora's animation module (`arora-animation-module`, from
  crates.io) plays a device's animations, host-linked in `crates/vizij`
  (`modules::animation`); `module_animation` converts a face's authored
  animation into its clip format 2.0.
- **Node graph**: a graph runs on arora-sdk's behavior tree: `vizij-graph-core`
  imports it as a tree's detached nodes, calls of `arora-dataflow-nodes`'
  functions, and runs it (`TreeRuntime`).
  Features like `urdf_ik` are enabled by default and surfaced through the wasm
  build scripts. The wasm crate handles JSON normalisation for JS consumers.
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

- **Private by default.** An item becomes `pub(crate)` or `pub` when a real
  caller needs it, never to test it: a public item is a contract whose removal
  is a major. A design that must expose a lot is a smell to raise.
- **DRY, without factoring ahead of need.** Extract on real redundancy or a
  function grown very long: every extraction is indirection paid on each read.
  Name an extracted free function for what it does; code reused within one
  function becomes a closure.
- **Generic means indifferent.** A type parameter depends on the minimal
  interface the code needs; a trait restating one concrete type's whole
  vocabulary is not generic.
- **Fix a cross-platform problem at the layer that makes platforms uniform.**
  A platform-specific mitigation (an OS-only opt-out, a browser-only timer) is
  a labelled stopgap, agreed before it lands, because per-platform patches
  multiply and entrench the wrong design.
- **Measure performance in instructions, not wall time** (callgrind's `Ir`,
  `time -l`'s "instructions retired", wasmtime fuel for a guest): builds and
  agents share the machines, so timings there are noise.
- **Trace a design before changing it.** Read its docs and history, and ask
  its author, before proposing to remove or bypass part of it: what looks
  redundant may be a deliberate control.

## Design Rules

These constrain every change; each carries its reason so it applies to cases
it does not name.

- **Vizij is built on Arora; expose Arora, don't wrap it.** Vizij uses Arora's
  types and traits directly, never a Vizij-side mirror trait: a mirror is a
  second contract to keep in sync. A function Arora already provides needs a
  written reason to exist in a Vizij client (`@vizij/runtime`); when the host
  lacks an operation remote clients have, add it to Arora. Vizij types stay
  concrete Rust types declared as Arora types, never a merged `Value` enum.
- **Vizij-specific code lives here**, never in arora-sdk: nodes, the speech
  providers, anything naming a face.
- **The node graph is Vizij's behavior interpreter**
  ([`node-graph.md`](crates/interop/vizij-arora-behavior/docs/node-graph.md)).
  A face's program is a behavior run: `spawn` starts it, `halt` stops it, its
  state is the run's status key. Pause is a halt (outputs hold) and resume a
  new spawn. A reset to rest is an explicit step of the behavior or of the
  client, never an interpreter-wide assumption.
- **Features go through a module's declared functions.** What a device holds
  (animations, programs) is loaded and unloaded through the calls any client
  sends, at any time, and its state is observable through the module: a real
  device gains resources from clients that never saw its build. Declare a
  missing inverse (`unload`) instead of adding a host-only method.
- **A module never writes keys.** A call has in and out parameters; a step's
  outputs are named by their tracks' targets, and the behavior or client
  chooses where to write them. No intermediate store copy exists without a
  reader.
- **An Arora key is a `/`-separated path**; a trailing `.attribute` is a
  naming convention only.
- **A face is a component of a device**, not the device: "device" names only
  an Arora instance standing alone (the `vizij` binary). A robot adds a face
  as one more HAL.
- **Apps drive a face through its Arora API** — its methods and the data
  store — on the vizij standard (`standard/vizij/…`) by default. ROS4HRI is a
  profile for ROS robots: docs, comments and changesets describe the vizij
  standard plainly and name ROS4HRI only where it is the subject.
- **Visemes are not ROS4HRI's**: extend the driver, never add viseme keys to
  the ROS4HRI profile.
- **Never change a standard ROS interface type**, even with an added field;
  add a distinct interface and document which one a client chooses. A type is
  matched by name and, from Jazzy, by a hash of its fields, so an extended
  type fails to match upstream peers or mis-decodes.
- **Prove ROS delivery with a native peer** (rclpy or the `ros2` CLI, as
  `crates/vizij/tests/ros4hri/` does): two `ros2-client` peers match each
  other's type hash whatever it is. Check `ros2 topic info` shows a publisher
  before trusting a silent `ros2 topic echo`.
- **A speech provider is a sibling module on the one `say` contract**, under
  its own module id; the consumer picks it with a feature flag (`tts-piper`),
  never a switch inside a module.
- **A library never reads `argv`.** The binary parses it; Arora is not patched
  for one app's command line. A feature is named for the concept it gates, not
  for its consumer. Stopping a run is dropping its future.

## Writing Conventions

Every text a person reads — docs, comments, changesets, pull requests:

- **State what is true now, and why.** History belongs in git: no "was", "no
  longer", "we first tried", no attribution, no incident counts. A lesson
  becomes a rule with its reason: "don't do Y, because X". Rejected designs go
  in a proposal or design note, with why they were set aside.
- **Write a comment for the code's client**: what the item does, its effects,
  and the "why" that stops a reader from deleting something that looks dead.
  An item defines itself by what it does, never by its callers. In
  `build.rs`, `Cargo.toml` and CI YAML, a line or two.
- **Vocabulary.** A keyframed asset is an "animation", never a "clip" (the
  module's published `AnimationClip` type keeps its name). A module's name has
  no `-module` suffix (the module `@vizij/animation`); the package shipping it
  does (`@vizij/animation-module`). Say "wire format" or "messages", never
  "Arora protocol". Before naming a public item, search the existing
  vocabulary across Vizij, Arora and Studio and reuse it.

## Versions and Releases

A version reaching `main` is a release: `publish-crates.yml` and
`publish-npm.yml` publish it (README › Publishing & Versioning). Get versions
right before merging.

- **A breaking change bumps the real major**, below 1.0 too: cargo's 0.x-minor
  convention is not used. Dependents re-pin in the same change, and a
  dependent whose API carries the changed types bumps its own major. Public
  fields added to a struct nobody builds by literal ship as a minor.
- **A requirement names the minor the code calls into** (`"1.2"`, not `"1"`),
  raised in the change that first uses it: a lockfile holding 1.0.0 satisfies
  `"1"`. Path dependencies still carry `version =`.
- **Every published crate keeps a Keep a Changelog `CHANGELOG.md`**; npm
  packages get theirs from changesets. A stacked pull request that bumps no
  version publishes nothing.
- **Of two pull requests raising one crate to the same version**, the second
  to merge takes the next one.
- **A new crate or npm package's first version is published by hand**:
  trusted publishing cannot create one.
- **New crates, packages and modules are `Apache-2.0`.** A crate linking GPL
  code stays GPL (`vizij-piper`); forks keep their upstream licence. The
  `studio` feature, which pulls Semio's Studio client, stays opt-in so an
  Apache build never includes it unasked; a shipped app may turn it on.
- **In a workflow, `actions/setup-node` takes `package-manager-cache: false`**
  unless pnpm is installed before it, since `package.json` declares
  `packageManager`.

## Pull Requests and Issues

- **Issues live in Linear, not GitHub issues.** Link a pull request or ticket
  with a Markdown link: Linear rewrites a bare `#N` into a link to another
  repository's pull request.
- **A title states the change.** The body opens with an **Intent** paragraph —
  what the change is for and what it deliberately does not do — then the
  current behaviour, the change, and why it matters. Review suggestions are
  measured against it: corrections are applied, additions beyond it declined
  on the thread, with a ticket for a real problem.
- **A finished pull request is ready, not draft**: draft reads as "not ready
  for review".
- **Pull requests are rebase-merged**, so each commit stands on its own.
- **A stacked pull request is retargeted when the one below merges**: merged
  branches are not deleted, so GitHub does not retarget it, and merging it
  lands it on the dead branch.
- **Read `git diff --cached --stat` before committing** and stage by path or
  with `git add -u`: `git add -A` in a shared checkout commits content lost
  under you as a revert.

## Build Traps

- `[patch.crates-io]` is silently ignored when the patched version does not
  satisfy the requirement. Patch a crate's whole closure, never one crate: a
  lone patched `arora-bridge-ros2` pulls path copies of its dependencies
  beside the registry ones ("multiple versions of crate `arora_bridge`").
  Requiring a version not yet on crates.io blocks every build in the
  workspace, since cargo resolves the whole lock.
- `bindeps` is workspace-wide: one member using it breaks the stable parse of
  the workspace. Artifact paths reach build scripts only as
  `CARGO_<KIND>_FILE_<DEP>_<crate>`.
- Cargo applies no LTO to a lib whose `crate-type` includes `rlib`. When a
  smaller wasm matters, build a `cdylib`-only crate; `wasm-opt --strip-debug`
  is the larger saving either way.
- On `wasm32-unknown-unknown`, tokio compiles only with `sync`, `macros`,
  `io-util`, `rt` and `time`; never `features = ["full"]`. A
  `[target.'cfg(…)'.build-dependencies]` gate is evaluated against the host,
  so it cannot keep a build dependency out of a wasm build.
- glam ≥ 0.32 changed its Euler conventions: compose `Rz·Ry·Rx` explicitly
  instead of `Quat::from_euler`.

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
