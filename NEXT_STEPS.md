# Next Steps for vizij npm Consumers

This branch finalizes the API Review refactor and tightens the shared Value typings that ship
with the Vizij wasm wrappers. The changes are designed to be drop-in, but the items below are
worth double-checking before updating the `vizij-web` repo or any other consumer.

## What Changed
- **Shared Value helpers** – `@vizij/value-json` now owns the canonical `ValueJSON` union and
  the `toValueJSON` helper. The union no longer widens to `unknown`, so TypeScript now enforces
  that callers pass primitives, the normalized `{ type, data }` form, or one of the legacy
  shorthands. Use `toValueJSON(...)` when in doubt.
- **Common wasm loader** – All wasm wrappers (`@vizij/animation-wasm`, `@vizij/node-graph-wasm`,
  `@vizij/orchestrator-wasm`) delegate to the shared `@vizij/wasm-loader`, keeping ABI checks and
  `file://` handling consistent between Node and browser builds.
- **Rust/TS alignment** – Value/Shape normalization now lives in `vizij_api_core::json`, and the
  wasm crates call straight into it. Type discriminants are consistently lowercase in both Rust
  and TypeScript; generated typings reflect that surface.
- **Shared fixtures** – Orchestrator demos/tests pull from the new
  `crates/orchestrator/.../fixtures` JSON so the Rust and JS suites stay in sync.
- **pnpm workspace** – The npm packages are managed by the new pnpm workspace, so `pnpm install`
  in `vizij-rs/` installs every wrapper plus the shared helpers in one pass.

## Consumer Checklist
1. **Install updated packages**
   - Pull the latest `vizij-rs` changes, then from the repo root run `corepack enable && pnpm install`.
   - Rebuild wasm outputs (`pnpm run build:wasm`) before relinking into `vizij-web`.

2. **Adjust TypeScript where necessary**
   - Fix any compile errors that appear because `ValueJSON` no longer accepts `unknown`. In most
     cases replacing ad-hoc literals with `toValueJSON(value)` is enough.
   - If you previously relied on capitalized discriminants (`"Float"`), update literals/tests to
     the lowercase form (`"float"`).

3. **Verify runtime flows**
   - In `vizij-web`, rerun the usual integration smoke tests (animation ramp, chained graphs, and
     orchestrator demos). The wrappers now share the loader, so initialization should be identical
     but this confirms ABI checks are happy.

4. **Update docs/notes downstream**
   - Mention that `@vizij/value-json` is the new source of truth for Value helpers and that all
     packages expose the same surface via this dependency.

## Optional Follow-Ups
- Consider swapping any remaining custom value builders or loader shims in `vizij-web` for the
  shared helpers to keep drift risk low.
- Add a CI gate in `vizij-web` that compiles with the updated typings so Value schema changes fail
  fast.
