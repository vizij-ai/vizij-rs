# Vizij RS Workspace

Vizij RS is the Rust workspace that powers Vizij's real-time animation and node graph engines. It houses the native core logic,
Bevy engine integrations, and WebAssembly bindings that are republished as npm packages for the front-end repository `vizij-web`.
This README explains how everything fits together and how to set up a local development environment that spans Rust, WASM, and
JavaScript tooling.

## Overview

* **Domain stacks** – The workspace currently ships animation and node-graph stacks, with additional stacks (e.g., blackboard,
  behaviour tree) planned. Each stack follows the same pattern: a core Rust crate that feeds both a Bevy ECS adapter and a
  WASM binding (which is re-exported through an npm wrapper for JavaScript consumers).
* **Multi-language toolchain** – Rust crates are built with Cargo, WASM targets use `wasm-bindgen`/`wasm-pack`, and npm packages
  provide typed front-end entry points. The workspace is designed so the Rust repo can publish versioned crates and npm artifacts
  without the web repo pulling in the Rust toolchain.
* **Shared JSON contracts** – Animations use the `StoredAnimation` schema. Node graphs use `GraphSpec`/`ValueJSON`/`ShapeJSON`.
  These formats travel between native hosts, Bevy, and the browser.

## Architecture

```
vizij-rs/
├── crates/
│   ├── api/
│   │   ├── vizij-api-core           # Shared Value/Shape/TypedPath definitions
│   │   ├── bevy_vizij_api           # Bevy helpers for applying WriteOps
│   │   └── vizij-api-wasm           # wasm-bindgen helpers for Value/WriteBatch JSON
│   ├── animation/
│   │   ├── vizij-animation-core      # Engine-agnostic animation runtime (Rust)
│   │   ├── bevy_vizij_animation      # Bevy plugin built on the animation core
│   │   └── vizij-animation-wasm      # wasm-bindgen bindings consuming the core
│   └── node-graph/
│       ├── vizij-graph-core          # Deterministic data-flow graph evaluator (Rust)
│       ├── bevy_vizij_graph          # Bevy plugin consuming the graph core
│       └── vizij-graph-wasm          # wasm-bindgen adapter consuming the graph core
├── npm/
│   ├── @vizij/animation-wasm         # npm package that re-exports the animation WASM pkg
│   └── @vizij/node-graph-wasm        # npm package wrapping the node graph WASM pkg
└── scripts/                          # Helper scripts for building/linking WASM outputs
```

* **Rust core crates** encapsulate the domain logic and are published on crates.io.
* **Bevy adapters** bridge the cores into a Bevy application (resources, systems, fixed timesteps).
* **WASM crates** compile the cores to `cdylib` + JS glue via `wasm-bindgen`.
* **API crates** provide shared Value/Shape contracts plus lightweight Bevy and WASM helpers that are reused across domain stacks.

Animation stack relationships:

```
vizij-animation-core
├─ bevy_vizij_animation
└─ vizij-animation-wasm → npm/@vizij/animation-wasm
```

Node-graph stack relationships:

```
vizij-graph-core
├─ bevy_vizij_graph
└─ vizij-graph-wasm → npm/@vizij/node-graph-wasm
```

Each `*-core` crate feeds both the Bevy plugin and the WASM binding directly—the Bevy and WASM layers do not depend on each
other.
* **npm workspaces** vend the generated `pkg/` output with a stable ESM entry for front-end consumers such as `vizij-web`.

The companion repo `vizij-web` consumes the npm packages. During local development the two repos are linked with `npm link` so
changes to Rust propagate immediately to the Vite dev servers.

## Installation

