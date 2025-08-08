use std::collections::HashMap;

use bevy::prelude::*;
use bevy::reflect::GetPath;
use tracing::warn;
use bevy_ecs::hierarchy::ChildOf as Parent;
use super::path::BevyPath;
use nalgebra::UnitQuaternion;

use crate::{
    animation::AnimationData,
    ecs::{
        components::{AnimationBinding, AnimationInstance, AnimationPlayer},
        resources::{AnimationOutput, FrameBlendData, IdMapping},
    },
    event::AnimationEvent,
    interpolation::InterpolationRegistry,
    player::playback_state::PlaybackState,
    value::{Transform, Value, Vector3, Vector4},
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
    new_instances_query: Query<(Entity, &Parent, &AnimationInstance), Added<AnimationInstance>>,
    player_query: Query<&AnimationPlayer>,
    animations: Res<Assets<AnimationData>>,
    children_query: Query<&Children>,
    name_query: Query<&Name>,
) {
    for (instance_entity, parent, instance) in new_instances_query.iter() {
        if let Ok(player) = player_query.get(parent.parent()) {
            if let Some(target_root) = player.target_root {
                if let Some(animation_data) = animations.get(&instance.animation) {
                    let mut bindings = HashMap::new();
                    for track in animation_data.tracks.values() {
                        let target_str = track.target.trim();
                        if target_str.is_empty() {
                            warn!(
                                "Track '{}' has empty target; skipping binding",
                                track.id
                            );
                            continue;
                        }

                        let (entity_part_opt, prop_path_str) =
                            match target_str.rsplit_once('/') {
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
                            bindings.insert(track.id, (target_entity, path));
                        } else {
                            warn!(
                                "Failed to resolve entity path '{}' for track '{}'",
                                entity_part_opt.unwrap_or_default(),
                                track.id
                            );
                        }
                    }

                    if bindings.is_empty() {
                        warn!(
                            "No valid bindings created for instance {:?}; skipping",
                            instance_entity
                        );
                    } else {
                        commands
                            .entity(instance_entity)
                            .insert(AnimationBinding { bindings });
                    }
                }
            }
        }
    }
}

