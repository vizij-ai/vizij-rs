//! The speech skill's exterior contract: the `say` function — its ids and
//! described signature, identical across the text-to-speech providers — and
//! the fragment that implements the skill.
//!
//! A provider is a host module whose `say(text, voice) -> Status` call is
//! re-invoked each tick while `Running` (the poll-on-tick contract) and
//! streams the viseme at the audio playhead through the mutable `viseme`
//! parameter, as one of the face standard's shapes. The skill's fragment
//! hosts that call on the run's own argument bundle and drives the lips from
//! the streamed viseme; the device registers the provider under the
//! function id so the fragment's call reaches it.

use std::collections::HashMap;

use arora_behavior_tree_types::STATUS_ENUMERATION_ID;
use arora_types::record::module::frozen::{Function, Parameter};
use arora_types::record::ty::{FrozenScalar, FrozenTy, PrimitiveKind};
use arora_types::record::{FrozenReference, Version};
use uuid::Uuid;
use vizij_arora_host::skills;

use crate::TaskFragment;

/// The `say` function's id and those of its parameters — identical across
/// providers, which is what lets a provider crate implement the call without
/// depending on the skill that hosts it.
pub use vizij_arora_host::skills::{
    SAY_ID, SAY_TEXT_PARAM_ID, SAY_VISEME_PARAM_ID, SAY_VOICE_PARAM_ID, SILENCE_VISEME,
};

/// `say(text, voice) -> Status`, with a mutable `viseme` out-parameter. The
/// `Status` return is the task-run marker a bridge exposes as an action.
pub fn say_signature() -> Function {
    let mut parameters = HashMap::new();
    let mut parameter_ordering = Vec::new();
    for (id, name, kind, mutable) in [
        (SAY_TEXT_PARAM_ID, "text", PrimitiveKind::String, false),
        (SAY_VOICE_PARAM_ID, "voice", PrimitiveKind::String, false),
        (SAY_VISEME_PARAM_ID, "viseme", PrimitiveKind::String, true),
    ] {
        parameter_ordering.push(id);
        parameters.insert(
            id,
            Parameter {
                name: name.to_string(),
                ty: FrozenTy::from(kind),
                mutable,
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

/// The parameter `id → name` map the fragment serves as `task/<name>`
/// inputs: the call's inputs, not its `viseme` output.
fn say_parameters() -> HashMap<Uuid, String> {
    HashMap::from([
        (SAY_TEXT_PARAM_ID, "text".to_string()),
        (SAY_VOICE_PARAM_ID, "voice".to_string()),
    ])
}

/// The say task fragment, parsed from the shipped asset with the face's rig
/// prefix on the controls it writes — what the device's interpreter grafts
/// per run.
pub fn say_fragment(rig_prefix: &str) -> TaskFragment {
    TaskFragment::parse_with_rig_prefix(skills::SAY_JSON, rig_prefix, say_parameters())
        .expect("the shipped say asset parses")
}

/// The say fragment the device registers, honoring the face's pinned
/// override: a bundle-embedded `skill::say` fragment replaces the shipped
/// behavior. An embedded fragment that does not hold the task contract is
/// refused loudly and the built-in serves.
pub fn say_fragment_from(
    embedded: &[(String, serde_json::Value)],
    rig_prefix: &str,
) -> TaskFragment {
    if let Some((_, spec)) = embedded.iter().find(|(id, _)| id == skills::SAY_FUNCTION) {
        match TaskFragment::parse_with_rig_prefix(&spec.to_string(), rig_prefix, say_parameters()) {
            Ok(fragment) => {
                log::info!("say: the face's embedded skill fragment overrides the built-in");
                return fragment;
            }
            Err(e) => log::warn!("embedded say fragment refused ({e}); the built-in serves"),
        }
    }
    say_fragment(rig_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fragment_hosts_the_say_call_and_writes_the_lips() {
        let fragment = say_fragment("rig/f/");
        let spec = serde_json::to_value(&fragment.spec).unwrap();
        let nodes = spec["nodes"].as_array().unwrap();
        assert!(nodes
            .iter()
            .any(|n| n["params"]["function"] == SAY_ID.to_string()));
        let outputs: Vec<&str> = nodes
            .iter()
            .filter(|n| n["kind"] == "output" || n["type"] == "output")
            .filter_map(|n| n["params"]["path"].as_str())
            .collect();
        assert!(outputs.contains(&"rig/f/standard/vizij/viseme/PP"));
        assert!(outputs.contains(&"rig/f/standard/vizij/viseme"));
        assert!(outputs.contains(&"task/status"));
        assert!(outputs.contains(&"task/feedback"));
    }

    #[test]
    fn the_signature_streams_the_viseme_as_an_out_parameter() {
        let signature = say_signature();
        let viseme = &signature.parameters[&SAY_VISEME_PARAM_ID];
        assert!(viseme.mutable);
        assert_eq!(viseme.name, "viseme");
        assert!(!signature.parameters[&SAY_TEXT_PARAM_ID].mutable);
    }
}
