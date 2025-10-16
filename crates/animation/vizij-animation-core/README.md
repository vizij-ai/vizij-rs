# vizij-animation-core

> **Engine-agnostic animation runtime for Vizij – deterministic sampling, blending, and event emission in pure Rust.**

`vizij-animation-core` parses Vizij animation assets, manages players and instances, samples tracks with cubic-bezier easing, and emits typed changes that downstream hosts can apply to rigs or renderers. It powers the Bevy plugin (`bevy_vizij_animation`) and the WebAssembly binding (`vizij-animation-wasm`).

---

## Table of Contents

1. [Overview](#overview)
2. [Features](#features)
3. [Installation](#installation)
4. [Quick Start](#quick-start)
5. [Usage Workflow](#usage-workflow)
6. [Key Concepts](#key-concepts)
7. [Development & Testing](#development--testing)
8. [Related Packages](#related-packages)

---

## Overview

- **Pure Rust runtime** with predictable memory usage and zero engine dependencies.
- **Canonical data model** for animations (`AnimationData`, `Track`, `Keyframe`) plus a JSON-friendly `StoredAnimation` schema.
- **Engine** type that stores animations, manages players/instances, and produces `Outputs` each frame.
- **Runtime utilities** for baking animations, estimating derivatives, and serialising data for tooling.

---

## Features

- Cubic-bezier easing with per-key overrides and sensible defaults.
- Support for scalar, vector, quaternion, colour, transform, boolean, and text tracks.
- Deterministic player state machine with loop modes (`Loop`, `Once`, `PingPong`) and playback controls.
- Instance-level blending (weight, time scaling, offsets) across multiple animations per player.
- Optional derivative export for downstream tooling and analysis.
- Event dispatch for playback changes and animation-authored notifications.

---

## Installation

```bash
cargo add vizij-animation-core
```

The crate targets Rust 2021 and exposes no optional features. Companion crates provide additional environments:

- `bevy_vizij_animation` – Bevy plugin built on this core.
- `vizij-animation-wasm` – wasm-bindgen binding used in web applications.

---

## Quick Start

```rust
use vizij_animation_core::{Engine, InstanceCfg, Inputs};
use vizij_animation_core::stored_animation::parse_stored_animation_json;

let json = include_str!("../../../fixtures/animations/vector-pose-combo.json");
let stored = parse_stored_animation_json(json)?;

let mut engine = Engine::new(Default::default());
let anim = engine.load_animation(stored);

let player = engine.create_player("demo");
engine.add_instance(player, anim, InstanceCfg::default());

let outputs = engine.update_values(1.0 / 60.0, Inputs::default());

for change in &outputs.changes {
    println!("{} => {:?}", change.key, change.value);
}
```

### Multi-player blending

```rust
use vizij_animation_core::{Engine, InstanceCfg, Inputs};
use vizij_animation_core::stored_animation::parse_stored_animation_json;
use vizij_test_fixtures::animations;

fn main() -> anyhow::Result<()> {
    let locomotion = parse_stored_animation_json(&animations::json("vector-pose-combo")?)?;
    let accent = parse_stored_animation_json(&animations::json("loop-window")?)?;

    let mut engine = Engine::default();
    let player = engine.create_player("character");
    engine.add_instance(
        player,
        engine.load_animation(locomotion),
        InstanceCfg {
            weight: 1.0,
            time_scale: 1.0,
            ..Default::default()
        },
    );
    engine.add_instance(
        player,
        engine.load_animation(accent),
        InstanceCfg {
            weight: 0.35,
            time_scale: 1.0,
            ..Default::default()
        },
    );

    let outputs = engine.update_values(1.0 / 60.0, Inputs::default());
    for change in outputs.changes {
        println!("{} => {:?}", change.key, change.value);
    }
    Ok(())
}
```

Instances are blended in insertion order. Adjust `weight`, `time_scale`, and `start_offset` to compose layered motion before applying the resulting `Outputs` to your rig.

---

## Usage Workflow

1. **Parse Animation Data**
   - Use `parse_stored_animation_json` for assets exported from Vizij tooling.
   - Alternatively deserialize `AnimationData` directly if you control authoring pipelines.
2. **Construct an Engine**
   - `Engine::new(Config)` (or `Engine::default()`) accepts buffer sizing hints via [`Config`](#engineconfig-tuning): adjust scratch capacities when sampling dense rigs, raise `max_events_per_tick` for verbose telemetry, or carry feature toggles.
3. **Load Animations**
   - `Engine::load_animation(data)` stores animation content and returns an `AnimId` handle.
4. **Create Players**
   - `Engine::create_player(name)` returns a `PlayerId`. Players track playback time, speed, loop mode, and instance membership.
5. **Attach Instances**
   - `Engine::add_instance(player, anim, InstanceCfg)` binds an animation to a player with weight, time-scale, start offset, and enabled state.
6. **Bind Targets**
   - Provide a `TargetResolver` (e.g., through `Engine::prebind`) to map canonical target paths to the IDs your host consumes.
7. **Update Each Frame**
   - Call `Engine::update_values(dt_seconds, Inputs)` (or `update_values_and_derivatives`) to advance playback and collect `Outputs`.
   - Apply `Outputs.changes` in your host (rig, renderer, etc.) and process `Outputs.events` for instrumentation or game logic.

---

## Key Concepts

### Data Model

- **AnimationData** – Internal representation with duration (seconds), track list, and optional metadata.
- **StoredAnimation** – Distribution format expressed in milliseconds with normalised `stamp` keypoints (0..1). Each point contains optional `transitions.in/out` cubic-bezier control points.
- **Track** – Couples a canonical target with keyframes and a value kind. Supports per-key interpolation overrides.
- **Value Types** – Scalars, Vec2/Vec3/Vec4, Quaternion, Colour RGBA, Transform (TRS), Boolean, Text. Transform interpolation decomposes into TRS components.

### Engine Components

- **Animations** – Stored in an internal library keyed by `AnimId`.
- **Players** – Manage playback state, mode (`Loop`, `Once`, `PingPong`), speed, time window, and attached instances.
- **Instances** – Bind an animation to a player with weight/time-scale/start offset/enabled flags and a `BindingSet`.
- **Bindings** – Map canonical target paths to host IDs via a `TargetResolver`. Prevents string comparisons during updates.
- **Outputs** – Provide a list of `Change { player, key, value }` and associated events. `OutputsWithDerivatives` adds optional derivative values per change.

### EngineConfig tuning

- `scratch_samples`, `scratch_values_*` – preallocate scratch buffers; raise these when you drive large skeletons or many numeric targets to minimise reallocations.
- `max_events_per_tick` – cap on events retained per frame; lower to apply backpressure or raise when authoring dense instrumentation.
- `features` – reserved for future toggles (SIMD, parallel). Leave at default unless experimenting with feature branches.

Most projects can rely on `Config::default()`, but headless baking tools or orchestration-heavy hosts benefit from tailoring these hints to their workload.

### Baking & Derivatives

- `bake_animation_data` – Generates sampled animation data at a fixed frame rate for export.
- `bake_animation_data_with_derivatives` – Adds derivative tracks using finite differencing (`derivative_epsilon` configurable via `BakingConfig`).
- Export helpers serialise baked bundles back to JSON for tooling or offline optimisation.

### Events & Inputs

- **Inputs** – Aggregate player commands (`Play`, `Pause`, `Seek`, `SetSpeed`, `SetLoopMode`) and per-instance updates (weight/time-scale/start offset/enabled).
- **Events** – Emitted for playback state transitions, loop completions, custom animation events, and warnings (e.g., binding failures).

### Outputs & derivatives

- `Outputs.changes[n]` and `OutputsWithDerivatives.changes[n]` reference the same sampled value ordering. When derivatives are requested, each `ChangeWithDerivative` carries the numeric derivative in the same slot as the value that produced it.
- Derivatives are optional (`Option<Value>`). Non-numeric tracks (booleans, text) persist `None`, whereas numeric tracks use the same typed envelope as the primary value (`Float`, `Vec3`, etc.).
- Events are shared between both output structures; switching to derivatives does not drop instrumentation signals.

---

## Development & Testing

Run the crate’s test suite:

```bash
cargo test -p vizij-animation-core
```

Integration and parity tests live under `tests/`, covering:

- Stored animation parsing and JSON serialisation.
- Blending correctness across scalar/vector/quat tracks.
- Player command semantics and loop window enforcement.
- Derivative baking accuracy.

Helpful workspace scripts:

```bash
pnpm run test:rust                # fmt, clippy, and tests for the entire workspace
pnpm run build:wasm:animation     # rebuilds the WebAssembly adapter that embeds this crate
pnpm run watch:wasm:animation     # continuous rebuild (requires cargo-watch)
```

---

## Related Packages

- [`bevy_vizij_animation`](../../animation/bevy_vizij_animation/README.md) – Bevy plugin that wires this engine into ECS systems.
- [`vizij-animation-wasm`](../vizij-animation-wasm/README.md) – wasm-bindgen binding used in web runtimes.
- [`@vizij/animation-wasm`](../../../npm/@vizij/animation-wasm/README.md) – npm wrapper with loader utilities and ABI checks.
- [`vizij-orchestrator-core`](../../orchestrator/vizij-orchestrator-core/README.md) – Coordinates animation and node graph controllers via a shared blackboard.

Questions or contributions? Please file an issue in the main Vizij repository—well-documented behaviour keeps animation playback predictable. 🎬
