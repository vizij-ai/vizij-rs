//! The node graph's tests, run on the behavior tree: each graph is
//! evaluated by a [`TreeRuntime`].
//!
//! - [`tests`] holds the nodes' own tests, and [`blend_tests`],
//!   [`spring_tests`], [`zero_dt_tests`], [`required_inputs_tests`] and
//!   [`external_function_tests`] those of the blend, smoothing, clock,
//!   required-input and external-call nodes.
//! - [`structure_tests`] holds the topology and selection tests.
//! - [`fixture_tests`] runs every fixture evaluating every node and what the
//!   outputs read, and again with its parameters edited in place halfway
//!   through.
//! - [`param_edit_tests`] holds the parameter edits the tree makes in place
//!   and those it refuses, and [`placement_tests`] where a call to another
//!   module goes, read from its function's description.
//!
//! [`TreeRuntime`]: crate::tree_runtime::TreeRuntime
mod blend_tests;
mod external_function_tests;
mod fixture_tests;
mod param_edit_tests;
mod placement_tests;
mod required_inputs_tests;
mod spring_tests;
mod structure_tests;
mod tests;
mod zero_dt_tests;
