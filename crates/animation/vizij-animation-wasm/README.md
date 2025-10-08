# vizij-animation-wasm

> **WebAssembly bridge for Vizij’s animation engine – use Vizij playback and baking APIs from JavaScript/TypeScript.**

`vizij-animation-wasm` compiles `vizij-animation-core` to WASM and exposes bindings consumed by the npm package `@vizij/animation-wasm`.

---

## Table of Contents

1. [Overview](#overview)
2. [Exports](#exports)
3. [Building](#building)
4. [Usage](#usage)
5. [Key Details](#key-details)
6. [Development & Testing](#development--testing)
7. [Related Packages](#related-packages)

---

## Overview

- Compiles to a `cdylib` through `wasm-bindgen`; `abi_version() == 2` enforces compatibility with the JS wrapper.
- Exposes a `VizijAnimation` class plus helper functions for loading animations, creating players, adding instances, updating playback, and baking outputs.
- Provides JSON normalisation for `StoredAnimation` assets and guard rails for invalid durations/control points.
- Mirrors the Rust engine API closely so behaviour stays consistent across native and browser environments.

---

## Exports

| Export | Description |
|--------|-------------|
| `class VizijAnimation` | Methods: `load_animation`, `load_stored_animation`, `create_player`, `add_instance`, `prebind`, `update_values`, `update_values_and_derivatives`, `update`, `bake_animation`, `bake_animation_with_derivatives`, `list_players`, `list_instances`, `set_input`, etc. |
| `abi_version() -> u32` | Returns `2`; the npm wrapper asserts this during `init()`. |
| Helper functions | Utility conversions (legacy value JSON, etc.) reused by the wrapper. |

The npm wrapper (`@vizij/animation-wasm`) layers a higher-level `Engine` class on top of these bindings.

---

## Building

```bash
pnpm run build:wasm:animation
```

Manual build:

```bash
wasm-pack build crates/animation/vizij-animation-wasm \
  --target bundler \
  --out-dir pkg \
  --release
```

The result lands in `crates/animation/vizij-animation-wasm/pkg/` and is copied into the npm package during the build script.

---

## Usage

Via npm wrapper:

```ts
import { init, Engine } from "@vizij/animation-wasm";

await init();
const eng = new Engine();

const animId = eng.loadAnimation(storedAnimationJson, { format: "stored" });
const player = eng.createPlayer("demo");
eng.addInstance(player, animId);
eng.prebind((path) => path); // map canonical path to your target ID

const outputs = eng.updateValues(1 / 60);
console.log(outputs.changes, outputs.events);

const baked = eng.bakeAnimationWithDerivatives(animId, { frame_rate: 60 });
console.log(baked.values.tracks.length, baked.derivatives.tracks.length);
```

Low-level binding:

```ts
import initWasm, { VizijAnimation, abi_version } from "@vizij/animation-wasm/pkg";

await initWasm();
console.log("ABI", abi_version());
const raw = new VizijAnimation();
const animId = raw.load_stored_animation(JSON.stringify(storedAnimationJson));
const playerId = raw.create_player("demo");
raw.add_instance(playerId, animId, undefined);
const result = raw.update_values(1 / 60, "{}");
console.log(JSON.parse(result));
```

---

## Key Details

- **StoredAnimation** – Duration in ms, keypoint `stamp` values 0..1, per-key transitions (`transitions.in/out`). Missing control points default to classic ease-in-out. Boolean/text tracks use step interpolation.
- **Prebinding** – `prebind(resolver)` receives canonical path strings and should return the handle you want in `Change.key`. Return `null`/`undefined` to leave bindings unresolved.
- **Outputs** – `updateValues` returns `{ changes, events }`. `updateValuesAndDerivatives` includes `derivative` per change for numeric tracks. Derivatives are finite differences with configurable epsilon when baking.
- **Inputs** – Accept playback commands and per-instance updates. The wrapper exports TypeScript types mirroring the Rust `Inputs` struct.
- **Baking** – `bakeAnimation` and `bakeAnimationWithDerivatives` return JSON with track metadata, frame rate, and sampled values. The derivative variant keeps track ordering aligned (`{ values, derivatives }`).
- **Error Handling** – Invalid JSON or configuration errors throw `JsError` with helpful messages (e.g., negative frame rate, mismatched value kinds).

---

## Development & Testing

```bash
# Rust-side tests
cargo test -p vizij-animation-wasm

# npm wrapper tests
pnpm run build:wasm:animation
cd npm/@vizij/animation-wasm
pnpm test
```

The npm test suite compares WASM outputs against fixtures to ensure parity with the native engine.

---

## Related Packages

- [`vizij-animation-core`](../vizij-animation-core/README.md) – underlying animation engine.
- [`@vizij/animation-wasm`](../../../npm/@vizij/animation-wasm/README.md) – npm wrapper with ESM entry and TypeScript types.
- [`@vizij/animation-react`](../../../vizij-web/packages/@vizij/animation-react/README.md) – React provider built on the wrapper.

Documentation improvements? Open an issue—consistent bindings keep Vizij animation accessible across platforms. 🎬
