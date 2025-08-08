use std::collections::HashMap;

use super::path::BevyPath;
use bevy::ecs::world::Mut;
use bevy::prelude::*;
use bevy::reflect::GetPath;
use nalgebra::UnitQuaternion;
use tracing::{debug, warn};

use crate::{
    animation::{AnimationData, BakedAnimationData},
    ecs::{
        components::{AnimationBinding, AnimationInstance, AnimationPlayer},
        resources::{AnimationOutput, EngineTime, FrameBlendData, IdMapping},
    },
    event::AnimationEvent,
    interpolation::InterpolationRegistry,
    player::playback_state::PlaybackState,
    value::{euler::Euler, Color as AnimColor, Transform, Value, Vector2, Vector3, Vector4},
    AnimationTime, PlaybackMode,
};

/// Helper to find a descendant entity by a slash-separated path of names.
fn find_entity_by_path(
    mut current_entity: Entity,
    path_parts: &[&str],
    children_query: &Query<&Children>,
    name_query: &Query<&Name>,
) -> Option<Entity> {
    if path_parts.is_empty() || (path_parts.len() == 1 && path_parts[0].is_empty()) {
        return Some(current_entity);
    }

    for part in path_parts {
        let mut found_child = None;
        if let Ok(children) = children_query.get(current_entity) {
            for child_entity in children {
                if let Ok(name) = name_query.get(*child_entity) {
                    if name.as_str() == *part {
                        found_child = Some(*child_entity);
                        break;
                    }
                }
            }
        }
        if let Some(found) = found_child {
            current_entity = found;
        } else {
            return None; // Path not found
        }
    }
    Some(current_entity)
}

/// Binds new animation instances to their target entities and properties.
pub fn bind_new_animation_instances_system(
    mut commands: Commands,
    new_instances_query: Query<(Entity, &AnimationInstance), Added<AnimationInstance>>,
    player_query: Query<(Entity, &AnimationPlayer)>,
    animations: Res<Assets<AnimationData>>,
    baked_animations: Res<Assets<BakedAnimationData>>,
    children_query: Query<&Children>,
    name_query: Query<&Name>,
) {
    for (instance_entity, instance) in new_instances_query.iter() {
        debug!("bind: new instance {:?} detected", instance_entity);
        let player_opt = player_query.iter().find(|(p_ent, _)| {
            if let Ok(children) = children_query.get(*p_ent) {
                children.contains(&instance_entity)
            } else {
                false
            }
        });
        if let Some((_p_ent, player)) = player_opt {
            if let Some(target_root) = player.target_root {
                if let Some(animation_data) = animations.get(&instance.animation) {
                    let mut raw_bindings = HashMap::new();
                    for track in animation_data.tracks.values() {
                        let target_str = track.target.trim();
                        if target_str.is_empty() {
                            warn!("Track '{}' has empty target; skipping binding", track.id);
                            continue;
                        }

                        let (entity_part_opt, prop_path_str) = match target_str.rsplit_once('/') {
                            Some((entity_part, prop_part)) => (Some(entity_part), prop_part),
                            None => (None, target_str),
                        };

                        if prop_path_str.trim().is_empty() {
                            warn!(
                                "Track '{}' has empty property path in target '{}'",
                                track.id, target_str
                            );
                            continue;
                        }

                        let path = match BevyPath::parse(prop_path_str) {
                            Ok(p) => p,
                            Err(_) => {
                                warn!(
                                    "Failed to parse property path '{}' for track '{}'",
                                    prop_path_str, track.id
                                );
                                continue;
                            }
                        };

                        let entity_path_parts: Vec<&str> = entity_part_opt
                            .unwrap_or_default()
                            .split('/')
                            .filter(|p| !p.is_empty())
                            .collect();

                        if let Some(target_entity) = find_entity_by_path(
                            target_root,
                            &entity_path_parts,
                            &children_query,
                            &name_query,
                        ) {
                            raw_bindings.insert(track.id, (target_entity, path));
                        } else {
                            warn!(
                                "Failed to resolve entity path '{}' for track '{}'",
                                entity_part_opt.unwrap_or_default(),
                                track.id
                            );
                        }
                    }

                    // Build baked track bindings if baked data is available
                    let mut baked_bindings = HashMap::new();
                    if let Some(baked_data) = baked_animations
                        .iter()
                        .find(|(_, b)| b.animation_id == animation_data.id)
                        .map(|(_, b)| b)
                    {
                        for target in baked_data.track_targets() {
                            let target_str = target.trim();
                            if target_str.is_empty() {
                                continue;
                            }
                            let (entity_part_opt, prop_path_str) = match target_str.rsplit_once('/')
                            {
                                Some((entity_part, prop_part)) => (Some(entity_part), prop_part),
                                None => (None, target_str),
                            };

                            if prop_path_str.trim().is_empty() {
                                continue;
                            }

                            let path = match BevyPath::parse(prop_path_str) {
                                Ok(p) => p,
                                Err(_) => {
                                    warn!(
                                        "Failed to parse property path '{}' for baked track '{}'",
                                        prop_path_str, target_str
                                    );
                                    continue;
                                }
                            };

                            let entity_path_parts: Vec<&str> = entity_part_opt
                                .unwrap_or_default()
                                .split('/')
                                .filter(|p| !p.is_empty())
                                .collect();

                            if let Some(target_entity) = find_entity_by_path(
                                target_root,
                                &entity_path_parts,
                                &children_query,
                                &name_query,
                            ) {
                                baked_bindings
                                    .insert(target_str.to_string(), (target_entity, path));
                            } else {
                                warn!(
                                    "Failed to resolve entity path '{}' for baked track '{}'",
                                    entity_part_opt.unwrap_or_default(),
                                    target_str
                                );
                            }
                        }
                    }

                    if raw_bindings.is_empty() && baked_bindings.is_empty() {
                        warn!(
                            "No valid bindings created for instance {:?}; skipping",
                            instance_entity
                        );
                    } else {
                        debug!(
                            "bind: instance {:?} created {} raw, {} baked bindings",
                            instance_entity,
                            raw_bindings.len(),
                            baked_bindings.len()
                        );
                        commands.entity(instance_entity).insert(AnimationBinding {
                            raw_track_bindings: raw_bindings,
                            baked_track_bindings: baked_bindings,
                        });
                    }
                }
            }
        }
    }
}

