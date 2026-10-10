//! The animation module, registered **host-side** — no wasm, no artifact
//! dependency, so the workspace stays on stable.
//!
//! The module's interface is its Rust declaration,
//! [`vizij_animation_module::animation`]: the ids every function and parameter
//! is registered under and the signature each is described with come from it,
//! so a graph `ExternalFunction` node dispatches to this module exactly as it
//! would to the loaded wasm guest, and method introspection lists every
//! function.
//!
//! The declared functions act on the wasm guest's global state; a host has no
//! executor to scope that state by, so this module owns an [`AnimationModule`]
//! of its own and dispatches to it — two devices in one process never share
//! players. Each closure reads its arguments by parameter id: the order a
//! caller sends them in carries no meaning.
//!
//! An animation loads through the declared functions, whoever the client:
//! [`module_animation`] converts a face's authored animation into the
//! `AnimationClip` that `load_animation` takes, its tracks keyed through the
//! face's rig.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use arora::{HostModule, ModuleBuilder};
use arora_types::call::{Call, CallError, CallResult};
use arora_types::module::declared::AroraModule;
use arora_types::record::module::frozen::{ExportKind, Function};
use arora_types::value::Value;
use arora_types::AroraType;
use uuid::Uuid;
use vizij_animation_module::animation::{self, ids};
use vizij_animation_module::{
    AnimTrack, AnimationClip, AnimationModule, Keypoint, PlayerState, TrackOutput, TransitionHandle,
};
use vizij_arora_host::contents::{Animation, AnimationTrack};
use vizij_arora_host::ChannelKeys;

/// A registered function's body: the call's arguments in, its return out.
type Body = Box<dyn FnMut(&Call) -> Result<Value, CallError>>;

