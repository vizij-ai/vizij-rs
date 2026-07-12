//! Host interface for the [`ExternalFunction`](crate::types::NodeType::ExternalFunction) node.
//!
//! The graph core knows nothing about how an external function is resolved or run: an
//! [`ExternalFunction`](crate::types::NodeType::ExternalFunction) node carries only an opaque
//! function handle plus opaque arg-key handles. Evaluating such a node calls out to a host that
//! implements [`ExternalFunctions`], which owns all the domain meaning.

use uuid::Uuid;
use vizij_api_core::Value;

/// Host that resolves and invokes an external function a node references.
///
/// `function` is the opaque handle from the node's params; `args` pairs each opaque arg-key handle
/// (from `param_ids`) with the value flowing into the matching variadic `args` input, in order.
pub trait ExternalFunctions {
    /// Invoke `function` with `args`, returning its result value or an error message.
    fn call(&mut self, function: Uuid, args: &[(Uuid, Value)]) -> Result<Value, String>;
}
