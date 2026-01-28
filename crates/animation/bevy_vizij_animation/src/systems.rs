//! ECS systems for binding, stepping, and applying Vizij animation outputs.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::components::{VizijBindingHint, VizijTargetRoot};
use crate::resources::{BindingIndex, FixedDt, PendingOutputs, TargetProp};
use crate::VizijEngine;
use vizij_animation_core::{inputs::Inputs, outputs::Change, TargetResolver};

/// Build a canonical handle for an entity name + transform property.
///
/// Handles follow the shape `{base}/Transform.<prop>`.
fn make_handle(name: &str, prop: TargetProp) -> String {
    match prop {
        TargetProp::Translation => format!("{name}/Transform.translation"),
        TargetProp::Rotation => format!("{name}/Transform.rotation"),
        TargetProp::Scale => format!("{name}/Transform.scale"),
    }
}

/// Walk descendants under each `VizijTargetRoot` and rebuild the `BindingIndex`.
///
/// Entities without a `Name` component are skipped. When a `VizijBindingHint` is present,
/// its `path` replaces the `Name` for the canonical handle prefix.
///
/// This system replaces the entire index each run rather than incrementally updating it.
/// Run it in `Update` (default) or add change detection if you need to reduce per-frame work.
pub fn build_binding_index_system(
    roots: Query<Entity, With<VizijTargetRoot>>,
    children: Query<&Children>,
    names: Query<(&Name, Option<&VizijBindingHint>)>,
    mut index: ResMut<BindingIndex>,
) {
    let mut map: HashMap<String, (Entity, TargetProp)> = HashMap::new();

    // Depth-first traversal from each root
    /// Internal helper for `walk` in Bevy animation systems.
    fn walk(
        e: Entity,
        map: &mut HashMap<String, (Entity, TargetProp)>,
        names: &Query<(&Name, Option<&VizijBindingHint>)>,
        children: &Query<&Children>,
    ) {
        if let Ok((name, hint)) = names.get(e) {
            let base = hint
                .map(|h| h.path.clone())
                .unwrap_or_else(|| name.as_str().to_string());
            // Register three canonical transform handles
            map.insert(
                make_handle(&base, TargetProp::Translation),
                (e, TargetProp::Translation),
            );
            map.insert(
                make_handle(&base, TargetProp::Rotation),
                (e, TargetProp::Rotation),
            );
            map.insert(
                make_handle(&base, TargetProp::Scale),
                (e, TargetProp::Scale),
            );
        }
        if let Ok(cs) = children.get(e) {
            for &c in cs.iter() {
                walk(c, map, names, children);
            }
        }
    }

    for root in roots.iter() {
        walk(root, &mut map, &names, &children);
    }

    index.map = map;
}

/// Bridge the core prebind call into the ECS.
///
/// Resolves canonical track target paths to string handles recorded in `BindingIndex`.
/// The handles are the canonical strings themselves, keeping the Bevy adapter aligned
/// with core output keys.
///
/// If a `WriterRegistry` resource is present, this registers transform setters for each
/// bound handle so `apply_outputs_system` can drive `Transform` updates through the
/// typed write path.
///
/// # Examples
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_vizij_animation::{systems, VizijAnimationPlugin};
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(VizijAnimationPlugin)
///     .add_systems(Update, systems::prebind_core_system)
///     .run();
/// ```
pub fn prebind_core_system(
    mut eng: ResMut<VizijEngine>,
    index: Res<BindingIndex>,
    mut registry: Option<ResMut<bevy_vizij_api::WriterRegistry>>,
) {
    struct Resolver<'a> {
        idx: &'a BindingIndex,
    }
    impl<'a> TargetResolver for Resolver<'a> {
        /// Internal helper for `resolve` in Bevy animation systems.
        fn resolve(&mut self, path: &str) -> Option<String> {
            if self.idx.map.contains_key(path) {
                Some(path.to_string())
            } else {
                None
            }
        }
    }
    let mut resolver = Resolver { idx: &index };
    eng.0.prebind(&mut resolver);

    // If a WriterRegistry is present, register simple Transform setters for each bound handle.
    // This allows external code to apply WriteBatches using the same canonical paths.
    if let Some(reg) = registry.as_deref_mut() {
        // For each binding handle -> (Entity, TargetProp), map to a base path and register setters.
        // Handles are of the form "{base}/Transform.translation" etc. Extract base by trimming suffix.
        for (handle, (entity, _prop)) in index.map.iter() {
            // Determine suffix and base
            const TRANSLATION_SUFFIX: &str = "/Transform.translation";
            const ROTATION_SUFFIX: &str = "/Transform.rotation";
            const SCALE_SUFFIX: &str = "/Transform.scale";

            if handle.ends_with(TRANSLATION_SUFFIX) {
                if let Some(base) = handle.strip_suffix(TRANSLATION_SUFFIX) {
                    bevy_vizij_api::register_transform_setters_for_entity(reg, base, *entity);
                }
            } else if handle.ends_with(ROTATION_SUFFIX) {
                if let Some(base) = handle.strip_suffix(ROTATION_SUFFIX) {
                    bevy_vizij_api::register_transform_setters_for_entity(reg, base, *entity);
                }
            } else if handle.ends_with(SCALE_SUFFIX) {
                if let Some(base) = handle.strip_suffix(SCALE_SUFFIX) {
                    bevy_vizij_api::register_transform_setters_for_entity(reg, base, *entity);
                }
            } else {
                // For non-transform handles we could register generic setters in the future.
            }
        }
    }
}

