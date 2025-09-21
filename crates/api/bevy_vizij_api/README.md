# bevy_vizij_api

`bevy_vizij_api` is a lightweight bridge between Vizij's engine-agnostic `vizij-api-core` contracts and the Bevy ECS. It ships a
thread-safe registry of writer callbacks plus helpers for applying write batches inside a Bevy `World`. Higher-level plugins (e.g.
`bevy_vizij_animation`, `bevy_vizij_graph`) build atop this crate to apply animation or graph outputs to game entities.

## Overview

* Provides a `WriterRegistry` Bevy resource that maps canonical `TypedPath` strings to setter closures.
* Exposes `apply_write_batch` to walk a `WriteBatch` and invoke registered setters against the Bevy `World`.
* Includes `register_transform_setters_for_entity` as a ready-made example for mapping TRS values onto `Transform` components.
* Leaves binding strategy up to the host or higher-level plugins—nothing in this crate assumes a particular scene graph layout.

## Architecture

```
+-----------------+        +----------------+        +----------------+
| WriteBatch      | -----> | WriterRegistry | -----> | Bevy World     |
| (vizij-api-core)|        | (path -> fn)   |        | (components)   |
+-----------------+        +----------------+        +----------------+
```

1. Engines emit `WriteBatch` values (path/value/shape triplets).
2. Application code or plugins register setters on the `WriterRegistry` for each canonical path they care about.
3. `apply_write_batch` looks up setters and mutates Bevy entities/components accordingly.

## Installation

Add the crate to any Bevy app that consumes Vizij engine outputs:

```bash
cargo add bevy_vizij_api
```

## Setup

1. Insert `WriterRegistry::default()` (or `WriterRegistry::new()`) as a Bevy resource.
2. Register setters for the canonical paths you expect your engines to emit.
3. When a `WriteBatch` arrives, call `apply_write_batch(&registry, world, &batch)` inside a system or schedule stage.

## Usage

```rust
use bevy::prelude::*;
use bevy_vizij_api::{apply_write_batch, register_transform_setters_for_entity, WriterRegistry};
use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<WriterRegistry>()
        .add_systems(Startup, setup_registry)
        .add_systems(Update, drive_transforms)
        .run();
}

fn setup_registry(mut commands: Commands, mut registry: ResMut<WriterRegistry>) {
    let cube = commands
        .spawn((Name::new("cube"), Transform::default(), GlobalTransform::default()))
        .id();

    register_transform_setters_for_entity(&mut registry, "robot/cube", cube);
}

fn drive_transforms(mut world: World) {
    let registry = world.resource::<WriterRegistry>().clone();

    let mut batch = WriteBatch::new();
    let path = TypedPath::parse("robot/cube/Transform.translation").unwrap();
    batch.push(WriteOp::new_with_shape(path, Value::vec3(0.0, 1.0, 0.0), None));

    apply_write_batch(&registry, &mut world, &batch);
}
```

## Key Details

* `WriterRegistry` stores setters in an `Arc<Mutex<...>>`, making it cheap to clone and share across systems/threads.
* Setters receive `(&mut World, &TypedPath, &Value)` and are free to query additional components or resources.
* Helpers register both canonical TRS keys (`foo/Transform.translation`) and back-compat aliases (`foo.translation`).
* Writes with no registered setter are ignored—this lets hosts opt-in path-by-path without panicking.

## Testing

Run the crate's tests to validate the helper behaviour:

```bash
cargo test -p bevy_vizij_api
```
