# Vizij Animation Crates

This directory contains the animation stack used throughout Vizij. The crates are designed to share the same core logic while
serving multiple runtimes—native Rust hosts, Bevy ECS games, and browser/Node environments via WebAssembly.

## Overview

* **`vizij-animation-core`** – Pure Rust engine that parses animation data, evaluates tracks, blends values, and emits outputs.
* **`bevy_vizij_animation`** – Bevy plugin that wires the core engine into ECS schedules and applies outputs to Bevy components.
* **`vizij-animation-wasm`** – `wasm-bindgen` bindings that expose the engine to JavaScript/TypeScript consumers. Published to
  npm as `@vizij/animation-wasm`.
* **Shared data format** – Animations are serialized as `StoredAnimation` JSON with normalized keypoints and cubic-bezier
  transitions. Hosts can also supply the engine’s internal `AnimationData` JSON when needed.

## Architecture

```
          +--------------------------+
          | vizij-animation-core     |
          |  (Rust library)          |
          +-----------+--------------+
                      |
        +-------------+--------------+
        |                            |
        v                            v
 +----------------------+   +------------------------+
 | bevy_vizij_animation |   | vizij-animation-wasm   |
 |  (Bevy Plugin)       |   |  (wasm-bindgen bindings)|
 +----------------------+   +-----------+------------+
                                        |
                              +---------v-----------+
                              | @vizij/animation-   |
                              | wasm (npm wrapper)  |
                              +---------------------+
```

* The core crate owns sampling, blending, and the update loop. It has no engine-specific dependencies.
* The Bevy plugin wraps the core engine in resources/systems, builds canonical bindings from the ECS world, and applies outputs.
* The WASM crate exports a JS class mirroring the engine API and feeds the npm package for web consumers (the Bevy and WASM
  layers are independent peers that both depend on the core crate).

## Installation

From an external Rust project, add the crates you need via `cargo add` (replace the version with the published release):

```bash
cargo add vizij-animation-core
cargo add bevy_vizij_animation             # if you use Bevy
```

Optional features:

* `vizij-animation-core` exposes the default feature set and requires no extra flags.
* `bevy_vizij_animation` and `vizij-animation-wasm` inherit Bevy/wasm dependencies and have no additional feature flags.

For JavaScript consumers install the npm package (published from this workspace):

```bash
npm install @vizij/animation-wasm
```

## Setup

Inside this repository:

1. Build/test the Rust crates:
   ```bash
   cargo test -p vizij-animation-core
   cargo test -p bevy_vizij_animation
   ```
2. Generate the WASM pkg output:
   ```bash
   node scripts/build-animation-wasm.mjs
   ```
3. (Optional) Link the npm package into `vizij-web` for live development:
   ```bash
   npm --workspace npm/@vizij/animation-wasm run build
   (cd npm/@vizij/animation-wasm && npm link)
   ```
4. Use `npm run watch:wasm` in this repo to rebuild the WASM crate automatically when Rust code changes (requires
   `cargo install cargo-watch`).

## Usage

### Core crate

* Create an `Engine`, load animation data (either `AnimationData` or `StoredAnimation` JSON), create players/instances, and call
  `update_values(dt, inputs)` (or `update_values_and_derivatives(dt, inputs)`) each tick.
* Outputs contain `changes` (resolved target key/value pairs) and `events` (playback notifications, keypoint hits, warnings).
* Use `bake_animation`/`bake_animation_with_derivatives` for offline sampling. The derivative variant returns
  `(BakedAnimationData, BakedDerivativeAnimationData)` sharing track order and cadence.

### Bevy plugin

* Add `VizijAnimationPlugin` to your `App`.
* Mark a root entity with `VizijTargetRoot`; the plugin builds a canonical binding map by traversing named descendants or entities
  with `VizijBindingHint`.
* The plugin schedules systems that prebind the engine, run fixed updates, and apply outputs to `Transform` components.

### WASM binding / npm package

* `await init()` once; the wrapper verifies `abi_version() === 2` and reports rebuild instructions if the JS glue/wasm binary is
  stale.
* Construct `VizijAnimation` or the higher-level `Engine` wrapper from the npm package.
* Use the same workflow as the Rust engine: load animations, create players/instances, optionally call `prebind` with a resolver
  callback, and `updateValues(dt)` or `updateValuesAndDerivatives(dt)` each frame. Outputs are plain JSON structures suitable for
  React state or other consumers.
* Baking helpers mirror the Rust tuples as `{ values, derivatives }` to simplify JSON plumbing on the TypeScript side.

## Key Details

* **StoredAnimation JSON** – Duration in milliseconds, track `stamp` values normalized 0..1, per-keypoint cubic-bezier control
  points via `transitions.in/out`, and support for scalar/vector/quat/color/bool/text values. Missing control points default to
  `{out: {x:0.42, y:0}, in: {x:0.58, y:1}}`. Boolean/string tracks use step interpolation.
