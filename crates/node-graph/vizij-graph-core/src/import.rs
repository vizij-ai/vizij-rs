//! The importer: a Vizij node graph ([`GraphSpec`], its JSON form) as the
//! nodes of a behavior tree, run by `arora-behavior-tree`.
//!
//! The graph translates node for node:
//!
//! - **A node** is a call to its kind's function ([`crate::functions`]),
//!   which the tree holds **detached**: it runs when what it outputs is read,
//!   once per tick, and only when its arguments changed. Its node parameters
//!   are literal arguments; a stateful node's state is its literal `state`
//!   argument, which the call writes back.
//! - **An edge** is an argument reading the source node's output: `@node`
//!   for a node whose one port is `out`, a selection of the port's field out
//!   of the structure a multi-port node returns otherwise. An edge's selector
//!   is a [`select`](crate::functions::SELECT) node between the two.
//! - **An inline default** is a literal argument, for an input no edge feeds
//!   or one whose edge names a port the source does not have.
//! - **An `input` node** is a call to [`vizij_functions::input_function`],
//!   reading the store key its path names, a variable of that name. A node
//!   reading the clock reads the variables [`ImportOptions::time`] and
//!   [`ImportOptions::dt`].
//! - **An `output` node** is a call to [`vizij_functions::output_function`].
//!   With a path, it binds its return value to the store key of its path:
//!   it is a **sink**, written at every tick after the tree ticks; of two
//!   sinks of one key, the later wins, as the later `output` does in the
//!   graph. An `output` with no path, which writes the keys a batch of
//!   records names, is listed in [`ImportedGraph::keyed_outputs`] for its
//!   host to apply.
//! - **A node declaring the shapes of its outputs** is followed by a call to
//!   [`vizij_functions::check_function`], which its readers read in its
//!   place: a value the node returns that does not fit its declared shape
//!   fails the call. An `input` or `output` node checks its own `out`.
//! - **A node that acts** at each evaluation (`externalfunction`, `taskrun`,
//!   `spawn`) is a leaf the tree holds under its root `PARALLEL`, in the
//!   graph's order, ticked at every tick. Its return value is the tree-local
//!   variable `.node/<id>`, which its readers read. A graph with no such node
//!   has a `SUCCEED` leaf as its root.
use std::collections::{HashMap, HashSet};

use arora_behavior::FunctionIndex;
use arora_behavior_tree::nodes::{PARALLEL_FUNCTION_ID, SUCCEED_FUNCTION_ID};
use arora_behavior_tree::schema::{Expression, Node, NodeParameterId, _RET_PARAM_ID};
use arora_behavior_tree::variable::VariableResolver;
use arora_behavior_tree::{load_behavior_tree_nodes_with, BehaviorTree};
use arora_types::data::Key;
use uuid::Uuid;
use vizij_api_core::Value;

use crate::eval::plan::output_layout;
use crate::functions::{
    declares_input, dt_param, encode, encode_selector, external_function, function_id, input_param,
    is_stateful, node_param, output_field, placement, ports_param, reads_dt, reads_time,
    select_function, selector_param, state_param, time_param, VARIADIC_PORTS,
};
use crate::kind::Kind;
use crate::params::Params;
use crate::types::{GraphSpec, InputConnection, NodeSpec, NodeType};
use crate::vizij_functions;

pub use crate::functions::Placement;

/// Where a translated graph reads its clock from.
#[derive(Clone, Debug)]
pub struct ImportOptions {
    /// The variable the `time` and `oscillator` nodes read the graph's time
    /// from: seconds as a float, or nanoseconds as an unsigned integer.
    pub time: String,
    /// The variable the smoothing nodes read the step's duration from, in
    /// the same units.
    pub dt: String,
}

impl Default for ImportOptions {
    /// The device's clock: `arora/time` and `arora/dt`.
    fn default() -> Self {
        Self {
            time: arora_behavior::built_in::TIME.to_string(),
            dt: arora_behavior::built_in::DT.to_string(),
        }
    }
}

