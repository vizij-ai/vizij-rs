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
use vizij_animation_module::{AnimationClip, AnimationModule, PlayerState, TrackOutput};

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
                Ok(array_of::<TrackOutput>(a.borrow_mut().step(dt_ns)))
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
                let start_time = arg(call, ids::bake::START_TIME, "start_time")?;
                let end_time = arg(call, ids::bake::END_TIME, "end_time")?;
                Ok(Value::from(
                    a.borrow().bake(anim, frame_rate, start_time, end_time),
                ))
            })
        }),
        (ids::bake_with_derivatives::FUNCTION, {
            let a = a(());
            Box::new(move |call| {
                let anim = arg(call, ids::bake_with_derivatives::ANIM, "anim")?;
                let frame_rate = arg(call, ids::bake_with_derivatives::FRAME_RATE, "frame_rate")?;
                let start_time = arg(call, ids::bake_with_derivatives::START_TIME, "start_time")?;
                let end_time = arg(call, ids::bake_with_derivatives::END_TIME, "end_time")?;
                Ok(Value::from(a.borrow().bake_with_derivatives(
                    anim, frame_rate, start_time, end_time,
                )))
            })
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
from_arg!(u32 => U32, u64 => U64, f32 => F32, String => String);

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

/// Records as the typed `Value::ArrayStructure` a declared array return is:
/// the element type named once, so an empty batch is still well-formed.
fn array_of<T: AroraType + Into<Value>>(records: Vec<T>) -> Value {
    Value::array_of_type::<T>(records.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_types::value::StructureField;
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
    }

    /// `vizij-arora-host`'s animation source names the module's functions, the
    /// `step` parameter and the `TrackOutput` fields by id; they are the
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
            node("states")["params"]["function"],
            ids::player_states::FUNCTION.to_string()
        );
        let apply = node("apply");
        assert_eq!(apply["params"]["key_field"], field("default_key"));
        assert_eq!(apply["params"]["value_field"], field("value"));
    }

    #[test]
    fn every_declared_function_is_registered_and_described() {
        let declared = function_modules();
        assert_eq!(declared.len(), 15);
        let module = host_module();
        let described: Vec<Uuid> = module.descriptions().iter().map(|d| d.id).collect();
        for function in declared.keys() {
            assert!(described.contains(function), "{function} is described");
        }
    }
}
