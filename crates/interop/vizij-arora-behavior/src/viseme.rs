//! The viseme skill's exterior contract: the described `play_viseme`
//! function a device registers, and the fragment that implements it.
//!
//! The behavior is data — `vizij-arora-host`'s play_viseme skill fragment,
//! grafted per run by the interpreter ([`TaskFragment`]) — so the module
//! here carries only the contract: the described signature a bridge
//! discovers (DescribeMethods) and exposes as an action. Signature and
//! fragment derive from one parameter list
//! ([`skills::PLAY_VISEME_PARAMS`](vizij_arora_host::skills::PLAY_VISEME_PARAMS)),
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

/// The viseme module's id on the device.
pub fn module_id() -> Uuid {
    gen_uuid_from_str("viseme-module")
}

/// The play_viseme function's id.
pub fn play_viseme_id() -> Uuid {
    gen_uuid_from_str(skills::PLAY_VISEME_FUNCTION)
}

/// The play_viseme task fragment, parsed from the shipped asset with the
/// face's rig prefix on the controls it writes — what the device's
/// interpreter grafts per run. Exclusive: a new call takes over the lips,
/// the run before it ends preempted.
pub fn play_viseme_fragment(rig_prefix: &str) -> TaskFragment {
    TaskFragment::parse_with_rig_prefix(
        skills::PLAY_VISEME_JSON,
        rig_prefix,
        play_viseme_parameters(),
    )
    .expect("the shipped play_viseme asset parses")
    .exclusive()
}

/// The play_viseme fragment the device registers, honoring the face's
/// pinned override: a bundle-embedded `skill::play_viseme` fragment replaces
/// the shipped behavior. An embedded fragment that does not hold the task
/// contract is refused loudly and the built-in serves.
pub fn play_viseme_fragment_from(
    embedded: &[(String, serde_json::Value)],
    rig_prefix: &str,
) -> TaskFragment {
    if let Some((_, spec)) = embedded
        .iter()
        .find(|(id, _)| id == skills::PLAY_VISEME_FUNCTION)
    {
        match TaskFragment::parse_with_rig_prefix(
            &spec.to_string(),
            rig_prefix,
            play_viseme_parameters(),
        ) {
            Ok(fragment) => {
                log::info!(
                    "play_viseme: the face's embedded skill fragment overrides the built-in"
                );
                return fragment.exclusive();
            }
            Err(e) => {
                log::warn!("embedded play_viseme fragment refused ({e}); the built-in serves")
            }
        }
    }
    play_viseme_fragment(rig_prefix)
}

/// The parameter `id → name` map shared by the fragment and the signature.
pub fn play_viseme_parameters() -> HashMap<Uuid, String> {
    skills::PLAY_VISEME_PARAMS
        .iter()
        .map(|name| (gen_uuid_from_str(name), name.to_string()))
        .collect()
}

/// The described play_viseme signature: `(shape, weight)` returning the
/// behavior `Status` — the task-run marker a bridge exposes as an action.
pub fn play_viseme_signature() -> Function {
    let kinds = [
        PrimitiveKind::String, // shape, one of the standard's 15
        PrimitiveKind::F32,    // weight, [0, 1]
    ];
    let mut parameters = HashMap::new();
    let mut parameter_ordering = Vec::new();
    for (name, kind) in skills::PLAY_VISEME_PARAMS.iter().zip(kinds) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fragment_parses_and_takes_the_rig_prefix() {
        let fragment = play_viseme_fragment("rig/f/");
        let spec = serde_json::to_value(&fragment.spec).unwrap();
        let outputs: Vec<&str> = spec["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["kind"] == "output" || n["type"] == "output")
            .filter_map(|n| n["params"]["path"].as_str())
            .collect();
        assert!(outputs.contains(&"rig/f/standard/vizij/viseme/aa"));
        assert!(outputs.contains(&"rig/f/standard/vizij/viseme"));
        assert!(outputs.contains(&"task/status"));
        assert!(outputs.contains(&"task/feedback"));
    }

    #[test]
    fn the_signature_is_action_shaped_over_the_parameter_list() {
        let signature = play_viseme_signature();
        let names: Vec<&str> = signature
            .parameter_ordering
            .iter()
            .map(|id| signature.parameters[id].name.as_str())
            .collect();
        assert_eq!(names, skills::PLAY_VISEME_PARAMS);
        assert!(matches!(
            signature.return_ty,
            FrozenTy::FrozenScalar(FrozenScalar { ref reference }) if reference.id == STATUS_ENUMERATION_ID
        ));
    }
}
