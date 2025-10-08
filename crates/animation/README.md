# Vizij Animation Stack

> **Core engine, Bevy integration, and WebAssembly bridge for Vizij’s animation system.**

The `crates/animation` directory contains the Rust sources that power Vizij’s runtime animation features and the bindings that surface them to other environments. This README explains how the crates relate to one another, when to use each, and how to build, test, and publish them.

---

## Table of Contents

1. [Crate Map](#crate-map)
2. [Typical Workflows](#typical-workflows)
3. [Building & Testing](#building--testing)
4. [Publishing](#publishing)
5. [Reference Links](#reference-links)

---

## Crate Map

| Crate | Description | Consumers |
|-------|-------------|-----------|
| [`vizij-animation-core`](vizij-animation-core/README.md) | Deterministic animation engine (parsing, playback, blending, baking). | Native hosts, orchestrator runtime, wasm binding, Bevy plugin. |
| [`bevy_vizij_animation`](bevy_vizij_animation/README.md) | Bevy plugin wrapping the core engine in ECS systems and component bindings. | Bevy games/tools that need Vizij animation playback. |
| [`vizij-animation-wasm`](vizij-animation-wasm/README.md) | `wasm-bindgen` binding that exposes the core engine to JavaScript/TypeScript. | npm package `@vizij/animation-wasm`, vizij-web demos/apps. |

All crates share the same data model (`StoredAnimation`, `AnimationData`, `Change`, `Event`) defined in `vizij-animation-core`.

---

## Typical Workflows

### Native Rust host

1. Add `vizij-animation-core` to your project.
2. Load animation data (either `StoredAnimation` JSON or generated `AnimationData`).
3. Create an `Engine`, load animations, create players/instances, and call `update_values(dt, inputs)` each frame.
4. Apply `Outputs.changes` to your rig/renderer and handle `Outputs.events` for instrumentation.

### Bevy project

1. Depend on both `vizij-animation-core` and `bevy_vizij_animation`.
2. Add `VizijAnimationPlugin` to your `App`.
3. Tag a root entity with `VizijTargetRoot` (or use `VizijBindingHint` on specific entities) so the plugin can build canonical bindings.
4. Load animations via the `VizijEngine` resource, create players/instances, and let the plugin schedule handle playback updates.

### Web / JavaScript consumer

1. Install the npm package published from `vizij-animation-wasm`: `npm install @vizij/animation-wasm`.
2. Call `await init()` once to initialise the WASM module (includes ABI guard).
3. Construct the provided `Engine` wrapper, load stored animations, and use the same player/instance/update loop as the Rust engine.
4. Use helpers like `bakeAnimationWithDerivatives` when you need baked outputs for tooling.

---

## Building & Testing

All commands run from the repository root unless noted otherwise.

```bash
# Run Rust tests for the stack
cargo test -p vizij-animation-core
cargo test -p bevy_vizij_animation

# Build the WASM binding (prod)
pnpm run build:wasm:animation

# Continuous WASM rebuild during development (requires cargo-watch)
pnpm run watch:wasm:animation
```

The WASM build script emits `crates/animation/vizij-animation-wasm/pkg/` which is then consumed by the npm wrapper in `npm/@vizij/animation-wasm`.

Vitest-based tests for the npm wrapper live alongside the package; run them with:

```bash
pnpm --filter "@vizij/animation-wasm" test
```

---

## Publishing

When preparing a release, keep crate and npm package versions in sync:

1. Bump versions in `vizij-animation-core`, `vizij-animation-wasm`, `bevy_vizij_animation`, and `npm/@vizij/animation-wasm`.
2. Publish the Rust crates (`cargo publish -p …`) in dependency order: core → wasm → Bevy.
3. Rebuild the WASM artefacts (`pnpm run build:wasm:animation`).
4. Publish the npm package from `npm/@vizij/animation-wasm`.
5. Update change logs as needed.

The helper script `scripts/dry-run-release.sh` runs through the build/publish checks without pushing artifacts.

---

## Reference Links

- [vizij-animation-core README](vizij-animation-core/README.md)
- [bevy_vizij_animation README](bevy_vizij_animation/README.md)
- [vizij-animation-wasm README](vizij-animation-wasm/README.md)
- [@vizij/animation-wasm README](../../npm/@vizij/animation-wasm/README.md)

Found something outdated? Please open an issue or ping the Vizij animation team—great docs keep the stack approachable. 🎬
