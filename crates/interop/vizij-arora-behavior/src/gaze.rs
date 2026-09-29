//! The gaze skill: the look_at contract's re-export and the fragment that
//! implements it.
//!
//! The behavior is data — `vizij-arora-host`'s look_at skill fragment,
//! grafted per run by the interpreter ([`TaskFragment`]). No module
//! implements look_at: the fragment carries the contract's description, so
//! the interpreter describes the method — what a bridge discovers
//! (DescribeMethods) and an exposure profile binds to the ROS4HRI
//! `/skill/look_at` action (`interaction_skills/LookAt`) — and a remote
//! spawns it through the interpreter module.

use std::collections::HashMap;

use arora_types::record::module::frozen;
use uuid::Uuid;
use vizij_arora_host::skills;

use crate::TaskFragment;

pub use vizij_arora_host::skills::{look_at, LookAt};

/// The look_at task fragment, parsed from the shipped asset — what the
/// device's interpreter grafts per goal.
pub fn look_at_fragment() -> TaskFragment {
    TaskFragment::parse(skills::LOOK_AT_JSON, look_at_parameters())
        .expect("the shipped look_at asset parses")
        .described(look_at_description())
}

/// look_at as the contract describes it.
fn look_at_description() -> frozen::Export {
    look_at::descriptions()
        .remove(&look_at::ids::look_at::FUNCTION)
        .expect("the look_at contract declares look_at")
}

/// The look_at fragment the device registers, honoring the face's pinned
/// override: a bundle-embedded `skill::look_at` fragment replaces the shipped
/// behavior (the `standard::<id>` precedence, applied to the skill plane). An
/// embedded fragment that does not hold the task contract is refused loudly
/// and the built-in serves.
pub fn look_at_fragment_from(embedded: &[(String, serde_json::Value)]) -> TaskFragment {
    if let Some((_, spec)) = embedded.iter().find(|(id, _)| id == look_at::NAME) {
        match TaskFragment::parse(&spec.to_string(), look_at_parameters()) {
            Ok(fragment) => {
                log::info!("look_at: the face's embedded skill fragment overrides the built-in");
                return fragment.described(look_at_description());
            }
            Err(e) => log::warn!("embedded look_at fragment refused ({e}); the built-in serves"),
        }
    }
    look_at_fragment()
}

/// The parameter `id → name` map the fragment serves as `task/<name>`
/// inputs: every parameter of the call.
pub fn look_at_parameters() -> HashMap<Uuid, String> {
    [
        look_at::ids::look_at::POLICY,
        look_at::ids::look_at::TARGET,
        look_at::ids::look_at::FRAME,
    ]
    .into_iter()
    .zip(skills::LOOK_AT_PARAMS)
    .map(|(id, name)| (id, name.to_string()))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_behavior::BehaviorInterpreter;

    /// The fragment, built-in or the face's own, carries look_at's
    /// description: the interpreter describes the method.
    #[test]
    fn the_interpreter_describes_look_at_from_its_fragment() {
        let embedded = vec![(
            look_at::NAME.to_string(),
            serde_json::from_str(skills::LOOK_AT_JSON).unwrap(),
        )];
        for fragment in [look_at_fragment(), look_at_fragment_from(&embedded)] {
            let mut graph = crate::ProcessingGraph::from_spec(
                crate::parse_spec(r#"{ "nodes": [], "edges": [] }"#).unwrap(),
            )
            .unwrap();
            graph.set_task_fragment(look_at::ids::look_at::FUNCTION, fragment);
            let described = graph.described_methods();
            assert_eq!(described.len(), 1);
            assert_eq!(
                described[&look_at::ids::look_at::FUNCTION],
                look_at_description()
            );
        }
    }

    #[test]
    fn the_fragment_serves_the_call_s_parameters_by_their_contract_ids() {
        assert_eq!(
            look_at_parameters(),
            HashMap::from([
                (look_at::ids::look_at::POLICY, "policy".to_string()),
                (look_at::ids::look_at::TARGET, "target".to_string()),
                (look_at::ids::look_at::FRAME, "frame".to_string()),
            ])
        );
    }
}
