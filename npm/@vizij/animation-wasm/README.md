# @vizij/animation-wasm

WebAssembly wrapper around Vizij's animation engine. Provides a small, efficient JS/TS API to load animations (core or StoredAnimation JSON), create players and instances, prebind targets for efficient updates, and step the simulation deterministically.

This package is the primary JavaScript entry point to the animation engine for both browser and Node runtimes.

- Engine core: vizij-animation-core (Rust)
- WASM bindings: vizij-animation-wasm (Rust)
- This npm package: stable ESM entry and ergonomic wrapper

## Features

- Unified init() that works in both browser and Node
- Ergonomic Engine wrapper class (loadAnimation, createPlayer, addInstance, prebind, update)
- JSON-centric inputs/outputs with strong TypeScript types
- ABI version guard to catch mismatched builds
- Designed to integrate with React provider @vizij/animation-react

## Install

This package is built within the Vizij monorepo. If you are using this repo:

- Build the WASM package into `pkg/`:
  ```bash
  # from vizij-rs/
  node scripts/build-animation-wasm.mjs
  ```

- Build the TypeScript wrapper:
  ```bash
  # from vizij-rs/npm/@vizij/animation-wasm
  npm run build
  ```

In external projects, install the published package once available via npm.

## Quick Start

```ts
import { init, Engine, abi_version } from "@vizij/animation-wasm";

// 1) Initialize the wasm module once (browser or Node)
await init();
console.log("ABI", abi_version()); // numeric ABI guard (e.g., 1)

// 2) Create an Engine instance (optional config)
const eng = new Engine();

// 3) Load a StoredAnimation (recommended format)
const stored: import("@vizij/animation-wasm").StoredAnimation = {
  name: "Scalar Ramp",
  duration: 2000, // ms
  tracks: [
    {
      id: "t0",
      name: "Scalar Demo",
      animatableId: "demo/scalar",
      points: [
        { id: "k0", stamp: 0.0, value: 0 },
        { id: "k1", stamp: 1.0, value: 1 },
      ],
    },
  ],
  groups: {},
};

// loadAnimation auto-detects "stored" by presence of tracks
const animId = eng.loadAnimation(stored, { format: "stored" });

// 4) Create a player and instance
const playerId = eng.createPlayer("demo");
const instId = eng.addInstance(playerId, animId);

// 5) Prebind targets (optional but recommended):
// Map canonical paths -> small string keys. Numbers are accepted too.
eng.prebind((path) => {
  const map: Record<string, string> = { "demo/scalar": "demo/scalar" };
  return map[path] ?? null;
});

// 6) Step the simulation; Inputs are optional
const outputs = eng.update(0.016); // seconds
console.log(outputs.changes);
// => [{ player: 0, key: "demo/scalar", value: { type: "Scalar", data: 0.008 } }, ...]
```

## API Reference

### init(input?: InitInput): Promise<void>

- Initializes the wasm module once.
- Browser: defaults to fetching `../pkg/vizij_animation_wasm_bg.wasm` relative to this module.
- Node: automatically reads the wasm file from disk (no fetch required).
- Optionally pass an explicit `InitInput` (URL/Request/Response/BufferSource/WebAssembly.Module).

Also exported:
- `abi_version(): number` — numeric ABI guard (throws in wrapper if mismatch is detected).

### class Engine

Ergonomic wrapper around the wasm class. Ensure `await init()` has completed before constructing.

- constructor(config?: Config)

  Optional `Config` to hint capacities and event limits at engine init:
  ```ts
  interface Config {
    scratch_samples?: number;
    scratch_values_scalar?: number;
    scratch_values_vec?: number;
    scratch_values_quat?: number;
    max_events_per_tick?: number;
    features?: { reserved0?: boolean };
  }
  ```

- loadAnimation(data: AnimationData | StoredAnimation, opts?: { format?: "core" | "stored" }): AnimId

  Load either:
  - StoredAnimation (recommended) — detects automatically when `tracks` is present.
  - Core-format `AnimationData` when you have engine-internal JSON.

- createPlayer(name: string): PlayerId

- addInstance(player: PlayerId, anim: AnimId, cfg?: InstanceCfg): InstId

  `cfg` matches the engine’s instance configuration JSON (future-compatible pass-through).
  Minimal usage typically passes `undefined`.

