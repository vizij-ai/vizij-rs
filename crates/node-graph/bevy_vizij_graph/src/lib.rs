//! Bevy adapter for evaluating Vizij node graphs.
//!
//! The plugin owns the active [`GraphSpec`], advances a persistent [`GraphRuntime`], applies
//! parameter updates from Bevy events, and exposes the latest output snapshot for inspection
//! or world writes through `bevy_vizij_api`.

use bevy::prelude::*;
use hashbrown::HashMap;
use vizij_api_core::value as vocab;
use vizij_api_core::{Value, VizijKind};
use vizij_graph_core::{evaluate_all, GraphRuntime, GraphSpec, NodeId, PortValue};

/// Resource containing the currently active graph specification.
#[derive(Resource, Default, Clone)]
pub struct GraphResource(pub GraphSpec<Value>);

/// Snapshot of the latest per-node outputs produced by evaluation.
#[derive(Resource, Default, Clone)]
pub struct GraphOutputs(pub HashMap<NodeId, HashMap<String, PortValue<Value>>>);

/// Persistent runtime so stateful nodes (springs, dampers, etc.) can integrate across frames.
#[derive(Resource, Default)]
pub struct GraphRuntimeResource(pub GraphRuntime<Value>);

/// Convert a Value into a coarse f32 scalar for node parameter assignment.
/// The value is classified once against the vizij vocabulary and decoded
/// through its accessors. Rules:
/// - Float -> value
/// - Bool -> 1.0 / 0.0
/// - Vec2/3/4 / Vector -> first component (or 0.0 if missing)
/// - Quat / ColorRgba -> first component, Transform -> translation x (conservative)
/// - Record -> first field by name, Array -> first item (recursing)
/// - Enum -> recurse into the payload
/// - Text / values outside the vocabulary -> 0.0
fn value_to_f32(v: &Value) -> f32 {
    match vocab::kind(v) {
        VizijKind::Float => vocab::as_float(v).unwrap_or(0.0),
        VizijKind::Bool => f32::from(vocab::as_bool(v) == Some(true)),
        VizijKind::Vec2 => vocab::as_vec2(v).map_or(0.0, |a| a[0]),
        VizijKind::Vec3 => vocab::as_vec3(v).map_or(0.0, |a| a[0]),
        VizijKind::Vec4 => vocab::as_vec4(v).map_or(0.0, |a| a[0]),
        VizijKind::Quat => vocab::as_quat(v).map_or(0.0, |a| a[0]),
        VizijKind::ColorRgba => vocab::as_color_rgba(v).map_or(0.0, |a| a[0]),
        VizijKind::Transform => vocab::as_transform(v).map_or(0.0, |t| t.translation[0]),
        VizijKind::Vector => vocab::as_vector(v)
            .and_then(|xs| xs.first().copied())
            .unwrap_or(0.0),
        VizijKind::Record => vocab::as_record(v)
            .and_then(|fields| fields.first().map(|(_, val)| value_to_f32(val)))
            .unwrap_or(0.0),
        VizijKind::Array => vocab::as_array(v)
            .and_then(|items| items.first().map(value_to_f32))
            .unwrap_or(0.0),
        VizijKind::Enum => {
            vocab::as_enumeration(v).map_or(0.0, |(_, payload)| value_to_f32(payload))
        }
        VizijKind::Text | VizijKind::Other => 0.0,
    }
}

#[derive(Event)]
pub struct SetNodeParam {
    pub node: NodeId,
    pub key: String,
    pub value: Value,
}

/// Public graph time state derived from Bevy `Time`.
#[derive(Resource, Default, Clone)]
pub struct GraphTime {
    pub t: f32,
    pub dt: f32,
}

/// Plugin that installs the graph resources, parameter event, and evaluation systems.
pub struct VizijGraphPlugin;

impl Plugin for VizijGraphPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GraphResource(GraphSpec::default().with_cache()))
            .insert_resource(GraphOutputs::default())
            .insert_resource(GraphTime { t: 0.0, dt: 0.0 })
            .insert_resource(GraphRuntimeResource::default())
            .add_event::<SetNodeParam>()
            .add_systems(Update, system_time)
            .add_systems(Update, system_set_params)
            .add_systems(Update, system_eval);
    }
}

fn system_time(time: Res<Time>, mut gt: ResMut<GraphTime>) {
    gt.dt = time.delta_seconds();
    if !gt.dt.is_finite() {
        gt.dt = 0.0;
    }
    gt.t += gt.dt;
}