/// A node of the graph as the tree holds it.
#[derive(Clone, Debug)]
pub struct GraphNode {
    /// Its id in the graph.
    pub id: String,
    /// Its kind.
    pub kind: NodeType,
    /// The tree node that calls its function.
    pub tree_id: Uuid,
    /// Its output ports, in order. A node whose ports are `out` alone
    /// returns that port's value; any other returns a structure of them.
    pub ports: Vec<String>,
    /// Whether the tree holds it, as a leaf ticked at every tick, rather
    /// than detached.
    pub attached: bool,
    /// Whether the tree leaves it out: a call to a function that returns
    /// nothing and writes nothing back ([`Placement::Ignored`]). It has no
    /// ports, so an edge from it reads as an input with no edge.
    pub ignored: bool,
    /// The tree node checking the shapes the node declares of its outputs,
    /// which its readers read in its place.
    pub check: Option<Uuid>,
}

impl GraphNode {
    /// Whether the node returns the value of its one port, `out`.
    pub fn returns_out(&self) -> bool {
        self.ports.len() == 1 && self.ports[0] == "out"
    }

    /// The value of its port `port` in `returned`, what its call returned:
    /// `None` for a port it does not have, or one its evaluation left unset.
    pub fn port_value(&self, returned: &Value, port: &str) -> Option<Value> {
        if self.returns_out() {
            return (port == "out" && *returned != Value::Unit).then(|| returned.clone());
        }
        let Value::Structure(structure) = returned else {
            return None;
        };
        let field = output_field(port);
        structure
            .fields
            .iter()
            .find(|candidate| candidate.id == field)
            .map(|field| (*field.value).clone())
            .filter(|value| *value != Value::Unit)
    }
}

/// An `output` node with no path, which writes each record of the batch it
/// receives under the key the record names.
#[derive(Clone, Debug)]
pub struct KeyedOutput {
    /// The node's id in the graph.
    pub id: String,
    /// The tree node that calls its function.
    pub tree_id: Uuid,
    /// The record field holding each entry's key.
    pub key_field: Uuid,
    /// The record field holding each entry's value.
    pub value_field: Uuid,
}

/// A graph translated to the nodes of a behavior tree.
#[derive(Clone, Debug)]
pub struct ImportedGraph {
    /// The tree's nodes, its root first.
    pub nodes: Vec<Node>,
    /// The name of every variable the nodes read or write, by id: a store
    /// key, or a tree-local name starting with `.`.
    pub names: HashMap<Uuid, String>,
    /// The graph's nodes, in the graph's order.
    pub graph_nodes: Vec<GraphNode>,
    /// The `output` nodes with no path, in the graph's order.
    pub keyed_outputs: Vec<KeyedOutput>,
}

/// The namespace of the tree node ids a graph's node ids derive from.
const NODES: Uuid = Uuid::from_u128(0x6e6f_6465_2d67_7261_7068_0000_0000_0002);
/// The namespace of variable ids, derived from their names.
const VARIABLES: Uuid = Uuid::from_u128(0x6e6f_6465_2d67_7261_7068_0000_0000_0003);
/// The namespace of the ids of the check nodes, derived from the id of the
/// node each checks. Each tree node the importer adds has its ids in a
/// namespace of its own, apart from the graph's nodes': a graph's node ids
/// are any strings, so no prefix keeps a node from taking an added one's.
const CHECKS: Uuid = Uuid::from_u128(0x6e6f_6465_2d67_7261_7068_0000_0000_0004);
/// The namespace of the ids of the select nodes: under it, the id of the
/// node whose input each feeds, then that input's name.
const SELECTS: Uuid = Uuid::from_u128(0x6e6f_6465_2d67_7261_7068_0000_0000_0005);
/// The tree's root. Not a name-based id, so no node's id is it.
const ROOT: Uuid = Uuid::from_u128(0x6e6f_6465_2d67_7261_7068_0000_0000_0006);

/// The tree node id of the graph's node `id`.
pub fn tree_node_id(id: &str) -> Uuid {
    Uuid::new_v5(&NODES, id.as_bytes())
}

