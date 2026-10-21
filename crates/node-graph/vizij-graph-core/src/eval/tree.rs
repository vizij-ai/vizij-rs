//! Evaluation of a graph with control nodes; see [`crate::control`] for what
//! runs when.
//!
//! The [`TreePlan`] is derived from the [`PlanCache`] once per plan
//! generation:
//! - which nodes are lazy, and the eager nodes in the plan's order;
//! - for each control node, its children: each child's source, the lazy
//!   nodes its branch runs in the plan's order, and the nodes of its branch
//!   no other branch reads, which halt with it.
//!
//! A tick runs the eager nodes in order. A control node among them ticks its
//! children through [`Children`], which runs a child's branch node by node,
//! each node at most once per tick. The tick ends with the halts of the
//! shared nodes no branch ticked.
use std::mem;

use crate::control::{Children, ControlRegistry, Status, Ticker};
use crate::schema::{child_group, CHILD_GROUP};
use crate::types::{GraphSpec, NodeType};

use super::graph_runtime::NodeRuntimeState;
use super::plan::PlanCache;
use super::value_layout::PortValue;
use super::{eval_node, evaluate_one, ByName, GraphRuntime, NodeFunctions};

/// A child of a control node.
#[derive(Debug, Clone)]
struct Child {
    /// The node the child edge comes from, and its port; `None` for a child
    /// port that holds an inline default.
    source: Option<(usize, usize)>,
    /// The input slot of the child port, for a default.
    slot: usize,
    /// The lazy nodes the branch runs, in the plan's order.
    branch: Vec<usize>,
    /// The nodes of the branch no other branch reads, readers first: what a
    /// halt of this child halts.
    exclusive: Vec<usize>,
}

/// The lazy analysis of a plan; see the [module docs](self).
#[derive(Debug, Default)]
pub(crate) struct TreePlan {
    generation: u64,
    /// The nodes' ids, for carrying what each node did across a rebuild.
    ids: Vec<String>,
    has_controls: bool,
    controls: Vec<bool>,
    lazy: Vec<bool>,
    eager: Vec<usize>,
    children: Vec<Vec<Child>>,
    /// The lazy nodes that keep state, readers first: what the end of a
    /// tick halts when no branch ticked them.
    stateful: Vec<usize>,
}

/// What a graph with control nodes keeps between ticks (see
/// [`crate::control`]); opaque to hosts.
#[derive(Debug, Default)]
pub struct ControlState {
    plan: Option<TreePlan>,
    tick: u64,
    /// The tick each node last ran at; `0` for never, or halted.
    ran: Vec<u64>,
}

impl ControlState {
    /// Bring the analysis up to date with `plan`, and say whether the graph
    /// has a control node.
    pub(crate) fn ensure(&mut self, spec: &GraphSpec, plan: &PlanCache) -> Result<bool, String> {
        let current = self
            .plan
            .as_ref()
            .is_some_and(|tree| tree.generation == plan.generation());
        if !current {
            let tree = TreePlan::build(spec, plan)?;
            // A node keeps when it last ran across an edit, by its id, so a
            // running action the edit leaves unticked is still halted.
            let ran = {
                let previous: hashbrown::HashMap<&str, u64> = self
                    .plan
                    .as_ref()
                    .map(|old| {
                        old.ids
                            .iter()
                            .map(String::as_str)
                            .zip(self.ran.iter().copied())
                            .collect()
                    })
                    .unwrap_or_default();
                tree.ids
                    .iter()
                    .map(|id| previous.get(id.as_str()).copied().unwrap_or(0))
                    .collect()
            };
            self.ran = ran;
            self.plan = Some(tree);
        }
        Ok(self.plan.as_ref().is_some_and(|tree| tree.has_controls))
    }
}

/// Whether a node of `kind` keeps state a halt drops. A URDF chain is a
/// cache of its params, not state, and survives.
fn keeps_state(kind: &NodeType) -> bool {
    matches!(
        kind,
        NodeType::Spring
            | NodeType::Damp
            | NodeType::Slew
            | NodeType::TaskRun
            | NodeType::Spawn
            | NodeType::Action
    ) || child_group(kind).is_some()
}

