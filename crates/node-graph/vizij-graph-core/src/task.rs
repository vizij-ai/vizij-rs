//! The behavior `Status` vocabulary a [`TaskRun`](crate::types::NodeType::TaskRun)
//! node speaks on the value plane.
//!
//! A run's lifecycle travels as the behavior `Status` enumeration — `Running` /
//! `Success` / `Failure` over unit payloads — the value an interpreter writes to
//! a run's status key and the one the ROS action plane maps to a goal status.
//! The type and variant ids come from `arora-behavior-tree-types`, the crate
//! declaring the enumeration, so the encoding cannot drift from the rest of the
//! Arora ecosystem. The graph core only constructs these values and recognizes
//! terminality; it never interprets the payloads.

use arora_behavior_tree_types::{
    STATUS_ENUMERATION_ID, STATUS_FAILURE_VARIANT_ID, STATUS_RUNNING_VARIANT_ID,
    STATUS_SUCCESS_VARIANT_ID,
};
use uuid::Uuid;
use vizij_api_core::value::{Enumeration, Value};

fn status(variant_id: Uuid) -> Value {
    Value::Enumeration(Enumeration {
        id: STATUS_ENUMERATION_ID,
        variant_id,
        value: Box::new(Value::Unit),
    })
}

/// The `Status::Running` value: the run is live and will be advanced again.
pub fn running() -> Value {
    status(STATUS_RUNNING_VARIANT_ID)
}

/// The `Status::Success` value: the run ended cleanly.
pub fn success() -> Value {
    status(STATUS_SUCCESS_VARIANT_ID)
}

/// The `Status::Failure` value: the run ended in failure — also what a halt
/// writes, since a halted run did not reach its goal.
pub fn failure() -> Value {
    status(STATUS_FAILURE_VARIANT_ID)
}

/// Whether `value` is a terminal `Status` — `Success` or `Failure`. Anything
/// that is not a `Status` enumeration is not terminal.
pub fn is_terminal(value: &Value) -> bool {
    match value {
        Value::Enumeration(e) if e.id == STATUS_ENUMERATION_ID => {
            e.variant_id != STATUS_RUNNING_VARIANT_ID
        }
        _ => false,
    }
}

/// Coerce a module call's return into the run's `Status`: a `Status` value
/// passes through; any other return means the call completed its work in one
/// invocation — `Success`.
pub fn coerce(ret: Value) -> Value {
    match &ret {
        Value::Enumeration(e) if e.id == STATUS_ENUMERATION_ID => ret,
        _ => success(),
    }
}
