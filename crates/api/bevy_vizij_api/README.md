# bevy_vizij_api

> **Bevy utilities for applying Vizij `WriteBatch` updates using canonical TypedPaths.**

`bevy_vizij_api` bridges the shared contracts from `vizij-api-core` into the Bevy ECS. It provides a registry of setter callbacks keyed by `TypedPath` and helpers that map common paths (e.g., transforms) onto Bevy components. Higher-level plugins such as `bevy_vizij_animation` build on this crate.

---

## Table of Contents

1. [Overview](#overview)
2. [Installation](#installation)
3. [Quick Start](#quick-start)
4. [Key Concepts](#key-concepts)
5. [Development & Testing](#development--testing)
6. [Related Crates](#related-crates)

---

## Overview

- `WriterRegistry` resource storing path → setter registrations.
- `apply_write_batch` helper that iterates a `WriteBatch` and invokes matching setters.
- Convenience functions for common bindings (e.g., `register_transform_setters_for_entity`).
- Thread-safe design (`Arc<Mutex<_>>`) so the registry can be cloned and used in parallel schedules.

---

## Installation

```bash
cargo add bevy_vizij_api
```

The crate targets the same Bevy version as the rest of the Vizij stack; check the workspace lockfile for the current requirement.

---

## Quick Start

```rust
use bevy::prelude::*;
use bevy_vizij_api::{
    apply_write_batch,
    register_transform_setters_for_entity,
    WriterRegistry,
};
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

---

## Key Concepts

### WriterRegistry

- Internally stores setters in an `Arc<Mutex<Vec<(TypedPath, Setter)>>`.
- Setters have the signature `Fn(&mut World, &TypedPath, &Value) + Send + Sync + 'static`.
- Cloneable, allowing registration during setup and use during update systems without borrowing conflicts.

### Applying Batches

- `apply_write_batch(&registry, world, &batch)` looks up each `WriteOp.path` and invokes registered setters.
- Unregistered paths are ignored, making it safe to opt-in incrementally.
- Shapes are available on each `WriteOp` if you need metadata during application.

### Helpers

- `register_transform_setters_for_entity(registry, base_path, entity)` registers translation/rotation/scale setters for `Transform`.
- Additional helpers can be added by downstream crates—`bevy_vizij_animation` registers animation-specific bindings using this API.

---

## Development & Testing

```bash
cargo test -p bevy_vizij_api
```

The test suite covers registry registration, TRS helpers, and batch application.

---

## Related Crates

- [`vizij-api-core`](../vizij-api-core/README.md) – Shared Value/Shape/TypedPath data model.
- [`bevy_vizij_animation`](../../animation/bevy_vizij_animation/README.md) – Animation plugin built on top of this API.
- [`vizij-orchestrator-core`](../../orchestrator/vizij-orchestrator-core/README.md) – Produces write batches that can be applied via this registry.

Spot a gap? File an issue—clean application layers make Vizij engines easy to embed in Bevy. 🧩
