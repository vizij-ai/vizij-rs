//! The viseme skills: the play_viseme, set_viseme and play_visemes
//! contracts' re-exports and the fragments that implement them.
//!
//! The behavior is data — `vizij-arora-host`'s skill fragments, grafted per
//! run by the interpreter ([`TaskFragment`]). No module implements them: the
//! fragments carry the contracts' descriptions, so the interpreter describes
//! the methods — what a bridge discovers (DescribeMethods) and exposes — and
//! a remote spawns them through the interpreter module.
//!
//! Every player of the lips — these three and the say skill — is exclusive
//! in one domain, [`LIPS`]: one run owns the lips at a time, and a new
//! command of any of them takes over from the run before it, which ends
//! preempted.

use std::collections::HashMap;

use arora_types::record::module::frozen;
use uuid::Uuid;
use vizij_arora_host::skills;

use crate::TaskFragment;

pub use vizij_arora_host::skills::{
    play_viseme, play_visemes, set_viseme, PlayViseme, PlayVisemes, SetViseme,
};

/// The exclusivity domain of the lips: one player owns them at a time.
pub const LIPS: &str = "lips";

/// play_viseme as the contract describes it.
fn play_viseme_description() -> frozen::Export {
    play_viseme::descriptions()
        .remove(&play_viseme::ids::play_viseme::FUNCTION)
        .expect("the play_viseme contract declares play_viseme")
}

/// The play_viseme task fragment, parsed from the shipped asset with the
/// face's rig prefix on the controls it writes — what the device's
/// interpreter grafts per run. Exclusive in [`LIPS`]: a new call takes over
/// the lips, the run before it ends preempted.
pub fn play_viseme_fragment(rig_prefix: &str) -> TaskFragment {
    TaskFragment::parse_with_rig_prefix(
        skills::PLAY_VISEME_JSON,
        rig_prefix,
        play_viseme_parameters(),
    )
    .expect("the shipped play_viseme asset parses")
    .exclusive_in(LIPS)
    .described(play_viseme_description())
}

/// The play_viseme fragment the device registers, honoring the face's
/// pinned override: a bundle-embedded `skill::play_viseme` fragment replaces
/// the shipped behavior. An embedded fragment that does not hold the task
/// contract is refused loudly and the built-in serves.
pub fn play_viseme_fragment_from(
    embedded: &[(String, serde_json::Value)],
    rig_prefix: &str,
) -> TaskFragment {
    if let Some((_, spec)) = embedded.iter().find(|(id, _)| id == play_viseme::NAME) {
        match TaskFragment::parse_with_rig_prefix(
            &spec.to_string(),
            rig_prefix,
            play_viseme_parameters(),
        ) {
            Ok(fragment) => {
                log::info!(
                    "play_viseme: the face's embedded skill fragment overrides the built-in"
                );
                return fragment
                    .exclusive_in(LIPS)
                    .described(play_viseme_description());
            }
            Err(e) => {
                log::warn!("embedded play_viseme fragment refused ({e}); the built-in serves")
            }
        }
    }
    play_viseme_fragment(rig_prefix)
}

/// The parameter `id → name` map the fragment serves as `task/<name>`
/// inputs: every parameter of the call.
pub fn play_viseme_parameters() -> HashMap<Uuid, String> {
    [
        play_viseme::ids::play_viseme::SHAPE,
        play_viseme::ids::play_viseme::WEIGHT,
        play_viseme::ids::play_viseme::DURATION,
    ]
    .into_iter()
    .zip(skills::PLAY_VISEME_PARAMS)
    .map(|(id, name)| (id, name.to_string()))
    .collect()
}

/// The parameter `id → name` map the set_viseme fragment serves as
/// `task/<name>` inputs.
pub fn set_viseme_parameters() -> HashMap<Uuid, String> {
    HashMap::from([(
        set_viseme::ids::set_viseme::VISEME,
        skills::SET_VISEME_PARAMS[0].to_string(),
    )])
}

/// The set_viseme task fragment, with the face's rig prefix on the controls
/// it writes, exclusive in [`LIPS`].
pub fn set_viseme_fragment(rig_prefix: &str) -> TaskFragment {
    let description = set_viseme::descriptions()
        .remove(&set_viseme::ids::set_viseme::FUNCTION)
        .expect("the set_viseme contract declares set_viseme");
    TaskFragment::parse_with_rig_prefix(
        skills::SET_VISEME_JSON,
        rig_prefix,
        set_viseme_parameters(),
    )
    .expect("the shipped set_viseme asset parses")
    .exclusive_in(LIPS)
    .described(description)
}

/// The parameter `id → name` map the play_visemes fragment serves as
/// `task/<name>` inputs.
pub fn play_visemes_parameters() -> HashMap<Uuid, String> {
    use play_visemes::ids::play_visemes as ids;
    [ids::VISEMES, ids::STARTS, ids::DURATIONS]
        .into_iter()
        .zip(skills::PLAY_VISEMES_PARAMS)
        .map(|(id, name)| (id, name.to_string()))
        .collect()
}

/// The play_visemes task fragment, with the face's rig prefix on the
/// controls it writes, exclusive in [`LIPS`].
pub fn play_visemes_fragment(rig_prefix: &str) -> TaskFragment {
    let description = play_visemes::descriptions()
        .remove(&play_visemes::ids::play_visemes::FUNCTION)
        .expect("the play_visemes contract declares play_visemes");
    TaskFragment::parse_with_rig_prefix(
        skills::PLAY_VISEMES_JSON,
        rig_prefix,
        play_visemes_parameters(),
    )
    .expect("the shipped play_visemes asset parses")
    .exclusive_in(LIPS)
    .described(description)
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
    fn the_fragment_serves_the_call_s_parameters_by_their_contract_ids() {
        assert_eq!(
            play_viseme_parameters(),
            HashMap::from([
                (play_viseme::ids::play_viseme::SHAPE, "shape".to_string()),
                (play_viseme::ids::play_viseme::WEIGHT, "weight".to_string()),
                (
                    play_viseme::ids::play_viseme::DURATION,
                    "duration".to_string()
                ),
            ])
        );
    }
}
