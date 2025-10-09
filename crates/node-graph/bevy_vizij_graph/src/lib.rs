use bevy::prelude::*;
use hashbrown::HashMap;
use vizij_api_core::Value;
use vizij_graph_core::{evaluate_all, GraphRuntime, GraphSpec, NodeId, PortValue};

#[derive(Resource, Default, Clone)]
pub struct GraphResource(pub GraphSpec);

#[derive(Resource, Default, Clone)]
pub struct GraphOutputs(pub HashMap<NodeId, HashMap<String, PortValue>>);

/// Persistent runtime so stateful nodes (springs, dampers, etc.) can integrate across frames.
#[derive(Resource, Default)]
pub struct GraphRuntimeResource(pub GraphRuntime);

/// Convert a Value into a coarse f32 scalar for node parameter assignment.
/// Rules:
/// - Float -> value
/// - Bool -> 1.0 / 0.0
/// - VecN / Vector -> first component (or 0.0 if missing)
/// - Quat / ColorRgba / Transform -> first component (conservative)
/// - Enum -> recurse into inner value
/// - Text / others -> 0.0
fn value_to_f32(v: &Value) -> f32 {
    match v {
        Value::Float(f) => *f,
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Vec2(a) => a[0],
        Value::Vec3(a) => a[0],
        Value::Vec4(a) => a[0],
        Value::Quat(a) => a[0],
        Value::ColorRgba(a) => a[0],
        Value::Transform { translation, .. } => translation[0],
        Value::Vector(vec) => vec.first().copied().unwrap_or(0.0),
        Value::Record(map) => map.values().next().map(value_to_f32).unwrap_or(0.0),
        Value::Array(items) => items.first().map(value_to_f32).unwrap_or(0.0),
        Value::List(items) => items.first().map(value_to_f32).unwrap_or(0.0),
        Value::Tuple(items) => items.first().map(value_to_f32).unwrap_or(0.0),
        Value::Enum(_, boxed) => value_to_f32(boxed.as_ref()),
        Value::Text(_) => 0.0,
    }
}

#[derive(Event)]
pub struct SetNodeParam {
    pub node: NodeId,
    pub key: String,
    pub value: Value,
}

#[derive(Resource, Default, Clone)]
pub struct GraphTime {
    pub t: f32,
    pub dt: f32,
}

pub struct VizijGraphPlugin;

impl Plugin for VizijGraphPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GraphResource::default())
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