1. **Install prerequisites**
   * Rust stable via [`rustup`](https://rustup.rs/) (install the default toolchain and `wasm32-unknown-unknown` target).
   * Node.js ≥ 18 and npm ≥ 9.
   * `wasm-pack` and `wasm-bindgen-cli` for building the WASM crates:
     ```bash
     cargo install wasm-pack wasm-bindgen-cli
     ```
   * Optional developer tools: `cargo install cargo-watch` for autorebuild loops.
2. **Clone the repository**
   ```bash
   git clone https://github.com/vizij-ai/vizij-rs.git
   cd vizij-rs
   ```
3. **Install npm workspace dependencies** (used by the wrapper packages):
   ```bash
   npm install
   ```
4. **(Optional) Clone vizij-web** if you plan to link the npm packages locally.

## Setup

Follow these steps the first time you prepare a development environment:

1. **Bootstrap git hooks** (formats, clippy, tests):
   ```bash
   bash scripts/install-git-hooks.sh
   ```
2. **Build both WASM crates** so the npm wrappers have fresh `pkg/` output. The root `package.json` exposes shortcuts that wrap
   the Node build scripts:
   ```bash
   npm run build:wasm:animation
   npm run build:wasm:graph
   ```
3. **Link the npm packages into vizij-web** (from this repo). The helper will rebuild both wrappers and register the global
   `npm link` targets in one go:
   ```bash
   npm run link:wasm
   ```
   Then, in the `vizij-web` repository:
   ```bash
   npm install
   npm run link:wasm
   ```
   The web repo’s script simply runs `npm link @vizij/animation-wasm @vizij/node-graph-wasm`, wiring its `node_modules/`
   entries back to these locally built packages.
4. **Vite dev servers in vizij-web already preserve symlinks and un-ignore the linked wasm packages.** No additional config is
   required; just ensure you restart any running dev server after relinking so it picks up new symlinks.

## Usage

Common workflows from the root of `vizij-rs`:

| Task | Command |
|------|---------|
| Format code | `cargo fmt --all` |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` |
| Test everything | `cargo test --workspace` |
| Build animation WASM pkg | `npm run build:wasm:animation` |
| Build node-graph WASM pkg | `npm run build:wasm:graph` |
| Watch animation WASM builds | `npm run watch:wasm:animation` *(requires `cargo-watch`)* |
| Watch node-graph WASM builds | `npm run watch:wasm:graph` *(requires `cargo-watch`)* |
| Publish dry run | `scripts/dry-run-release.sh` (builds crates and npm packages without publishing) |

When linked to `vizij-web`, run its dev servers (e.g., `npm run dev:animation`) and iterate on Rust code with the watcher. The
Vite server reloads when the WASM `pkg/` contents change.

## Key Details

* **StoredAnimation JSON** – The canonical animation format. Tracks contain normalized keypoint timestamps (`stamp` 0..1) and
  cubic-bezier control points (`transitions.in/out`) with defaults. Values support scalars, vectors, quaternions, colors, and
  step-only booleans/strings.
* **GraphSpec JSON** – Describes node graphs with typed ports, selectors on edges, staged inputs (`Input` nodes), and explicit
  `Output` sinks that emit `WriteOp { path, value, shape }` structures.
* **Versioning** – Keep crate versions in sync with their WASM/npm counterparts (e.g., `vizij-animation-core` ↔
  `vizij-animation-wasm` ↔ `@vizij/animation-wasm`). Publish the core crate first, then release each dependent crate (Bevy plugin,
  WASM binding) before shipping the npm wrapper.
* **Optional features** – Node-graph crates expose an `urdf_ik` feature (enabled by default) for robotics integrations; WASM
  crates also expose a `console_error` feature to hook panics into the browser console.
* **Repo scripts** –
  * `npm run build:wasm:animation` / `npm run build:wasm:graph` wrap the underlying Node scripts to refresh WASM outputs.
  * `scripts/build-animation-wasm.mjs` / `scripts/build-graph-wasm.mjs` contain the raw build logic used by the npm shortcuts.
    - The graph script now forwards `--features urdf_ik` to ensure the packaged wasm exposes the URDF IK/FK node family.
  * `npm run watch:wasm:animation` / `npm run watch:wasm:graph` trigger `cargo watch` loops that rebuild WASM artifacts on change
    (requires `cargo-watch`).
  * `scripts/install-git-hooks.sh` installs pre-commit/pre-push hooks mirroring the CI checks.
  * `scripts/dry-run-release.sh` runs the publish checks without releasing artifacts.

## Animation Derivatives & Baking

* **Runtime updates** – `Engine::update_values(dt, Inputs)` returns the classic `Outputs` list. Call
  `Engine::update_values_and_derivatives` (or the WASM wrapper `engine.updateValuesAndDerivatives`) to receive
  `OutputsWithDerivatives`, where each change includes an optional `derivative` field. Non-numeric tracks emit `None`/`null`.
* **Baking outputs** – `Engine::bake_animation` returns `BakedAnimationData`. Use
  `Engine::bake_animation_with_derivatives` to receive the paired derivative tracks. In WASM/TypeScript the bundle is normalised
  to `{ values, derivatives }`, each mirroring the `tracks`/`frame_rate` schema.
* **Configuration** – `BakingConfig` now supports `derivative_epsilon` to override the finite-difference window when estimating
  derivatives. Negative or zero frame rates/config values are rejected during WASM parsing.
* **Derivative model** – Derivatives use a symmetric finite difference sampled around the current parameter (default epsilon
  `1e-3`). Quaternion derivatives are currently component-wise (a good approximation for small deltas) with a TODO to upgrade to
  angular velocity/log mapping. Bool/Text tracks intentionally produce `None`/`null` derivatives.
* **ABI guard** – The animation WASM exports `abi_version() === 2`. The npm wrapper verifies this during `init()` and throws with
  guidance when the JS glue or `.wasm` file is stale. Rebuild with `cargo build -p vizij-animation-wasm --target wasm32-unknown-unknown && npm run build:wasm:animation` whenever the ABI changes.

## Examples

### Sample: Using the animation engine in Rust

```rust
use vizij_animation_core::{Engine, InstanceCfg, parse_stored_animation_json};

let json = std::fs::read_to_string("tests/fixtures/new_format.json")?;
let stored = parse_stored_animation_json(&json)?;
let mut engine = Engine::new(Default::default());
let anim = engine.load_animation(stored);
let player = engine.create_player("demo");
engine.add_instance(player, anim, InstanceCfg::default());

let outputs = engine.update_values(1.0 / 60.0, Default::default());
println!("changes: {:?}", outputs.changes);
```

### Sample: Using the animation WASM package in TypeScript

```ts
import { init, Engine } from "@vizij/animation-wasm";

await init();
const eng = new Engine();
const anim = eng.loadAnimation({
  duration: 1000,
  tracks: [
    {
      id: "pos-x",
      animatableId: "cube/Transform.translation",
      points: [
        { id: "k0", stamp: 0.0, value: 0 },
        { id: "k1", stamp: 1.0, value: 1 },
      ],
    },
  ],
  groups: {},
}, { format: "stored" });
const player = eng.createPlayer("demo");
eng.addInstance(player, anim);
console.log(eng.updateValues(1 / 60).changes);

// Derivative-friendly update
const outputs = eng.updateValuesAndDerivatives(1 / 60);
for (const change of outputs.changes) {
  console.log(change.key, change.value, change.derivative ?? null);
}

// Baking bundle
const baked = eng.bakeAnimationWithDerivatives(anim, {
  frame_rate: 60,
  derivative_epsilon: 5e-4,
});
console.log(baked.values.tracks[0].values.length, baked.derivatives.tracks[0].values.length);
```

### Sample: Evaluating a node graph core-side

```rust
use vizij_graph_core::{evaluate_all, GraphRuntime};
use vizij_graph_core::spec::GraphSpec;

let spec: GraphSpec = serde_json::from_str(include_str!("tests/fixtures/simple_graph.json"))?;
let mut runtime = GraphRuntime::default();
let result = evaluate_all(&mut runtime, &spec)?;
for (node_id, ports) in &result.nodes {
    println!("node {node_id:?} ports: {ports:?}");
}
```

Refer to the crate and package READMEs for deeper dives into each component, including architecture diagrams, API documentation,
and troubleshooting tips.
