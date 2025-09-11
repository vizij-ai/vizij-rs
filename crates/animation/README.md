# Vizij Animation Crates

This repository contains the core animation system for Vizij, designed for high performance, deterministic updates, and portability across different runtimes. The system is split into three primary Rust crates: a generic core, a Bevy engine adapter, and a WebAssembly (WASM) wrapper.

## Overview

The goal of this project is to provide a single, unified animation player that can power both native applications (e.g., games, robotics) and web-based experiences from the same animation data and core logic. It emphasizes zero or low per-frame allocations, one-time binding of animation targets, and a clear separation between the animation engine and the host application (renderer, scene graph, etc.).

## Architecture

The architecture is composed of a central `core` crate with thin adapters for specific environments.

```
+----------------------+          +----------------------+          +----------------------+
| vizij-animation-core | <------> | vizij-animation-wasm | <------> | @vizij/animation-*   |
| (Rust, no engine)    |          | (Rust FFI)           |          | (npm pkg + React)    |
+----------------------+          +----------------------+          +----------------------+
           ^
           |
+----------------------+          +----------------------+          +----------------------+
| bevy_vizij_animation |  ----->  |  Bevy World / ECS   |  <-----> | App / Game / Robot   |
| (Rust Bevy plugin)   |          |  (components, sys)  |          | (native)             |
+----------------------+          +----------------------+          +----------------------+
```

- **`vizij-animation-core`**: A pure Rust library with no engine-specific dependencies. It defines the data formats, owns the animation logic (sampling, blending, time management), and exposes a simple `update` function.
- **`bevy_vizij_animation`**: A Bevy plugin that integrates the core engine into a Bevy ECS application. It acts as a bridge, mapping Bevy components to animation targets and applying the computed animation values back to the world.
- **`vizij-animation-wasm`**: A WebAssembly wrapper that exposes the core engine's functionality to JavaScript/TypeScript. This allows the animation system to run efficiently in a web browser or Node.js environment.

## Core Concepts

### Data Model (`StoredAnimation`)

The system uses a standardized JSON format, `StoredAnimation`, for defining animation clips. This format is designed to be human-readable and easy to generate from various authoring tools.

- **Duration**: Specified in milliseconds at the root of the animation file.
- **Keypoints**: Timestamps (`stamp`) are normalized from `0.0` to `1.0` within the track's duration.
- **Values**: Supports a range of data types, including `boolean`, `number`, `string`, vectors (`{x,y}`, `{x,y,z}`), Euler angles (`{r,p,y}`), and colors (`{r,g,b}`, `{h,s,l}`).
- **Transitions**: Animation curves are defined by per-keypoint transitions.

**Example `StoredAnimation` Snippet:**
```json
{
  "id": "anim-1",
  "name": "My Animation",
  "duration": 5000,
  "tracks": [
    {
      "id": "track-0",
      "name": "Position X",
      "animatableId": "cube-position-x",
      "points": [
        {
          "id": "k0",
          "stamp": 0.0,
          "value": -2,
          "transitions": { "out": { "x": 0.65, "y": 0 } }
        },
        {
          "id": "k1",
          "stamp": 0.25,
          "value": 0,
          "transitions": { "in": { "x": 0.35, "y": 1 } }
        }
      ]
    }
  ]
}
```

### Transition Model (Cubic Bezier)

The primary transition model is a **cubic-bezier** timing function applied to each segment between keypoints. This provides smooth, customizable easing similar to CSS animations.

- For a segment from `P0` to `P1`, the curve is defined by `P0.transitions.out` and `P1.transitions.in`.
- If control points are missing, they default to a standard "ease-in-out" curve:
  - `out`: `{ "x": 0.42, "y": 0 }`
  - `in`: `{ "x": 0.58, "y": 1 }`
- `boolean` and `string` tracks use **step** interpolation, holding the previous value until the next keypoint is reached.

### Binding and Targets

To avoid costly string lookups in the animation hot loop, the engine performs a **one-time binding** step. It resolves human-readable target paths (e.g., `"Head/Transform.rotation"`) into direct, efficient handles. This binding is performed upfront and only updated when the scene structure changes.

### Update Pipeline

