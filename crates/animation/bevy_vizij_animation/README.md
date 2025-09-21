# bevy_vizij_animation

`bevy_vizij_animation` adapts `vizij-animation-core` for the Bevy game engine. It injects the core engine as a resource, builds a
binding index from the ECS world, runs fixed-timestep updates, and applies sampled animation outputs to Bevy components.

## Overview

* Ships as a standard Bevy plugin (`VizijAnimationPlugin`).
* Provides ECS components/resources to mark animation roots, override binding paths, and access the core engine instance.
* Integrates tightly with Bevy schedules (`Startup`, `Update`, `FixedUpdate`).
* Suitable for games, robotics simulators, or any Bevy app that needs deterministic animation playback.

## Architecture

```
+------------------------------+
| VizijAnimationPlugin         |
|  - Adds resources & systems  |
+------------------------------+
            |
            v
+------------------------------+
| Resources                    |
|  VizijEngine(Engine)        |
|  BindingIndex               |
|  PendingOutputs             |
|  FixedDt                    |
+------------------------------+
            |
            v
+------------------------------+
| Systems                      |
|  build_binding_index         |
|  prebind_core                |
|  fixed_update_core           |
|  apply_outputs               |
+------------------------------+
            |
            v
+------------------------------+
| Bevy World                   |
|  VizijTargetRoot component   |
|  VizijBindingHint component  |
|  Transform updates           |
+------------------------------+
```

* **Binding index** – Traverses the world under `VizijTargetRoot` to map canonical paths (e.g., `"Arm/Transform.rotation"`) to
  entities/target properties.
* **Prebinding** – Calls into the core engine to resolve canonical strings to handles before the hot update loop.
* **Fixed update** – Uses `FixedDt` (default 1/60s) to advance the engine deterministically. Outputs are staged in
  `PendingOutputs` and applied to `Transform` components.

## Installation

Add the crate to your Bevy project (replace the version with the published release):

```bash
cargo add bevy_vizij_animation
```

Optional feature:

* `urdf_ik` – Enables robotics/inverse kinematics helpers in the core engine. Enabled by default; disable with
  `--no-default-features` if you do not need URDF support.

## Setup

1. **Add the plugin** to your app:
   ```rust
   use bevy::prelude::*;
   use bevy_vizij_animation::VizijAnimationPlugin;

   App::new()
       .add_plugins(DefaultPlugins)
       .add_plugins(VizijAnimationPlugin)
       .run();
   ```
2. **Mark an animation root** using `VizijTargetRoot` and spawn named entities beneath it. The plugin will discover bindings
   automatically.
3. **Load animations** into the `VizijEngine` resource during startup or when assets become available.
4. **(Optional) Adjust fixed timestep** by setting the `FixedDt` resource if you need a cadence other than 1/60s.

## Usage

Common ECS elements exposed by the crate:

* `VizijEngine` – Wrapper around the core `Engine`. Access it via `ResMut<VizijEngine>` to load animations, create players, and
  configure instances.
* `VizijTargetRoot` – Component marking a subtree for automatic binding discovery.
* `VizijBindingHint` – Component to override the canonical path base for an entity (instead of using the `Name` component).
* `BindingIndex` – Resource mapping canonical paths to `(Entity, TargetProp)` pairs. Mostly used internally but available for
  debugging/inspection.
* `PendingOutputs` – Staging area for the most recent outputs. Systems can inspect this if they want to react before the default
  application step runs.

## Key Details

* **Canonical paths** – Defaults to `<EntityName>/Transform.translation|rotation|scale`. Use `VizijBindingHint { path }` to
  override the base for an entity or to create hierarchical names that differ from Bevy’s `Name`.
* **Supported targets** – The plugin currently applies outputs to `Transform` components (translation/rotation/scale). Extend the
  `apply_outputs` system if you need to drive additional components.
* **Scheduling** –
  * `build_binding_index_system` (Update) maintains the binding map when the world changes.
  * `prebind_core_system` (Update) refreshes core bindings when new canonical paths appear.
  * `fixed_update_core_system` (FixedUpdate) advances the engine using the configured timestep.
  * `apply_outputs_system` (FixedUpdate) applies changes to Bevy components.
* **Player/instance control** – Use the inputs API from `vizij-animation-core` (player commands, instance updates) via
  `VizijEngine::update_with_inputs` or by mutating players/instances on the core engine directly.

## Examples

### Minimal Bevy app

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
    let json = include_str!("../../vizij-animation-core/tests/fixtures/new_format.json");
    let stored = parse_stored_animation_json(json).expect("valid animation");

    let anim = eng.0.load_animation(stored);
    let player = eng.0.create_player("demo");
    eng.0.add_instance(player, anim, InstanceCfg::default());
}
```

### Adjusting the fixed timestep

```rust
use bevy::prelude::*;
use bevy_vizij_animation::{VizijAnimationPlugin, FixedDt};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(VizijAnimationPlugin)
        .insert_resource(FixedDt(1.0 / 120.0)) // 120 Hz update
        .run();
}
```

## Testing

Run the crate’s integration tests from the workspace root:

```bash
cargo test -p bevy_vizij_animation
```

Tests construct a Bevy `App`, load StoredAnimation fixtures, and verify that `Transform` components receive the expected values
and that canonical binding hints resolve correctly.

## Troubleshooting

* **No animation applied** – Ensure the animated entity is a descendant of a `VizijTargetRoot` and has either a `Name` component
  or an explicit `VizijBindingHint`.
* **Bindings missing after spawning entities at runtime** – The binding index updates during the `Update` schedule. If you spawn
  entities and expect immediate binding, make sure the systems run in the correct order or trigger a manual rebuild by removing
  and re-adding `VizijTargetRoot`.
* **Mismatch between StoredAnimation targets and Bevy names** – Use `VizijBindingHint { path: "custom/path" }` to match the JSON
  animation target keys.
