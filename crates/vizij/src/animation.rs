//! The animation module, registered **host-side** — no wasm, no artifact
//! dependency, so the workspace stays on stable.
//!
//! Built like arora's interpreter module: a [`ModuleBuilder`] with one closure
//! per animation function, finished into a [`HostModule`] the device registers
//! with [`AroraBuilder::with_host_module`](arora::AroraBuilder::with_host_module).
//! Each closure marshals the `Call` at the `Value` boundary using the module
//! crate's own generated conversions (`TryFrom<Value>` in, `Into<Value>` out)
//! and calls its native functions, which run the same `vizij-animation-core`
//! engine the wasm module wraps. A graph `ExternalFunction` node then dispatches
//! `step`/`player_states` to these exactly as it would to the loaded wasm guest.

use std::collections::HashMap;

use arora::{HostModule, ModuleBuilder};
use arora_types::call::{Call, CallError, CallResult};
use arora_types::value::{StructureWithoutId, Value};
use uuid::Uuid;
use vizij_animation_module::{ids, AnimationClip, PlayerState, TrackOutput};

/// The animation module as a host module: its functions dispatch in-process,
/// under the same ids the wasm module exports.
pub fn host_module() -> HostModule {
    ModuleBuilder::new(ids::MODULE)
        .function(ids::LOAD_ANIMATION, |call| {
            u32_result(vizij_animation_module::load_animation(arg_clip(&call)))
        })
        .function(ids::CREATE_PLAYER, |call| {
            u32_result(vizij_animation_module::create_player(arg_string(&call, 0)))
        })
        .function(ids::ADD_INSTANCE, |call| {
            u32_result(vizij_animation_module::add_instance(
                arg_u32(&call, 0),
                arg_u32(&call, 1),
            ))
        })
        .function(ids::STEP, |call| {
            let outputs = vizij_animation_module::step(arg_u64(&call, 0));
            value_result(array_structure(ids::TRACK_OUTPUT_TYPE, outputs))
        })
        .function(ids::PLAY, |call| {
            u32_result(vizij_animation_module::play(arg_u32(&call, 0)))
        })
        .function(ids::PAUSE, |call| {
            u32_result(vizij_animation_module::pause(arg_u32(&call, 0)))
        })
        .function(ids::STOP, |call| {
            u32_result(vizij_animation_module::stop(arg_u32(&call, 0)))
        })
        .function(ids::SEEK, |call| {
            u32_result(vizij_animation_module::seek(
                arg_u32(&call, 0),
                arg_u64(&call, 1),
            ))
        })
        .function(ids::SET_SPEED, |call| {
            u32_result(vizij_animation_module::set_speed(
                arg_u32(&call, 0),
                arg_f32(&call, 1),
            ))
        })
        .function(ids::SET_LOOP, |call| {
            u32_result(vizij_animation_module::set_loop(
                arg_u32(&call, 0),
                arg_string(&call, 1),
            ))
        })
        .function(ids::SET_WEIGHT, |call| {
            u32_result(vizij_animation_module::set_weight(
                arg_u32(&call, 0),
                arg_u32(&call, 1),
                arg_f32(&call, 2),
            ))
        })
        .function(ids::REMOVE_INSTANCE, |call| {
            u32_result(vizij_animation_module::remove_instance(
                arg_u32(&call, 0),
                arg_u32(&call, 1),
            ))
        })
        .function(ids::PLAYER_STATES, |_call| {
            let states = vizij_animation_module::player_states();
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