impl TreePlan {
    /// Analyse `plan`. Refuses a child edge with a selector, and a node a
    /// control node ticks that an eager node reads.
    pub(crate) fn build(spec: &GraphSpec, plan: &PlanCache) -> Result<Self, String> {
        let count = spec.nodes.len();
        let controls: Vec<bool> = spec
            .nodes
            .iter()
            .map(|node| child_group(&node.kind).is_some())
            .collect();
        let child_slots = |node: usize| {
            controls[node]
                .then(|| plan.layouts[node].inputs.variadic_range(CHILD_GROUP))
                .flatten()
                .map_or(0..0, |range| range.start..range.start + range.len)
        };
        let is_child_edge = |node: usize, slot: usize| child_slots(node).contains(&slot);

        // Who reads each node: (reader, its input slot).
        let mut readers: Vec<Vec<(usize, usize)>> = vec![Vec::new(); count];
        for (node, bindings) in plan.input_bindings.iter().enumerate() {
            for (slot, binding) in bindings.iter().enumerate() {
                if let Some(source) = &binding.source {
                    readers[source.node_idx].push((node, slot));
                    if is_child_edge(node, slot) && source.selector.is_some() {
                        return Err(format!(
                            "the child edge from '{}' to '{}' has a selector: a child is read \
                             whole, as a status",
                            spec.nodes[source.node_idx].id, spec.nodes[node].id
                        ));
                    }
                }
            }
        }

        let mut position = vec![0; count];
        for (at, node) in plan.order.iter().enumerate() {
            position[*node] = at;
        }

        // Readers come after what they read: walk the order backwards.
        let mut lazy = vec![false; count];
        for &node in plan.order.iter().rev() {
            lazy[node] = !readers[node].is_empty()
                && readers[node]
                    .iter()
                    .all(|&(reader, slot)| is_child_edge(reader, slot) || lazy[reader]);
        }
        for (node, node_readers) in readers.iter().enumerate() {
            if lazy[node] || !node_readers.iter().any(|&(r, s)| is_child_edge(r, s)) {
                continue;
            }
            let (eager_reader, _) = node_readers
                .iter()
                .find(|&&(reader, slot)| !is_child_edge(reader, slot) && !lazy[reader])
                .expect("a node with a child edge is lazy unless an eager node reads it");
            return Err(format!(
                "'{}' is a child of a control node, and '{}' reads it outside any control \
                 node: a node a control node ticks runs only when ticked, so everything that \
                 reads it must be ticked too",
                spec.nodes[node].id, spec.nodes[*eager_reader].id
            ));
        }

        // The lazy nodes `start` reads, through data edges only (`data`) or
        // through child edges too, in the plan's order.
        let upstream = |start: usize, through_children: bool| {
            let mut seen = vec![false; count];
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(node) = stack.pop() {
                for (slot, binding) in plan.input_bindings[node].iter().enumerate() {
                    let Some(source) = &binding.source else {
                        continue;
                    };
                    if !through_children && is_child_edge(node, slot) {
                        continue;
                    }
                    if lazy[source.node_idx] && !seen[source.node_idx] {
                        seen[source.node_idx] = true;
                        stack.push(source.node_idx);
                    }
                }
            }
            let mut found: Vec<usize> = (0..count).filter(|node| seen[*node]).collect();
            found.sort_by_key(|node| position[*node]);
            found
        };

        let mut children = vec![Vec::new(); count];
        for control in (0..count).filter(|node| controls[*node]) {
            for slot in child_slots(control) {
                let binding = &plan.input_bindings[control][slot];
                let Some(source) = &binding.source else {
                    children[control].push(Child {
                        source: None,
                        slot,
                        branch: Vec::new(),
                        exclusive: Vec::new(),
                    });
                    continue;
                };
                let root = source.node_idx;
                let branch = upstream(root, false);
                // A node is exclusive to this child when everything reading
                // it is exclusive too, or is this child edge itself.
                let mut exclusive = vec![false; count];
                let mut order = Vec::new();
                for &node in upstream(root, true).iter().rev() {
                    exclusive[node] = readers[node].iter().all(|&(reader, reader_slot)| {
                        (reader == control && reader_slot == slot && node == root)
                            || exclusive[reader]
                    });
                    if exclusive[node] {
                        order.push(node);
                    }
                }
                children[control].push(Child {
                    source: Some((root, source.slot)),
                    slot,
                    branch,
                    exclusive: order,
                });
            }
        }

        let mut eager: Vec<usize> = plan.order.iter().copied().filter(|n| !lazy[*n]).collect();
        eager.shrink_to_fit();
        let mut stateful: Vec<usize> = plan
            .order
            .iter()
            .rev()
            .copied()
            .filter(|node| lazy[*node] && keeps_state(&spec.nodes[*node].kind))
            .collect();
        stateful.shrink_to_fit();
        Ok(TreePlan {
            generation: plan.generation(),
            ids: spec.nodes.iter().map(|node| node.id.clone()).collect(),
            has_controls: controls.iter().any(|control| *control),
            controls,
            lazy,
            eager,
            children,
            stateful,
        })
    }
}

