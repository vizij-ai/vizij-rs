//! Node-function abstraction for the
//! [`ExternalFunction`](crate::types::NodeType::ExternalFunction) node.
//!
//! The graph core knows nothing about how a node-function is resolved or run: an
//! [`ExternalFunction`](crate::types::NodeType::ExternalFunction) node carries only a stable
//! function id (a string) plus opaque arg-key handles. Evaluating such a node calls out to a
//! [`NodeFunctions`] host that owns all the domain meaning.

use hashbrown::HashMap;
use uuid::Uuid;

use crate::graph_value::GraphValue;

/// A single external function a node can invoke.
///
/// Identified by a stable [`id`](NodeFunction::id) used for serialization and by remote callers;
/// [`call`](NodeFunction::call) maps the argument values to a result value.
pub trait NodeFunction<V: GraphValue> {
    /// Stable identifier of this function.
    fn id(&self) -> &str;
    /// Invoke the function with `args`, returning its result value or an error message.
    ///
    /// Each entry pairs an opaque arg-key handle (from the node's `param_ids`) with the value
    /// flowing into the matching variadic `args` input, in order.
    fn call(&mut self, args: &[(Uuid, V)]) -> Result<V, String>;
}

/// The host the graph consults to invoke a node-function by id.
///
/// `function` is the stable id from the node's params; `args` pairs each opaque arg-key handle
/// with the value flowing into the matching variadic `args` input, in order.
pub trait NodeFunctions<V: GraphValue> {
    /// Invoke the function identified by `function` with `args`.
    fn call(&mut self, function: &str, args: &[(Uuid, V)]) -> Result<V, String>;
}

/// A registry of [`NodeFunction`]s keyed by their stable id.
///
/// Implements [`NodeFunctions`] by dispatching each call to the matching entry, so a set of
/// individual node-functions can serve as a host without further wiring.
pub struct NodeFunctionRegistry<V: GraphValue> {
    functions: HashMap<String, Box<dyn NodeFunction<V>>>,
}

impl<V: GraphValue> Default for NodeFunctionRegistry<V> {
    fn default() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }
}

impl<V: GraphValue> NodeFunctionRegistry<V> {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `function` under its own [`NodeFunction::id`], replacing any prior entry.
    pub fn register(&mut self, function: Box<dyn NodeFunction<V>>) {
        self.functions.insert(function.id().to_string(), function);
    }
}

impl<V: GraphValue> NodeFunctions<V> for NodeFunctionRegistry<V> {
    fn call(&mut self, function: &str, args: &[(Uuid, V)]) -> Result<V, String> {
        let entry = self
            .functions
            .get_mut(function)
            .ok_or_else(|| format!("no node-function registered for id '{function}'"))?;
        entry.call(args)
    }
}
