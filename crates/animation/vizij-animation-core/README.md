# vizij-animation-core

`vizij-animation-core` is the engine-agnostic heart of Vizij’s animation system. It parses animation assets, evaluates tracks
with cubic-bezier easing, manages players/instances, and emits deterministic outputs that can be consumed by native hosts, Bevy,
or WebAssembly bindings.

## Overview

* Pure Rust crate with no engine or renderer dependencies.
* Provides the data model (`AnimationData`, `Track`, `Value`) and the runtime (`Engine`, `Instance`, `Player`).
* Designed for zero/low per-frame allocations after initialization.
* Consumed by the `bevy_vizij_animation` plugin and the `vizij-animation-wasm` crate.

## Architecture

```
+--------------------------+
| Config / Scratch arenas  |
+-------------+------------+
              |
+-------------v------------+
| Engine                   |
|  - Animation store       |
|  - Player registry       |
|  - Instance bindings     |
+-------------+------------+
              |
+-------------v------------+
| Samplers & blending      |
|  - Track sampling        |
|  - Cubic-bezier easing   |
|  - Value accumulation    |
+-------------+------------+
              |
+-------------v------------+
| Outputs (changes/events) |
+--------------------------+
```

* Parsing utilities convert JSON (`AnimationData` or `StoredAnimation`) into runtime-ready structures.
* The engine owns the hot loop: advance players, sample tracks, accumulate target contributions, blend, and emit outputs.
* Bindings map canonical string targets to compact handles to avoid string comparisons during updates.

## Installation

Add the crate to your project:

```bash
cargo add vizij-animation-core
```

The crate uses Rust 2021 and exposes no optional features. To build the WebAssembly bindings, use the sibling crate
`vizij-animation-wasm`.

## Setup

Typical workflow when embedding the engine:

1. **Parse animation data** – Use `parse_stored_animation_json` for the standardized JSON format or deserialize `AnimationData`
   directly when you control the authoring pipeline.
2. **Instantiate the engine** – `Engine::new(Config)` accepts optional scratch-buffer sizes and event limits. Defaults are tuned
   for small/medium scenes.
3. **Load animations** – Store animations with `Engine::load_animation`, receiving an `AnimId` handle.
4. **Create players and instances** – Players own playback state. `Engine::add_instance` binds an animation to a player with
   `InstanceCfg` (weight, loop window, start offset, etc.).
5. **Bind targets** – Resolve canonical target paths to handles via `Engine::prebind` or use host integrations that do this for
   you (e.g., the Bevy plugin).
6. **Run the update loop** – Call `Engine::update(dt_seconds, Inputs)` each frame. Outputs contain resolved `Change` entries and
   event notifications.

## Usage

Minimal example:

```rust
use vizij_animation_core::{Engine, InstanceCfg, parse_stored_animation_json};

let json = std::fs::read_to_string("../../fixtures/animations/vector-pose-combo.json")?;
let stored = parse_stored_animation_json(&json)?;

let mut engine = Engine::new(Default::default());
let anim = engine.load_animation(stored);
let player = engine.create_player("demo");
engine.add_instance(player, anim, InstanceCfg::default());

let outputs = engine.update(1.0 / 60.0, Default::default());
for change in outputs.changes {
    println!("{} => {:?}", change.key, change.value);
}
```

For integration details (binding callbacks, player commands, instance updates) see the crate documentation and tests under
`tests/`.

## Key Details

### Data model

* **AnimationData** – Name, duration (seconds), `tracks: Vec<Track>`, optional metadata map.
* **Track** – Canonical target path, value kind, keyframes, default interpolation kind.
* **Keyframe** – Absolute time (`t` seconds), value payload, optional interpolation override for the following segment.
* **Value types** – Scalars, Vec2/Vec3/Vec4, Quat, Color (RGBA), Transform (TRS), Bool, Text.
* **StoredAnimation JSON** – External-friendly schema with duration in milliseconds and normalized keypoint `stamp` values (0..1).
  Includes per-keypoint `transitions.in/out` cubic-bezier control points.

### Transition model

* Every segment uses a cubic-bezier easing curve defined by adjacent keypoint transitions.
* Defaults: `out = {x: 0.42, y: 0}`, `in = {x: 0.58, y: 1}` (classic ease-in-out).
* Linear curves normalize to `Bezier(0,0,1,1)`.
* Boolean and Text values use step interpolation (value holds until the next key).
* Quaternion interpolation uses shortest-arc NLERP and re-normalizes the result.
* Transform values decompose into translation/rotation/scale; translation/scale lerp linearly, rotation NLERPs.

### Outputs

* `Change` – `{ player, key, value }` where `key` is the bound string/handle and `value` is a tagged union (Scalar, Vec3, etc.).
* `Event` – Playback lifecycle (started/paused/stopped), keypoint notifications, time changes, warnings, or custom events emitted
  by animations.
* Inputs can include player commands (play/pause/seek), loop window updates, and per-instance weight/time-scale tweaks.

### Error handling & validation

* Parsing returns `anyhow::Error` to surface schema issues clearly.
* Mixed value kinds targeting the same canonical path are ignored at runtime (fail-soft policy).
* The engine guards against out-of-range timestamps, NaN inputs, and unbound targets.

## Examples

### Parsing StoredAnimation JSON

```rust
use vizij_animation_core::parse_stored_animation_json;

let json = r#"{
  "id": "anim-const",
  "name": "Const",
  "duration": 1000,
  "tracks": [
    {
      "id": "t0",
      "name": "Translation",
      "animatableId": "node/Transform.translation",
      "points": [
        { "id": "k0", "stamp": 0.0, "value": { "x": 1, "y": 2, "z": 3 } },
        { "id": "k1", "stamp": 1.0, "value": { "x": 1, "y": 2, "z": 3 } }
      ]
    }
  ],
  "groups": {}
}"#;

let anim = parse_stored_animation_json(json)?;
assert_eq!(anim.tracks.len(), 1);
```

### Baking animations

```rust
use vizij_animation_core::baking::{
    bake_animation_data_with_derivatives,
    BakingConfig,
    export_baked_json,
    export_baked_with_derivatives_json,
};

let anim = vizij_animation_core::AnimationData::default();
let cfg = BakingConfig { frame_rate: 60.0, start_time: 0.0, end_time: None, ..Default::default() };
let (values, derivatives) = bake_animation_data_with_derivatives(vizij_animation_core::AnimId(0), &anim, &cfg);
let values_json = export_baked_json(&values);
let bundle_json = export_baked_with_derivatives_json(&values, &derivatives);
println!("values: {}", values_json);
println!("values + derivatives: {}", bundle_json);
```

## Testing

Run the crate’s tests from the workspace root:

```bash
cargo test -p vizij-animation-core
```

For WebAssembly compatibility, execute the integration tests in `vizij-animation-wasm`:

```bash
scripts/run-wasm-tests.sh
```

## Additional Resources

* Fixtures demonstrating the StoredAnimation schema: `fixtures/animations/vector-pose-combo.json`, `fixtures/animations/simple-scalar-ramp.json`, etc.
* Engine usage examples under `examples/` (if present) and integration tests covering blending, binding, and event emission.
* `vizij-animation-wasm` README for the JavaScript API surface that wraps this crate.
