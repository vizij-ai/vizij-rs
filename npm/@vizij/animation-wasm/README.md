# @vizij/animation-wasm

`@vizij/animation-wasm` is the npm package that re-exports the WebAssembly build of `vizij-animation-core`. It bundles the
generated `pkg/` artifacts, a stable ESM entry point, and TypeScript definitions so web projects can load Vizij’s animation engine
with minimal friction.

## Overview

* Wraps the Rust crate `vizij-animation-wasm` (compiled with `wasm-bindgen`).
* Provides a high-level `Engine` class plus the raw `VizijAnimation` bindings for advanced usage.
* Ships with TypeScript types for the engine, value unions, StoredAnimation JSON, and input/output structures.
* Designed to run in both browsers and Node (auto-detects environment when loading the WASM binary).

## Architecture

```
vizij-animation-core (Rust) --wasm-bindgen--> vizij-animation-wasm (cdylib) --npm--> @vizij/animation-wasm
       ^                         |                                            |
       |                         |                                            +-- src/index.ts (Engine wrapper, types)
       |                         +-- pkg/ (wasm-pack output: .wasm, .js glue)  +-- dist/ (bundled entry)
       +-- Shared JSON (StoredAnimation, Outputs, Inputs)
```

* The Rust core owns the animation runtime.
* `vizij-animation-wasm` exposes the runtime to JS via `wasm-bindgen`.
* This package re-exports the generated glue with an ergonomic wrapper and additional helpers (`init`, `abi_version`, samples).

## Installation

Published packages can be installed via npm:

```bash
npm install @vizij/animation-wasm
```

For local development inside the `vizij-rs` workspace:

```bash
# From repo root
node scripts/build-animation-wasm.mjs
cd npm/@vizij/animation-wasm
npm install
npm run build
```

To link into the `vizij-web` repo while iterating:

```bash
(cd npm/@vizij/animation-wasm && npm link)
# in vizij-web/
npm link @vizij/animation-wasm
```

## Setup

1. Call `await init()` once during application startup. The helper chooses the correct WASM loading strategy for browser or Node
   environments. You may pass an explicit `InitInput` if you need custom fetching.
2. Create an `Engine` (high-level wrapper) or the raw `VizijAnimation` class.
3. Load animations (StoredAnimation JSON recommended), create players/instances, optionally prebind targets, and call
   `update(dt)` each frame.

## Usage

### High-level API

```ts
import { init, Engine, abi_version } from "@vizij/animation-wasm";

await init();
console.log("ABI", abi_version());

const eng = new Engine();
const animId = eng.loadAnimation(storedAnimationJson, { format: "stored" });
const playerId = eng.createPlayer("demo");
eng.addInstance(playerId, animId);
eng.prebind((path) => path); // optional resolver

const outputs = eng.update(1 / 60);
console.log(outputs.changes);
```

### Low-level bindings

```ts
import init, { VizijAnimation, abi_version } from "@vizij/animation-wasm/pkg";

await init();
const raw = new VizijAnimation();
const animId = raw.load_stored_animation(JSON.stringify(storedAnimationJson));
const player = raw.create_player("demo");
raw.add_instance(player, animId, undefined);
const outputs = JSON.parse(raw.update(0.016, undefined));
```

## Key Details

* **StoredAnimation JSON** – Duration in milliseconds, track keypoints with normalized `stamp` values (0..1), per-keypoint cubic
  bezier control points via `transitions.in/out`, and support for scalar/vector/quat/color/bool/text values.
* **Outputs** – `{ changes: Change[], events: CoreEvent[] }`. Each `Change` includes the resolved key and a tagged union value
  (Scalar, Vec3, Transform, etc.). Events mirror the Rust engine’s playback notifications (started, paused, keypoint reached,
  warnings, etc.).
* **Inputs** – Optional `Inputs` payload supports player commands (play/pause/seek/loop) and per-instance updates (weights,
  timescale, enabled flag, start offset).
* **Environment detection** – Browser builds load the `.wasm` via fetch relative to the module URL; Node builds read from disk.
  Bundlers may log that Node modules (`node:path`, `fs`) were externalized—this is expected.
* **ABI guard** – `abi_version()` ensures the JS wrapper and WASM binary agree. The `Engine` wrapper throws if the numbers differ
  after `init()`.
* **TypeScript support** – `src/types.d.ts` exports `Value`, `StoredAnimation`, `Inputs`, `Outputs`, and helper types.

## Examples

### StoredAnimation definition

```ts
const storedAnimation = {
  name: "Scalar Ramp",
  duration: 2000,
  tracks: [
    {
      id: "t0",
      name: "Scalar Demo",
      animatableId: "cube/scalar",
      points: [
        { id: "k0", stamp: 0.0, value: 0 },
        { id: "k1", stamp: 1.0, value: 1 },
      ],
    },
  ],
  groups: {},
};
```

### Player commands & instance updates

```ts
eng.update(1 / 60, {
  player_cmds: [
    { Play: { player: playerId } },
    { SetLoopMode: { player: playerId, mode: "Loop" } },
  ],
  instance_updates: [
    { player: playerId, inst: 0, weight: 0.5, enabled: true },
  ],
});
```

## Testing

This package piggybacks on the Rust crate’s tests. To run them locally:

```bash
# From repo root
scripts/run-wasm-tests.sh
```

The script builds the WASM binary, runs Node-based tests, and compares results to the native engine. You can also run
`npm run build` to ensure the wrapper bundles correctly.

## Troubleshooting

* **Missing default export** – Ensure you import from the npm package (`@vizij/animation-wasm`). The generated `pkg/` exposes a
  default `init` export; the wrapper re-exports both default and named bindings.
* **“Call init()” errors** – Always `await init()` before constructing `Engine`/`VizijAnimation`.
* **ABI mismatch** – Rebuild the WASM package (`node scripts/build-animation-wasm.mjs`) and reinstall to align versions.
* **TypeScript cannot find pkg files** – Re-run the build to regenerate `pkg/` and ensure the wrapper’s `src/index.ts` exports both
  default and named bindings.