/// The animation module as a host module over an engine of its own: every
/// declared function, dispatched in-process under its declared id.
pub fn host_module() -> HostModule {
    let state = Rc::new(RefCell::new(AnimationModule::new()));
    let a = |_: ()| state.clone();
    let bodies: Vec<(Uuid, Body)> = vec![
        (ids::load_animation::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let clip = arg(call, ids::load_animation::CLIP, "clip")?;
                Ok(Value::from(a.borrow_mut().load_animation(clip)))
            })
        }),
        (ids::create_player::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let name = arg(call, ids::create_player::NAME, "name")?;
                Ok(Value::from(a.borrow_mut().create_player(name)))
            })
        }),
        (ids::add_instance::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::add_instance::PLAYER, "player")?;
                let anim = arg(call, ids::add_instance::ANIM, "anim")?;
                Ok(Value::from(a.borrow_mut().add_instance(player, anim)))
            })
        }),
        (ids::step::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let dt_ns = arg(call, ids::step::DT_NS, "dt_ns")?;
                let time_ns = arg(call, ids::step::TIME_NS, "time_ns")?;
                Ok(array_of::<TrackOutput>(a.borrow_mut().step(dt_ns, time_ns)))
            })
        }),
        (ids::play::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::play::PLAYER, "player")?;
                Ok(Value::from(a.borrow_mut().play(player)))
            })
        }),
        (ids::pause::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::pause::PLAYER, "player")?;
                Ok(Value::from(a.borrow_mut().pause(player)))
            })
        }),
        (ids::stop::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::stop::PLAYER, "player")?;
                Ok(Value::from(a.borrow_mut().stop(player)))
            })
        }),
        (ids::seek::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::seek::PLAYER, "player")?;
                let time_ns = arg(call, ids::seek::TIME_NS, "time_ns")?;
                Ok(Value::from(a.borrow_mut().seek(player, time_ns)))
            })
        }),
        (ids::set_speed::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::set_speed::PLAYER, "player")?;
                let speed = arg(call, ids::set_speed::SPEED, "speed")?;
                Ok(Value::from(a.borrow_mut().set_speed(player, speed)))
            })
        }),
        (ids::set_loop::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::set_loop::PLAYER, "player")?;
                let mode = arg(call, ids::set_loop::MODE, "mode")?;
                Ok(Value::from(a.borrow_mut().set_loop(player, mode)))
            })
        }),
        (ids::set_weight::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::set_weight::PLAYER, "player")?;
                let instance = arg(call, ids::set_weight::INSTANCE, "instance")?;
                let weight = arg(call, ids::set_weight::WEIGHT, "weight")?;
                Ok(Value::from(
                    a.borrow_mut().set_weight(player, instance, weight),
                ))
            })
        }),
        (ids::remove_instance::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::remove_instance::PLAYER, "player")?;
                let instance = arg(call, ids::remove_instance::INSTANCE, "instance")?;
                Ok(Value::from(
                    a.borrow_mut().remove_instance(player, instance),
                ))
            })
        }),
        (ids::player_states::FUNCTION, {
            let a = a(());
            Box::new(move |_call| Ok(array_of::<PlayerState>(a.borrow().player_states())))
        }),
        (ids::bake::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let anim = arg(call, ids::bake::ANIM, "anim")?;
                let frame_rate = arg(call, ids::bake::FRAME_RATE, "frame_rate")?;
                let start_ns = arg(call, ids::bake::START_NS, "start_ns")?;
                let end_ns = arg(call, ids::bake::END_NS, "end_ns")?;
                Ok(optional(
                    a.borrow().bake(anim, frame_rate, start_ns, end_ns),
                ))
            })
        }),
        (ids::bake_with_derivatives::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let anim = arg(call, ids::bake_with_derivatives::ANIM, "anim")?;
                let frame_rate = arg(call, ids::bake_with_derivatives::FRAME_RATE, "frame_rate")?;
                let start_ns = arg(call, ids::bake_with_derivatives::START_NS, "start_ns")?;
                let end_ns = arg(call, ids::bake_with_derivatives::END_NS, "end_ns")?;
                Ok(optional(
                    a.borrow()
                        .bake_with_derivatives(anim, frame_rate, start_ns, end_ns),
                ))
            })
        }),
        (ids::unload_animation::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let anim = arg(call, ids::unload_animation::ANIM, "anim")?;
                Ok(Value::from(a.borrow_mut().unload_animation(anim)))
            })
        }),
        (ids::remove_player::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::remove_player::PLAYER, "player")?;
                Ok(Value::from(a.borrow_mut().remove_player(player)))
            })
        }),
        (ids::add_instance_with_weight::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::add_instance_with_weight::PLAYER, "player")?;
                let anim = arg(call, ids::add_instance_with_weight::ANIM, "anim")?;
                let weight = arg(call, ids::add_instance_with_weight::WEIGHT, "weight")?;
                Ok(Value::from(
                    a.borrow_mut()
                        .add_instance_with_weight(player, anim, weight),
                ))
            })
        }),
        (ids::reload_animation::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let anim = arg(call, ids::reload_animation::ANIM, "anim")?;
                let clip = arg(call, ids::reload_animation::CLIP, "clip")?;
                Ok(Value::from(a.borrow_mut().reload_animation(anim, clip)))
            })
        }),
        (ids::set_window::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::set_window::PLAYER, "player")?;
                let start_ns = arg(call, ids::set_window::START_NS, "start_ns")?;
                let end_ns = arg(call, ids::set_window::END_NS, "end_ns")?;
                Ok(Value::from(
                    a.borrow_mut().set_window(player, start_ns, end_ns),
                ))
            })
        }),
        (ids::play_at::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::play_at::PLAYER, "player")?;
                let time_ns = arg(call, ids::play_at::TIME_NS, "time_ns")?;
                Ok(Value::from(a.borrow_mut().play_at(player, time_ns)))
            })
        }),
        (ids::set_start_offset::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::set_start_offset::PLAYER, "player")?;
                let instance = arg(call, ids::set_start_offset::INSTANCE, "instance")?;
                let offset_ns = arg(call, ids::set_start_offset::OFFSET_NS, "offset_ns")?;
                Ok(Value::from(
                    a.borrow_mut().set_start_offset(player, instance, offset_ns),
                ))
            })
        }),
        (ids::set_time_scale::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let player = arg(call, ids::set_time_scale::PLAYER, "player")?;
                let instance = arg(call, ids::set_time_scale::INSTANCE, "instance")?;
                let time_scale = arg(call, ids::set_time_scale::TIME_SCALE, "time_scale")?;
                Ok(Value::from(
                    a.borrow_mut().set_time_scale(player, instance, time_scale),
                ))
            })
        }),
        (ids::step_values::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let dt_ns = arg(call, ids::step_values::DT_NS, "dt_ns")?;
                let time_ns = arg(call, ids::step_values::TIME_NS, "time_ns")?;
                Ok(Value::from(a.borrow_mut().step_values(dt_ns, time_ns)))
            })
        }),
        (ids::output_keys::FUNCTION, {
            let a = a(());
            Box::new(move |_call| Ok(Value::from(a.borrow().output_keys())))
        }),
    ];

    let mut signatures = signatures();
    bodies
        .into_iter()
        .fold(
            ModuleBuilder::new(ids::MODULE),
            |builder, (id, mut body)| {
                let (name, signature) = signatures
                    .remove(&id)
                    .expect("every registered function is declared");
                builder.described_function(id, name, signature, move |call| {
                    Ok(CallResult {
                        ret: body(&call)?,
                        mutated: Vec::new(),
                    })
                })
            },
        )
        .build()
}

