//! Bevy integration for evaluating Vizij node graphs.
//!
//! This plugin owns a [`GraphRuntime`] resource, stages time via [`GraphTime`],
//! evaluates the current [`GraphSpec`], and applies resulting
//! [`vizij_api_core::WriteBatch`] writes when a `bevy_vizij_api::WriterRegistry`
//! is present.
//!
//! The API is intentionally lightweight: applications can mutate [`GraphResource`]
//! and send [`SetNodeParam`] events to drive graph parameters.

use bevy::prelude::*;
use hashbrown::HashMap;
use vizij_api_core::Value;
use vizij_graph_core::{evaluate_all, GraphRuntime, GraphSpec, NodeId, PortValue};

/// Graph specification to evaluate each frame.
///
/// Update this resource with a new [`GraphSpec`] when you want to switch the
/// graph being evaluated. For best runtime performance, ensure the spec is
/// preprocessed with [`GraphSpec::with_cache`].
///
/// When swapping graphs, reset the runtime state in [`GraphRuntimeResource`]
/// to avoid leaking cached node state across specs.
///
/// # Examples
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_vizij_graph::{GraphResource, GraphRuntimeResource};
/// use vizij_graph_core::GraphSpec;
///
/// fn swap_graph(mut spec: ResMut<GraphResource>, mut runtime: ResMut<GraphRuntimeResource>) {
///     *spec = GraphResource(GraphSpec::default().with_cache());
///     runtime.0.reset_for_spec();
/// }
/// ```
#[derive(Resource, Default, Clone)]
pub struct GraphResource(pub GraphSpec);

/// Snapshot of per-node output ports after evaluation.
///
/// This resource is refreshed every frame by [`system_eval`] and can be read by
/// gameplay systems or debugging tools. The outer key is the node id, and the
/// inner map is keyed by output port name.
#[derive(Resource, Default, Clone)]
pub struct GraphOutputs(pub HashMap<NodeId, HashMap<String, PortValue>>);

/// Persistent runtime so stateful nodes (springs, dampers, etc.) can integrate across frames.
///
/// Reset or replace this resource if you need to clear accumulated state or after
/// swapping [`GraphResource`] to a different spec.
///
/// # Examples
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_vizij_graph::GraphRuntimeResource;
///
/// fn reset_runtime(mut runtime: ResMut<GraphRuntimeResource>) {
///     runtime.0.reset_for_spec();
/// }
/// ```
#[derive(Resource, Default)]
pub struct GraphRuntimeResource(pub GraphRuntime);

/// Convert a Value into a coarse f32 scalar for node parameter assignment.
///
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

/// Event for updating a node parameter by key.
///
/// Known keys map onto [`vizij_graph_core::types::NodeParams`] fields. Most
/// numeric fields are coerced to `f32` using [`value_to_f32`]. Examples of
/// recognized keys include `"value"`, `"frequency"`, `"min"`, and `"max"`.
///
/// Unknown keys are ignored.
///
/// # Examples
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_vizij_graph::SetNodeParam;
/// use vizij_api_core::Value;
///
/// fn nudge_param(mut writer: EventWriter<SetNodeParam>) {
///     writer.send(SetNodeParam {
///         node: "gain".to_string(),
///         key: "value".into(),
///         value: Value::Float(1.25),
///     });
/// }
/// ```
#[derive(Event)]
pub struct SetNodeParam {
    /// Node identifier to update.
    pub node: NodeId,
    /// Parameter key, matching [`vizij_graph_core::types::NodeParams`] fields.
    pub key: String,
    /// New parameter value.
    pub value: Value,
}

/// Monotonic time state forwarded into the runtime.
///
/// The `dt` value is clamped to `>= 0.0` and forced to `0.0` if non-finite.
/// Override this resource before evaluation if you need fixed-step timing.
#[derive(Resource, Default, Clone)]
pub struct GraphTime {
    /// Accumulated time in seconds.
    pub t: f32,
    /// Step delta in seconds.
    pub dt: f32,
}

/// Registers graph evaluation systems and resources.
///
/// When a `bevy_vizij_api::WriterRegistry` resource is present, the plugin will apply
/// `WriteBatch` outputs to the Bevy world. Otherwise it only updates [`GraphOutputs`].
///
/// # Examples
/// ```no_run
/// use bevy::prelude::*;
/// use vizij_graph_core::GraphSpec;
/// use bevy_vizij_graph::{GraphResource, VizijGraphPlugin};
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(VizijGraphPlugin)
///     .insert_resource(GraphResource(GraphSpec::default().with_cache()))
///     .run();
/// ```
pub struct VizijGraphPlugin;

impl Plugin for VizijGraphPlugin {
    /// Builds internal state.
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

/// Update [`GraphTime`] based on Bevy's [`Time`] resource.
fn system_time(time: Res<Time>, mut gt: ResMut<GraphTime>) {
    gt.dt = time.delta_seconds();
    if !gt.dt.is_finite() {
        gt.dt = 0.0;
    }
    gt.t += gt.dt;
}

/// Apply [`SetNodeParam`] events to the current [`GraphSpec`].
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

/// Evaluate the graph and apply any writes to the Bevy world.
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
