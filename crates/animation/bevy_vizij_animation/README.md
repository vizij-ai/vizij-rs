# bevy_vizij_animation

Bevy plugin that adapts `vizij-animation-core` for ECS-based applications. It wires the engine into Bevy schedules, builds a binding index from your scene, prebinds engine targets, runs fixed-timestep updates, and applies evaluated outputs back onto Bevy `Transform`s.

This crate does not own rendering or scene graph logic beyond binding and application. It is a thin adapter over the core crate.

## Features

- Bevy `Plugin` that:
  - Inserts a shared `Engine` resource (from `vizij-animation-core`)
  - Builds a `BindingIndex` of canonical target paths to ECS entities
  - Prebinds the core engine to avoid per-frame string lookups
  - Runs fixed-timestep updates and applies outputs to `Transform`
- Deterministic fixed update via `FixedUpdate` schedule
- Works with the new StoredAnimation JSON schema (via core’s parser)

## Concepts

- Canonical Target Path
  - String key describing where a track writes. For transforms:
    - `"<name>/Transform.translation"`
    - `"<name>/Transform.rotation"`
    - `"<name>/Transform.scale"`
  - Derived from entity `Name`, optionally overridden with `VizijBindingHint.path`.

- Binding Index
  - `BindingIndex` resource maps canonical target paths to `(Entity, TargetProp)`.
  - Core prebind uses this to resolve track targets to string handles.

- Pending Outputs
  - Staging area (`PendingOutputs`) for `Change`s emitted by the core engine each frame, applied in a follow-up system.

## API Surface

Public types exported from this crate:

- Plugin:
  - `VizijAnimationPlugin`

- Resources:
  - `VizijEngine(Engine)` — wrapper around core `Engine`
  - `BindingIndex` — map of `String -> (Entity, TargetProp)`
  - `PendingOutputs` — staging buffer for core output `Change`s
  - `FixedDt(f32)` — fixed timestep size in seconds (defaults to `1.0/60.0`)

- Components:
  - `VizijTargetRoot` — marks a root entity under which canonical bindings will be collected
  - `VizijBindingHint { path: String }` — optional override for the canonical path base

- Systems (registered for you by the plugin):
  - `build_binding_index_system` — builds/updates `BindingIndex`
  - `prebind_core_system` — resolves canonical target paths to string handles in the core
  - `fixed_update_core_system` — steps the engine using `FixedDt`
  - `apply_outputs_system` — applies `Change` values to `Transform`s

## How to Use

Add the plugin:

```rust
use bevy::prelude::*;
use bevy_vizij_animation::VizijAnimationPlugin;

fn main() {
  App::new()
    .add_plugins(DefaultPlugins)
    .add_plugins(VizijAnimationPlugin)
    .run();
}
```

Create a scene and a root:

```rust
use bevy::prelude::*;
use bevy_vizij_animation::{VizijTargetRoot, VizijEngine};

fn setup(mut commands: Commands) {
  // Root under which bindings will be discovered
  let root = commands.spawn(VizijTargetRoot).id();

  // Named entity to bind
  let node = commands
    .spawn((
      Name::new("node"),
      Transform::default(),
      GlobalTransform::default(),
    ))
    .id();

  // Ensure `node` is a descendant of the root
  commands.entity(root).add_child(node);
}
```

Load an animation and add an instance:

```rust
use bevy::prelude::*;
use bevy_vizij_animation::VizijEngine;
use vizij_animation_core::{parse_stored_animation_json, InstanceCfg};

fn load(mut eng: ResMut<VizijEngine>) {
  let json_str = std::fs::read_to_string("tests/fixtures/new_format.json").unwrap();
  let anim = parse_stored_animation_json(&json_str).expect("parse stored animation");

  let aid = eng.0.load_animation(anim);
  let pid = eng.0.create_player("demo");
  let _iid = eng.0.add_instance(pid, aid, InstanceCfg::default());
}
```

The plugin will:
- In `Update`: build the binding index and prebind the core engine
- In `FixedUpdate`: call `Engine::update` using `FixedDt` and apply outputs to `Transform`

## Canonical Target Paths

By default, canonical paths for a named entity `myNode`:

- `myNode/Transform.translation`
- `myNode/Transform.rotation`
- `myNode/Transform.scale`

You can override the base with `VizijBindingHint { path }` attached to an entity. The binder will register paths under the hint rather than the name.

## Integration Notes

- Transition model: cubic-bezier per segment using per-keypoint `transitions.out`/`transitions.in` with defaults. Linear and legacy cubic normalize to bezier shapes in the sampler.
- Values:
  - Numeric-like (Scalar/Vec2/Vec3/Vec4/Color): component-wise linear interpolation under eased t
  - Quat: shortest-arc NLERP + normalize
  - Transform: TRS decomposition (translate/scale linear, rotation NLERP)
  - Bool/Text: step semantics
- Mixed kinds on the same entity path are ignored (fail-soft).

## Testing

This crate provides integration tests that:
- Build a Bevy `App` with `VizijAnimationPlugin`
- Load animations and verify `Transform` properties are applied
- Include a StoredAnimation-based test to validate parsing and binding

Run tests:

```bash
cargo test -p bevy_vizij_animation
```

## License

See the workspace root for licensing details.
