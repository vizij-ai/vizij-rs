# Vizij‑RS

Welcome! This file guides AI coding agents—whether you’re using Gemini CLI, Claude Code or OpenAI Codex—on how to work effectively in the `vizij-rs` repository. It provides project context, workflows, coding conventions, build/test commands, and guidelines for collaboration. Keep it updated as the project evolves.

## Project overview and structure

`vizij-rs` is a multi‑crate Rust workspace powering Vizij’s animation and node‑graph systems. The workspace root `Cargo.toml` defines these members:

* **crates/animation/vizij-animation-core** – Engine‑agnostic core animation logic.
* **crates/animation/bevy\_vizij\_animation** – Bevy plugin wrapping the animation core for the Bevy engine.
* **crates/animation/vizij-animation-wasm** – WebAssembly bindings for the animation core using `wasm‑bindgen`. Defines `crate-type = ["cdylib", "rlib"]`.
* **crates/node-graph/vizij-graph-core** – Core node‑graph engine used by Vizij.
* **crates/node-graph/bevy\_vizij\_graph** – Bevy plugin integrating the node graph into Bevy.
* **crates/node-graph/vizij-graph-wasm** – wasm bindings for the graph core, with optional `console_error` feature hooking panic messages into the browser console.

All crates use Rust 2021 edition and share common dependencies via `[workspace.dependencies]` (serde, serde\_json, thiserror, hashbrown, wasm‑bindgen, js‑sys, anyhow, bevy).  Some crates depend on others via `path` and `version` fields.

## Key commands and workflows

Use these commands to interact with the workspace. Always run commands from the repository root unless noted otherwise.

### Building and testing

* **Build everything (release):**

  ```bash
  cargo build --workspace --release
  ```
* **Run all tests:**

  ```bash
  cargo test --workspace
  ```
* **Format and lint:**

  ```bash
  cargo fmt --all
  cargo clippy --all-targets --all-features -- -D warnings
  ```
* **Build a wasm crate:** (replace `<crate>` with a specific wasm crate name)

  ```bash
  cargo build -p <crate> --target wasm32-unknown-unknown --release
  wasm-bindgen --target bundler --out-dir crates/<path>/pkg target/wasm32-unknown-unknown/release/<crate>.wasm
  ```
* **Publish a crate:** Always perform a dry‑run first and publish crates in dependency order (core → plugin → wasm):

  ```bash
  cargo publish -p vizij-animation-core --dry-run
  cargo publish -p vizij-animation-core
  # repeat for other crates, updating versions as needed
  ```

### Commit and pull request guidelines

1. **Use semantic versioning** – Major for breaking API changes, minor for new features, patch for bug fixes. Update crate versions in `Cargo.toml` and adjust `path` and `version` dependencies accordingly.
2. **Write conventional commit messages** – Prefix commit subjects with `feat:`, `fix:`, `refactor:`, `chore:` etc., and mention affected crate(s) if multiple.
3. **Create small, focused pull requests** – Each PR should compile, run tests and address a single concern. Include clear descriptions, link related issues and provide testing instructions.
4. **Run programmatic checks** – Always run `cargo fmt`, `cargo clippy` and `cargo test` (and wasm builds when relevant) before submitting a PR. All commands must succeed.
5. **Peer review** – Request code review from another developer. Explain the rationale behind changes and document any new public API.

## Coding conventions and implementation guidelines

* **Simplicity first** – Start with the simplest working solution; avoid over‑engineering.
* **Ask for clarification** – When requirements are unclear, explicitly request clarification rather than guessing. Use your agent’s question mechanisms.
* **Rust style** – Follow `rustfmt` formatting. Use 4‑space indentation and `snake_case` for identifiers. Keep functions small and focused.
* **Error handling** – Prefer returning `Result` types. Use the `anyhow` crate for broad error propagation and `thiserror` for defining custom error types. Do not panic unless absolutely necessary.
* **Public API** – Minimise the public surface. Use `pub(crate)` for items internal to the workspace. Provide Rustdoc comments and usage examples for all public functions, types and macros.
* **Feature flags** – Group optional functionality under crate features. Keep default features lean and enable additional ones (e.g. `console_error`) explicitly.

