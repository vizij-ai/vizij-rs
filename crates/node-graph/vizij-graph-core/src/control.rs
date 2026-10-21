//! Behavior control in the node graph: nodes that tick other nodes.
//!
//! A **control node** ticks its children, the nodes wired into its `child_N`
//! inputs, in an order and as many times as its logic decides. The nodes a
//! child needs run when the child is ticked, not by themselves, so a branch
//! that is not ticked costs nothing. That is what a behavior tree's control
//! nodes do, and the graph gains them without a second language: a condition
//! is a computed value, an action is a host call, and both are nodes like
//! any other.
//!
//! ```text
//!            sensor/distance ─► mul ─► clamp ─► lessthan ─► condition ─┐
//!                                                                      ├─► fallback ─► output
//!   battery ─► lessthan ─► condition ─► sequence ◄─ action(dock) ──────┘
//! ```
//!
//! # What runs when
//!
//! - **A child edge** is an edge into a control node's `child_N` input. A node
//!   is **lazy** when it has readers and every one of them is a child edge or
//!   a lazy node. A lazy node runs only when a control node ticks the child it
//!   serves. Every other node is **eager** and runs every tick, in the graph's
//!   order, as every node of a graph with no control node does.
//! - **Ticking a child** runs the lazy nodes it needs, its *branch*, in the
//!   graph's order, and reads the child's value as a status: a `Status` value
//!   is itself, any other value is `Success` (see [`Status::of_child`]). So
//!   any node can be a child: an `output` writes its key when it is ticked.
//! - **Once per tick.** A node runs at most once per tick, however many
//!   branches need it: a value shared by several branches is computed by the
//!   first one ticked and read by the others.
//! - **A ticked node is read only inside control.** A node a control node
//!   ticks may be read only by other lazy nodes; a graph where an eager node
//!   reads it is refused, since the eager reader would run it every tick.
//!
//! # Halting
//!
//! A node that keeps state (a `damp`, a `spring`, a `sequence_with_memory`, a
//! running `action`, a registered control) is **halted** when it stops being
//! ticked: its state is dropped, and a running action's call is cancelled
//! through [`NodeFunctions::halt`](crate::eval::NodeFunctions::halt). A
//! smoothing node starts again from its input, so a branch ticked anew does
//! not jump.
//!
//! - **At once,** when a control node stops ticking a child it ticked at the
//!   previous tick, or halts one explicitly ([`Children::halt`]): the nodes of
//!   that child's branch that no other branch reads are halted before the
//!   control node returns, so a fallback cancels the action it leaves before
//!   it ticks the next one.
//! - **At the end of the tick,** for a node several branches share: it is
//!   halted when none of them ticked it at this tick, having been ticked at
//!   the previous one.
//!
//! # Extending
//!
//! Every control node implements [`Control`]. The built-in ones are node
//! kinds of their own (`sequence`, `sequence_with_memory`, `fallback`,
//! `parallel`, `inverter`); a host adds others by registering a factory in a
//! [`ControlRegistry`] under a name, which a `control` node names in its
//! `control` param.
use hashbrown::HashMap;

use crate::types::{NodeParams, NodeSpec, NodeType};

pub use crate::task::Status;

/// A node that ticks its children. One instance exists per node, created
/// when the node is first ticked and dropped when it is halted, so it may
/// keep what it needs between ticks. It is `Send + Sync`, as the runtime that
/// holds it is.
pub trait Control: std::fmt::Debug + Send + Sync {
    /// This tick's status. `children` ticks and halts the node's children;
    /// a child it ticked at the previous tick and does not tick now is halted
    /// when this returns.
    fn tick(&mut self, children: &mut Children<'_>) -> Result<Status, String>;
}

/// What a [`Control`] reaches its children through.
pub(crate) trait Ticker {
    fn tick_child(&mut self, node: usize, child: usize) -> Result<Status, String>;
    fn halt_child(&mut self, node: usize, child: usize) -> Result<(), String>;
}

/// A control node's children, for the duration of one tick.
pub struct Children<'t> {
    pub(crate) ticker: &'t mut dyn Ticker,
    pub(crate) node: usize,
    pub(crate) params: &'t NodeParams,
    pub(crate) ticked: Vec<bool>,
}

impl Children<'_> {
    /// How many children the node has.
    pub fn len(&self) -> usize {
        self.ticked.len()
    }

    /// Whether the node has no child.
    pub fn is_empty(&self) -> bool {
        self.ticked.is_empty()
    }

    /// The node's params.
    pub fn params(&self) -> &NodeParams {
        self.params
    }

    /// Tick child `child`: run its branch, and return its status. A child
    /// ticked twice in one tick runs once and gives the same status.
    pub fn tick(&mut self, child: usize) -> Result<Status, String> {
        let ticked = self
            .ticked
            .get_mut(child)
            .ok_or_else(|| format!("no child {child}"))?;
        *ticked = true;
        self.ticker.tick_child(self.node, child)
    }

    /// Halt child `child` now: the nodes of its branch no other branch reads
    /// drop their state, and a running action is cancelled.
    pub fn halt(&mut self, child: usize) -> Result<(), String> {
        if child >= self.ticked.len() {
            return Err(format!("no child {child}"));
        }
        self.ticker.halt_child(self.node, child)
    }
}