The engine follows a deterministic, multi-stage update pipeline on each tick:
1.  **Advance Players**: Update each player's timeline based on `dt`, speed, and loop settings.
2.  **Accumulate**: For each animation instance, sample the relevant tracks at the correct local time and accumulate the weighted `(target, value)` contributions.
3.  **Blend & Apply**: Blend the accumulated values for each target. The core engine does not modify external state; it produces a set of final outputs.
4.  **Emit Output**: The host adapter (`bevy` or `wasm`) receives the outputs and applies them to the application's world (e.g., updating Bevy `Transform` components or providing a JSON object to JavaScript).

## Crates

### `vizij-animation-core`

The engine-agnostic heart of the system. It is responsible for:
- Parsing the `StoredAnimation` JSON format.
- The `Engine -> Player -> Instance` ownership model.
- Deterministic sampling, accumulation, and blending logic.
- A wide range of supported `Value` types, including `Scalar`, `Vec3`, `Quat`, `Color`, and step-only `Bool` and `Text`.

### `bevy_vizij_animation`

A Bevy plugin that makes it easy to use the animation engine in an ECS context.
- Wraps the core `Engine` in a Bevy `Resource`.
- Automatically discovers and binds animatable entities using a `VizijTargetRoot` marker component.
- Runs the engine's update loop within Bevy's `FixedUpdate` schedule for determinism.
- Applies animation outputs to Bevy `Transform` components.

### `vizij-animation-wasm`

A lightweight WASM wrapper that exposes the core engine to JavaScript and TypeScript.
- Provides a simple JavaScript `class VizijAnimation` API.
- Accepts JSON for configuration and animation data.
- Returns outputs as a structured JSON object for easy consumption in web frontends.
- Includes an ABI version check to ensure compatibility between the WASM module and the JS wrapper.

## Usage Examples

### Core Rust

```rust
use vizij_animation_core::{parse_stored_animation_json, Engine, InstanceCfg};

let json_str = std::fs::read_to_string("tests/fixtures/new_format.json").unwrap();
let anim = parse_stored_animation_json(&json_str).expect("parse");

let mut engine = Engine::new(Default::default());
let aid = engine.load_animation(anim);
let pid = engine.create_player("demo");
let _iid = engine.add_instance(pid, aid, InstanceCfg::default());

// In your game loop:
let outputs = engine.update(1.0 / 60.0, Default::default());
// Apply outputs.changes to your world...
```

### Bevy

```rust
use bevy::prelude::*;
use bevy_vizij_animation::{VizijAnimationPlugin, VizijTargetRoot};

fn main() {
  App::new()
    .add_plugins(DefaultPlugins)
    .add_plugins(VizijAnimationPlugin)
    .add_systems(Startup, setup)
    .run();
}

fn setup(mut commands: Commands) {
  // The plugin will discover and bind named entities under this root
  let root = commands.spawn(VizijTargetRoot).id();

  let node = commands.spawn((Name::new("node"), Transform::default())).id();
  commands.entity(root).add_child(node);

  // Load animations into the `VizijEngine` resource...
}
```

### JavaScript/TypeScript (WASM)

```ts
import { VizijAnimation } from "@vizij/animation-wasm";

const engine = new VizijAnimation();

const animJson = { /* ... StoredAnimation JSON ... */ };
const animId = engine.load_stored_animation(animJson);
const playerId = engine.create_player("demo");
engine.add_instance(playerId, animId);

// In your render loop:
const outputs = engine.update(0.016);
console.log(outputs.changes);
// e.g., [{ player: 0, key: "cube-position-x", value: { type: "Scalar", data: -1.95 } }]
```

## Development Status

The project is actively developed. The core features are implemented and tested, but some tasks remain.

### Remaining Implementation Tasks
- Finalize the transition model simplification in `vizij-animation-core` to rely exclusively on cubic-bezier and step interpolations, removing legacy paths.
- Update or retire test fixtures that use old transition types.
- Refresh documentation and examples to fully reflect the new `StoredAnimation` JSON schema and defaults.

### Remaining Testing Tasks
- Verify shortest-arc logic for quaternion interpolation.
- Add specific tests for `BindingTable` upsert/overwrite logic.
- Add tests for decomposed TRS (Transform) blending.
- Add validation for normalized quaternions in baked output.
- Instrument and assert zero per-tick allocations in the hot path.

## Building and Testing

You can run tests for each crate from the workspace root:

```bash
# Test the core engine
cargo test -p vizij-animation-core

# Test the Bevy adapter
cargo test -p bevy_vizij_animation

# Test the WASM wrapper (requires wasm-pack)
./scripts/run-wasm-tests.sh
```

