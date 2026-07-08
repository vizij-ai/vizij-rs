//! Host-callout seam for graph nodes that must invoke behaviour living outside
//! the graph.
//!
//! Today the only escape from a node handler to the host is the [`Output`] sink
//! pushing a write into `rt.writes`. A [`ModuleCall`] node needs the *opposite*
//! direction: to call *out* to the host, hand it arguments, and receive a value
//! back. [`GraphHost`] is that seam.
//!
//! It is deliberately **arora-agnostic** — this keeps `vizij-graph-core` free of
//! any arora dependency. The host speaks in graph-core's own [`Value`] and an
//! opaque, extensible [`CallTarget`]; the concrete arora `CallBridge` binding
//! lives in the interop layer (`vizij-arora-behavior`), which is the only place
//! arora types appear.
//!
//! [`Output`]: crate::types::NodeType::Output
//! [`ModuleCall`]: crate::types::NodeType::ModuleCall

use vizij_api_core::Value;

/// An extensible descriptor of *what* a graph node is asking the host to invoke.
///
/// The single variant today is a module-function target. The enum is
/// intentionally open-ended and `#[non_exhaustive]`: the design note
/// (`docs/design-note-behavior-recursion-and-interpreter-composition.md`, §2)
/// prescribes a future `Behavior { interpreter, behavior }` variant so a graph
/// node can hand off to *another interpreter* (a behaviour-tree, or a sub-graph)
/// through this **same** seam, with no change to [`GraphHost`]. Widening the
/// target — not re-cutting the seam — is how that future stays additive.
///
/// The ids here are graph-core's own opaque strings, **not** arora types: the
/// host maps them onto whatever call vocabulary it speaks.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallTarget {
    /// Invoke `function` within `module`. Both ids are host-opaque strings.
    ModuleFn {
        /// Module identifier (host-opaque).
        module: String,
        /// Function identifier within the module (host-opaque).
        function: String,
    },
    // Future (design note §2):
    //   Behavior { interpreter: String, behavior: String },
    // Run a named behaviour under a named interpreter — makes interpreters
    // first-class callables and expresses graph-in-graph recursion as a
    // `Behavior` target whose interpreter is the graph interpreter itself.
}

/// The seam by which a graph node calls out to its host.
///
/// A [`ModuleCall`](crate::types::NodeType::ModuleCall) node builds a
/// [`CallTarget`] plus an args [`Value`], calls [`GraphHost::call`], and exposes
/// the returned [`Value`] as its output. The host performs the dispatch — e.g.
/// over an arora `CallBridge` — and is free to fail with a message.
pub trait GraphHost {
    /// Dispatch `target` with `args`, returning the produced value.
    fn call(&mut self, target: &CallTarget, args: Value) -> Result<Value, String>;
}

/// A [`GraphHost`] with nothing wired up: every call fails.
///
/// Graphs that contain no [`ModuleCall`](crate::types::NodeType::ModuleCall)
/// node evaluate perfectly well with this host — only *actually hitting* a
/// `ModuleCall` node surfaces the error. [`evaluate_all`] uses `NoHost`
/// internally so existing, host-less callers keep their signature; wire a real
/// host through [`evaluate_all_with_host`] when a graph needs to call out.
///
/// [`evaluate_all`]: crate::eval::evaluate_all
/// [`evaluate_all_with_host`]: crate::eval::evaluate_all_with_host
pub struct NoHost;

impl GraphHost for NoHost {
    fn call(&mut self, target: &CallTarget, _args: Value) -> Result<Value, String> {
        Err(format!(
            "graph evaluated without a GraphHost, but a ModuleCall node targeted {target:?}; \
             evaluate with `evaluate_all_with_host` and supply a host"
        ))
    }
}