/// The id of the variable named `name`.
pub fn variable_id(name: &str) -> Uuid {
    Uuid::new_v5(&VARIABLES, name.as_bytes())
}

/// The tree-local variable an attached node's return value is bound to.
fn attached_output(id: &str) -> String {
    format!(".node/{id}")
}

impl ImportedGraph {
    /// Build the behavior tree, binding each variable `resolver` resolves to
    /// its store slot and every other to a tree-local cell.
    pub fn build(&self, resolver: &VariableResolver) -> Result<BehaviorTree, String> {
        load_behavior_tree_nodes_with(self.nodes.clone(), resolver, &self.names)
            .map_err(|e| e.to_string())
    }

    /// The graph's node `id`.
    pub fn node(&self, id: &str) -> Option<&GraphNode> {
        self.graph_nodes.iter().find(|node| node.id == id)
    }

    /// What `tree` holds as the return value of the graph's node `node`:
    /// the cache of a detached node's last call, the variable an attached
    /// node writes.
    pub fn returned(&self, tree: &BehaviorTree, node: &GraphNode) -> Option<Value> {
        if node.attached {
            tree.argument_value(node.tree_id, _RET_PARAM_ID)
        } else {
            tree.detached_output(node.tree_id, _RET_PARAM_ID)
        }
    }
}

/// Translate a node graph from its JSON form: a `GraphSpec`, or a document
/// holding one under `spec`, in any form
/// [`normalize_graph_spec_value`](vizij_api_core::json::normalize_graph_spec_value)
/// accepts.
pub fn import_json(
    json: &str,
    options: &ImportOptions,
) -> Result<(GraphSpec, ImportedGraph), String> {
    let mut document: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("node graph JSON: {e}"))?;
    if let Some(spec) = document
        .get("spec")
        .filter(|spec| spec.get("nodes").is_some())
    {
        document = spec.clone();
    }
    vizij_api_core::json::normalize_graph_spec_value(&mut document)
        .map_err(|e| format!("node graph JSON: {e:?}"))?;
    let spec: GraphSpec =
        serde_json::from_value(document).map_err(|e| format!("node graph JSON: {e}"))?;
    let imported = import(&spec, options)?;
    Ok((spec, imported))
}

/// Translate `spec`.
///
/// Refuses, naming the node: an edge naming a node the graph does not have,
/// two edges into one input, two nodes of one id, and a variadic port past
/// the last a function declares ([`VARIADIC_PORTS`]). A cycle is refused when
/// the tree is lowered. A call to another module is placed as
/// [`placement`] says with no function described: ticked at every tick.
pub fn import(spec: &GraphSpec, options: &ImportOptions) -> Result<ImportedGraph, String> {
    import_with(spec, options, &FunctionIndex::new())
}