/// Recalculates the cached duration for animation players when their child instances change.
pub fn update_player_durations_system(
    mut player_query: Query<
        (&Children, &mut AnimationPlayer),
        Or<(Added<Children>, Changed<Children>)>,
    >,
    instance_query: Query<&AnimationInstance>,
    animations: Res<Assets<AnimationData>>,
) {
    for (children, mut player) in player_query.iter_mut() {
        let mut max_duration = AnimationTime::zero();
        for child in children.iter() {
            if let Ok(instance) = instance_query.get(child) {
                if let Some(animation_data) = animations.get(&instance.animation) {
                    let scale = instance.time_scale.abs() as f64;
                    let instance_duration_seconds = if scale > 0.0 {
                        animation_data.duration().as_seconds() / scale
                    } else {
                        0.0
                    };
                    let end_seconds = instance.start_time.as_seconds() + instance_duration_seconds;
                    if end_seconds > max_duration.as_seconds() {
                        max_duration = AnimationTime::from_seconds(end_seconds).unwrap();
                    }
                }
            }
        }
        player.duration = max_duration;
    }
}

/// Updates the timelines of all animation players.
pub fn update_animation_players_system(
    mut player_query: Query<(Entity, &mut AnimationPlayer)>,
    children_query: Query<&Children>,
    instance_query: Query<&AnimationInstance>,
    animations: Res<Assets<AnimationData>>,
    engine_time: Res<EngineTime>,
    mut event_writer: EventWriter<AnimationEvent>,
) {
    for (player_entity, mut player) in player_query.iter_mut() {
        // Opportunistically compute duration if it's zero to avoid timeline stalling
        if player.duration.as_seconds() == 0.0 {
            if let Ok(children) = children_query.get(player_entity) {
                let mut max_duration = AnimationTime::zero();
                for child in children.iter() {
                    if let Ok(instance) = instance_query.get(child) {
                        if let Some(animation_data) = animations.get(&instance.animation) {
                            let scale = instance.time_scale.abs() as f64;
                            let instance_duration_seconds = if scale > 0.0 {
                                animation_data.duration().as_seconds() / scale
                            } else {
                                0.0
                            };
                            let end_seconds =
                                instance.start_time.as_seconds() + instance_duration_seconds;
                            if end_seconds > max_duration.as_seconds() {
                                max_duration = AnimationTime::from_seconds(end_seconds).unwrap();
                            }
                        }
                    }
                }
                player.duration = max_duration;
            }
        }

        if player.playback_state == PlaybackState::Playing {
            let delta = engine_time.delta_seconds * player.speed;
            let duration_secs = player.duration.as_seconds();
            let mut new_time = player.current_time.as_seconds() + delta;
            debug!(
                "update: player='{}' delta={:.6} before={:.6} tentative={:.6} duration={:.6} speed={:.3}",
                player.name,
                delta,
                player.current_time.as_seconds(),
                new_time,
                duration_secs,
                player.speed
            );
            let mut ended = false;

            match player.mode {
                PlaybackMode::Loop => {
                    if duration_secs > 0.0 {
                        new_time = new_time.rem_euclid(duration_secs);
                    } else {
                        new_time = 0.0;
                    }
                }
                PlaybackMode::PingPong => {
                    if duration_secs > 0.0 {
                        while new_time > duration_secs {
                            new_time = duration_secs - (new_time - duration_secs);
                            player.speed = -player.speed;
                        }
                        while new_time < 0.0 {
                            new_time = -new_time;
                            player.speed = -player.speed;
                        }
                    } else {
                        new_time = 0.0;
                    }
                }
                PlaybackMode::Once => {
                    if new_time >= duration_secs {
                        new_time = duration_secs;
                        player.playback_state = PlaybackState::Ended;
                        ended = true;
                    } else if new_time < 0.0 {
                        new_time = 0.0;
                        player.playback_state = PlaybackState::Ended;
                        ended = true;
                    }
                }
            }

            debug!(
                "update: player='{}' final_time={:.6} state={:?}",
                player.name, new_time, player.playback_state
            );
            player.current_time = AnimationTime::from_seconds(new_time).unwrap();

            if ended {
                let timestamp = AnimationTime::from_seconds(engine_time.elapsed_seconds).unwrap();
                if let Ok(children) = children_query.get(player_entity) {
                    for child in children.iter() {
                        if let Ok(instance) = instance_query.get(child) {
                            let animation_id = format!("{:?}", instance.animation);
                            event_writer.write(AnimationEvent::playback_ended(
                                animation_id,
                                player.name.clone(),
                                timestamp,
                                player.current_time,
                            ));
                        }
                    }
                }
            }
        }
    }
}

