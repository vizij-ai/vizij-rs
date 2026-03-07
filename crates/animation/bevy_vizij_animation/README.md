# bevy_vizij_animation

> Bevy plugin that embeds Vizij's animation engine into ECS schedules.

`bevy_vizij_animation` wraps `vizij-animation-core` for Bevy applications. It builds binding maps from your scene hierarchy, advances the shared animation engine on `FixedUpdate`, and applies sampled outputs through Bevy components or the shared `WriterRegistry`.

## Overview

- Adds `VizijAnimationPlugin`.
- Exposes `VizijEngine(pub Engine)` as the shared engine resource.
- Exposes binding helpers: `VizijTargetRoot`, `VizijBindingHint`, and `BindingIndex`.
- Exposes timing/output resources: `FixedDt` and `PendingOutputs`.
- Inserts `bevy_vizij_api::WriterRegistry` and uses it while applying engine outputs.

The crate currently has no local Cargo feature flags.

## Installation

```bash
cargo add bevy_vizij_animation
```

## Quick Start

```rust
use bevy::prelude::*;
use bevy_vizij_animation::{VizijAnimationPlugin, VizijEngine, VizijTargetRoot};
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

## Key Concepts

| Item | Purpose |
|------|---------|
| `VizijTargetRoot` | Marks a hierarchy that should be scanned for canonical animation bindings. |
| `VizijBindingHint` | Overrides the canonical path for one entity when `Name` is not sufficient. |
| `VizijEngine` | Shared `vizij_animation_core::Engine` resource used to load animations and manage players. |
| `BindingIndex` | Cached mapping from canonical target paths to Bevy entities. |
| `FixedDt` | Fixed timestep resource used by the engine update system. |
| `PendingOutputs` | Latest engine outputs staged before they are applied to the world. |

The plugin installs these systems:

1. `build_binding_index_system` in `Update`
2. `prebind_core_system` in `Update`, after binding index rebuild
3. `fixed_update_core_system` in `FixedUpdate`
4. `apply_outputs_system` in `FixedUpdate`, after engine stepping

## Configuration

- Insert a custom `FixedDt` resource before or after adding the plugin if you need a cadence other than `1.0 / 60.0`.
- Attach `VizijBindingHint` to entities whose animation path should differ from their Bevy `Name`.
- Register extra setters in `bevy_vizij_api::WriterRegistry` if you want writes to affect non-`Transform` components.

Example custom setter:

```rust
use bevy::prelude::*;
use bevy_vizij_api::WriterRegistry;
use vizij_api_core::Value;

#[derive(Component)]
struct AnimatedLight;

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

## Development And Testing

```bash
cargo test -p bevy_vizij_animation
```

Tests cover binding discovery, fixed-update progression, and transform application.

## Related Crates

- [`vizij-animation-core`](../vizij-animation-core/README.md)
- [`bevy_vizij_api`](../../api/bevy_vizij_api/README.md)
- [`vizij-orchestrator-core`](../../orchestrator/vizij-orchestrator-core/README.md)