// --- A face's animations, as the module takes them ---------------------------

/// `animation` as the animation module's [`AnimationClip`]: each track keyed
/// by the store key its channel names (`keys`), its keyframes stamped over the
/// animation's duration, and each segment timed as its left keyframe's
/// interpolation says (the track's when the keyframe says none): `linear`
/// (the default), `step` (the left value held to the next keyframe), or
/// `cubic` (the engine's default ease — the bundle carries no tangents). A
/// track without keyframes is left out.
pub fn module_animation(animation: &Animation, keys: &ChannelKeys) -> AnimationClip {
    let duration = if animation.duration > 0.0 {
        animation.duration
    } else {
        1.0
    };
    AnimationClip {
        name: animation
            .name
            .clone()
            .unwrap_or_else(|| animation.id.clone()),
        duration: (animation.duration * 1000.0).round().max(1.0) as u32,
        tracks: animation
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| !track.keyframes.is_empty())
            .map(|(index, track)| {
                let id = format!("{}:{index}", animation.id);
                AnimTrack {
                    points: keypoints(track, duration, &id),
                    id,
                    name: track.channel.clone(),
                    animatable_id: keys.key(&track.channel),
                }
            })
            .collect(),
    }
}

/// A track's keyframes as the module's keypoints (see [`module_animation`]): a
/// `step` segment gains a keypoint just before its right end, holding the
/// left value.
fn keypoints(track: &AnimationTrack, duration: f64, track_id: &str) -> Vec<Keypoint> {
    /// How far before a step's end its held value gives way, in
    /// animation-normalized time.
    const STEP_EDGE: f32 = 1e-5;
    let handle = |x: f32, y: f32| vec![TransitionHandle { x, y }];
    let point = |id: String, stamp: f32, value: f64| Keypoint {
        id,
        stamp,
        value: Value::F32(value as f32),
        transitions_in: Vec::new(),
        transitions_out: Vec::new(),
    };
    let mut points: Vec<Keypoint> = Vec::new();
    for (index, keyframe) in track.keyframes.iter().enumerate() {
        let stamp = (keyframe.time / duration).clamp(0.0, 1.0) as f32;
        let mut current = point(format!("{track_id}:{index}"), stamp, keyframe.value);
        if let (Some(left), Some(previous)) = (index.checked_sub(1), points.last_mut()) {
            let segment = track.keyframes[left]
                .interpolation
                .as_deref()
                .or(track.interpolation.as_deref())
                .map(str::to_ascii_lowercase);
            match segment.as_deref() {
                Some("cubic" | "cubicspline") => {}
                Some("step") => {
                    let held = previous.value.clone();
                    let edge = (stamp - STEP_EDGE).max(previous.stamp);
                    previous.transitions_out = handle(0.0, 0.0);
                    let mut hold = point(format!("{track_id}:{index}~"), edge, 0.0);
                    hold.value = held;
                    hold.transitions_in = handle(1.0, 1.0);
                    hold.transitions_out = handle(0.0, 0.0);
                    points.push(hold);
                    current.transitions_in = handle(1.0, 1.0);
                }
                _ => {
                    previous.transitions_out = handle(0.0, 0.0);
                    current.transitions_in = handle(1.0, 1.0);
                }
            }
        }
        points.push(current);
    }
    points
}