/// Samples all animations and accumulates the values for blending.
pub fn accumulate_animation_values_system(
    instance_query: Query<(Entity, &AnimationInstance, &AnimationBinding)>,
    player_query: Query<(Entity, &AnimationPlayer)>,
    children_query: Query<&Children>,
    animations: Res<Assets<AnimationData>>,
    baked_animations: Res<Assets<BakedAnimationData>>,
    mut interpolation_registry: ResMut<InterpolationRegistry>,
    mut blend_data: ResMut<FrameBlendData>,
) {
    blend_data.blended_values.clear();

    for (instance_entity, instance, binding) in instance_query.iter() {
        let player_opt = player_query.iter().find(|(p_ent, _)| {
            if let Ok(children) = children_query.get(*p_ent) {
                children.contains(&instance_entity)
            } else {
                false
            }
        });
        if let Some((_p_ent, player)) = player_opt {
            if player.playback_state == PlaybackState::Playing
                || player.playback_state == PlaybackState::Ended
            {
                let instance_time = (player.current_time.as_seconds()
                    - instance.start_time.as_seconds())
                    * instance.time_scale as f64;
                let instance_time = AnimationTime::from_seconds(instance_time.max(0.0)).unwrap();
                debug!(
                    "accumulate: instance {:?} local_time={:.6} weight={:.3}",
                    instance_entity,
                    instance_time.as_seconds(),
                    instance.weight
                );

                if let Some(animation_data) = animations.get(&instance.animation) {
                    for (track_id, (target_entity, path)) in &binding.raw_track_bindings {
                        if let Some(track) = animation_data.tracks.get(track_id) {
                            let transition = animation_data
                                .get_track_transition_for_time(instance_time, &track.id);
                            if let Some(value) = track.value_at_time(
                                instance_time,
                                &mut interpolation_registry,
                                transition,
                                animation_data,
                            ) {
                                blend_data
                                    .blended_values
                                    .entry((*target_entity, path.clone()))
                                    .or_default()
                                    .push((instance.weight, value));
                            }
                        }
                    }

                    if let Some(baked_data) = baked_animations
                        .iter()
                        .find(|(_, b)| b.animation_id == animation_data.id)
                        .map(|(_, b)| b)
                    {
                        for (target_str, (target_entity, path)) in &binding.baked_track_bindings {
                            // Skip if a raw track already targets this entity/path
                            let has_raw = binding
                                .raw_track_bindings
                                .values()
                                .any(|(e, p)| *e == *target_entity && p == path);
                            if has_raw {
                                continue;
                            }
                            if let Some(value) =
                                baked_data.get_value_at_time(target_str, instance_time)
                            {
                                blend_data
                                    .blended_values
                                    .entry((*target_entity, path.clone()))
                                    .or_default()
                                    .push((instance.weight, value.clone()));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Blends the accumulated values and applies them to the target components using reflection.
pub fn blend_and_apply_animation_values_system(world: &mut World) {
    let mut blend_data = world.resource_mut::<FrameBlendData>();
    let blend_data_map = std::mem::take(&mut blend_data.blended_values);
    for ((entity, path), values) in blend_data_map {
        if values.is_empty() {
            continue;
        }

        let total_weight: f32 = values.iter().map(|(w, _)| *w).sum();
        if total_weight == 0.0 {
            continue;
        }

        let value_type = values[0].1.value_type();
        let final_value = match value_type {
            crate::value::ValueType::Transform => {
                let mut final_pos = Vector3::zero();
                let mut final_scale = Vector3::zero();
                let mut final_rot = nalgebra::Quaternion::new(0.0, 0.0, 0.0, 0.0);

                for (weight, value) in &values {
                    if let Value::Transform(t) = value {
                        let w = weight / total_weight;
                        final_pos.x += t.position.x * w as f64;
                        final_pos.y += t.position.y * w as f64;
                        final_pos.z += t.position.z * w as f64;

                        final_scale.x += t.scale.x * w as f64;
                        final_scale.y += t.scale.y * w as f64;
                        final_scale.z += t.scale.z * w as f64;

                        final_rot.coords.x += t.rotation.x * w as f64;
                        final_rot.coords.y += t.rotation.y * w as f64;
                        final_rot.coords.z += t.rotation.z * w as f64;
                        final_rot.coords.w += t.rotation.w * w as f64;
                    }
                }
                let final_rot_unit = UnitQuaternion::new_normalize(final_rot);
                let rot = Vector4::new(
                    final_rot_unit.coords.x,
                    final_rot_unit.coords.y,
                    final_rot_unit.coords.z,
                    final_rot_unit.coords.w,
                );
                Value::Transform(Transform::new(final_pos, rot, final_scale))
            }
            _ => {
                let mut final_components = vec![0.0; values[0].1.interpolatable_components().len()];
                for (weight, value) in &values {
                    let components = value.interpolatable_components();
                    for (i, comp) in components.iter().enumerate() {
                        final_components[i] += comp * (weight / total_weight) as f64;
                    }
                }
                Value::from_components(value_type, &final_components)
                    .unwrap_or_else(|_| values[0].1.clone())
            }
        };

        if let Some(component_name) = path.component() {
            if let Some(mut comp_ref) = reflect_component_mut(world, entity, component_name) {
                let target: Option<&mut dyn Reflect> = if let Some(sub) = path.property() {
                    comp_ref
                        .reflect_path_mut(&sub)
                        .ok()
                        .and_then(|p| p.try_as_reflect_mut())
                } else {
                    Some(&mut *comp_ref)
                };

                if let Some(target) = target {
                    apply_value_to_reflect(target, &final_value);
                    debug!("blend_apply: entity={:?} path={} applied", entity, path);
                }
            }
        }
    }
}

/// Collects the final animated values and populates the `AnimationOutput` resource.
pub fn collect_animation_output_system(world: &mut World) {
    let mut children_query = world.query::<&Children>();
    let mut instance_query = world.query::<(&AnimationInstance, &AnimationBinding)>();
    let players = { world.resource::<IdMapping>().players.clone() };
    let animations = world.resource::<Assets<AnimationData>>();
    let baked_animations = world.resource::<Assets<BakedAnimationData>>();

    let mut new_values = HashMap::new();

    for (player_id, player_entity) in players.iter() {
        let mut player_output = HashMap::new();
        if let Ok(children) = children_query.get(world, *player_entity) {
            for child_entity in children.iter() {
                if let Ok((instance, binding)) = instance_query.get(world, child_entity) {
                    if let Some(anim_data) = animations.get(&instance.animation) {
                        for (track_id, (target_entity, path)) in &binding.raw_track_bindings {
                            if let Some(track) = anim_data.tracks.get(track_id) {
                                if let Some(value) =
                                    get_component_value(world, *target_entity, path)
                                {
                                    player_output.insert(track.target.clone(), value);
                                }
                            }
                        }

                        if let Some(baked_data) = baked_animations
                            .iter()
                            .find(|(_, b)| b.animation_id == anim_data.id)
                            .map(|(_, b)| b)
                        {
                            for (target_str, (target_entity, path)) in &binding.baked_track_bindings
                            {
                                if let Some(value) =
                                    get_component_value(world, *target_entity, path)
                                {
                                    player_output.insert(target_str.clone(), value);
                                }
                            }
                        }
                    }
                }
            }
        }
        new_values.insert(player_id.clone(), player_output);
    }

    let _ = animations;
    let _ = baked_animations;

    let mut output = world.resource_mut::<AnimationOutput>();
    output.values = new_values;
}

fn reflect_component_mut<'a>(
    world: &'a mut World,
    entity: Entity,
    component_name: &str,
) -> Option<Mut<'a, dyn Reflect>> {
    let type_id: std::any::TypeId = if component_name == "Transform" {
        std::any::TypeId::of::<bevy::prelude::Transform>()
    } else {
        let registry = world.resource::<AppTypeRegistry>();
        let reg = registry.read();
        reg.get_with_type_path(component_name)?.type_id()
    };
    world.get_reflect_mut(entity, type_id).ok()
}

fn reflect_component<'a>(
    world: &'a World,
    entity: Entity,
    component_name: &str,
) -> Option<&'a dyn Reflect> {
    let type_id: std::any::TypeId = if component_name == "Transform" {
        std::any::TypeId::of::<bevy::prelude::Transform>()
    } else {
        let registry = world.resource::<AppTypeRegistry>();
        let reg = registry.read();
        reg.get_with_type_path(component_name)?.type_id()
    };
    world.get_reflect(entity, type_id).ok()
}

/// Convert a reflected field to a [`Value`].
fn reflect_to_value(val: &dyn Reflect) -> Option<Value> {
    if let Some(v) = val.downcast_ref::<f32>() {
        Some(Value::Float(*v as f64))
    } else if let Some(v) = val.downcast_ref::<f64>() {
        Some(Value::Float(*v))
    } else if let Some(v) = val.downcast_ref::<i32>() {
        Some(Value::Integer(*v as i64))
    } else if let Some(v) = val.downcast_ref::<i64>() {
        Some(Value::Integer(*v))
    } else if let Some(v) = val.downcast_ref::<bool>() {
        Some(Value::Boolean(*v))
    } else if let Some(v) = val.downcast_ref::<String>() {
        Some(Value::String(v.clone()))
    } else if let Some(v) = val.downcast_ref::<Vec2>() {
        Some(Value::Vector2(Vector2::new(v.x as f64, v.y as f64)))
    } else if let Some(v) = val.downcast_ref::<Vec3>() {
        Some(Value::Vector3(Vector3::new(
            v.x as f64, v.y as f64, v.z as f64,
        )))
    } else if let Some(v) = val.downcast_ref::<Vec4>() {
        Some(Value::Vector4(Vector4::new(
            v.x as f64, v.y as f64, v.z as f64, v.w as f64,
        )))
    } else if let Some(v) = val.downcast_ref::<Quat>() {
        Some(Value::Vector4(Vector4::new(
            v.x as f64, v.y as f64, v.z as f64, v.w as f64,
        )))
    } else if let Some(v) = val.downcast_ref::<bevy::prelude::Transform>() {
        Some(Value::Transform(Transform::new(
            Vector3::new(
                v.translation.x as f64,
                v.translation.y as f64,
                v.translation.z as f64,
            ),
            Vector4::new(
                v.rotation.x as f64,
                v.rotation.y as f64,
                v.rotation.z as f64,
                v.rotation.w as f64,
            ),
            Vector3::new(v.scale.x as f64, v.scale.y as f64, v.scale.z as f64),
        )))
    } else if let Some(v) = val.downcast_ref::<Transform>() {
        Some(Value::Transform(v.clone()))
    } else if let Some(v) = val.downcast_ref::<Vector2>() {
        Some(Value::Vector2(*v))
    } else if let Some(v) = val.downcast_ref::<Vector3>() {
        Some(Value::Vector3(*v))
    } else if let Some(v) = val.downcast_ref::<Vector4>() {
        Some(Value::Vector4(*v))
    } else if let Some(v) = val.downcast_ref::<Euler>() {
        Some(Value::Euler(*v))
    } else if let Some(v) = val.downcast_ref::<AnimColor>() {
        Some(Value::Color(v.clone()))
    } else {
        None
    }
}

fn get_component_value(world: &World, entity: Entity, path: &BevyPath) -> Option<Value> {
    let component_name = path.component()?;
    let comp_ref = reflect_component(world, entity, component_name)?;
    let field = if let Some(sub) = path.property() {
        comp_ref.reflect_path(&sub).ok()?.try_as_reflect()?
    } else {
        comp_ref
    };
    reflect_to_value(field)
}

/// Apply a [`Value`] to a reflected field.
fn apply_value_to_reflect(target: &mut dyn Reflect, value: &Value) {
    match value {
        Value::Float(f) => {
            if let Some(v) = target.downcast_mut::<f32>() {
                *v = *f as f32;
            } else if let Some(v) = target.downcast_mut::<f64>() {
                *v = *f;
            }
        }
        Value::Integer(i) => {
            if let Some(v) = target.downcast_mut::<i32>() {
                *v = *i as i32;
            } else if let Some(v) = target.downcast_mut::<i64>() {
                *v = *i;
            }
        }
        Value::Boolean(b) => {
            if let Some(v) = target.downcast_mut::<bool>() {
                *v = *b;
            }
        }
        Value::String(s) => {
            if let Some(v) = target.downcast_mut::<String>() {
                *v = s.clone();
            }
        }
        Value::Vector2(v2) => {
            if let Some(v) = target.downcast_mut::<Vec2>() {
                *v = Vec2::new(v2.x as f32, v2.y as f32);
            } else if let Some(v) = target.downcast_mut::<Vector2>() {
                *v = *v2;
            }
        }
        Value::Vector3(v3) => {
            if let Some(v) = target.downcast_mut::<Vec3>() {
                *v = Vec3::new(v3.x as f32, v3.y as f32, v3.z as f32);
            } else if let Some(v) = target.downcast_mut::<Vector3>() {
                *v = *v3;
            }
        }
        Value::Vector4(v4) => {
            if let Some(v) = target.downcast_mut::<Vec4>() {
                *v = Vec4::new(v4.x as f32, v4.y as f32, v4.z as f32, v4.w as f32);
            } else if let Some(v) = target.downcast_mut::<Quat>() {
                *v = Quat::from_xyzw(v4.x as f32, v4.y as f32, v4.z as f32, v4.w as f32);
            } else if let Some(v) = target.downcast_mut::<Vector4>() {
                *v = *v4;
            }
        }
        Value::Euler(e) => {
            if let Some(v) = target.downcast_mut::<Euler>() {
                *v = *e;
            }
        }
        Value::Color(c) => {
            if let Some(v) = target.downcast_mut::<AnimColor>() {
                *v = c.clone();
            }
        }
        Value::Transform(t) => {
            if let Some(v) = target.downcast_mut::<Transform>() {
                *v = t.clone();
            } else if let Some(v) = target.downcast_mut::<bevy::prelude::Transform>() {
                *v = bevy::prelude::Transform {
                    translation: Vec3::new(
                        t.position.x as f32,
                        t.position.y as f32,
                        t.position.z as f32,
                    ),
                    rotation: Quat::from_xyzw(
                        t.rotation.x as f32,
                        t.rotation.y as f32,
                        t.rotation.z as f32,
                        t.rotation.w as f32,
                    ),
                    scale: Vec3::new(t.scale.x as f32, t.scale.y as f32, t.scale.z as f32),
                };
            }
        }
    }
}

/// Removes ID mappings for entities that have been despawned.
pub fn cleanup_id_mapping_on_despawned_system(
    mut removed_players: RemovedComponents<AnimationPlayer>,
    mut removed_instances: RemovedComponents<AnimationInstance>,
    mut id_mapping: ResMut<IdMapping>,
) {
    for entity in removed_players.read() {
        id_mapping.players.retain(|_, e| *e != entity);
    }
    for entity in removed_instances.read() {
        id_mapping.instances.retain(|_, e| *e != entity);
    }
}
