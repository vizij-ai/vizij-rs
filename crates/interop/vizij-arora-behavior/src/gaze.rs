//! The gaze skill's exterior contract: the described `look_at` function a
//! device registers, and the fragment that implements it.
//!
//! The behavior is data — `vizij-arora-host`'s look_at skill fragment,
//! grafted per run by the interpreter ([`TaskFragment`]) — so the module
//! here carries only the contract: the described signature a bridge
//! discovers (DescribeMethods) and an exposure profile binds to the ROS4HRI
//! `/skill/look_at` action (`interaction_skills/LookAt`). Signature and
//! fragment derive from one parameter list
//! ([`skills::LOOK_AT_PARAMS`](vizij_arora_host::skills::LOOK_AT_PARAMS)),
//! so they cannot drift.

use std::collections::HashMap;

use arora_behavior_tree_types::STATUS_ENUMERATION_ID;
use arora_types::gen_uuid_from_str;
use arora_types::record::module::frozen::{Function, Parameter};
use arora_types::record::ty::{FrozenScalar, FrozenTy, PrimitiveKind};
use arora_types::record::{FrozenReference, Version};
use uuid::Uuid;
use vizij_arora_host::skills;

use crate::TaskFragment;

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
    TaskFragment::parse(skills::LOOK_AT_JSON, look_at_parameters())
        .expect("the shipped look_at asset parses")
}

/// The look_at fragment the device registers, honoring the face's pinned
/// override: a bundle-embedded `skill::look_at` fragment replaces the shipped
/// behavior (the `standard::<id>` precedence, applied to the skill plane). An
/// embedded fragment that does not hold the task contract is refused loudly
/// and the built-in serves.
pub fn look_at_fragment_from(embedded: &[(String, serde_json::Value)]) -> TaskFragment {
    if let Some((_, spec)) = embedded
        .iter()
        .find(|(id, _)| id == skills::LOOK_AT_FUNCTION)
    {
        match TaskFragment::parse(&spec.to_string(), look_at_parameters()) {
            Ok(fragment) => {
                log::info!("look_at: the face's embedded skill fragment overrides the built-in");
                return fragment;
            }
            Err(e) => log::warn!("embedded look_at fragment refused ({e}); the built-in serves"),
        }
    }
    look_at_fragment()
}

/// The parameter `id → name` map shared by the fragment and the signature.
pub fn look_at_parameters() -> HashMap<Uuid, String> {
    skills::LOOK_AT_PARAMS
        .iter()
        .map(|name| (gen_uuid_from_str(name), name.to_string()))
        .collect()
}

/// The described look_at signature: `(policy, target, frame)` returning the
/// behavior `Status` — the task-run marker a bridge exposes as an action.
pub fn look_at_signature() -> Function {
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