/// `function id -> module id` over the animation functions — what
/// `ProcessingGraph::set_function_modules` needs so a bare function handle
/// (the animation source's `step`/`player_states` nodes) routes to this module.
pub fn function_modules() -> HashMap<Uuid, Uuid> {
    signatures()
        .into_keys()
        .map(|function| (function, ids::MODULE))
        .collect()
}

/// Every declared function's name and frozen signature, from the
/// declaration's record.
fn signatures() -> HashMap<Uuid, (String, Function)> {
    animation::Module::record(Uuid::nil())
        .exports
        .into_iter()
        .map(|(id, export)| {
            let ExportKind::Function(function) = export.kind;
            (id, (export.name, function))
        })
        .collect()
}

// --- Call marshaling ---------------------------------------------------------

/// A value a closure reads from a call argument.
trait FromArg: Sized {
    fn from_arg(value: Value) -> Option<Self>;
    /// The value when the argument is absent: `None` for an optional
    /// parameter, a failed call for any other.
    fn absent() -> Option<Self> {
        None
    }
}

macro_rules! from_arg {
    ($($ty:ty => $variant:ident),*) => {$(
        impl FromArg for $ty {
            fn from_arg(value: Value) -> Option<Self> {
                match value {
                    Value::$variant(v) => Some(v),
                    _ => None,
                }
            }
        }
    )*};
}
from_arg!(u32 => U32, u64 => U64, i64 => I64, f32 => F32, String => String);

impl FromArg for AnimationClip {
    fn from_arg(value: Value) -> Option<Self> {
        AnimationClip::try_from(value).ok()
    }
}

/// An optional parameter: absent, or `Value::Option(None)`, is `None`; a
/// present value arrives wrapped in `Value::Option` or as its bare element.
impl<T: FromArg> FromArg for Option<T> {
    fn from_arg(value: Value) -> Option<Self> {
        match value {
            Value::Option(None) => Some(None),
            Value::Option(Some(inner)) => T::from_arg(*inner).map(Some),
            bare => T::from_arg(bare).map(Some),
        }
    }
    fn absent() -> Option<Self> {
        Some(None)
    }
}

/// Read the argument under parameter `id`, whatever its position in the call.
fn arg<T: FromArg>(call: &Call, id: Uuid, name: &str) -> Result<T, CallError> {
    match call.args.iter().find(|field| field.id == id) {
        Some(field) => T::from_arg((*field.value).clone()).ok_or_else(|| CallError::Generic {
            message: format!("parameter `{name}`: unexpected value {}", field.value),
        }),
        None => T::absent().ok_or_else(|| CallError::Generic {
            message: format!("missing parameter `{name}`"),
        }),
    }
}

/// A declared optional return: `Value::Option`, its record inside when present.
fn optional<T: Into<Value>>(record: Option<T>) -> Value {
    Value::Option(record.map(|record| Box::new(record.into())))
}

