# @vizij/animation-wasm

> **Vizij’s real-time animation engine for JavaScript and TypeScript.**

This package ships the official WebAssembly build of `vizij-animation-core` together with a TypeScript-friendly wrapper, ABI guards, and sample assets. Use it to load Vizij animation clips, create players/instances, stream outputs, and bake derivative-friendly bundles without compiling Rust.

---

## Table of Contents

1. [Overview](#overview)
2. [Key Concepts](#key-concepts)
3. [Installation](#installation)
4. [API](#api)
5. [Usage](#usage)
6. [Fixtures](#fixtures)
7. [Development & Testing](#development--testing)
8. [Related Packages](#related-packages)

---

## Overview

- Compiled directly from Vizij’s animation runtime (`vizij-animation-core`) via `wasm-bindgen`.
- Includes a high-level `Engine` class, low-level bindings, TypeScript declarations, and ready-to-use fixtures.
- Runs in browsers and Node; `init()` chooses the correct loader and validates the ABI (`abi_version() === 2`).
- Designed and maintained by the Vizij team—this npm package is the canonical distribution of the animation engine for JavaScript consumers.

---

## Key Concepts

- **StoredAnimation JSON** – Normalised animation format with duration, tracks, and cubic-bezier transitions. Recommended for authoring and interchange.
- **Engine** – High-level API mirroring the Rust engine (load animations, create players, add instances, prebind targets, update values, bake clips).
- **Players & Instances** – Players track playback state; instances attach animations with weight/time-scale/start-offset controls.
- **Outputs & Events** – `updateValues` returns per-target `ValueJSON` payloads plus engine events (play/pause, keyframe hits, warnings).
- **Derivatives** – Optional finite-difference derivatives are available at runtime and via `bakeAnimationWithDerivatives`.
- **ABI Guard** – `abi_version()` ensures the JS glue and `.wasm` binary stay in sync. Rebuild when versions diverge.

---

## Installation

```bash
npm install @vizij/animation-wasm
# or pnpm add @vizij/animation-wasm
```

For local development inside the Vizij workspace:

```bash
pnpm run build:wasm:animation      # regenerate pkg/
cd npm/@vizij/animation-wasm
pnpm install
pnpm run build
```

Link into `vizij-web` while iterating:

```bash
(cd npm/@vizij/animation-wasm && pnpm link --global)
(cd ../vizij-web && pnpm link @vizij/animation-wasm)
```

---

## Bundler Configuration

`@vizij/animation-wasm` now mirrors the import strategy used by the orchestrator and node-graph packages: it tries a static ESM import first, then falls back to a runtime URL when necessary. Ensure your bundler is configured to emit the `.wasm` file. For Webpack/Next.js:

```js
// next.config.js
module.exports = {
  webpack: (config) => {
    config.experiments = { ...(config.experiments ?? {}), asyncWebAssembly: true };
    config.module.rules.push({
      test: /\.wasm$/,
      type: "asset/resource",
    });
    return config;
  },
};
```

When serving the binary from a custom location, hand `init()` a string URL:

```ts
await init("https://cdn.example.com/vizij/vizij_animation_wasm_bg.wasm");
```

String URLs keep Webpack’s helper from wrapping the value and calling `.replace()` on a `URL` object.

---

## API

```ts
async function init(input?: InitInput): Promise<void>;
function abi_version(): number;

class Engine {
  constructor(config?: Config);
  loadAnimation(data: StoredAnimation | AnimationData, opts?: { format?: "stored" | "core" }): AnimId;
  createPlayer(name: string): PlayerId;
  addInstance(player: PlayerId, anim: AnimId, cfg?: InstanceCfg): InstId;
  prebind(resolver: (path: string) => string | number | null | undefined): void;
  updateValues(dtSeconds: number, inputs?: Inputs): Outputs;
  updateValuesAndDerivatives(dtSeconds: number, inputs?: Inputs): OutputsWithDerivatives;
  update(dtSeconds: number, inputs?: Inputs): Outputs; // alias for compatibility
  bakeAnimation(anim: AnimId, cfg?: BakingConfig): BakedAnimationData;
  bakeAnimationWithDerivatives(anim: AnimId, cfg?: BakingConfig): BakedAnimationBundle;
  listPlayers(): PlayerInfo[];
  listAnimations(): AnimationInfo[];
  // …additional helpers mirroring vizij-animation-core
}

// Raw bindings (rarely needed directly)
class VizijAnimation { /* same surface but string/JSON based */ }
```

All types (`StoredAnimation`, `Inputs`, `Outputs`, etc.) are exported from `src/types`.

---

## Usage

```ts
import { init, Engine } from "@vizij/animation-wasm";

await init();

const engine = new Engine();
const animId = engine.loadAnimation(storedAnimationJson, { format: "stored" });
const player = engine.createPlayer("demo");
engine.addInstance(player, animId);

engine.prebind((path) => path); // map canonical target path to your handle

const outputs = engine.updateValues(1 / 60);
console.log(outputs.changes, outputs.events);

const withDerivatives = engine.updateValuesAndDerivatives(1 / 60);
console.log(withDerivatives.changes.map(({ key, derivative }) => ({ key, derivative })));

const baked = engine.bakeAnimationWithDerivatives(animId, { frame_rate: 60 });
console.log(baked.values.tracks.length, baked.derivatives.tracks.length);
```

Low-level binding usage (when you want direct access to the wasm-bindgen class without the `Engine` wrapper):

```ts
import { init, VizijAnimation } from "@vizij/animation-wasm";

await init();
const raw = new VizijAnimation();
const animId = raw.load_stored_animation(storedAnimationJson);
const player = raw.create_player("demo");
raw.add_instance(player, animId, undefined);
const outputs = raw.update_values(0.016, undefined);
```

### Custom loader options

`init(input?: InitInput)` accepts anything understood by `@vizij/wasm-loader`: `URL`, `Response`, `ArrayBuffer`, `Uint8Array`, or a precompiled `WebAssembly.Module`.

```ts
import { init } from "@vizij/animation-wasm";

await init("https://cdn.example.com/vizij/vizij_animation_wasm_bg.wasm");
```

You can also pass a `URL`, `Response`, `ArrayBuffer`, `Uint8Array`, or `WebAssembly.Module` when a host needs explicit control over asset loading.

---

## Fixtures

The package re-exports helpers from `@vizij/test-fixtures`:

```ts
import { loadAnimationFixture } from "@vizij/animation-wasm";

const stored = await loadAnimationFixture("pose-quat-transform");
engine.loadAnimation(stored, { format: "stored" });
```

Fixtures are useful for smoke testing integrations or demoing the engine without writing your own assets.

Available fixture names (synchronised with `vizij-test-fixtures`):

| Fixture | Description |
|---------|-------------|
| `pose-quat-transform` | Transform animation showcasing translation + quaternion tracks. |
| `vector-pose-combo` | Mixed scalar/vector tracks for blending tests. |
| `loop-window` | Demonstrates loop windows and playback commands. |

Use `listAnimationFixtures()` to enumerate available fixtures at runtime.

---

## Bundler Notes

- The package exposes an ESM entry (`dist/animation-wasm/src/index.js`) and loads the generated `pkg/` asset through `@vizij/wasm-loader`.
- For Vite, add `optimizeDeps.exclude = ["@vizij/animation-wasm"]` to avoid pre-bundling the wasm artefact.
- Webpack >=5 handles wasm automatically. Enable `experiments.asyncWebAssembly = true` if you are on an older configuration.
- The package delegates loading to `@vizij/wasm-loader`, which memoises initialisation so multiple `init()` calls reuse the same module.

---

## Development & Testing

```bash
pnpm run build:wasm:animation          # ensure pkg/ is fresh
cd npm/@vizij/animation-wasm
pnpm test
```

The package test script runs the built Node test harness in `dist/animation-wasm/tests/all.test.js` after rebuilding the wrapper.

---

## Related Packages

- [`vizij-animation-wasm`](../../crates/animation/vizij-animation-wasm/README.md) – Rust source of these bindings.
- [`vizij-animation-core`](../../crates/animation/vizij-animation-core/README.md) – underlying engine logic.
- [`@vizij/value-json`](../value-json/README.md) – canonical value helpers used internally.

Need help or spotted an inconsistency? Open an issue—reliable bindings keep animation workflows smooth. 🎥