impl ControlState {
    /// Whether node `index` is lazy under the last evaluation's plan.
    pub(crate) fn is_lazy(&self, index: usize) -> bool {
        self.plan
            .as_ref()
            .and_then(|tree| tree.lazy.get(index).copied())
            .unwrap_or(false)
    }
}

/// Refuse what [`TreePlan::build`] refuses in `spec`, under `plan`.
pub(crate) fn check(spec: &GraphSpec, plan: &PlanCache) -> Result<(), String> {
    TreePlan::build(spec, plan).map(drop)
}

/// Evaluate a graph with control nodes; the caller has brought the plan up
/// to date and started the tick ([`super::evaluate_all_inner`]).
pub(crate) fn evaluate(
    rt: &mut GraphRuntime,
    spec: &GraphSpec,
    plan: &PlanCache,
    functions: Option<&mut dyn NodeFunctions>,
    controls: &ControlRegistry,
    by_name: ByName,
) -> Result<(), String> {
    let mut state = mem::take(&mut rt.control);
    state.tick += 1;
    let tree = state.plan.take().expect("the caller ensured the tree plan");
    let result = {
        let mut evaluation = Evaluation {
            rt,
            spec,
            plan,
            tree: &tree,
            ran: &mut state.ran,
            tick: state.tick,
            functions,
            controls,
            by_name,
        };
        evaluation.run()
    };
    state.plan = Some(tree);
    rt.control = state;
    result
}

struct Evaluation<'a, 'f> {
    rt: &'a mut GraphRuntime,
    spec: &'a GraphSpec,
    plan: &'a PlanCache,
    tree: &'a TreePlan,
    ran: &'a mut Vec<u64>,
    tick: u64,
    functions: Option<&'f mut dyn NodeFunctions>,
    controls: &'a ControlRegistry,
    by_name: ByName,
}

impl Evaluation<'_, '_> {
    fn host(&mut self) -> Option<&mut dyn NodeFunctions> {
        match self.functions {
            Some(ref mut functions) => Some(&mut **functions),
            None => None,
        }
    }

    /// Whether `node` ran at the tick before this one. `0` is never.
    fn ran_before(&self, node: usize) -> bool {
        self.ran[node] != 0 && self.ran[node] == self.tick - 1
    }

    fn run(&mut self) -> Result<(), String> {
        let tree = self.tree;
        for &node in &tree.eager {
            self.evaluate(node)?;
        }
        // A shared node no branch ticked at this tick, ticked at the
        // previous one, halts now.
        for &node in &tree.stateful {
            if self.ran_before(node) {
                self.halt(node);
            }
        }
        Ok(())
    }

