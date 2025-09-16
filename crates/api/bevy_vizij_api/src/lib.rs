// Minimal Bevy adapter: a writer registry and apply sink.
//
// This crate intentionally does not force any automatic binding strategy.
// Instead it provides:
//  - WriterRegistry: a thread-safe map of string path -> setter closure
//  - apply_write_batch: apply a WriteBatch by invoking registered setters
//
// Adapters (the application code or higher-level plugins) can register
// typed setters for specific TypedPath strings (for example, "robot1/Arm/Joint3.translation")
// and provide closures that know how to locate & mutate the appropriate component(s)
// in the Bevy `World` given the TypedPath and Value.
//
// This keeps vizij-api-core engine-agnostic while providing a small, well-scoped
// Bevy integration surface.

use bevy::prelude::*;
use std::sync::{Arc, Mutex};

use vizij_api_core::{TypedPath, Value, WriteBatch};

pub type SetterFn = dyn Fn(&mut World, &TypedPath, &Value) + Send + Sync + 'static;

/// Registry of typed setters keyed by canonical TypedPath string.
/// This implementation uses an Arc<Mutex<...>> so callers can register setters
/// and lookup them at runtime without requiring the boxed setter to be Clone.
#[derive(Resource, Clone, Default)]
pub struct WriterRegistry {
    inner: Arc<Mutex<hashbrown::HashMap<String, Arc<SetterFn>>>>,
}

impl WriterRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        WriterRegistry {
            inner: Arc::new(Mutex::new(hashbrown::HashMap::new())),
        }
    }

    /// Register a setter for a specific canonical path string. If a setter already
    /// exists for that path it will be overwritten.
    pub fn register_setter<F>(&self, path: impl Into<String>, f: F)
    where
        F: Fn(&mut World, &TypedPath, &Value) + Send + Sync + 'static,
    {
        let mut guard = self.inner.lock().unwrap();
        guard.insert(path.into(), Arc::new(f));
    }

    /// Try to get a setter for a canonical path string. Returns a cloned Arc pointer
    /// to the setter if present.
    pub fn get_setter(&self, path: &str) -> Option<Arc<SetterFn>> {
        let guard = self.inner.lock().unwrap();
        guard.get(path).cloned()
    }
}

/// Apply a WriteBatch to the provided Bevy `World` using the given `WriterRegistry`.
/// For every WriteOp, we look up a setter using the WriteOp.path.to_string() key and
/// call it. If no setter is found, the write is ignored.
pub fn apply_write_batch(registry: &WriterRegistry, world: &mut World, batch: &WriteBatch) {
    for op in batch.iter() {
        let key = op.path.to_string();
        if let Some(setter) = registry.get_setter(&key) {
            (setter)(world, &op.path, &op.value);
        }
    }
}