- prebind(resolver: (path: string) => string | number | null | undefined): void

  Resolve canonical target paths (e.g., `"node/Transform.translation"`) into small keys you control. Return string or number. Numbers are coerced to strings internally.

- update(dtSeconds: number, inputs?: Inputs): Outputs

  Steps the simulation by `dt` seconds and returns `Outputs`. See Types for shapes.

### Low-level class VizijAnimation

For advanced usage, the raw wasm-bound class `VizijAnimation` is also exported. Prefer `Engine` unless you need the exact low-level surface.

## Types Overview

All TypeScript type definitions are included in `src/types.d.ts`.

### Values

Tagged union, normalized for transport:

```ts
type Value =
  | { type: "Scalar"; data: number }
  | { type: "Vec2"; data: [number, number] }
  | { type: "Vec3"; data: [number, number, number] }
  | { type: "Vec4"; data: [number, number, number, number] }
  | { type: "Quat"; data: [number, number, number, number] } // (x, y, z, w)
  | { type: "Color"; data: [number, number, number, number] } // RGBA
  | {
      type: "Transform";
      data: {
        translation: [number, number, number];
        rotation: [number, number, number, number]; // quat (x,y,z,w)
        scale: [number, number, number];
      };
    }
  | { type: "Bool"; data: boolean }
  | { type: "Text"; data: string };
```

### Outputs

```ts
interface Change {
  player: number; // PlayerId
  key: string;    // resolved target key
  value: Value;
}

type CoreEvent =
  | { PlaybackStarted: { player: number; animation?: string | null } }
  | { PlaybackPaused: { player: number } }
  | { PlaybackStopped: { player: number } }
  | { PlaybackResumed: { player: number } }
  | { PlaybackEnded: { player: number; animation_time: number } }
  | { TimeChanged: { player: number; old_time: number; new_time: number } }
  | {
      KeypointReached: {
        player: number;
        track_path: string;
        key_index: number;
        value: Value;
        animation_time: number;
      };
    }
  | { PerformanceWarning: { metric: string; value: number; threshold: number } }
  | { Error: { message: string } }
  | { Custom: { kind: string; data: unknown } };

interface Outputs {
  changes: Change[];
  events: CoreEvent[];
}
```

### Inputs

Send optional commands applied before each step:

```ts
type LoopMode = "Once" | "Loop" | "PingPong";

type PlayerCommand =
  | { Play: { player: number } }
  | { Pause: { player: number } }
  | { Stop: { player: number } }
  | { SetSpeed: { player: number; speed: number } }
  | { Seek: { player: number; time: number } }
  | { SetLoopMode: { player: number; mode: LoopMode } }
  | { SetWindow: { player: number; start_time: number; end_time?: number | null } };

interface InstanceUpdate {
  player: number;
  inst: number;
  weight?: number;
  time_scale?: number;
  start_offset?: number;
  enabled?: boolean;
}

interface Inputs {
  player_cmds?: PlayerCommand[];
  instance_updates?: InstanceUpdate[];
}
```

### StoredAnimation (new format)

The standardized JSON format the engine expects. Minimal shape:

```ts
interface StoredAnimation {
  name?: string;
  duration: number; // milliseconds
  tracks: {
    id: string;
    name?: string;
    animatableId: string; // canonical path
    points: Array<{
      id: string;
      stamp: number; // [0..1]
      value: number
           | { x: number; y: number }
           | { x: number; y: number; z: number }
           | { r: number; p: number; y: number }
           | { r: number; g: number; b: number }
           | { h: number; s: number; l: number }
           | boolean
           | string;
      transitions?: { in?: { x: number; y: number }; out?: { x: number; y: number } };
    }> ;
    settings?: { color?: string };
  }[];
  groups?: Record<string, unknown>;
}
```

See `vizij-spec/Animation.md` for details.

## Browser vs Node

- Browser: `await init()` uses a URL to the wasm binary. Bundlers like Vite may log that Node modules (path/url/fs) were externalized from this module; those are used only in Node paths and can be ignored in the browser.
- Node: `await init()` auto-reads the wasm bytes from disk using `fs/promises`. No fetch required.

## Troubleshooting

- "Call init()" errors: Ensure `await init()` before using Engine or VizijAnimation.
- ABI mismatch: If the engine ABI changes, the wrapper throws after init with a clear error (expected vs got). Rebuild the WASM package.
- Vite warnings about `node:` imports: Safe to ignore, browser path avoids Node-only modules during runtime.

## License

See repository root for licensing details.