/// [`import`] `spec`, placing each call to another module by the
/// description `functions` gives of its function ([`placement`]).
pub fn import_with(
    spec: &GraphSpec,
    options: &ImportOptions,
    functions: &FunctionIndex,
) -> Result<ImportedGraph, String> {
    let connections = spec.input_connections()?;
    let mut seen = HashSet::new();
    for node in &spec.nodes {
        if !seen.insert(node.id.as_str()) {
            return Err(format!("two nodes share the id '{}'", node.id));
        }
    }

    // The ports each node's edges read, and so its output layout.
    let mut referenced: HashMap<&str, hashbrown::HashSet<String>> = HashMap::new();
    for edge in &spec.edges {
        referenced
            .entry(edge.from.node_id.as_str())
            .or_default()
            .insert(edge.from.output.clone());
    }
    let empty = hashbrown::HashSet::new();
    let params: Vec<Params> = spec.nodes.iter().map(|node| node.params.params()).collect();
    let mut graph_nodes = Vec::with_capacity(spec.nodes.len());
    for (node, params) in spec.nodes.iter().zip(&params) {
        let placed = match node.kind.kind() {
            Some(kind) => placement(&kind, params, functions),
            None => Placement::Detached,
        };
        let ignored = placed == Placement::Ignored;
        graph_nodes.push(GraphNode {
            id: node.id.clone(),
            kind: node.kind.clone(),
            tree_id: tree_node_id(&node.id),
            ports: match placed {
                Placement::Ignored => Vec::new(),
                _ => ports_of(
                    node,
                    params,
                    referenced.get(node.id.as_str()).unwrap_or(&empty),
                ),
            },
            attached: placed == Placement::Attached,
            ignored,
            check: (!ignored && checks_shapes(node)?).then(|| check_node_id(&node.id)),
        });
    }
    let by_id: HashMap<&str, &GraphNode> = graph_nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    let mut names = HashMap::new();
    let mut variable = |name: &str| {
        let id = variable_id(name);
        names.insert(id, name.to_string());
        Expression::VariableId(id)
    };

    let mut tree_nodes = Vec::with_capacity(spec.nodes.len() + 1);
    let mut selects = Vec::new();
    let mut checks = Vec::new();
    let mut keyed_outputs = Vec::new();
    let no_connections = hashbrown::HashMap::new();
    for ((node, params), graph_node) in spec.nodes.iter().zip(&params).zip(&graph_nodes) {
        if graph_node.ignored {
            continue;
        }
        let kind = node.kind.kind();
        let mut arguments: HashMap<Uuid, Expression> = HashMap::with_capacity(8);
        let wired = connections.get(&node.id).unwrap_or(&no_connections);
        let mut ports: Vec<(&String, &InputConnection)> = wired.iter().collect();
        ports.sort_by(|a, b| a.0.cmp(b.0));
        for (port, connection) in ports {
            let Some(parameter) = input_parameter(&node.kind, port) else {
                if kind
                    .as_ref()
                    .is_some_and(|kind| is_variadic_past_the_last(kind, port))
                {
                    return Err(format!(
                        "node '{}' wires its input '{port}', past the {VARIADIC_PORTS} ports \
                         of its variadic group a function declares",
                        node.id
                    ));
                }
                // An input the node's evaluation never reads.
                continue;
            };
            let read = connection.node_id.as_ref().and_then(|source| {
                let source = by_id.get(source.as_str())?;
                source
                    .ports
                    .contains(&connection.output_key)
                    .then(|| port_read(source, &connection.output_key, &mut variable))
            });
            let argument = match (read, &connection.default_value) {
                (Some(read), _) => match &connection.selector {
                    Some(selector) if !selector.is_empty() => {
                        let select_id = Uuid::new_v5(
                            &Uuid::new_v5(&SELECTS, node.id.as_bytes()),
                            port.as_bytes(),
                        );
                        selects.push(Node {
                            id: select_id,
                            function: select_function(),
                            arguments: HashMap::from([
                                (input_param("in"), read),
                                (
                                    selector_param(),
                                    Expression::Value(encode_selector(selector)),
                                ),
                                (
                                    node_param(),
                                    Expression::Value(Value::String(node.id.clone())),
                                ),
                            ]),
                            children: None,
                        });
                        Expression::NodeArgument(NodeParameterId {
                            node: select_id,
                            parameter: _RET_PARAM_ID,
                        })
                    }
                    _ => read,
                },
                (None, Some(default)) => Expression::Value(default.clone()),
                (None, None) => continue,
            };
            arguments.insert(parameter, argument);
        }

        for (parameter, value) in node_arguments(node, params)? {
            arguments.insert(parameter, Expression::Value(value));
        }
        if let Some(kind) = &kind {
            if !graph_node.returns_out() {
                arguments.insert(
                    ports_param(),
                    Expression::Value(Value::ArrayString(graph_node.ports.clone())),
                );
            }
            if reads_time(kind) {
                arguments.insert(time_param(), variable(&options.time));
            }
            if reads_dt(kind) {
                arguments.insert(dt_param(), variable(&options.dt));
            }
            if is_stateful(kind) {
                arguments.insert(state_param(), Expression::Value(Value::Unit));
            }
        }
        if node.kind == NodeType::Input {
            if let Some(path) = &node.params.path {
                arguments.insert(
                    vizij_functions::in_param(),
                    variable(&Key::new(path.to_string()).path),
                );
            }
        }
        if node.kind == NodeType::Output {
            match (
                &node.params.path,
                node.params.key_field,
                node.params.value_field,
            ) {
                (Some(path), ..) => {
                    arguments.insert(_RET_PARAM_ID, variable(&path.to_string()));
                }
                (None, Some(key_field), Some(value_field)) => keyed_outputs.push(KeyedOutput {
                    id: node.id.clone(),
                    tree_id: graph_node.tree_id,
                    key_field,
                    value_field,
                }),
                _ => {}
            }
        }
        if graph_node.attached {
            arguments.insert(_RET_PARAM_ID, variable(&attached_output(&node.id)));
        }
        if let Some(check) = graph_node.check {
            checks.push(check_node(node, graph_node, check, &mut variable)?);
        }
        tree_nodes.push(Node {
            id: graph_node.tree_id,
            function: function_of(&node.kind),
            arguments,
            children: None,
        });
    }

    let leaves: Vec<Uuid> = graph_nodes
        .iter()
        .filter(|node| node.attached)
        .map(|node| node.tree_id)
        .collect();
    let root = if leaves.is_empty() {
        Node {
            id: ROOT,
            function: SUCCEED_FUNCTION_ID,
            arguments: HashMap::new(),
            children: None,
        }
    } else {
        Node {
            id: ROOT,
            function: PARALLEL_FUNCTION_ID,
            arguments: HashMap::new(),
            children: Some(leaves),
        }
    };
    let nodes = std::iter::once(root)
        .chain(tree_nodes)
        .chain(checks)
        .chain(selects)
        .collect();
    Ok(ImportedGraph {
        nodes,
        names,
        graph_nodes,
        keyed_outputs,
    })
}