/// Builds the [`Control`] of a `control` node from its spec.
pub type ControlFactory = Box<dyn Fn(&NodeSpec) -> Result<Box<dyn Control>, String> + Send + Sync>;

/// The control nodes a host adds, by the name a `control` node's `control`
/// param gives.
#[derive(Default)]
pub struct ControlRegistry {
    factories: HashMap<String, ControlFactory>,
}

impl ControlRegistry {
    /// A registry with no host control node: the built-in ones only.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `factory` under `name`, replacing what was there.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        factory: impl Fn(&NodeSpec) -> Result<Box<dyn Control>, String> + Send + Sync + 'static,
    ) {
        self.factories.insert(name.into(), Box::new(factory));
    }

    /// The control of node `spec`: a built-in one for its kind, or the one a
    /// `control` node names.
    pub(crate) fn create(&self, spec: &NodeSpec) -> Result<Box<dyn Control>, String> {
        Ok(match spec.kind {
            NodeType::Sequence => Box::new(Sequence),
            NodeType::SequenceWithMemory => Box::new(SequenceWithMemory::default()),
            NodeType::Fallback => Box::new(Fallback),
            NodeType::Parallel => Box::new(Parallel {
                success_threshold: spec.params.success_threshold,
            }),
            NodeType::Inverter => Box::new(Inverter),
            NodeType::Control => {
                let name = spec.params.control.as_deref().ok_or_else(|| {
                    format!(
                        "control node '{}' names no control in params.control",
                        spec.id
                    )
                })?;
                let factory = self.factories.get(name).ok_or_else(|| {
                    format!(
                        "control node '{}' names the control '{name}', which the host did not \
                         register",
                        spec.id
                    )
                })?;
                factory(spec)?
            }
            _ => return Err(format!("node '{}' is not a control node", spec.id)),
        })
    }
}

/// `sequence`: in order from the first child at every tick; stops at the
/// first child that does not succeed.
#[derive(Debug)]
struct Sequence;

impl Control for Sequence {
    fn tick(&mut self, children: &mut Children<'_>) -> Result<Status, String> {
        for child in 0..children.len() {
            match children.tick(child)? {
                Status::Success => {}
                status => return Ok(status),
            }
        }
        Ok(Status::Success)
    }
}

/// `sequence_with_memory`: resumes at the child that was running.
#[derive(Debug, Default)]
struct SequenceWithMemory {
    current: usize,
}

impl Control for SequenceWithMemory {
    fn tick(&mut self, children: &mut Children<'_>) -> Result<Status, String> {
        while self.current < children.len() {
            match children.tick(self.current)? {
                Status::Success => self.current += 1,
                Status::Running => return Ok(Status::Running),
                Status::Failure => {
                    self.current = 0;
                    return Ok(Status::Failure);
                }
            }
        }
        self.current = 0;
        Ok(Status::Success)
    }
}

/// `fallback`: in order from the first child at every tick; stops at the
/// first child that does not fail.
#[derive(Debug)]
struct Fallback;

impl Control for Fallback {
    fn tick(&mut self, children: &mut Children<'_>) -> Result<Status, String> {
        for child in 0..children.len() {
            match children.tick(child)? {
                Status::Failure => {}
                status => return Ok(status),
            }
        }
        Ok(Status::Failure)
    }
}

/// `parallel`: every child at every tick, against a success threshold.
#[derive(Debug)]
struct Parallel {
    success_threshold: Option<u32>,
}

impl Control for Parallel {
    fn tick(&mut self, children: &mut Children<'_>) -> Result<Status, String> {
        let count = children.len();
        let needed = self
            .success_threshold
            .map_or(count, |threshold| (threshold as usize).min(count));
        let mut statuses = Vec::with_capacity(count);
        for child in 0..count {
            statuses.push(children.tick(child)?);
        }
        let succeeded = statuses.iter().filter(|s| **s == Status::Success).count();
        let failed = statuses.iter().filter(|s| **s == Status::Failure).count();
        let status = if succeeded >= needed {
            Status::Success
        } else if failed > count - needed {
            Status::Failure
        } else {
            return Ok(Status::Running);
        };
        for (child, child_status) in statuses.iter().enumerate() {
            if *child_status == Status::Running {
                children.halt(child)?;
            }
        }
        Ok(status)
    }
}

/// `inverter`: its one child, `Success` and `Failure` swapped.
#[derive(Debug)]
struct Inverter;

impl Control for Inverter {
    fn tick(&mut self, children: &mut Children<'_>) -> Result<Status, String> {
        if children.len() != 1 {
            return Err(format!(
                "an inverter has one child, this one has {}",
                children.len()
            ));
        }
        Ok(match children.tick(0)? {
            Status::Success => Status::Failure,
            Status::Failure => Status::Success,
            Status::Running => Status::Running,
        })
    }
}
