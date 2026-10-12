//! The expression skills: the set_expression and set_facial_action_units
//! contracts' re-exports and the fragments that implement them.
//!
//! Both set the face's whole expression — a named expression or a
//! valence/arousal point, or FACS action units — and hold it until changed.
//! They describe the same configuration, so they conflict: their fragments
//! are exclusive in one domain, [`EXPRESSION`], and the latest run of either
//! takes the face over, crossfading from where it is. No module implements
//! them: the fragments carry the contracts' descriptions, so the interpreter
//! describes the methods and a remote spawns them through the interpreter
//! module.

use std::collections::HashMap;

use uuid::Uuid;
use vizij_arora_host::skills;

use crate::TaskFragment;

pub use vizij_arora_host::skills::{
    set_expression, set_facial_action_units, SetExpression, SetFacialActionUnits,
};

/// The exclusivity domain of the face's expression: one run owns it at a
/// time.
pub const EXPRESSION: &str = "expression";

/// The parameter `id → name` map the set_expression fragment serves as
/// `task/<name>` inputs.
pub fn set_expression_parameters() -> HashMap<Uuid, String> {
    use set_expression::ids::set_expression as ids;
    [ids::EXPRESSION, ids::VALENCE, ids::AROUSAL]
        .into_iter()
        .zip(skills::SET_EXPRESSION_PARAMS)
        .map(|(id, name)| (id, name.to_string()))
        .collect()
}

/// The set_expression task fragment, with the face's rig prefix on the
/// controls it writes, exclusive in [`EXPRESSION`].
pub fn set_expression_fragment(rig_prefix: &str) -> TaskFragment {
    let description = set_expression::descriptions()
        .remove(&set_expression::ids::set_expression::FUNCTION)
        .expect("the set_expression contract declares set_expression");
    TaskFragment::parse_with_rig_prefix(
        skills::SET_EXPRESSION_JSON,
        rig_prefix,
        set_expression_parameters(),
    )
    .expect("the shipped set_expression asset parses")
    .exclusive_in(EXPRESSION)
    .described(description)
}

/// The parameter `id → name` map the set_facial_action_units fragment
/// serves as `task/<name>` inputs.
pub fn set_facial_action_units_parameters() -> HashMap<Uuid, String> {
    use set_facial_action_units::ids::set_facial_action_units as ids;
    [ids::CODES, ids::INTENSITIES]
        .into_iter()
        .zip(skills::SET_FACIAL_ACTION_UNITS_PARAMS)
        .map(|(id, name)| (id, name.to_string()))
        .collect()
}

/// The set_facial_action_units task fragment, with the face's rig prefix on
/// the controls it writes, exclusive in [`EXPRESSION`].
pub fn set_facial_action_units_fragment(rig_prefix: &str) -> TaskFragment {
    let description = set_facial_action_units::descriptions()
        .remove(&set_facial_action_units::ids::set_facial_action_units::FUNCTION)
        .expect("the set_facial_action_units contract declares set_facial_action_units");
    TaskFragment::parse_with_rig_prefix(
        skills::SET_FACIAL_ACTION_UNITS_JSON,
        rig_prefix,
        set_facial_action_units_parameters(),
    )
    .expect("the shipped set_facial_action_units asset parses")
    .exclusive_in(EXPRESSION)
    .described(description)
}