fn system_set_params(mut ev: EventReader<SetNodeParam>, mut g: ResMut<GraphResource>) {
    for e in ev.read() {
        if let Some(node) = g.0.nodes.iter_mut().find(|n| n.id == e.node) {
            match e.key.as_str() {
                "value" => node.params.value = Some(e.value.clone()),
                "frequency" => {
                    node.params.frequency = Some(value_to_f32(&e.value));
                }
                "noise_seed" => {
                    node.params.noise_seed = Some(value_to_f32(&e.value));
                }
                "octaves" => {
                    node.params.octaves = Some(value_to_f32(&e.value));
                }
                "lacunarity" => {
                    node.params.lacunarity = Some(value_to_f32(&e.value));
                }
                "persistence" => {
                    node.params.persistence = Some(value_to_f32(&e.value));
                }
                "phase" => {
                    node.params.phase = Some(value_to_f32(&e.value));
                }
                "min" => {
                    node.params.min = value_to_f32(&e.value);
                }
                "max" => {
                    node.params.max = value_to_f32(&e.value);
                }
                "in_min" => {
                    node.params.in_min = Some(value_to_f32(&e.value));
                }
                "in_max" => {
                    node.params.in_max = Some(value_to_f32(&e.value));
                }
                "out_min" => {
                    node.params.out_min = Some(value_to_f32(&e.value));
                }
                "out_max" => {
                    node.params.out_max = Some(value_to_f32(&e.value));
                }
                "x" => {
                    node.params.x = Some(value_to_f32(&e.value));
                }
                "y" => {
                    node.params.y = Some(value_to_f32(&e.value));
                }
                "z" => {
                    node.params.z = Some(value_to_f32(&e.value));
                }
                "bone1" => {
                    node.params.bone1 = Some(value_to_f32(&e.value));
                }
                "bone2" => {
                    node.params.bone2 = Some(value_to_f32(&e.value));
                }
                "bone3" => {
                    node.params.bone3 = Some(value_to_f32(&e.value));
                }
                "index" => {
                    node.params.index = Some(value_to_f32(&e.value));
                }
                "stiffness" => {
                    node.params.stiffness = Some(value_to_f32(&e.value));
                }
                "damping" => {
                    node.params.damping = Some(value_to_f32(&e.value));
                }
                "mass" => {
                    node.params.mass = Some(value_to_f32(&e.value));
                }
                "half_life" => {
                    node.params.half_life = Some(value_to_f32(&e.value));
                }
                "max_rate" => {
                    node.params.max_rate = Some(value_to_f32(&e.value));
                }
                _ => { /* ignore unknown keys */ }
            }
        }
    }
}

fn system_eval(world: &mut World) {
    // Pull resources from the World (exclusive system).
    let Some(g) = world.get_resource::<GraphResource>().cloned() else {
        return;
    };
    let Some(gt) = world.get_resource::<GraphTime>().cloned() else {
        return;
    };
    let (batch, snapshot) = {
        let Some(mut runtime) = world.get_resource_mut::<GraphRuntimeResource>() else {
            return;
        };

        let dt = if gt.dt.is_finite() {
            gt.dt.max(0.0)
        } else {
            0.0
        };
        runtime.0.t = gt.t;
        runtime.0.dt = dt;
        if let Err(err) = evaluate_all(&mut runtime.0, &g.0) {
            bevy::log::error!("graph evaluation error: {err}");
        }

        let batch = runtime.0.writes.clone();
        let snapshot = runtime.0.outputs.clone();
        (batch, snapshot)
    };

    // Apply batch to world if WriterRegistry is present. Use resource_scope to avoid borrow conflicts.
    if world.contains_resource::<bevy_vizij_api::WriterRegistry>() {
        world.resource_scope(|world, reg: Mut<bevy_vizij_api::WriterRegistry>| {
            bevy_vizij_api::apply_write_batch(&reg, world, &batch);
        });
    }

    // Preserve the GraphOutputs resource for inspection.
    if let Some(mut out) = world.get_resource_mut::<GraphOutputs>() {
        out.0 = snapshot.clone();
    } else {
        // In case it wasn't inserted for some reason, insert it now.
        world.insert_resource(GraphOutputs(snapshot));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_to_f32_decodes_the_vocabulary() {
        assert_eq!(value_to_f32(&vocab::float(1.5)), 1.5);
        assert_eq!(value_to_f32(&vocab::bool_(true)), 1.0);
        assert_eq!(value_to_f32(&vocab::bool_(false)), 0.0);
        assert_eq!(value_to_f32(&vocab::vec2([2.0, 9.0])), 2.0);
        assert_eq!(value_to_f32(&vocab::vec3([3.0, 9.0, 9.0])), 3.0);
        assert_eq!(value_to_f32(&vocab::vec4([4.0, 9.0, 9.0, 9.0])), 4.0);
        assert_eq!(value_to_f32(&vocab::quat([0.5, 0.0, 0.0, 1.0])), 0.5);
        assert_eq!(
            value_to_f32(&vocab::color_rgba([0.25, 0.0, 0.0, 1.0])),
            0.25
        );
        assert_eq!(
            value_to_f32(&vocab::transform(vizij_api_core::Transform {
                translation: [7.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
            })),
            7.0
        );
        assert_eq!(value_to_f32(&vocab::vector(vec![6.0, 9.0])), 6.0);
        assert_eq!(value_to_f32(&vocab::vector(vec![])), 0.0);
        // Records read their first field by name order.
        assert_eq!(
            value_to_f32(&vocab::record([
                ("b", vocab::float(9.0)),
                ("a", vocab::float(8.0)),
            ])),
            8.0
        );
        assert_eq!(
            value_to_f32(&vocab::array(vec![vocab::vec3([5.0, 0.0, 0.0])])),
            5.0
        );
        assert_eq!(
            value_to_f32(&vocab::enumeration("on", vocab::float(2.5))),
            2.5
        );
        assert_eq!(value_to_f32(&vocab::text("nope")), 0.0);
        assert_eq!(value_to_f32(&Value::U32(3)), 0.0);
    }
}
