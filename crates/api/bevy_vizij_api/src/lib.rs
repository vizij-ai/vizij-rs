//! Bevy application helpers for applying Vizij write batches.
//!
//! The crate intentionally avoids imposing a binding strategy. Instead it provides a shared
//! [`WriterRegistry`] plus convenience helpers for mapping canonical typed paths onto Bevy
//! world mutations. Higher-level adapters register setters and then call [`apply_write_batch`]
//! with the runtime writes they want to project into ECS state.
//!
//! Values arrive as [`vizij_api_core::Value`] (Arora's runtime value). Setters decode them
//! once at this boundary — through the vocabulary accessors in [`vizij_api_core::value`] —
//! into Bevy math types; no dynamic value survives past the decode helpers below.

use bevy::prelude::*;
use std::sync::{Arc, Mutex};

use vizij_api_core::value as vocab;
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

/// Decode a value into a Bevy [`Vec3`].
///
/// Accepts a `vec3` structure, a numeric vector (missing components filled
/// with `fill`), or a scalar float (splatted across all three components).
pub fn as_bevy_vec3(val: &Value, fill: f32) -> Option<Vec3> {
    if let Some(a) = vocab::as_vec3(val) {
        return Some(Vec3::from_array(a));
    }
    if let Some(v) = vocab::as_vector(val) {
        return Some(Vec3::new(
            v.first().copied().unwrap_or(fill),
            v.get(1).copied().unwrap_or(fill),
            v.get(2).copied().unwrap_or(fill),
        ));
    }
    vocab::as_float(val).map(Vec3::splat)
}

/// Decode a value into a Bevy [`Quat`].
///
/// Accepts a `quat` structure or a numeric vector read as `[x, y, z, w]`
/// (missing components default to identity, i.e. `[0, 0, 0, 1]`).
pub fn as_bevy_quat(val: &Value) -> Option<Quat> {
    if let Some(q) = vocab::as_quat(val) {
        return Some(Quat::from_xyzw(q[0], q[1], q[2], q[3]));
    }
    vocab::as_vector(val).map(|v| {
        Quat::from_xyzw(
            v.first().copied().unwrap_or(0.0),
            v.get(1).copied().unwrap_or(0.0),
            v.get(2).copied().unwrap_or(0.0),
            v.get(3).copied().unwrap_or(1.0),
        )
    })
}

/// Decode a `transform` structure into a Bevy [`Transform`].
pub fn as_bevy_transform(val: &Value) -> Option<Transform> {
    let t = vocab::as_transform(val)?;
    Some(Transform {
        translation: Vec3::from_array(t.translation),
        rotation: Quat::from_xyzw(t.rotation[0], t.rotation[1], t.rotation[2], t.rotation[3]),
        scale: Vec3::from_array(t.scale),
    })
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
/// on each invocation and apply the Value. Decoding is intentionally lenient:
/// translation/scale go through [`as_bevy_vec3`] (vec3, vector, or splatted float)
/// and rotation through [`as_bevy_quat`] (quat or vector).
pub fn register_transform_setters_for_entity(
    registry: &mut WriterRegistry,
    base_path: &str,
    entity: Entity,
) {
    fn set_transform(world: &mut World, entity: Entity, apply: impl FnOnce(&mut Transform)) {
        if let Some(mut e) = world.get_entity_mut(entity) {
            if let Some(mut tr) = e.get_mut::<Transform>() {
                apply(&mut tr);
            }
        }
    }

    let base = base_path.to_string();
    // Translation setters: canonical key "base/Transform.translation" plus the
    // "base.translation" alias.
    for key in [
        format!("{base}/Transform.translation"),
        format!("{base}.translation"),
    ] {
        registry.register_setter(key, move |world, _path, val| {
            if let Some(v) = as_bevy_vec3(val, 0.0) {
                set_transform(world, entity, |tr| tr.translation = v);
            }
        });
    }

    // Rotation setters: canonical key "base/Transform.rotation" plus the
    // "base.rotation" alias.
    for key in [
        format!("{base}/Transform.rotation"),
        format!("{base}.rotation"),
    ] {
        registry.register_setter(key, move |world, _path, val| {
            if let Some(q) = as_bevy_quat(val) {
                set_transform(world, entity, |tr| tr.rotation = q);
            }
        });
    }

    // Scale setters: canonical key "base/Transform.scale" plus the
    // "base.scale" alias. Missing vector components fill with 1.0.
    for key in [format!("{base}/Transform.scale"), format!("{base}.scale")] {
        registry.register_setter(key, move |world, _path, val| {
            if let Some(v) = as_bevy_vec3(val, 1.0) {
                set_transform(world, entity, |tr| tr.scale = v);
            }
        });
    }
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
            vocab::vec3([1.0, 2.0, 3.0]),
        ));

        apply_write_batch(&registry, &mut world, &batch);

        // Assert transform updated
        let tr = world.get::<Transform>(entity).unwrap();
        assert_eq!(tr.translation, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn decoders_read_vocabulary_and_fallbacks() {
        // vec3 structure and float splat
        assert_eq!(
            as_bevy_vec3(&vocab::vec3([1.0, 2.0, 3.0]), 0.0),
            Some(Vec3::new(1.0, 2.0, 3.0))
        );
        assert_eq!(
            as_bevy_vec3(&vocab::float(2.0), 0.0),
            Some(Vec3::splat(2.0))
        );
        // short vector fills missing components
        assert_eq!(
            as_bevy_vec3(&vocab::vector(vec![5.0]), 1.0),
            Some(Vec3::new(5.0, 1.0, 1.0))
        );
        // quat structure and short-vector identity fill
        assert_eq!(
            as_bevy_quat(&vocab::quat([0.0, 0.0, 0.0, 1.0])),
            Some(Quat::IDENTITY)
        );
        assert_eq!(as_bevy_quat(&vocab::vector(vec![])), Some(Quat::IDENTITY));
        // transform structure decodes into a Bevy Transform
        let t = vocab::transform(vizij_api_core::Transform {
            translation: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        });
        let decoded = as_bevy_transform(&t).expect("transform");
        assert_eq!(decoded.translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(decoded.rotation, Quat::IDENTITY);
        assert_eq!(decoded.scale, Vec3::ONE);
        // non-decodable values are rejected
        assert_eq!(as_bevy_vec3(&vocab::text("nope"), 0.0), None);
        assert_eq!(as_bevy_quat(&vocab::bool_(true)), None);
        assert_eq!(as_bevy_transform(&vocab::vec3([0.0; 3])), None);
    }
}