/// The output ports of `node`, of the parameters `params`, whose edges read
/// `referenced`, in order.
///
/// A port an edge names but the node's evaluation never sets is no port:
/// the edge reads as an input with no edge. An `input` or `output` node has
/// the one port `out`.
pub(crate) fn ports_of(
    node: &NodeSpec,
    params: &Params,
    referenced: &hashbrown::HashSet<String>,
) -> Vec<String> {
    let Some(kind) = node.kind.kind() else {
        return vec!["out".to_string()];
    };
    let declared: Vec<&str> = node.output_shapes.keys().map(String::as_str).collect();
    let set = output_layout(&kind, params, &declared, &hashbrown::HashSet::new());
    // Edges reading only ports the node sets leave its layout as it is.
    if referenced.iter().all(|port| set.slot(port).is_some()) {
        return set.slots;
    }
    let layout = output_layout(&kind, params, &declared, referenced);
    layout
        .slots
        .iter()
        .filter(|port| {
            set.slot(port).is_some()
                || layout.variadic_groups().values().any(|range| {
                    let at = layout.slot(port).unwrap_or(usize::MAX);
                    at >= range.start && at < range.start + range.len
                })
        })
        .cloned()
        .collect()
}

/// The literal arguments the parameters of `node` give its call, `params`
/// being those its kind's function reads: its node id, and each parameter
/// that differs from its default. An `input` node's are its store path,
/// its default value and its declared shape; an `output` node's, its
/// declared shape.
pub(crate) fn node_arguments(
    node: &NodeSpec,
    params: &Params,
) -> Result<Vec<(Uuid, Value)>, String> {
    let id = Value::String(node.id.clone());
    if node.kind.kind().is_some() {
        let mut arguments = encode(params)?;
        arguments.push((node_param(), id));
        return Ok(arguments);
    }
    let mut arguments = vec![(vizij_functions::node_param(), id)];
    if node.kind == NodeType::Input {
        if let Some(path) = &node.params.path {
            arguments.push((
                vizij_functions::path_param(),
                Value::String(path.to_string()),
            ));
        }
        if let Some(default) = &node.params.value {
            arguments.push((vizij_functions::default_param(), default.clone()));
        }
    }
    if let Some(shape) = node.output_shapes.get("out") {
        let shape = serde_json::to_string(shape).map_err(|e| e.to_string())?;
        arguments.push((vizij_functions::shape_param(), Value::String(shape)));
    }
    Ok(arguments)
}