## Testing and quality assurance

* **Unit tests** – Place unit tests in `#[cfg(test)]` modules adjacent to the code. For cross‑crate integration tests, place them under a `tests/` folder at the crate root.
* **Wasm tests** – Use `wasm-bindgen-test` for wasm crates. Run with `wasm-pack test --headless --firefox` or `--chrome`.
* **Property‑based tests** – For complex data structures, consider using `proptest` to validate behaviour across a wide range of inputs.
* **CI and coverage** – CI should run `cargo test`, `cargo clippy` and build wasm crates. Consider adding code coverage tools like `tarpaulin` or `cargo-llvm-cov` if coverage is desired.

## Developer environment and setup

* Install Rust via rustup: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh` and select the stable toolchain.
* Install `wasm-pack` and `wasm-bindgen-cli` using `cargo install wasm-pack wasm-bindgen-cli`.
* Use a recent Bevy version (0.14.x) for crates that integrate with Bevy.
* Configure your editor (e.g. VS Code with rust-analyzer) for syntax highlighting and inline diagnostics.

## Repository etiquette and additional notes

* **Keep the workspace building** – Do not commit broken code. If you must break the build (e.g. during a refactor), do so on a feature branch and fix tests before merging into `main`.
* **Add dependencies thoughtfully** – Evaluate whether new dependencies are necessary; avoid unnecessary bloat. Document reasons for any new dependency in the PR description.
* **Update downstream crates** – Changes in core crates may require updates in Bevy and wasm crates. Propagate API changes accordingly.
* **Major refactors** – For large architectural changes, create a design doc in `docs/` explaining the rationale, proposed changes and migration steps. Review this doc before implementing.
* **Version synchronization** – When releasing a new version of `vizij-animation-core` or `vizij-graph-core`, also release matching versions of `vizij-animation-wasm` and `vizij-graph-wasm` and update dependent crates. Align versions with related npm packages in `vizij-web`.

## Adding a new wasm npm package (e.g. blackboard)

When introducing a new wasm crate that also needs an npm package (as done for `@vizij/blackboard-wasm`):

1. Create the Rust crate under `crates/<domain>/vizij-<name>-wasm` with `crate-type = ["cdylib", "rlib"]` and build it using `wasm-pack build --target web` (follow existing wasm crates for dependencies and features).
2. Add / update a build script in `scripts/` (e.g. `build-blackboard-wasm.mjs`) invoking `wasm-pack build` pointing `--out-dir` to `npm/@vizij/<name>-wasm/pkg`.
3. Scaffold an npm workspace package under `npm/@vizij/<name>-wasm`:
   - `package.json` mirroring animation/node-graph packages (name, version, build script, `files` array containing `dist/` and `pkg/`).
   - `tsconfig.json` (see existing packages; typically ES2020 + `moduleResolution: Bundler`).
   - `src/index.ts` ESM entry that imports the generated `pkg/<wasm_pkg>.js` with a `.js` extension and provides an `init()` helper if needed.
   - `README.md` with build & usage instructions.
4. Ensure root `package.json` `workspaces` already matches `npm/@vizij/*` (no change usually needed) and optionally add convenience scripts (`build:wasm:<name>`, `link:wasm:<name>`).
5. Run from repo root: `npm install` (to register new workspace) then `npm run build:wasm:<name>` and `npm --workspace npm/@vizij/<name>-wasm run build`.
6. Optionally `npm run link:wasm:<name>` for local development linking.
7. Before publishing, confirm `pkg/` exists (wasm-pack output) and `dist/` exists (TypeScript build). The `prepublishOnly` script guards against missing `pkg/`.

This process was applied for `@vizij/blackboard-wasm` (version 0.1.0).