/// Records as the typed `Value::ArrayStructure` a declared array return is:
/// the element type named once, so an empty batch is still well-formed.
fn array_of<T: AroraType + Into<Value>>(records: Vec<T>) -> Value {
    Value::array_of_type::<T>(records.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_types::value::{Structure, StructureField};
    use vizij_arora_host::Bundle;
    use vizij_arora_store::BlackboardStore;

    fn device() -> arora::Arora {
        arora::Arora::builder()
            .with_data_store(Box::new(BlackboardStore::new()))
            .with_host_module(host_module())
            .build()
            .expect("build arora")
    }

    fn field(id: Uuid, value: Value) -> StructureField {
        StructureField {
            id,
            value: Box::new(value),
        }
    }

    fn call(device: &mut arora::Arora, function: Uuid, args: Vec<StructureField>) -> Value {
        device
            .call(Call {
                module_id: Some(ids::MODULE),
                id: function,
                args,
            })
            .expect("the call dispatches")
            .ret
    }

    fn player_count(device: &mut arora::Arora) -> usize {
        match call(device, ids::player_states::FUNCTION, Vec::new()) {
            Value::ArrayStructure { elements, .. } => elements.len(),
            other => panic!("expected an array of PlayerState, got {other:?}"),
        }
    }

    fn create_player(device: &mut arora::Arora, name: &str) -> u32 {
        match call(
            device,
            ids::create_player::FUNCTION,
            vec![field(ids::create_player::NAME, Value::String(name.into()))],
        ) {
            Value::U32(player) => player,
            other => panic!("expected a player id, got {other:?}"),
        }
    }

    #[test]
    fn two_devices_own_separate_animation_engines() {
        let mut first = device();
        let mut second = device();
        create_player(&mut first, "only-in-first");
        assert_eq!(player_count(&mut first), 1);
        assert_eq!(player_count(&mut second), 0);
    }

    /// VIZ-129: arguments are matched by parameter id, so a caller sending them
    /// in another order makes the same call.
    #[test]
    fn arguments_are_matched_by_id_whatever_their_order() {
        let mut device = device();
        let player = create_player(&mut device, "p");
        let player_arg = field(ids::set_weight::PLAYER, Value::U32(player));
        let instance = field(ids::set_weight::INSTANCE, Value::U32(7));
        let weight = field(ids::set_weight::WEIGHT, Value::F32(0.5));
        let declared = call(
            &mut device,
            ids::set_weight::FUNCTION,
            vec![player_arg.clone(), instance.clone(), weight.clone()],
        );
        let reversed = call(
            &mut device,
            ids::set_weight::FUNCTION,
            vec![weight, instance, player_arg],
        );
        assert_eq!(declared, Value::U32(7), "set_weight echoes the instance id");
        assert_eq!(reversed, declared);
    }

    #[test]
    fn a_missing_argument_fails_the_call_by_name() {
        let mut device = device();
        let error = device
            .call(Call {
                module_id: Some(ids::MODULE),
                id: ids::add_instance::FUNCTION,
                args: vec![field(ids::add_instance::ANIM, Value::U32(0))],
            })
            .expect_err("add_instance without its player");
        assert!(
            error.to_string().contains("missing parameter `player`"),
            "{error}"
        );
        let error = device
            .call(Call {
                module_id: Some(ids::MODULE),
                id: ids::add_instance_with_weight::FUNCTION,
                args: vec![
                    field(ids::add_instance_with_weight::PLAYER, Value::U32(0)),
                    field(ids::add_instance_with_weight::ANIM, Value::U32(0)),
                ],
            })
            .expect_err("add_instance_with_weight without its weight");
        assert!(
            error.to_string().contains("missing parameter `weight`"),
            "{error}"
        );
    }

    /// `vizij-arora-host`'s animation source names the module's functions, the
    /// `step` parameters and the `TrackOutput` fields by id; they are the
    /// declaration's.
    #[test]
    fn the_animation_source_names_the_declared_ids() {
        let (_, spec) = vizij_arora_host::animations_source();
        let node = |id: &str| {
            spec["nodes"]
                .as_array()
                .expect("nodes")
                .iter()
                .find(|node| node["id"] == id)
                .unwrap_or_else(|| panic!("node {id}"))
                .clone()
        };
        let arora_types::ty::low::TypeKind::Structure(output) = TrackOutput::arora_type().kind
        else {
            panic!("TrackOutput is a structure");
        };
        let field = |name: &str| {
            output
                .fields
                .iter()
                .find(|(_, field)| field.name == name)
                .map(|(id, _)| id.to_string())
                .unwrap_or_else(|| panic!("field {name}"))
        };
        let step = node("step");
        assert_eq!(step["params"]["function"], ids::step::FUNCTION.to_string());
        assert_eq!(step["params"]["param_ids"][0], ids::step::DT_NS.to_string());
        assert_eq!(
            step["params"]["param_ids"][1],
            ids::step::TIME_NS.to_string()
        );
        assert_eq!(
            node("states")["params"]["function"],
            ids::player_states::FUNCTION.to_string()
        );
        let apply = node("apply");
        assert_eq!(apply["params"]["key_field"], field("default_key"));
        assert_eq!(apply["params"]["value_field"], field("value"));
    }

    fn authored(json: serde_json::Value) -> Animation {
        vizij_arora_host::contents::animation(&json).expect("an animation")
    }

    /// Tracks key through the face's rig; keyframes stamp over the
    /// animation's duration; a segment is timed by its left keyframe's
    /// interpolation.
    #[test]
    fn an_animation_keys_its_tracks_through_the_rig_and_times_its_segments() {
        let bundle = Bundle::from_bundle_json(&serde_json::json!({
            "metadata": { "faceId": "f" },
            "graphs": [{ "kind": "rig", "spec": { "nodes": [
                { "id": "input_x", "type": "input", "params": { "path": "rig/f/x" } },
            ], "edges": [] } }],
        }));
        let converted = module_animation(
            &authored(serde_json::json!({ "id": "c", "duration": 2, "tracks": [
                { "channel": "x", "interpolation": "step", "keyframes": [
                    { "time": 0, "value": 0 },
                    { "time": 1, "value": 1, "interpolation": "linear" },
                    { "time": 2, "value": 0, "interpolation": "cubic" },
                ] },
                { "channel": "y", "keyframes": [] },
            ] })),
            &bundle.channel_keys(),
        );
        assert_eq!(converted.name, "c");
        assert_eq!(converted.duration, 2000);
        assert_eq!(
            converted.tracks.len(),
            1,
            "a track without keyframes is left out"
        );
        let track = &converted.tracks[0];
        assert_eq!(track.animatable_id, "rig/f/x");
        let points: Vec<(f32, f32, usize, usize)> = track
            .points
            .iter()
            .map(|p| {
                let Value::F32(v) = p.value else {
                    panic!("f32")
                };
                (p.stamp, v, p.transitions_in.len(), p.transitions_out.len())
            })
            .collect();
        // The step holds 0 until just before 0.5, then a linear segment.
        assert_eq!(points.len(), 4);
        assert_eq!(points[0], (0.0, 0.0, 0, 1));
        assert!(points[1].0 < 0.5 && points[1].0 > 0.49 && points[1].1 == 0.0);
        assert_eq!(points[2], (0.5, 1.0, 1, 1));
        assert_eq!(points[3], (1.0, 0.0, 1, 0));
    }

    /// `add_instance` adds an instance blending at 1;
    /// `add_instance_with_weight` at its weight, at 0 writing nothing.
    #[test]
    fn an_instance_added_through_a_call_takes_its_weight() {
        let mut device = device();
        let clip = |key: &str| {
            module_animation(
                &authored(serde_json::json!({ "id": key, "duration": 1, "tracks": [
                    { "channel": key, "keyframes": [{ "time": 0, "value": 1 }] },
                ] })),
                &ChannelKeys::default(),
            )
        };
        let player = create_player(&mut device, "p");
        for (key, weight) in [("loud", None), ("quiet", Some(0.0))] {
            let Value::U32(anim) = call(
                &mut device,
                ids::load_animation::FUNCTION,
                vec![field(ids::load_animation::CLIP, clip(key).into())],
            ) else {
                panic!("an animation id");
            };
            match weight {
                None => call(
                    &mut device,
                    ids::add_instance::FUNCTION,
                    vec![
                        field(ids::add_instance::PLAYER, Value::U32(player)),
                        field(ids::add_instance::ANIM, Value::U32(anim)),
                    ],
                ),
                Some(weight) => call(
                    &mut device,
                    ids::add_instance_with_weight::FUNCTION,
                    vec![
                        field(ids::add_instance_with_weight::PLAYER, Value::U32(player)),
                        field(ids::add_instance_with_weight::ANIM, Value::U32(anim)),
                        field(ids::add_instance_with_weight::WEIGHT, Value::F32(weight)),
                    ],
                ),
            };
        }
        let out = format!(
            "{:?}",
            call(
                &mut device,
                ids::step::FUNCTION,
                vec![field(ids::step::DT_NS, Value::U64(0))],
            )
        );
        assert!(
            out.contains("loud") && !out.contains("quiet"),
            "only the weighted instance writes: {out}"
        );
    }

    /// A `play_at` anchored in the `time_ns` that `step` is given starts the
    /// player at that instant, through the declared functions.
    #[test]
    fn play_at_starts_at_the_step_time_it_names() {
        let mut device = device();
        let player = create_player(&mut device, "anchored");
        let clip = module_animation(
            &authored(serde_json::json!({ "id": "a", "duration": 1, "tracks": [
                { "channel": "a", "keyframes": [
                    { "time": 0, "value": 0 }, { "time": 1, "value": 1 },
                ] },
            ] })),
            &ChannelKeys::default(),
        );
        let Value::U32(anim) = call(
            &mut device,
            ids::load_animation::FUNCTION,
            vec![field(ids::load_animation::CLIP, clip.into())],
        ) else {
            panic!("an animation id");
        };
        call(
            &mut device,
            ids::add_instance::FUNCTION,
            vec![
                field(ids::add_instance::PLAYER, Value::U32(player)),
                field(ids::add_instance::ANIM, Value::U32(anim)),
            ],
        );
        let step = |device: &mut arora::Arora, time_ns: u64| {
            call(
                device,
                ids::step::FUNCTION,
                vec![
                    field(ids::step::DT_NS, Value::U64(100_000_000)),
                    field(ids::step::TIME_NS, Value::U64(time_ns)),
                ],
            );
        };
        let playhead = |device: &mut arora::Arora| -> PlayerState {
            let Value::ArrayStructure { id, mut elements } =
                call(device, ids::player_states::FUNCTION, Vec::new())
            else {
                panic!("an array of PlayerState");
            };
            let fields = elements.remove(0).fields;
            PlayerState::try_from(Value::Structure(Structure { id, fields }))
                .expect("a PlayerState")
        };
        call(
            &mut device,
            ids::stop::FUNCTION,
            vec![field(ids::stop::PLAYER, Value::U32(player))],
        );
        step(&mut device, 1_000_000_000);
        let anchored = call(
            &mut device,
            ids::play_at::FUNCTION,
            vec![
                field(ids::play_at::PLAYER, Value::U32(player)),
                field(ids::play_at::TIME_NS, Value::U64(1_150_000_000)),
            ],
        );
        assert_eq!(anchored, Value::U32(player));
        step(&mut device, 1_100_000_000);
        assert_eq!(playhead(&mut device).state, "stopped");
        step(&mut device, 1_200_000_000);
        let state = playhead(&mut device);
        assert_eq!(state.state, "playing");
        assert!(
            state.time_ns.abs_diff(50_000_000) < 1_000,
            "50 ms after the anchor, got {} ns",
            state.time_ns
        );
    }

    /// `bake` answers a call with a `BakedAnimation` record, its window
    /// arguments in nanoseconds, and `Value::Option(None)` for an animation
    /// not loaded.
    #[test]
    fn bake_answers_with_a_baked_animation_record() {
        let mut device = device();
        let clip = module_animation(
            &authored(serde_json::json!({ "id": "b", "duration": 1, "tracks": [
                { "channel": "b", "keyframes": [
                    { "time": 0, "value": 0 }, { "time": 1, "value": 1 },
                ] },
            ] })),
            &ChannelKeys::default(),
        );
        let Value::U32(anim) = call(
            &mut device,
            ids::load_animation::FUNCTION,
            vec![field(ids::load_animation::CLIP, clip.into())],
        ) else {
            panic!("an animation id");
        };
        let bake = |device: &mut arora::Arora, anim: u32| {
            call(
                device,
                ids::bake::FUNCTION,
                vec![
                    field(ids::bake::ANIM, Value::U32(anim)),
                    field(ids::bake::FRAME_RATE, Value::F32(10.0)),
                    field(ids::bake::END_NS, Value::U64(500_000_000)),
                ],
            )
        };
        let Value::Option(Some(baked)) = bake(&mut device, anim) else {
            panic!("a baked animation");
        };
        let baked = vizij_animation_module::BakedAnimation::try_from(*baked).expect("decodes");
        assert_eq!((baked.start_ns, baked.end_ns), (0, 500_000_000));
        let Value::ArrayValue(values) = &baked.tracks[0].values else {
            panic!("an array value");
        };
        assert_eq!(values.len(), 6);
        assert_eq!(bake(&mut device, anim + 1), Value::Option(None));

        // A window in seconds, the arguments' former unit, is refused.
        let error = device
            .call(Call {
                module_id: Some(ids::MODULE),
                id: ids::bake::FUNCTION,
                args: vec![
                    field(ids::bake::ANIM, Value::U32(anim)),
                    field(ids::bake::START_NS, Value::F32(0.5)),
                ],
            })
            .expect_err("an f32 start");
        assert!(error.to_string().contains("start_ns"), "{error}");
    }

    /// `step_values` answers with the values by position in the table
    /// `output_keys` answers with, both under one revision.
    #[test]
    fn step_values_follow_the_output_keys() {
        let mut device = device();
        let player = create_player(&mut device, "p");
        let clip = module_animation(
            &authored(serde_json::json!({ "id": "v", "duration": 1, "tracks": [
                { "channel": "a", "keyframes": [{ "time": 0, "value": 1 }] },
                { "channel": "b", "keyframes": [{ "time": 0, "value": 2 }] },
            ] })),
            &ChannelKeys::default(),
        );
        let Value::U32(anim) = call(
            &mut device,
            ids::load_animation::FUNCTION,
            vec![field(ids::load_animation::CLIP, clip.into())],
        ) else {
            panic!("an animation id");
        };
        call(
            &mut device,
            ids::add_instance::FUNCTION,
            vec![
                field(ids::add_instance::PLAYER, Value::U32(player)),
                field(ids::add_instance::ANIM, Value::U32(anim)),
            ],
        );
        let keys = vizij_animation_module::OutputKeys::try_from(call(
            &mut device,
            ids::output_keys::FUNCTION,
            Vec::new(),
        ))
        .expect("an OutputKeys record");
        assert_eq!(keys.keys.len(), 2);
        let step = vizij_animation_module::StepValues::try_from(call(
            &mut device,
            ids::step_values::FUNCTION,
            vec![field(ids::step_values::DT_NS, Value::U64(0))],
        ))
        .expect("a StepValues record");
        assert_eq!(step.revision, keys.revision);
        let Value::ArrayValue(values) = step.values else {
            panic!("an array value");
        };
        let by_key: Vec<(&str, &Value)> =
            keys.keys.iter().map(String::as_str).zip(&values).collect();
        assert!(
            by_key
                .iter()
                .any(|(k, v)| k.ends_with('a') && **v == Value::F32(1.0)),
            "{by_key:?}"
        );
        assert!(
            by_key
                .iter()
                .any(|(k, v)| k.ends_with('b') && **v == Value::F32(2.0)),
            "{by_key:?}"
        );
    }

    #[test]
    fn an_animation_reloads_through_a_call() {
        let mut device = device();
        let clip = |key: &str| {
            module_animation(
                &authored(serde_json::json!({ "id": "wave", "duration": 1, "tracks": [
                    { "channel": key, "keyframes": [{ "time": 0, "value": 1 }] },
                ] })),
                &ChannelKeys::default(),
            )
        };
        let player = create_player(&mut device, "p");
        let Value::U32(anim) = call(
            &mut device,
            ids::load_animation::FUNCTION,
            vec![field(ids::load_animation::CLIP, clip("before").into())],
        ) else {
            panic!("an animation id");
        };
        call(
            &mut device,
            ids::add_instance::FUNCTION,
            vec![
                field(ids::add_instance::PLAYER, Value::U32(player)),
                field(ids::add_instance::ANIM, Value::U32(anim)),
            ],
        );
        let reload = |device: &mut arora::Arora, anim: u32| {
            call(
                device,
                ids::reload_animation::FUNCTION,
                vec![
                    field(ids::reload_animation::CLIP, clip("after").into()),
                    field(ids::reload_animation::ANIM, Value::U32(anim)),
                ],
            )
        };
        assert_eq!(reload(&mut device, anim), Value::Boolean(true));
        assert_eq!(reload(&mut device, anim + 1), Value::Boolean(false));
        let out = format!(
            "{:?}",
            call(
                &mut device,
                ids::step::FUNCTION,
                vec![field(ids::step::DT_NS, Value::U64(0))],
            )
        );
        assert!(
            out.contains("after") && !out.contains("before"),
            "the instance plays the reloaded tracks: {out}"
        );
    }

    #[test]
    fn every_declared_function_is_registered_and_described() {
        let declared = function_modules();
        assert_eq!(declared.len(), 25);
        let module = host_module();
        let described: Vec<Uuid> = module.descriptions().iter().map(|d| d.id).collect();
        for function in declared.keys() {
            assert!(described.contains(function), "{function} is described");
        }
    }
}
