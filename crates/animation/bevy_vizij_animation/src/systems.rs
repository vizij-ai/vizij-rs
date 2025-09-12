use bevy::prelude::*;
use std::collections::HashMap;

use crate::components::{VizijBindingHint, VizijTargetRoot};
use crate::resources::{BindingIndex, FixedDt, PendingOutputs, TargetProp};
use crate::VizijEngine;
use vizij_animation_core::{inputs::Inputs, outputs::Change, TargetResolver};

/// Internal: build a canonical path for an entity from its Name and the requested transform prop.
fn make_handle(name: &str, prop: TargetProp) -> String {
    match prop {
        TargetProp::Translation => format!("{name}/Transform.translation"),
        TargetProp::Rotation => format!("{name}/Transform.rotation"),
        TargetProp::Scale => format!("{name}/Transform.scale"),
    }
}

/// Walks descendants under each VizijTargetRoot and populates the BindingIndex resource
/// mapping canonical handles to (Entity, TargetProp).
pub fn build_binding_index_system(
    roots: Query<Entity, With<VizijTargetRoot>>,
    children: Query<&Children>,
    names: Query<(&Name, Option<&VizijBindingHint>)>,
    mut index: ResMut<BindingIndex>,
) {
    let mut map: HashMap<String, (Entity, TargetProp)> = HashMap::new();

    // Depth-first traversal from each root
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

/// Bridges the core prebind call into the ECS: resolves canonical track target paths
/// to string handles recorded in BindingIndex. We use the same string as the handle.
pub fn prebind_core_system(mut eng: ResMut<VizijEngine>, index: Res<BindingIndex>) {
    struct Resolver<'a> {
        idx: &'a BindingIndex,
    }
    impl<'a> TargetResolver for Resolver<'a> {
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
}

/// Fixed timestep compute: call core update with fixed dt and stash Changes into PendingOutputs.
/// Inputs are left empty for v1; production apps will derive Inputs from gameplay state.
pub fn fixed_update_core_system(
    mut eng: ResMut<VizijEngine>,
    dt: Res<FixedDt>,
    mut pending: ResMut<PendingOutputs>,
) {
    let out = eng.0.update(dt.0, Inputs::default());
    // Replace pending changes with this tick's changes
    pending.changes.clear();
    pending.changes.extend(out.changes.iter().cloned());
}

/// Apply staged outputs back to ECS Transform components.
pub fn apply_outputs_system(
    index: Res<BindingIndex>,
    mut transforms: Query<&mut Transform>,
    mut pending: ResMut<PendingOutputs>,
) {
    for Change { key, value, .. } in pending.changes.drain(..) {
        if let Some((entity, prop)) = index.map.get(&key) {
            if let Ok(mut tf) = transforms.get_mut(*entity) {
                match (prop, &value) {
                    (
                        TargetProp::Translation,
                        vizij_animation_core::Value::Transform { translation, .. },
                    ) => {
                        tf.translation = Vec3::new(translation[0], translation[1], translation[2]);
                    }
                    (
                        TargetProp::Rotation,
                        vizij_animation_core::Value::Transform { rotation, .. },
                    ) => {
                        tf.rotation =
                            Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3])
                                .normalize();
                    }
                    (TargetProp::Scale, vizij_animation_core::Value::Transform { scale, .. }) => {
                        tf.scale = Vec3::new(scale[0], scale[1], scale[2]);
                    }
                    // Convenience: accept direct Vec3/Quat values too
                    (TargetProp::Translation, vizij_animation_core::Value::Vec3(v)) => {
                        tf.translation = Vec3::new(v[0], v[1], v[2]);
                    }
                    (TargetProp::Rotation, vizij_animation_core::Value::Quat(q)) => {
                        tf.rotation = Quat::from_xyzw(q[0], q[1], q[2], q[3]).normalize();
                    }
                    (TargetProp::Scale, vizij_animation_core::Value::Vec3(v)) => {
                        tf.scale = Vec3::new(v[0], v[1], v[2]);
                    }
                    // Unhandled kinds: ignore (fail-soft)
                    _ => {}
                }
            }
        }
    }
}
