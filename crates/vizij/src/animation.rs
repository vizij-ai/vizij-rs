//! The animation module, registered **host-side** — no wasm, no artifact
//! dependency, so the workspace stays on stable.
//!
//! Built like arora's interpreter module: a [`ModuleBuilder`] with one closure
//! per animation function, finished into a [`HostModule`] the device registers
//! with [`AroraBuilder::with_host_module`](arora::AroraBuilder::with_host_module).
//! Each closure marshals the `Call` at the `Value` boundary using the module
//! crate's own generated conversions (`TryFrom<Value>` in, `Into<Value>` out)
//! and calls the module's [`AnimationModule`], which runs the same
//! `vizij-animation-core` engine the wasm module wraps. A graph
//! `ExternalFunction` node then dispatches `step`/`player_states` to these
//! exactly as it would to the loaded wasm guest. Each host module owns its
//! own engine, so two devices in one process never share players.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use arora::{HostModule, ModuleBuilder};
use arora_types::call::{Call, CallError, CallResult};
use arora_types::value::{StructureWithoutId, Value};
use uuid::Uuid;
use vizij_animation_module::{ids, AnimationClip, AnimationModule, PlayerState, TrackOutput};

/// The animation module as a host module over an engine of its own: its
/// functions dispatch in-process, under the same ids the wasm module exports.
pub fn host_module() -> HostModule {
    let animation = Rc::new(RefCell::new(AnimationModule::new()));
    let a = animation.clone();
    let builder = ModuleBuilder::new(ids::MODULE).function(ids::LOAD_ANIMATION, move |call| {
        u32_result(a.borrow_mut().load_animation(arg_clip(&call)))
    });
    let a = animation.clone();
    let builder = builder.function(ids::CREATE_PLAYER, move |call| {
        u32_result(a.borrow_mut().create_player(arg_string(&call, 0)))
    });
    let a = animation.clone();
    let builder = builder.function(ids::ADD_INSTANCE, move |call| {
        u32_result(
            a.borrow_mut()
                .add_instance(arg_u32(&call, 0), arg_u32(&call, 1)),
        )
    });
    let a = animation.clone();
    let builder = builder.function(ids::STEP, move |call| {
        let outputs = a.borrow_mut().step(arg_u64(&call, 0));
        value_result(array_structure(ids::TRACK_OUTPUT_TYPE, outputs))
    });
    let a = animation.clone();
    let builder = builder.function(ids::PLAY, move |call| {
        u32_result(a.borrow_mut().play(arg_u32(&call, 0)))
    });
    let a = animation.clone();
    let builder = builder.function(ids::PAUSE, move |call| {
        u32_result(a.borrow_mut().pause(arg_u32(&call, 0)))
    });
    let a = animation.clone();
    let builder = builder.function(ids::STOP, move |call| {
        u32_result(a.borrow_mut().stop(arg_u32(&call, 0)))
    });
    let a = animation.clone();
    let builder = builder.function(ids::SEEK, move |call| {
        u32_result(a.borrow_mut().seek(arg_u32(&call, 0), arg_u64(&call, 1)))
    });
    let a = animation.clone();
    let builder = builder.function(ids::SET_SPEED, move |call| {
        u32_result(
            a.borrow_mut()
                .set_speed(arg_u32(&call, 0), arg_f32(&call, 1)),
        )
    });
    let a = animation.clone();
    let builder = builder.function(ids::SET_LOOP, move |call| {
        u32_result(
            a.borrow_mut()
                .set_loop(arg_u32(&call, 0), arg_string(&call, 1)),
        )
    });
    let a = animation.clone();
    let builder = builder.function(ids::SET_WEIGHT, move |call| {
        u32_result(a.borrow_mut().set_weight(
            arg_u32(&call, 0),
            arg_u32(&call, 1),
            arg_f32(&call, 2),
        ))
    });
    let a = animation.clone();
    let builder = builder.function(ids::REMOVE_INSTANCE, move |call| {
        u32_result(
            a.borrow_mut()
                .remove_instance(arg_u32(&call, 0), arg_u32(&call, 1)),
        )
    });
    let a = animation;
    builder
        .function(ids::PLAYER_STATES, move |_call| {
            let states = a.borrow().player_states();
            value_result(array_structure(ids::PLAYER_STATE_TYPE, states))
        })
        .build()
}

