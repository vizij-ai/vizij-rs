//! Node-function abstraction for the
//! [`ExternalFunction`](crate::types::NodeType::ExternalFunction) node.
//!
//! The graph core knows nothing about how a node-function is resolved or run: an
//! [`ExternalFunction`](crate::types::NodeType::ExternalFunction) node carries only a stable
//! function id (a [`Uuid`]) plus opaque arg-key handles. Evaluating such a node calls out to a
//! [`NodeFunctions`] host that owns all the domain meaning. Values are always the shared runtime
//! type [`vizij_api_core::Value`].

use hashbrown::HashMap;
use uuid::Uuid;
use vizij_api_core::Value;

/// A single external function a node can invoke.
///
/// Identified by a stable [`id`](NodeFunction::id) used for serialization and by remote callers;
/// [`call`](NodeFunction::call) maps the argument values to a result value.
pub trait NodeFunction {
    /// Stable identifier of this function.
    fn id(&self) -> Uuid;
    /// Invoke the function with `args`, returning its result value or an error message.
    ///
    /// Each entry pairs an opaque arg-key handle (from the node's `param_ids`) with the value
    /// flowing into the matching variadic `args` input, in order.
    fn call(&mut self, args: &[(Uuid, Value)]) -> Result<Value, String>;
}

/// The host the graph consults to invoke a node-function by id.
///
/// `function` is the stable id from the node's params; `args` pairs each opaque arg-key handle
/// with the value flowing into the matching variadic `args` input, in order.
pub trait NodeFunctions {
    /// Invoke the function identified by `function` with `args`.
    fn call(&mut self, function: Uuid, args: &[(Uuid, Value)]) -> Result<Value, String>;
}

/// A registry of [`NodeFunction`]s keyed by their stable id.
///
/// Implements [`NodeFunctions`] by dispatching each call to the matching entry, so a set of
/// individual node-functions can serve as a host without further wiring.
#[derive(Default)]
pub struct NodeFunctionRegistry {
    functions: HashMap<Uuid, Box<dyn NodeFunction>>,
}

impl NodeFunctionRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `function` under its own [`NodeFunction::id`], replacing any prior entry.
    pub fn register(&mut self, function: Box<dyn NodeFunction>) {
        self.functions.insert(function.id(), function);
    }
}

impl NodeFunctions for NodeFunctionRegistry {
    fn call(&mut self, function: Uuid, args: &[(Uuid, Value)]) -> Result<Value, String> {
        let entry = self
            .functions
            .get_mut(&function)
            .ok_or_else(|| format!("no node-function registered for id '{function}'"))?;
        entry.call(args)
    }
}
