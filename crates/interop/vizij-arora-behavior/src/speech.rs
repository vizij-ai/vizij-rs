//! The speech skill: the say contract's re-export — identical across the
//! text-to-speech providers — and the fragment that implements the skill.
//!
//! A provider implements [`Say`] under its own module id: its `say` call is
//! re-invoked each tick while `Running` (the poll-on-tick contract) and
//! streams through its mutable parameters the viseme at the audio playhead
//! (`viseme`, one of the face standard's shapes) and the utterance while its
//! audio plays (`speech`: the text from the moment playback starts, whether
//! or not synthesis has finished, empty before and after). The skill's
//! fragment hosts that call on the run's own argument bundle, drives the
//! lips from the streamed viseme and writes the utterance as the face's
//! speech state; the device routes the call to the provider it registered
//! by the function id.

use std::collections::HashMap;

use uuid::Uuid;
use vizij_arora_host::skills;

use crate::TaskFragment;

/// The say contract, which lets a provider crate implement the call without
/// depending on the skill that hosts it.
pub use vizij_arora_host::skills::{say, Say, SILENCE_VISEME};

/// The parameter `id → name` map the fragment serves as `task/<name>`
/// inputs: the call's inputs, not its `viseme` and `speech` outputs.
pub fn say_parameters() -> HashMap<Uuid, String> {
    [say::ids::say::TEXT, say::ids::say::VOICE]
        .into_iter()
        .zip(skills::SAY_PARAMS)
        .map(|(id, name)| (id, name.to_string()))
        .collect()
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
    if let Some((_, spec)) = embedded.iter().find(|(id, _)| id == say::NAME) {
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
            .any(|n| n["params"]["function"] == say::ids::say::FUNCTION.to_string()));
        let outputs: Vec<&str> = nodes
            .iter()
            .filter(|n| n["kind"] == "output" || n["type"] == "output")
            .filter_map(|n| n["params"]["path"].as_str())
            .collect();
        assert!(outputs.contains(&"rig/f/standard/vizij/viseme/PP"));
        assert!(outputs.contains(&"rig/f/standard/vizij/viseme"));
        assert!(outputs.contains(&"rig/f/standard/vizij/speech"));
        assert!(outputs.contains(&"task/status"));
        assert!(outputs.contains(&"task/feedback"));
    }

    #[test]
    fn the_fragment_serves_the_call_s_inputs_by_their_contract_ids() {
        assert_eq!(
            say_parameters(),
            HashMap::from([
                (say::ids::say::TEXT, "text".to_string()),
                (say::ids::say::VOICE, "voice".to_string()),
            ])
        );
    }
}