/// `function id -> module id` over the animation functions — what
/// `ProcessingGraph::set_function_modules` needs so a bare function handle
/// (the animation source's `step`/`player_states` nodes) routes to this module.
pub fn function_modules() -> HashMap<Uuid, Uuid> {
    [
        ids::LOAD_ANIMATION,
        ids::CREATE_PLAYER,
        ids::ADD_INSTANCE,
        ids::STEP,
        ids::PLAY,
        ids::PAUSE,
        ids::STOP,
        ids::SEEK,
        ids::SET_SPEED,
        ids::SET_LOOP,
        ids::SET_WEIGHT,
        ids::REMOVE_INSTANCE,
        ids::PLAYER_STATES,
    ]
    .into_iter()
    .map(|function| (function, ids::MODULE))
    .collect()
}

// --- Call marshaling ---------------------------------------------------------

fn value_result(value: Value) -> Result<CallResult, CallError> {
    Ok(CallResult {
        ret: value,
        mutated: Vec::new(),
    })
}

fn u32_result(value: u32) -> Result<CallResult, CallError> {
    value_result(Value::U32(value))
}

fn arg(call: &Call, index: usize) -> Option<&Value> {
    call.args.get(index).map(|field| field.value.as_ref())
}

fn arg_u32(call: &Call, index: usize) -> Option<u32> {
    match arg(call, index) {
        Some(Value::U32(n)) => Some(*n),
        _ => None,
    }
}

fn arg_u64(call: &Call, index: usize) -> Option<u64> {
    match arg(call, index) {
        Some(Value::U64(n)) => Some(*n),
        _ => None,
    }
}

fn arg_f32(call: &Call, index: usize) -> Option<f32> {
    match arg(call, index) {
        Some(Value::F32(f)) => Some(*f),
        _ => None,
    }
}

fn arg_string(call: &Call, index: usize) -> Option<String> {
    match arg(call, index) {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Decode the clip argument through the module's own generated conversion.
fn arg_clip(call: &Call) -> Option<AnimationClip> {
    arg(call, 0).and_then(|value| AnimationClip::try_from(value.clone()).ok())
}

/// Wrap a batch of records (each `Into<Value>` produces a `Value::Structure`)
/// into the `Value::ArrayStructure` the graph's path-less `output` node reads —
/// `type_id` names the element struct so an empty batch is still well-formed.
fn array_structure<T: Into<Value>>(type_id: Uuid, records: Vec<T>) -> Value {
    let elements = records
        .into_iter()
        .filter_map(|record| match record.into() {
            Value::Structure(structure) => Some(StructureWithoutId {
                fields: structure.fields,
            }),
            _ => None,
        })
        .collect();
    Value::ArrayStructure {
        id: type_id,
        elements,
    }
}

// Only the module's boundary structs satisfy `array_structure`'s bound.
const _: fn() = || {
    fn assert_into_value<T: Into<Value>>() {}
    assert_into_value::<TrackOutput>();
    assert_into_value::<PlayerState>();
};

#[cfg(test)]
mod tests {
    //! Two devices in one process own two engines.
    use super::*;
    use arora_types::call::Call;
    use arora_types::value::StructureField;
    use vizij_arora_store::BlackboardStore;

    fn device() -> arora::Arora {
        arora::Arora::builder()
            .with_data_store(Box::new(BlackboardStore::new()))
            .with_host_module(host_module())
            .build()
            .expect("build arora")
    }

    fn call(function: Uuid, args: Vec<Value>) -> Call {
        Call {
            module_id: Some(ids::MODULE),
            id: function,
            args: args
                .into_iter()
                .map(|value| StructureField {
                    id: Uuid::nil(),
                    value: Box::new(value),
                })
                .collect(),
        }
    }

    fn player_count(device: &mut arora::Arora) -> usize {
        let result = device
            .call(call(ids::PLAYER_STATES, Vec::new()))
            .expect("player_states");
        match result.ret {
            Value::ArrayStructure { elements, .. } => elements.len(),
            other => panic!("expected an array of PlayerState, got {other:?}"),
        }
    }

    #[test]
    fn two_devices_own_separate_animation_engines() {
        let mut first = device();
        let mut second = device();
        first
            .call(call(
                ids::CREATE_PLAYER,
                vec![Value::String("only-in-first".into())],
            ))
            .expect("create_player");
        assert_eq!(player_count(&mut first), 1);
        assert_eq!(player_count(&mut second), 0);
    }
}