/// Convenience: register simple Transform setters for a specific entity and base path.
/// This helper demonstrates how an application might bind a TypedPath to an entity's
/// Transform components. It registers three setters:
///   "{base_path}.translation"
///   "{base_path}.rotation"
///   "{base_path}.scale"
///
/// The `base_path` should be the canonical prefix, e.g., "robot1/Arm/Joint3".
/// The caller is responsible for ensuring the entity exists and remains valid.
///
/// This helper resolves the entity by the provided Entity value at registration time
/// by capturing the `Entity`. The closure will attempt to get the component mutably
/// on each invocation and apply the Value. The Value coercion rules are intentionally
/// simple: translation/scale accept Vec3 or Vector, rotation accepts Quat or Vector.
pub fn register_transform_setters_for_entity(
    registry: &mut WriterRegistry,
    base_path: &str,
    entity: Entity,
) {
    let base = base_path.to_string();
    // translation setter (match canonical key "base/Transform.translation")
    registry.register_setter(
        format!("{}/Transform.translation", base.clone()),
        move |world, _path, val| {
            if let Some(mut e) = world.get_entity_mut(entity) {
                if let Some(mut tr) = e.get_mut::<Transform>() {
                    match val {
                        Value::Vec3(a) => tr.translation = Vec3::new(a[0], a[1], a[2]),
                        Value::Vector(v) => {
                            tr.translation = Vec3::new(
                                v.first().copied().unwrap_or(0.0),
                                *v.get(1).unwrap_or(&0.0),
                                *v.get(2).unwrap_or(&0.0),
                            );
                        }
                        Value::Float(f) => tr.translation = Vec3::new(*f, *f, *f),
                        _ => {}
                    }
                }
            }
        },
    );
    // Back-compat alias: also register "base.translation"
    registry.register_setter(
        format!("{}.translation", base.clone()),
        move |world, _path, val| {
            if let Some(mut e) = world.get_entity_mut(entity) {
                if let Some(mut tr) = e.get_mut::<Transform>() {
                    match val {
                        Value::Vec3(a) => tr.translation = Vec3::new(a[0], a[1], a[2]),
                        Value::Vector(v) => {
                            tr.translation = Vec3::new(
                                v.first().copied().unwrap_or(0.0),
                                *v.get(1).unwrap_or(&0.0),
                                *v.get(2).unwrap_or(&0.0),
                            );
                        }
                        Value::Float(f) => tr.translation = Vec3::new(*f, *f, *f),
                        _ => {}
                    }
                }
            }
        },
    );

    // rotation setter (match canonical key "base/Transform.rotation")
    registry.register_setter(
        format!("{}/Transform.rotation", base.clone()),
        move |world, _path, val| {
            if let Some(mut e) = world.get_entity_mut(entity) {
                if let Some(mut tr) = e.get_mut::<Transform>() {
                    match val {
                        Value::Quat(q) => {
                            // bevy Quat is (x,y,z,w)
                            tr.rotation = Quat::from_xyzw(q[0], q[1], q[2], q[3]);
                        }
                        Value::Vector(v) => {
                            let x = v.first().copied().unwrap_or(0.0);
                            let y = *v.get(1).unwrap_or(&0.0);
                            let z = *v.get(2).unwrap_or(&0.0);
                            let w = *v.get(3).unwrap_or(&1.0);
                            tr.rotation = Quat::from_xyzw(x, y, z, w);
                        }
                        _ => {}
                    }
                }
            }
        },
    );
    // Back-compat alias: also register "base.rotation"
    registry.register_setter(
        format!("{}.rotation", base.clone()),
        move |world, _path, val| {
            if let Some(mut e) = world.get_entity_mut(entity) {
                if let Some(mut tr) = e.get_mut::<Transform>() {
                    match val {
                        Value::Quat(q) => {
                            tr.rotation = Quat::from_xyzw(q[0], q[1], q[2], q[3]);
                        }
                        Value::Vector(v) => {
                            let x = v.first().copied().unwrap_or(0.0);
                            let y = *v.get(1).unwrap_or(&0.0);
                            let z = *v.get(2).unwrap_or(&0.0);
                            let w = *v.get(3).unwrap_or(&1.0);
                            tr.rotation = Quat::from_xyzw(x, y, z, w);
                        }
                        _ => {}
                    }
                }
            }
        },
    );

    // scale setter (match canonical key "base/Transform.scale")
    registry.register_setter(
        format!("{}/Transform.scale", base.clone()),
        move |world, _path, val| {
            if let Some(mut e) = world.get_entity_mut(entity) {
                if let Some(mut tr) = e.get_mut::<Transform>() {
                    match val {
                        Value::Vec3(a) => tr.scale = Vec3::new(a[0], a[1], a[2]),
                        Value::Vector(v) => {
                            tr.scale = Vec3::new(
                                v.first().copied().unwrap_or(1.0),
                                *v.get(1).unwrap_or(&1.0),
                                *v.get(2).unwrap_or(&1.0),
                            );
                        }
                        Value::Float(f) => tr.scale = Vec3::splat(*f),
                        _ => {}
                    }
                }
            }
        },
    );
    // Back-compat alias: also register "base.scale"
    registry.register_setter(
        format!("{}.scale", base.clone()),
        move |world, _path, val| {
            if let Some(mut e) = world.get_entity_mut(entity) {
                if let Some(mut tr) = e.get_mut::<Transform>() {
                    match val {
                        Value::Vec3(a) => tr.scale = Vec3::new(a[0], a[1], a[2]),
                        Value::Vector(v) => {
                            tr.scale = Vec3::new(
                                v.first().copied().unwrap_or(1.0),
                                *v.get(1).unwrap_or(&1.0),
                                *v.get(2).unwrap_or(&1.0),
                            );
                        }
                        Value::Float(f) => tr.scale = Vec3::splat(*f),
                        _ => {}
                    }
                }
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{Transform, Vec3, World};

    #[test]
    fn registry_and_apply_roundtrip() {
        let mut world = World::new();
        // create an entity with Transform
        let entity = world.spawn(Transform::default()).id();

        let mut registry = WriterRegistry::new();
        register_transform_setters_for_entity(&mut registry, "robot1/Arm/Joint3", entity);

        // Build a WriteBatch to set translation
        let mut batch = WriteBatch::new();
        let path = TypedPath::parse("robot1/Arm/Joint3.translation").unwrap();
        batch.push(vizij_api_core::WriteOp::new(
            path,
            Value::Vec3([1.0, 2.0, 3.0]),
        ));

        apply_write_batch(&registry, &mut world, &batch);

        // Assert transform updated
        let tr = world.get::<Transform>(entity).unwrap();
        assert_eq!(tr.translation, Vec3::new(1.0, 2.0, 3.0));
    }
}
