# bevy_vizij_animation

> **Bevy plugin that embeds Vizij’s animation engine into ECS schedules.**

`bevy_vizij_animation` wraps `vizij-animation-core` for Bevy applications. It constructs bindings from your scene graph, advances the engine on a fixed timestep, and applies sampled values to Bevy components.

---

## Table of Contents

1. [Overview](#overview)
2. [Installation](#installation)
3. [Quick Start](#quick-start)
4. [Key Concepts](#key-concepts)
5. [Configuration](#configuration)
6. [Development & Testing](#development--testing)
7. [Related Crates](#related-crates)

---

## Overview

- Adds `VizijAnimationPlugin` to your Bevy app.
- Provides ECS components/resources for binding discovery (`VizijTargetRoot`, `VizijBindingHint`), engine access (`VizijEngine`), and fixed timestep control (`FixedDt`).
- Runs systems to rebuild bindings, prebind targets, advance the engine, and apply outputs.
- Supports StoredAnimation and AnimationData loading via the shared engine resource.

---

## Installation

```bash
cargo add bevy_vizij_animation
```

Feature flags:

| Feature | Default | Description |
|---------|---------|-------------|
| `urdf_ik` | ✔ | Pulls in robotics helpers from the core engine. Disable with `--no-default-features` if not needed. |

Enabling `urdf_ik` turns on the same feature flag in `vizij-animation-core`, compiling the robotics interpolation helpers that the wasm build ships with by default. Disable it only if you are certain no controllers rely on URDF-derived targets.

---

## Quick Start

```rust
use bevy::prelude::*;
use bevy_vizij_animation::{VizijAnimationPlugin, VizijTargetRoot, VizijEngine};
use vizij_animation_core::{parse_stored_animation_json, InstanceCfg};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(VizijAnimationPlugin)
        .add_systems(Startup, (setup_scene, load_animation))
        .run();
}

fn setup_scene(mut commands: Commands) {
    let root = commands.spawn(VizijTargetRoot).id();
    let cube = commands
        .spawn((Name::new("cube"), Transform::default(), GlobalTransform::default()))
        .id();
    commands.entity(root).add_child(cube);
}

fn load_animation(mut eng: ResMut<VizijEngine>) {
    let json = include_str!("../../../fixtures/animations/vector-pose-combo.json");
    let stored = parse_stored_animation_json(json).expect("valid animation");

    let anim = eng.0.load_animation(stored);
    let player = eng.0.create_player("demo");
    eng.0.add_instance(player, anim, InstanceCfg::default());
}
```

---

## Key Concepts

| Component/Resource | Purpose |
|--------------------|---------|
| `VizijTargetRoot` | Marks the root of a hierarchy that should be scanned for canonical bindings. |
| `VizijBindingHint` | Overrides the canonical path for an entity (defaults to the entity’s `Name`). |
| `VizijEngine` | Wrapper around `vizij_animation_core::Engine`; use it to load animations, manage players, enqueue inputs. |
| `FixedDt` | Controls the fixed timestep used for animation updates (default `1.0 / 60.0`). |
| `PendingOutputs` | Internal staging of the most recent engine outputs prior to applying them to components. |

Systems inserted by the plugin (simplified):

1. **Binding rebuild** – Scans `VizijTargetRoot` subtrees and populates the binding index.
2. **Prebind** – Calls into the core engine to resolve canonical paths to handles.
3. **Fixed update** – Advances the engine using `FixedDt` and records outputs.
4. **Apply outputs** – Writes values to `Transform` components (translation/rotation/scale). Extend this stage to support additional component types.

### System ordering

| Schedule | System | Notes |
|----------|--------|-------|
| `Update` | `build_binding_index_system` | Runs every frame to catch hierarchy changes. |
| `Update` | `prebind_core_system` | Declared with `.after(build_binding_index_system)` to guarantee bindings exist. |
| `FixedUpdate` | `fixed_update_core_system` | Consumes the fixed timestep (`FixedDt`) and advances the engine. |
| `FixedUpdate` | `apply_outputs_system` | Declared `.after(fixed_update_core_system)` so all sampled values are applied before the next frame. |

Insert custom systems with `.before(...)` / `.after(...)` against these labels, or move the fixed update stage to a different schedule with `.add_plugins(VizijAnimationPlugin::default())` plus `app.edit_schedule`.

---

## Configuration

- Override the fixed timestep by inserting `FixedDt(desired_delta)` before the plugin runs.
- Attach `VizijBindingHint { path: "custom/path" }` to entities when animation target names do not match Bevy `Name`s.
- The plugin currently applies animation changes to `Transform`. For custom behaviour, add your own system after `apply_outputs` to consume `PendingOutputs`.

### Non-`Transform` setters

You can register additional setters via the shared `WriterRegistry` resource to animate arbitrary components:

```rust
use bevy::prelude::*;
use bevy_vizij_animation::VizijAnimationPlugin;
use bevy_vizij_api::WriterRegistry;
use vizij_api_core::Value;

#[derive(Component)]
struct AnimatedLight;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, VizijAnimationPlugin))
        .add_systems(Startup, (spawn_light, register_light_setter))
        .run();
}

fn spawn_light(mut commands: Commands) -> Entity {
    commands
        .spawn((AnimatedLight, PointLight {
            intensity: 0.0,
            ..Default::default()
        }))
        .id()
}

fn register_light_setter(
    light: Query<Entity, With<AnimatedLight>>,
    mut registry: ResMut<WriterRegistry>,
) {
    let entity = light.single();
    registry.register_setter("rig/Lights/Main.intensity", move |world, _, value| {
        if let Some(mut e) = world.get_entity_mut(entity) {
            if let Some(mut light) = e.get_mut::<PointLight>() {
                if let Value::Float(intensity) = value {
                    light.intensity = *intensity;
                }
            }
        }
    });
}
```

Any writes against `rig/Lights/Main.intensity` are now routed to the light component, enabling material, audio, or VFX controls alongside transforms.

---

## Development & Testing

```bash
cargo test -p bevy_vizij_animation
```

Tests cover binding discovery, fixed-update progression, and component updates using sample animations.

For live experimentation run the `apps/demo-animation-studio` workspace in `vizij-web` after linking the local WASM builds.

---

## Related Crates

- [`vizij-animation-core`](../vizij-animation-core/README.md) – underlying engine logic.
- [`bevy_vizij_api`](../../api/bevy_vizij_api/README.md) – shared write-application utilities used by this plugin.
- [`vizij-orchestrator-core`](../../orchestrator/vizij-orchestrator-core/README.md) – can drive the engine alongside graphs via the orchestrator.

Questions or improvements? Open an issue—polished ECS integrations keep Vizij animation easy to adopt. 🎥