/// Fixed timestep compute: call core update with fixed dt and stash changes in `PendingOutputs`.
///
/// Inputs are left empty for v1; production apps should derive `Inputs` from gameplay state.
pub fn fixed_update_core_system(
    mut eng: ResMut<VizijEngine>,
    dt: Res<FixedDt>,
    mut pending: ResMut<PendingOutputs>,
) {
    let out = eng.0.update_values(dt.0, Inputs::default());
    // Replace pending changes with this tick's changes
    pending.changes.clear();
    pending.changes.extend(out.changes.iter().cloned());
}

/// Apply staged outputs by converting them to a typed `WriteBatch` and invoking
/// the bevy_vizij_api writer registry when available.
///
/// Writes whose keys do not parse as `TypedPath` are applied via the `BindingIndex`
/// fallback (Transform-only). When no registry is present, typed writes also fall back.
///
/// Returns early if the `BindingIndex` or `PendingOutputs` resources are missing.
pub fn apply_outputs_system(world: &mut World) {
    // Access required resources into locals to avoid borrow conflicts
    let index_map = if let Some(idx) = world.get_resource::<BindingIndex>() {
        idx.map.clone()
    } else {
        return;
    };
    let changes: Vec<Change> = {
        if let Some(mut pending) = world.get_resource_mut::<PendingOutputs>() {
            std::mem::take(&mut pending.changes)
        } else {
            return;
        }
    };

    // Build WriteBatch from pending changes (skip writes that don't parse as TypedPath)
    let mut batch = vizij_api_core::WriteBatch::new();
    let mut non_typed: Vec<(String, vizij_api_core::Value)> = Vec::new();

    for Change { key, value, .. } in changes.into_iter() {
        match vizij_api_core::TypedPath::parse(&key) {
            Ok(tp) => batch.push(vizij_api_core::WriteOp::new(tp, value)),
            Err(_) => non_typed.push((key, value)),
        }
    }

    // If we have a WriterRegistry, apply via the registry which will invoke registered setters.
    if world.contains_resource::<bevy_vizij_api::WriterRegistry>() {
        // Apply typed writes
        world.resource_scope(|world, reg: Mut<bevy_vizij_api::WriterRegistry>| {
            bevy_vizij_api::apply_write_batch(&reg, world, &batch);
        });

        // Apply any non-typed writes via fallback (lookup in BindingIndex)
        let mut q_tf = world.query::<&mut Transform>();
        for (path_str, val) in non_typed.iter() {
            if let Some((entity, prop)) = index_map.get(path_str) {
                if let Ok(mut tf) = q_tf.get_mut(world, *entity) {
                    match (prop, val) {
                        (
                            TargetProp::Translation,
                            vizij_api_core::Value::Transform {
                                translation: pos, ..
                            },
                        ) => {
                            tf.translation = Vec3::new(pos[0], pos[1], pos[2]);
                        }
                        (
                            TargetProp::Rotation,
                            vizij_api_core::Value::Transform { rotation: rot, .. },
                        ) => {
                            tf.rotation =
                                Quat::from_xyzw(rot[0], rot[1], rot[2], rot[3]).normalize();
                        }
                        (TargetProp::Scale, vizij_api_core::Value::Transform { scale, .. }) => {
                            tf.scale = Vec3::new(scale[0], scale[1], scale[2]);
                        }
                        (TargetProp::Translation, vizij_api_core::Value::Vec3(v)) => {
                            tf.translation = Vec3::new(v[0], v[1], v[2]);
                        }
                        (TargetProp::Rotation, vizij_api_core::Value::Quat(q)) => {
                            tf.rotation = Quat::from_xyzw(q[0], q[1], q[2], q[3]).normalize();
                        }
                        (TargetProp::Scale, vizij_api_core::Value::Vec3(v)) => {
                            tf.scale = Vec3::new(v[0], v[1], v[2]);
                        }
                        _ => {}
                    }
                }
            }
        }
        return;
    }

    // No registry: fallback to applying any WriteBatch items that map to binding handles,
    // plus non-typed writes as before.
    let mut q_tf = world.query::<&mut Transform>();

    for op in batch.iter() {
        let path_str = op.path.to_string();
        if let Some((entity, prop)) = index_map.get(&path_str) {
            if let Ok(mut tf) = q_tf.get_mut(world, *entity) {
                match (&prop, &op.value) {
                    (
                        TargetProp::Translation,
                        vizij_api_core::Value::Transform { translation, .. },
                    ) => {
                        tf.translation = Vec3::new(translation[0], translation[1], translation[2]);
                    }
                    (TargetProp::Rotation, vizij_api_core::Value::Transform { rotation, .. }) => {
                        tf.rotation =
                            Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3])
                                .normalize();
                    }
                    (TargetProp::Scale, vizij_api_core::Value::Transform { scale, .. }) => {
                        tf.scale = Vec3::new(scale[0], scale[1], scale[2]);
                    }
                    (TargetProp::Translation, vizij_api_core::Value::Vec3(v)) => {
                        tf.translation = Vec3::new(v[0], v[1], v[2]);
                    }
                    (TargetProp::Rotation, vizij_api_core::Value::Quat(q)) => {
                        tf.rotation = Quat::from_xyzw(q[0], q[1], q[2], q[3]).normalize();
                    }
                    (TargetProp::Scale, vizij_api_core::Value::Vec3(v)) => {
                        tf.scale = Vec3::new(v[0], v[1], v[2]);
                    }
                    _ => {}
                }
            }
        }
    }

    // Apply non-typed writes similarly
    for (path_str, val) in non_typed.iter() {
        if let Some((entity, prop)) = index_map.get(path_str) {
            if let Ok(mut tf) = q_tf.get_mut(world, *entity) {
                match (prop, val) {
                    (
                        TargetProp::Translation,
                        vizij_api_core::Value::Transform { translation, .. },
                    ) => {
                        tf.translation = Vec3::new(translation[0], translation[1], translation[2]);
                    }
                    (TargetProp::Rotation, vizij_api_core::Value::Transform { rotation, .. }) => {
                        tf.rotation =
                            Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3])
                                .normalize();
                    }
                    (TargetProp::Scale, vizij_api_core::Value::Transform { scale, .. }) => {
                        tf.scale = Vec3::new(scale[0], scale[1], scale[2]);
                    }
                    (TargetProp::Translation, vizij_api_core::Value::Vec3(v)) => {
                        tf.translation = Vec3::new(v[0], v[1], v[2]);
                    }
                    (TargetProp::Rotation, vizij_api_core::Value::Quat(q)) => {
                        tf.rotation = Quat::from_xyzw(q[0], q[1], q[2], q[3]).normalize();
                    }
                    (TargetProp::Scale, vizij_api_core::Value::Vec3(v)) => {
                        tf.scale = Vec3::new(v[0], v[1], v[2]);
                    }
                    _ => {}
                }
            }
        }
    }
}