/// The parameter of `kind`'s function an edge into its input `port` sets:
/// `None` for an input the function does not declare.
fn input_parameter(kind: &NodeType, port: &str) -> Option<Uuid> {
    match kind.kind() {
        Some(kind) => declares_input(&kind, port).then(|| input_param(port)),
        None => (*kind == NodeType::Output && port == "in").then(vizij_functions::in_param),
    }
}

/// Whether `node` declares the shape of an output its function does not
/// check: any for a node of the vocabulary; refused for an `input` or
/// `output` node, whose one port `out` its function checks.
fn checks_shapes(node: &NodeSpec) -> Result<bool, String> {
    if node.kind.kind().is_some() {
        return Ok(!node.output_shapes.is_empty());
    }
    match node.output_shapes.keys().find(|port| *port != "out") {
        Some(port) => Err(format!(
            "node '{}' declares the shape of an output '{port}' it does not have",
            node.id
        )),
        None => Ok(false),
    }
}

/// The tree node id of the check of the graph's node `id`.
fn check_node_id(id: &str) -> Uuid {
    Uuid::new_v5(&CHECKS, id.as_bytes())
}

/// The tree node `check` checking the shapes `node`, as `graph_node`, declares
/// of its outputs.
fn check_node(
    node: &NodeSpec,
    graph_node: &GraphNode,
    check: Uuid,
    variable: &mut impl FnMut(&str) -> Expression,
) -> Result<Node, String> {
    let shapes = serde_json::to_string(&node.output_shapes).map_err(|e| e.to_string())?;
    let mut arguments = HashMap::from([
        (
            vizij_functions::in_param(),
            returned_read(graph_node, variable),
        ),
        (
            vizij_functions::shapes_param(),
            Expression::Value(Value::String(shapes)),
        ),
        (
            vizij_functions::node_param(),
            Expression::Value(Value::String(node.id.clone())),
        ),
    ]);
    if !graph_node.returns_out() {
        arguments.insert(
            vizij_functions::ports_param(),
            Expression::Value(Value::ArrayString(graph_node.ports.clone())),
        );
    }
    Ok(Node {
        id: check,
        function: vizij_functions::check_function(),
        arguments,
        children: None,
    })
}

/// The function a node of the graph of `kind` calls.
fn function_of(kind: &NodeType) -> Uuid {
    match kind.kind() {
        Some(Kind::ExternalFunction) => external_function(),
        Some(kind) => function_id(&kind),
        None if *kind == NodeType::Input => vizij_functions::input_function(),
        None => vizij_functions::output_function(),
    }
}

/// How a node reads what the call of `source` returned.
fn returned_read(source: &GraphNode, variable: &mut impl FnMut(&str) -> Expression) -> Expression {
    if source.attached {
        variable(&attached_output(&source.id))
    } else {
        Expression::NodeArgument(NodeParameterId {
            node: source.tree_id,
            parameter: _RET_PARAM_ID,
        })
    }
}

/// How a node reads the port `port` of `source`.
fn port_read(
    source: &GraphNode,
    port: &str,
    variable: &mut impl FnMut(&str) -> Expression,
) -> Expression {
    let returned = match source.check {
        Some(check) => Expression::NodeArgument(NodeParameterId {
            node: check,
            parameter: _RET_PARAM_ID,
        }),
        None => returned_read(source, variable),
    };
    if source.returns_out() {
        returned
    } else {
        Expression::Select {
            source: Box::new(returned),
            path: Key::new(format!(".{}", output_field(port))),
        }
    }
}

/// Whether `port` is a port of `kind`'s variadic group past the last its
/// function declares.
fn is_variadic_past_the_last(kind: &Kind, port: &str) -> bool {
    let (group, index) = crate::eval::variadic::parse_variadic_key(port);
    let declared_group = crate::ports::ports(kind)
        .variadic_inputs
        .map(|variadic| variadic.id);
    declared_group == Some(group) && index.is_some_and(|index| index >= VARIADIC_PORTS)
}
