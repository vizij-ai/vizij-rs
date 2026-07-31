//! The gaze skill's exterior contract: the described `look_at` method the
//! device registers, and the fragment that implements it.
//!
//! The behavior is data — `vizij-arora-host`'s look_at skill fragment,
//! grafted per run by the interpreter ([`TaskFragment`]) — so the module
//! here carries only the contract: the described signature a bridge
//! discovers (DescribeMethods) and an exposure profile binds to the ROS4HRI
//! `/skill/look_at` action (`interaction_skills/LookAt`). Signature and
//! fragment derive from one parameter list ([`skills::LOOK_AT_PARAMS`]), so
//! they cannot drift.

use std::collections::HashMap;

use arora::{HostModule, ModuleBuilder};
use arora_behavior_tree_types::STATUS_ENUMERATION_ID;
use arora_types::call::CallResult;
use arora_types::gen_uuid_from_str;
use arora_types::record::module::frozen::{Function, Parameter};
use arora_types::record::ty::{FrozenScalar, FrozenTy, PrimitiveKind};
use arora_types::record::{FrozenReference, Version};
use uuid::Uuid;
use vizij_arora_behavior::{task, TaskFragment};
use vizij_arora_host::skills;

/// The gaze module's id on the device.
pub fn module_id() -> Uuid {
    gen_uuid_from_str("gaze-module")
}

/// The look_at function's id.
pub fn look_at_id() -> Uuid {
    gen_uuid_from_str(skills::LOOK_AT_FUNCTION)
}

/// The look_at task fragment, parsed from the shipped asset — what the
/// device's interpreter grafts per goal.
pub fn look_at_fragment() -> TaskFragment {
    let parameters: HashMap<Uuid, String> = skills::LOOK_AT_PARAMS
        .iter()
        .map(|name| (gen_uuid_from_str(name), name.to_string()))
        .collect();
    TaskFragment::parse(skills::LOOK_AT_JSON, parameters).expect("the shipped look_at asset parses")
}

/// The described look_at signature: `(policy, target, frame)` returning the
/// behavior `Status` — the task-run marker a bridge exposes as an action.
fn look_at_signature() -> Function {
    let kinds = [
        PrimitiveKind::String,   // policy
        PrimitiveKind::ArrayF32, // target (vec3, meters, face frame)
        PrimitiveKind::String,   // frame
    ];
    let mut parameters = HashMap::new();
    let mut parameter_ordering = Vec::new();
    for (name, kind) in skills::LOOK_AT_PARAMS.iter().zip(kinds) {
        let id = gen_uuid_from_str(name);
        parameter_ordering.push(id);
        parameters.insert(
            id,
            Parameter {
                name: name.to_string(),
                ty: FrozenTy::from(kind),
                mutable: false,
            },
        );
    }
    Function {
        parameters,
        parameter_ordering,
        return_ty: FrozenTy::FrozenScalar(FrozenScalar {
            reference: FrozenReference {
                id: STATUS_ENUMERATION_ID,
                version: Version::parse("1.0.0").expect("a valid version"),
            },
        }),
    }
}

/// The gaze module: the look_at contract, described so bridges discover it.
/// The closure is only reached when the device did not register the fragment
/// (a misconfiguration) — it fails the run rather than pretending to gaze.
pub fn host_module() -> HostModule {
    ModuleBuilder::new(module_id())
        .described_function(
            look_at_id(),
            skills::LOOK_AT_FUNCTION,
            look_at_signature(),
            |_call| {
                log::warn!("look_at invoked as a module call: no task fragment is registered");
                Ok(CallResult {
                    ret: task::failure(),
                    mutated: Vec::new(),
                })
            },
        )
        .build()
}