    /// Run `node` unless it ran at this tick.
    fn evaluate(&mut self, node: usize) -> Result<(), String> {
        if self.ran[node] == self.tick {
            return Ok(());
        }
        self.ran[node] = self.tick;
        if self.tree.controls[node] {
            return self.tick_control(node);
        }
        let Evaluation {
            rt,
            spec,
            plan,
            functions,
            by_name,
            ..
        } = self;
        let host: Option<&mut dyn NodeFunctions> = match functions {
            Some(functions) => Some(&mut **functions),
            None => None,
        };
        evaluate_one(rt, spec, plan, node, host, *by_name)
    }

    fn tick_control(&mut self, node: usize) -> Result<(), String> {
        let specs = self.spec;
        let spec = &specs.nodes[node];
        let mut control = match self.rt.node_states.remove(&spec.id) {
            Some(NodeRuntimeState::Control(control)) => control,
            _ => self.controls.create(spec)?,
        };
        let count = self.tree.children[node].len();
        let (status, ticked) = {
            let mut children = Children {
                ticker: self,
                node,
                params: &spec.params,
                ticked: vec![false; count],
            };
            let status = control.tick(&mut children);
            (status, children.ticked)
        };
        // A child ticked at the previous tick and not at this one halts now.
        for (child, was_ticked) in ticked.iter().enumerate() {
            if *was_ticked {
                continue;
            }
            if let Some((source, _)) = self.tree.children[node][child].source {
                if self.ran_before(source) {
                    self.halt_child(node, child)?;
                }
            }
        }
        let status = status.map_err(|message| format!("node '{}': {message}", spec.id))?;
        self.rt
            .node_states
            .insert(spec.id.clone(), NodeRuntimeState::Control(control));

        let layout = &self.plan.layouts[node].outputs;
        let mut ports = mem::take(&mut self.rt.outputs_vec[node]);
        ports.clear();
        {
            let mut outputs = eval_node::OutputSlots::new(&mut ports, layout);
            outputs.set_value("out", status.value())?;
        }
        if self.by_name == ByName::Kept {
            let named = eval_node::materialize_outputs(layout, &ports);
            self.rt.outputs.insert(spec.id.clone(), named);
        }
        self.rt.outputs_vec[node] = ports;
        Ok(())
    }

    /// Drop `node`'s state; cancel its call when it is a running action.
    fn halt(&mut self, node: usize) {
        let specs = self.spec;
        let id = &specs.nodes[node].id;
        match self.rt.node_states.remove(id) {
            Some(NodeRuntimeState::Action(action)) => {
                if let Some(functions) = self.host() {
                    functions.halt(action.module, action.function);
                }
            }
            // A URDF chain caches the node's params: it survives a halt.
            #[cfg(feature = "urdf_ik")]
            Some(state @ NodeRuntimeState::UrdfKinematics(_)) => {
                self.rt.node_states.insert(id.clone(), state);
            }
            _ => {}
        }
        self.ran[node] = 0;
    }
}

impl Ticker for Evaluation<'_, '_> {
    fn tick_child(&mut self, node: usize, child: usize) -> Result<Status, String> {
        let tree = self.tree;
        let child = &tree.children[node][child];
        let Some((source, port)) = child.source else {
            let default = self.plan.input_bindings[node][child.slot]
                .default
                .as_ref()
                .map(|port| port.value.clone());
            return Ok(default.as_ref().map_or(Status::Success, Status::of_child));
        };
        for &member in &child.branch {
            self.evaluate(member)?;
        }
        let value = self.rt.outputs_vec[source]
            .get(port)
            .map(|port: &PortValue| &port.value)
            .ok_or_else(|| {
                format!(
                    "child '{}' produced no value on the port its edge reads",
                    self.spec.nodes[source].id
                )
            })?;
        Ok(Status::of_child(value))
    }

    fn halt_child(&mut self, node: usize, child: usize) -> Result<(), String> {
        let tree = self.tree;
        for &member in &tree.children[node][child].exclusive {
            if self.ran[member] == self.tick || self.ran_before(member) {
                self.halt(member);
            }
        }
        Ok(())
    }
}