/// Updates the timelines of all animation players.
pub fn update_animation_players_system(
    mut player_query: Query<(Entity, &mut AnimationPlayer)>,
    children_query: Query<&Children>,
    instance_query: Query<&AnimationInstance>,
    animations: Res<Assets<AnimationData>>,
    time: Res<Time>,
    mut event_writer: EventWriter<AnimationEvent>,
) {
    for (player_entity, mut player) in player_query.iter_mut() {
        // Calculate player duration based on its instances
        let mut max_duration = AnimationTime::zero();
        if let Ok(children) = children_query.get(player_entity) {
            for child_entity in children {
                if let Ok(instance) = instance_query.get(*child_entity) {
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
        }
        let player_duration = max_duration;

        // Update time
        if player.playback_state == PlaybackState::Playing {
            let delta = time.delta_secs_f64() * player.speed;
            let new_time_seconds = player.current_time.as_seconds() + delta;

            if new_time_seconds >= player_duration.as_seconds() {
                match player.mode {
                    PlaybackMode::Loop => {
                        let wrapped_time =
                            new_time_seconds % player_duration.as_seconds().max(f64::EPSILON);
                        player.current_time = AnimationTime::from_seconds(wrapped_time).unwrap();
                    }
                    PlaybackMode::PingPong => {
                        player.current_time = player_duration;
                        player.speed = -player.speed;
                    }
                    PlaybackMode::Once => {
                        player.current_time = player_duration;
                        player.playback_state = PlaybackState::Ended;
                        let timestamp =
                            AnimationTime::from_seconds(time.elapsed_secs_f64()).unwrap();
                        if let Ok(children) = children_query.get(player_entity) {
                            for child in children {
                                if let Ok(instance) = instance_query.get(*child) {
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
            } else if new_time_seconds < 0.0 {
                match player.mode {
                    PlaybackMode::Loop => {
                        player.current_time = AnimationTime::from_seconds(
                            player_duration.as_seconds()
                                + (new_time_seconds
                                    % player_duration.as_seconds().max(f64::EPSILON)),
                        )
                        .unwrap();
                    }
                    PlaybackMode::PingPong => {
                        player.current_time = AnimationTime::zero();
                        player.speed = -player.speed;
                    }
                    PlaybackMode::Once => {
                        player.current_time = AnimationTime::zero();
                        player.playback_state = PlaybackState::Ended;
                    }
                }
            } else {
                player.current_time = AnimationTime::from_seconds(new_time_seconds).unwrap();
            }
        }
    }
}

/// Samples all animations and accumulates the values for blending.
pub fn accumulate_animation_values_system(
    instance_query: Query<(&Parent, &AnimationInstance, &AnimationBinding)>,
    player_query: Query<&AnimationPlayer>,
    animations: Res<Assets<AnimationData>>,
    mut interpolation_registry: ResMut<InterpolationRegistry>,
    mut blend_data: Local<FrameBlendData>,
) {
    blend_data.blended_values.clear();

    for (parent, instance, binding) in instance_query.iter() {
        if let Ok(player) = player_query.get(parent.parent()) {
            if player.playback_state != PlaybackState::Playing {
                continue;
            }

            if let Some(animation_data) = animations.get(&instance.animation) {
                let instance_time = (player.current_time.as_seconds()
                    - instance.start_time.as_seconds())
                    * instance.time_scale as f64;
                let instance_time = AnimationTime::from_seconds(instance_time.max(0.0)).unwrap();

                for (track_id, (target_entity, path)) in &binding.bindings {
                    if let Some(track) = animation_data.tracks.get(track_id) {
                        let transition =
                            animation_data.get_track_transition_for_time(instance_time, &track.id);
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
            }
        }
    }
}

/// Blends the accumulated values and applies them to the target components.
pub fn blend_and_apply_animation_values_system(
    mut blend_data: Local<FrameBlendData>,
    mut transforms: Query<&mut bevy::prelude::Transform>,
) {
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
                Value::from_components(value_type, &final_components).unwrap_or(values[0].1.clone())
            }
        };

        let path_str = path.to_string();
        let (root, sub_path) = path_str.split_once('.').unwrap_or((&path_str[..], ""));

        match root {
            "Transform" => {
                if let Ok(mut t) = transforms.get_mut(entity) {
                    match final_value {
                        Value::Transform(new_t) => {
                            let bevy_t = bevy::prelude::Transform {
                                translation: Vec3::new(
                                    new_t.position.x as f32,
                                    new_t.position.y as f32,
                                    new_t.position.z as f32,
                                ),
                                rotation: Quat::from_xyzw(
                                    new_t.rotation.x as f32,
                                    new_t.rotation.y as f32,
                                    new_t.rotation.z as f32,
                                    new_t.rotation.w as f32,
                                ),
                                scale: Vec3::new(
                                    new_t.scale.x as f32,
                                    new_t.scale.y as f32,
                                    new_t.scale.z as f32,
                                ),
                            };
                            if sub_path.is_empty() {
                                *t = bevy_t;
                            }
                        }
                        Value::Vector3(v) => {
                            let vec = Vec3::new(v.x as f32, v.y as f32, v.z as f32);
                            if let Ok(field) = t.reflect_path_mut(sub_path) {
                                if let Some(target) = field.try_downcast_mut::<Vec3>() {
                                    *target = vec;
                                }
                            }
                        }
                        Value::Vector4(q) => {
                            let quat = Quat::from_xyzw(q.x as f32, q.y as f32, q.z as f32, q.w as f32);
                            if let Ok(field) = t.reflect_path_mut(sub_path) {
                                if let Some(target) = field.try_downcast_mut::<Quat>() {
                                    *target = quat;
                                }
                            }
                        }
                        Value::Float(x) => {
                            let f32_val = x as f32;
                            if let Ok(field) = t.reflect_path_mut(sub_path) {
                                if let Some(target) = field.try_downcast_mut::<f32>() {
                                    *target = f32_val;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

/// Collects the final animated values and populates the `AnimationOutput` resource.
pub fn collect_animation_output_system(
    mut output: ResMut<AnimationOutput>,
    id_mapping: Res<IdMapping>,
    children_query: Query<&Children>,
    instance_query: Query<(&AnimationInstance, &AnimationBinding)>,
    animations: Res<Assets<AnimationData>>,
    transform_query: Query<&bevy::prelude::Transform>,
) {
    output.values.clear();

    for (player_id, player_entity) in id_mapping.players.iter() {
        let mut player_output = HashMap::new();
        if let Ok(children) = children_query.get(*player_entity) {
            for child_entity in children {
                if let Ok((instance, binding)) = instance_query.get(*child_entity) {
                    if let Some(anim_data) = animations.get(&instance.animation) {
                        for (track_id, (target_entity, path)) in &binding.bindings {
                            if let Some(track) = anim_data.tracks.get(track_id) {
                                let target_path_str = &track.target;
                                let path_str = path.to_string();
                                let (root, sub_path) =
                                    path_str.split_once('.').unwrap_or((&path_str[..], ""));
                                match root {
                                    "Transform" => {
                                        if let Ok(t) = transform_query.get(*target_entity) {
                                            let maybe_value = if sub_path.is_empty() {
                                                Some(Value::Transform(Transform::new(
                                                    Vector3::new(
                                                        t.translation.x as f64,
                                                        t.translation.y as f64,
                                                        t.translation.z as f64,
                                                    ),
                                                    Vector4::new(
                                                        t.rotation.x as f64,
                                                        t.rotation.y as f64,
                                                        t.rotation.z as f64,
                                                        t.rotation.w as f64,
                                                    ),
                                                    Vector3::new(
                                                        t.scale.x as f64,
                                                        t.scale.y as f64,
                                                        t.scale.z as f64,
                                                    ),
                                                )))
                                            } else if let Ok(val) = t.reflect_path(sub_path) {
                                                if let Some(v3) = val.try_downcast_ref::<Vec3>() {
                                                    Some(Value::Vector3(Vector3::new(
                                                        v3.x as f64,
                                                        v3.y as f64,
                                                        v3.z as f64,
                                                    )))
                                                } else if let Some(f) = val.try_downcast_ref::<f32>() {
                                                    Some(Value::Float(*f as f64))
                                                } else if let Some(q) = val.try_downcast_ref::<Quat>() {
                                                    Some(Value::Vector4(Vector4::new(
                                                        q.x as f64,
                                                        q.y as f64,
                                                        q.z as f64,
                                                        q.w as f64,
                                                    )))
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            };

                                            if let Some(v) = maybe_value {
                                                player_output.insert(target_path_str.clone(), v);
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
        output.values.insert(player_id.clone(), player_output);
    }
}