* **Transition model** – Every segment is sampled using the cubic-bezier easing curve derived from adjacent keypoint transitions.
  Linear curves normalize to Bezier(0,0,1,1). Quaternion interpolation uses shortest-arc NLERP with normalization. Transform
  tracks decompose to TRS and blend components individually.
* **Binding** – Before the hot loop runs, canonical target paths (e.g., `Head/Transform.rotation`) are resolved to user-defined
  handles. The Bevy plugin builds this map automatically; the WASM binding expects a resolver callback.
* **Performance** – The engine minimizes per-frame allocations; once animations and bindings are loaded the update path operates
  on preallocated scratch buffers.
* **Derivatives** – Runtime derivatives are estimated via symmetric finite difference (default epsilon `1e-3`). Bool/Text tracks
  return `None`/`null`, and quaternion derivatives are currently component-wise (TODO: angular velocity/log mapping). When
  baking, override the epsilon with `BakingConfig::derivative_epsilon` to trade accuracy for cost.
* **Baking bundles** – `bake_animation_with_derivatives` keeps value/derivative track ordering aligned. The WASM layer returns a
  bundle `{ values, derivatives }` so clients can destructure without tuple semantics.
* **BakingConfig** – Exposed fields: `frame_rate`, `start_time`, `end_time`, and optional `derivative_epsilon` (positive finite).
  Invalid values (negative frame rate, `end_time < start_time`) are rejected by the WASM parser.

## Examples

### Native Rust engine

```rust
use vizij_animation_core::{parse_stored_animation_json, Engine, InstanceCfg};

let json = std::fs::read_to_string("tests/fixtures/new_format.json")?;
let stored = parse_stored_animation_json(&json)?;

let mut engine = Engine::new(Default::default());
let anim = engine.load_animation(stored);
let player = engine.create_player("demo");
engine.add_instance(player, anim, InstanceCfg::default());

let outputs = engine.update_values(1.0 / 60.0, Default::default());
for change in outputs.changes {
    println!("{} => {:?}", change.key, change.value);
}
```

### Bevy integration

```rust
use bevy::prelude::*;
use bevy_vizij_animation::{VizijAnimationPlugin, VizijTargetRoot, VizijEngine};
use vizij_animation_core::{parse_stored_animation_json, InstanceCfg};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(VizijAnimationPlugin)
        .add_systems(Startup, setup)
        .add_systems(Startup, load_animations)
        .run();
}

fn setup(mut commands: Commands) {
    let root = commands.spawn(VizijTargetRoot).id();
    let node = commands
        .spawn((Name::new("cube"), Transform::default(), GlobalTransform::default()))
        .id();
    commands.entity(root).add_child(node);
}

fn load_animations(mut eng: ResMut<VizijEngine>) {
    let json = include_str!("../../vizij-animation-core/tests/fixtures/new_format.json");
    let stored = parse_stored_animation_json(json).expect("valid animation");
    let anim = eng.0.load_animation(stored);
    let player = eng.0.create_player("demo");
    eng.0.add_instance(player, anim, InstanceCfg::default());
}
```

### JavaScript via npm package

```ts
import { init, Engine } from "@vizij/animation-wasm";

await init();
const eng = new Engine();
const animId = eng.loadAnimation({
  duration: 2000,
  tracks: [
    {
      id: "pos-x",
      animatableId: "cube/Transform.translation",
      points: [
        { id: "k0", stamp: 0.0, value: 0 },
        { id: "k1", stamp: 1.0, value: 5 },
      ],
    },
  ],
  groups: {},
}, { format: "stored" });
const playerId = eng.createPlayer("demo");
eng.addInstance(playerId, animId);
eng.prebind((path) => path); // identity binding

const outputs = eng.updateValues(1 / 60);
console.log(outputs.changes);

const withDerivatives = eng.updateValuesAndDerivatives(1 / 60);
for (const change of withDerivatives.changes) {
  console.log(change.key, change.derivative ?? null);
}

const baked = eng.bakeAnimationWithDerivatives(animId, {
  frame_rate: 60,
  derivative_epsilon: 5e-4,
});
console.log(baked.values.tracks[0].values.length, baked.derivatives.tracks[0].values.length);
```

## Testing

* Core unit tests:
  ```bash
  cargo test -p vizij-animation-core
  ```
* Bevy adapter tests:
  ```bash
  cargo test -p bevy_vizij_animation
  ```
* WASM tests (Node-based runner):
  ```bash
  scripts/run-wasm-tests.sh
  ```

The WASM script builds the crate for `wasm32-unknown-unknown`, runs `wasm-bindgen` to produce JS glue, and executes the test
suite using Node.

## Remaining Work & Ideas

* Expand fixture coverage for the new JSON schema (additional transition shapes, color tracks, and transform binding cases).
* Add assertions that the hot path remains allocation-free and document profiling techniques for animation-heavy scenarios.
* Continue refining Bevy component coverage as more targets are supported (materials, cameras, custom data components).
