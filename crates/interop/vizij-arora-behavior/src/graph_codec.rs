//! Structural encoding of a Vizij graph spec as an Arora shared [`Graph`].
//!
//! This maps a [`GraphSpec`] onto the shared model **structurally**: one
//! shared-model [`Node`] per Vizij node, so every node, edge, and parameter is
//! individually addressable — the precondition for editing a Vizij graph through
//! `apply(GraphDiff)` instead of re-loading the whole spec (VIZ-79). It is the
//! [`ProcessingGraph`](crate::ProcessingGraph)'s running representation: a graph
//! is `encode`d on load and `decode`d back to a spec to evaluate.
//!
//! # Conventions
//!
//! The shared [`Node`] has no parameter bag — behavior is a bare
//! `function: Uuid`, and a node holds its own bindings ([`Node::bind`]), one
//! [`Source`] per bound slot. So:
//!
//! - **kind → `Node.function`**: a stable per-kind id, `gen_uuid_from_str("vizij/graph/kind/<kind>")`.
//! - **edge → a binding on the target node**: its input slot
//!   `gen_uuid_from_str("vizij/graph/in/<port>")` bound to
//!   `Source::Port(from)`, the source's output slot
//!   `gen_uuid_from_str("vizij/graph/out/<port>")`. An edge `selector` wraps the
//!   source in `Source::Select { source, path }`, the `Key` attribute path the
//!   runtime reads through [`Key::select`].
//! - **each present [`NodeParams`] field → its own value slot**:
//!   `gen_uuid_from_str("vizij/graph/param/<field>")` bound to
//!   `Source::Value(value)`. The field's value is carried through
//!   [`arora_types::value_serde`], so the mapping is generic over every kind —
//!   no per-node-kind code — while keeping each parameter a distinct, typed slot.
//! - **each inline [`input default`](NodeSpec::input_defaults) → a value on its
//!   input slot**: a default value on an *unconnected* input is the same
//!   value-bound-slot mechanism as a parameter, on the input slot. An input that
//!   also carries an edge takes the edge — the default is shadowed and never
//!   evaluates (this is [`GraphSpec::input_connections`], where an edge replaces
//!   a default), so structural encodes only the edge.
//!
//! # Names
//!
//! `gen_uuid_from_str` is one-way, so the names needed to rebuild the
//! `GraphSpec` (node ids, the kind string, port and parameter keys) ride as
//! **companion bindings** on the node that uses them, each a
//! `Source::Value(Value::String(name))`:
//!
//! - the node's own id and kind name, on two fixed slots;
//! - for each slot `S` the node binds, and for the output slot of each edge
//!   source it reads, `name_slot(S)` = `gen_uuid_from_str("vizij/graph/name/<S>")`.
//!
//! A node thus carries every name its own bindings decode by, so translating a
//! [`GraphSpecDiff`] needs no view of the running graph: an edge added to a node
//! the diff does not set is bound with its names, and a diff that removes a node
//! takes its names with it. Decoding reads a binding key `K` as a slot when the
//! node binds `name_slot(K)`, a parameter when `K` is that name's `param` slot,
//! an inline default or an edge when it is its `in` slot; a companion whose slot
//! is unbound names nothing and is ignored.
//!
//! # Declared-shape metadata
//!
//! [`output_shapes`](NodeSpec::output_shapes) and an input default's
//! [`shape`](vizij_graph_core::types::InputDefault::shape) are declared *type*
//! metadata, not values, so they have no per-value slot. They ride one reserved
//! JSON string slot per node (`meta_slot`), present only when a node
//! actually declares a shape (no real authored graph does — the runtime derives
//! shapes — but the eval path honors them where present, so they must survive a
//! round trip). This is the one carrier that is not a per-value slot.
//!
//! # Order
//!
//! A graph's order is its structure: `root` and each node's ordered
//! `children`. Two composites carry it, bound to functions of their own: a
//! **`flow`** holds a dataflow network, a node after every node it reads from
//! and otherwise in listed order; a **`layers`** holds parts of a behavior,
//! run one after the other. An encoded spec is one `flow` of its nodes in
//! listed order, and decoding lists the nodes in a pre-order walk of the
//! structure. Where two outputs write one path, the later-listed one wins: the
//! order of a composed behavior's sources survives the round trip, and with it
//! their precedence.
//!
//! # Not yet mapped
//!
//! An edge selector's `Field` name rides verbatim: reading it against a
//! UUID-keyed `Structure` would need the type registry to resolve the name to a
//! field id at encode time. That resolution is not done here yet — a `Field`
//! selector only round-trips a string-keyed `KeyValue` record or an array index
//! (see `selector_to_key`). Vizij's record selectors read `KeyValue`, so this
//! is correct for Vizij graphs today.

use std::collections::{HashMap, HashSet};

use arora_behavior::graph::{Binding, GraphDiff, Node, Port, Source};
use arora_behavior::Graph;
use arora_types::data::Key;
use arora_types::value::Value;
use arora_types::{gen_uuid_from_str, value_serde};
use uuid::Uuid;
use vizij_api_core::Shape;
use vizij_graph_core::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, InputDefault, NodeParams, NodeSpec,
    NodeType, Selector, SelectorSegment,
};

fn kind_function(kind: &NodeType) -> Result<(Uuid, String), String> {
    let name = kind_name(kind)?;
    Ok((gen_uuid_from_str(&format!("vizij/graph/kind/{name}")), name))
}

/// The serde tag a [`NodeType`] serializes to (e.g. `"add"`, `"centered_remap"`).
fn kind_name(kind: &NodeType) -> Result<String, String> {
    match serde_json::to_value(kind).map_err(|e| format!("serialize node kind: {e}"))? {
        serde_json::Value::String(s) => Ok(s),
        other => Err(format!("node kind did not serialize to a string: {other}")),
    }
}

pub(crate) fn node_id_uuid(id: &str) -> Uuid {
    gen_uuid_from_str(&format!("vizij/graph/node/{id}"))
}

fn in_slot(port: &str) -> Uuid {
    gen_uuid_from_str(&format!("vizij/graph/in/{port}"))
}

fn out_slot(port: &str) -> Uuid {
    gen_uuid_from_str(&format!("vizij/graph/out/{port}"))
}

fn param_slot(field: &str) -> Uuid {
    gen_uuid_from_str(&format!("vizij/graph/param/{field}"))
}

/// The reserved slot that carries a node's declared-shape metadata (see the
/// module's "Declared-shape metadata" section). Fixed per node; recognized by id
/// on decode, so it carries no name.
fn meta_slot() -> Uuid {
    gen_uuid_from_str("vizij/graph/meta")
}

/// The companion slot naming slot `slot` (see the module's "Names" section).
fn name_slot(slot: Uuid) -> Uuid {
    gen_uuid_from_str(&format!("vizij/graph/name/{slot}"))
}

/// The slot naming the node itself: its Vizij id, or a composite's name.
fn node_name_slot() -> Uuid {
    gen_uuid_from_str("vizij/graph/name/node")
}

/// The slot naming the node's kind.
fn kind_name_slot() -> Uuid {
    gen_uuid_from_str("vizij/graph/name/kind")
}

/// A name, as a companion binding carries it.
fn name_source(name: &str) -> Source {
    Source::Value(Value::String(name.to_string()))
}

/// The name a companion binding of `node` on `slot` carries.
fn bound_name(node: &Node, slot: Uuid) -> Option<&str> {
    match node.bind.get(&slot) {
        Some(Source::Value(Value::String(name))) => Some(name),
        _ => None,
    }
}

/// The node's own name: its Vizij id, or a composite's name.
pub(crate) fn node_name(node: &Node) -> Option<&str> {
    bound_name(node, node_name_slot())
}

/// The name of `slot` that `node` carries, if it binds one.
fn slot_name(node: &Node, slot: Uuid) -> Option<&str> {
    bound_name(node, name_slot(slot))
}

/// The node of `graph` with id `id`, to change in place.
pub(crate) fn node_mut<'a>(graph: &'a mut Graph, id: &Uuid) -> Option<&'a mut Node> {
    graph.nodes.iter_mut().find(|node| node.id == *id)
}

/// Put `node` in `graph`: in place of the node with its id, else after the
/// others.
fn put_node(graph: &mut Graph, node: Node) {
    match node_mut(graph, &node.id) {
        Some(held) => *held = node,
        None => graph.nodes.push(node),
    }
}

/// The function of a composite whose children run in their listed order, a
/// later child writing after an earlier one: the parts of a behavior.
pub(crate) fn layers_function() -> Uuid {
    gen_uuid_from_str("vizij/graph/composite/layers")
}

/// The function of a composite whose children are a dataflow network: a node
/// after every node it reads from, ties in their listed order.
pub(crate) fn flow_function() -> Uuid {
    gen_uuid_from_str("vizij/graph/composite/flow")
}

/// The id of the composite named `name`.
pub(crate) fn composite_id(name: &str) -> Uuid {
    gen_uuid_from_str(&format!("vizij/graph/group/{name}"))
}

/// Whether `node` is a composite — structure, not a Vizij node.
pub(crate) fn is_composite(node: &Node) -> bool {
    node.function == layers_function() || node.function == flow_function()
}

/// Add the composite `name`, of `function`, holding `children` in order, and
/// return its id. Replaces a composite of the same name.
pub(crate) fn insert_composite(
    graph: &mut Graph,
    function: Uuid,
    name: &str,
    children: Vec<Uuid>,
) -> Uuid {
    let id = composite_id(name);
    put_node(
        graph,
        Node {
            id,
            function,
            bind: [(node_name_slot(), name_source(name))].into(),
            children: Some(children),
        },
    );
    id
}

/// The present (non-null) fields of a [`NodeParams`], as `(field, json)` pairs.
///
/// `NodeParams` is a flat serde struct; a field is "present" when it serializes
/// to something other than JSON `null` (i.e. a set `Option`, or a
/// `#[serde(default)]` scalar). Each becomes its own parameter slot.
fn present_params(params: &NodeParams) -> Result<Vec<(String, serde_json::Value)>, String> {
    let json = serde_json::to_value(params).map_err(|e| format!("serialize node params: {e}"))?;
    let map = match json {
        serde_json::Value::Object(map) => map,
        other => {
            return Err(format!(
                "node params did not serialize to an object: {other}"
            ))
        }
    };
    Ok(map.into_iter().filter(|(_, v)| !v.is_null()).collect())
}

/// A node's declared-shape metadata as a JSON string, or `None` when it declares
/// none. Carries `output_shapes` and any input-default `shape` — see the
/// module's "Declared-shape metadata" section.
fn node_meta_json(node: &NodeSpec) -> Result<Option<String>, String> {
    let mut input_shapes = serde_json::Map::new();
    for (port, default) in &node.input_defaults {
        if let Some(shape) = &default.shape {
            let json = serde_json::to_value(shape).map_err(|e| format!("input shape json: {e}"))?;
            input_shapes.insert(port.clone(), json);
        }
    }
    if node.output_shapes.is_empty() && input_shapes.is_empty() {
        return Ok(None);
    }

    let mut meta = serde_json::Map::new();
    if !node.output_shapes.is_empty() {
        let json = serde_json::to_value(&node.output_shapes)
            .map_err(|e| format!("output_shapes json: {e}"))?;
        meta.insert("output_shapes".to_string(), json);
    }
    if !input_shapes.is_empty() {
        meta.insert(
            "input_shapes".to_string(),
            serde_json::Value::Object(input_shapes),
        );
    }
    serde_json::to_string(&serde_json::Value::Object(meta))
        .map(Some)
        .map_err(|e| format!("meta json: {e}"))
}

/// Encode a Vizij [`GraphSpec`] as an Arora shared [`Graph`] (see module docs).
///
/// Total over every valid spec — the whole-graph composition of `encode_node`
/// (one shared node, its parameter/input-default/metadata value slots and
/// names) and `edge_bindings` (the target's bound input, a `Select` for a
/// selector). The root is a `flow` holding the nodes in their listed order.
/// An edge into a node the spec does not have is refused.
pub fn encode(spec: &GraphSpec) -> Result<Graph, String> {
    let mut graph = Graph::empty();

    // Inputs fed by an edge: their inline default (if any) is shadowed and never
    // evaluates, so `encode_node` skips it and only the edge is encoded.
    let mut edged: HashMap<&str, HashSet<&str>> = HashMap::new();
    for edge in &spec.edges {
        edged
            .entry(edge.to.node_id.as_str())
            .or_default()
            .insert(edge.to.input.as_str());
    }

    for node in &spec.nodes {
        put_node(&mut graph, encode_node(node, edged.get(node.id.as_str()))?);
    }
    for edge in &spec.edges {
        let (target, bindings) = edge_bindings(edge);
        let node = node_mut(&mut graph, &target).ok_or_else(|| {
            format!(
                "an edge feeds node {:?}, which the graph lacks",
                edge.to.node_id
            )
        })?;
        node.bind.extend(bindings);
    }
    let children = spec
        .nodes
        .iter()
        .map(|node| node_id_uuid(&node.id))
        .collect();
    graph.root = Some(insert_composite(
        &mut graph,
        flow_function(),
        "main",
        children,
    ));

    Ok(graph)
}

/// Encode one Vizij node into its shared [`Node`]: its parameter,
/// inline-default and metadata value slots, and the names that decode it.
/// `edged_inputs` names the node's inputs that carry an edge (whose inline
/// default is shadowed and skipped). The whole-graph [`encode`] and a
/// `GraphDiff` node upsert share this.
fn encode_node(node: &NodeSpec, edged_inputs: Option<&HashSet<&str>>) -> Result<Node, String> {
    let (function, kind) = kind_function(&node.kind)?;
    let mut encoded = Node {
        id: node_id_uuid(&node.id),
        function,
        ..Node::default()
    };
    let bind = &mut encoded.bind;
    bind.insert(node_name_slot(), name_source(&node.id));
    bind.insert(kind_name_slot(), name_source(&kind));

    // Parameters: each present field is its own value slot.
    for (field, value) in present_params(&node.params)? {
        let slot = param_slot(&field);
        let literal = value_serde::to_value(&value).map_err(|e| format!("param to value: {e}"))?;
        bind.insert(slot, Source::Value(literal));
        bind.insert(name_slot(slot), name_source(&field));
    }

    // Inline input defaults: a default value on an unconnected input, on the
    // input slot. An edge on the same input takes precedence (default shadowed).
    for (port, default) in &node.input_defaults {
        if edged_inputs.is_some_and(|set| set.contains(port.as_str())) {
            continue;
        }
        let slot = in_slot(port);
        bind.insert(slot, Source::Value(default.value.clone()));
        bind.insert(name_slot(slot), name_source(port));
    }

    // Declared-shape metadata, when present: one reserved JSON string slot.
    if let Some(meta) = node_meta_json(node)? {
        bind.insert(meta_slot(), Source::Value(Value::String(meta)));
    }
    Ok(encoded)
}

/// The bindings one Vizij edge puts on its target node, and that node's id:
/// the target input bound to the source output — wrapped in a
/// [`Select`](Source::Select) carrying the `Key` path for a selector — with
/// the names of both slots. A binding replaces any other on the target input
/// (one source per input, as `GraphSpec::input_connections` takes it). Shared
/// by the whole-graph [`encode`] and a `GraphDiff` edge upsert.
fn edge_bindings(edge: &EdgeSpec) -> (Uuid, [(Uuid, Source); 3]) {
    let src_slot = out_slot(&edge.from.output);
    let dst_slot = in_slot(&edge.to.input);
    let mut source = Source::Port(Port::new(node_id_uuid(&edge.from.node_id), src_slot));
    if let Some(selector) = &edge.selector {
        source = Source::Select {
            source: Box::new(source),
            path: selector_to_key(selector),
        };
    }
    (
        node_id_uuid(&edge.to.node_id),
        [
            (dst_slot, source),
            (name_slot(dst_slot), name_source(&edge.to.input)),
            (name_slot(src_slot), name_source(&edge.from.output)),
        ],
    )
}

/// A Vizij edge [`Selector`] as an Arora [`Key`] attribute path. Each segment
/// becomes an attribute; a leading dot makes the whole path attributes (a
/// selector descends into a value, it does not name a store entity).
///
/// `Field` names ride verbatim (a `KeyValue` record is string-keyed, so the name
/// *is* the key — see [`Key::select`]). Mapping a `Field` on a UUID-keyed
/// `Structure` to its field id would need the type registry at encode time;
/// that resolution is not done here yet (VIZ-79).
fn selector_to_key(selector: &[SelectorSegment]) -> Key {
    let attributes: Vec<String> = selector
        .iter()
        .map(|segment| match segment {
            SelectorSegment::Field(name) => name.clone(),
            SelectorSegment::Index(index) => index.to_string(),
        })
        .collect();
    Key::new(format!(".{}", attributes.join(".")))
}

/// The inverse of [`selector_to_key`]: a numeric attribute is an `Index`, any
/// other an `Field`.
fn key_to_selector(path: &Key) -> Selector {
    path.get_attributes()
        .iter()
        .map(|attribute| match attribute.parse::<usize>() {
            Ok(index) => SelectorSegment::Index(index),
            Err(_) => SelectorSegment::Field((*attribute).to_string()),
        })
        .collect()
}

/// The graph's Vizij nodes in the order its structure gives: a pre-order walk
/// of the composites from the root, each composite's children in their listed
/// order, then every node no composite holds, by id. A child the graph no
/// longer has (an edit removed it) is skipped.
fn structural_order(graph: &Graph) -> Vec<&Node> {
    let by_id: HashMap<Uuid, &Node> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut order = Vec::with_capacity(graph.nodes.len());
    let mut seen = HashSet::new();
    let mut stack: Vec<Uuid> = graph.root.into_iter().collect();
    while let Some(id) = stack.pop() {
        let Some(node) = by_id.get(&id).copied() else {
            continue;
        };
        if !seen.insert(id) {
            continue;
        }
        if is_composite(node) {
            stack.extend(node.children.iter().flatten().rev());
        } else {
            order.push(node);
        }
    }
    let mut unplaced: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| !seen.contains(&node.id) && !is_composite(node))
        .collect();
    unplaced.sort_by_key(|node| node_name(node));
    order.extend(unplaced);
    order
}

/// Decode an Arora shared [`Graph`] produced by [`encode`] back into a
/// [`GraphSpec`], its nodes in the order the graph's structure gives
/// ([`structural_order`]). That order is the evaluation's: a node runs
/// after every node it reads from, and of two writers of one path the
/// later-listed wins.
pub fn decode(graph: &Graph) -> Result<GraphSpec, String> {
    let by_id: HashMap<Uuid, &Node> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    let id_of = |id: &Uuid| -> Result<String, String> {
        by_id
            .get(id)
            .and_then(|node| node_name(node))
            .map(str::to_string)
            .ok_or_else(|| format!("no name recorded for node {id}"))
    };
    let meta = meta_slot();

    let order = structural_order(graph);
    let mut nodes: Vec<NodeSpec> = Vec::with_capacity(order.len());
    let mut edges: Vec<EdgeSpec> = Vec::new();
    for node in order {
        let id = id_of(&node.id)?;
        let kind_name = bound_name(node, kind_name_slot())
            .ok_or_else(|| format!("no kind recorded for node {id}"))?;
        let kind: NodeType = serde_json::from_value(serde_json::Value::String(kind_name.into()))
            .map_err(|e| format!("rebuild node kind: {e}"))?;

        let mut params_obj = serde_json::Map::new();
        // Inline-default values, kept as the bound `Value` (not round-tripped
        // through JSON).
        let mut default_values: Vec<(String, Value)> = Vec::new();
        let mut node_meta = serde_json::Value::Null;
        for (slot, source) in &node.bind {
            // The reserved metadata slot: a JSON blob of declared shapes.
            if *slot == meta {
                let json = match source {
                    Source::Value(Value::String(json)) => json,
                    _ => return Err("the metadata slot is not bound to a JSON string".to_string()),
                };
                node_meta =
                    serde_json::from_str(json).map_err(|e| format!("node metadata json: {e}"))?;
                continue;
            }
            // A slot is a key the node names; the rest are names themselves.
            let Some(field) = slot_name(node, *slot) else {
                continue;
            };
            match source {
                // A value on a `param` slot is a parameter; on an `in` slot an
                // inline default. Recompute the slot id from the recorded name
                // to tell which (the namespaces differ, so the ids differ).
                Source::Value(value) if *slot == param_slot(field) => {
                    let json: serde_json::Value = value_serde::from_value(value.clone())
                        .map_err(|e| format!("value to json: {e}"))?;
                    params_obj.insert(field.to_string(), json);
                }
                Source::Value(value) if *slot == in_slot(field) => {
                    default_values.push((field.to_string(), value.clone()));
                }
                Source::Port(_) | Source::Select { .. } if *slot == in_slot(field) => {
                    // A `Select` over a `Port` is an edge that reads a sub-path
                    // of the source output; recover the Vizij selector from its
                    // `Key` path.
                    let (from, selector) = match source {
                        Source::Port(port) => (port, None),
                        Source::Select { source, path } => match source.as_ref() {
                            Source::Port(port) => (port, Some(key_to_selector(path))),
                            _ => {
                                return Err(
                                    "structural decode expects a Select over a Port".to_string()
                                )
                            }
                        },
                        _ => unreachable!("matched a port or a select"),
                    };
                    let output = slot_name(node, from.port).ok_or_else(|| {
                        format!(
                            "no name recorded for output {} read by node {id}",
                            from.port
                        )
                    })?;
                    edges.push(EdgeSpec {
                        from: EdgeOutputEndpoint {
                            node_id: id_of(&from.node)?,
                            output: output.to_string(),
                        },
                        to: EdgeInputEndpoint {
                            node_id: id.clone(),
                            input: field.to_string(),
                        },
                        selector,
                    });
                }
                _ => return Err(format!("unrecognized binding on slot {slot} of node {id}")),
            }
        }

        let params: NodeParams = serde_json::from_value(serde_json::Value::Object(params_obj))
            .map_err(|e| format!("rebuild node params: {e}"))?;
        let output_shapes = match node_meta.get("output_shapes") {
            Some(shapes) if !shapes.is_null() => serde_json::from_value(shapes.clone())
                .map_err(|e| format!("rebuild output_shapes: {e}"))?,
            _ => Default::default(),
        };
        let input_shapes = node_meta.get("input_shapes").and_then(|v| v.as_object());
        // Recombine each inline default's value (the bound `Value` on its `in`
        // slot) with its declared shape (from the metadata blob).
        let mut input_defaults: std::collections::HashMap<String, InputDefault> =
            std::collections::HashMap::new();
        for (port, value) in default_values {
            let shape = match input_shapes.and_then(|shapes| shapes.get(&port)) {
                Some(shape_json) => Some(
                    serde_json::from_value::<Shape>(shape_json.clone())
                        .map_err(|e| format!("rebuild input shape: {e}"))?,
                ),
                None => None,
            };
            input_defaults.insert(port, InputDefault { value, shape });
        }

        nodes.push(NodeSpec {
            id,
            kind,
            params,
            output_shapes,
            input_defaults: input_defaults.into_iter().collect(),
        });
    }

    Ok(GraphSpec {
        nodes,
        edges,
        version: 0,
        fingerprint: 0,
    })
}

/// A Vizij spec-level graph edit — the delta the editor computes and the wasm
/// boundary hands to the device, translated to an [`arora_behavior::GraphDiff`]
/// by [`spec_diff_to_graph_diff`].
///
/// An upserted node is **removed and set again**, which clears any stale
/// parameter/default binding (e.g. a parameter the edit dropped) and every
/// binding reading it. So **every edge incident to an upserted node must appear
/// in `upsert_edges`** — otherwise the node removal drops it and it is not
/// restored. The editor gathers those edges from the new spec; this keeps the
/// translation context-free (it needs no view of the running graph).
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphSpecDiff {
    /// Nodes to insert or replace (each removed then set again).
    #[serde(default)]
    pub upsert_nodes: Vec<NodeSpec>,
    /// Node ids to remove (with the edges incident to them).
    #[serde(default)]
    pub remove_nodes: Vec<String>,
    /// Edges to insert or replace, plus every edge incident to an upserted node.
    #[serde(default)]
    pub upsert_edges: Vec<EdgeSpec>,
    /// Edge targets to unwire — the `(node_id, input)` an edge fed.
    #[serde(default)]
    pub remove_edges: Vec<EdgeInputEndpoint>,
}

/// Translate a Vizij [`GraphSpecDiff`] into an Arora [`GraphDiff`] using the same
/// per-node/per-edge encoding as [`encode`]. Context-free: an upserted node is
/// removed then set again (see [`GraphSpecDiff`]), so the caller must include
/// every edge incident to an upserted node in `upsert_edges`. An edge into an
/// upserted node rides in the node set; one into another node is bound on it,
/// with its names.
pub fn spec_diff_to_graph_diff(diff: &GraphSpecDiff) -> Result<GraphDiff, String> {
    // Encode the upserted nodes, then their edges onto them, so an edge's
    // binding replaces any inline default on the same input (the edge wins —
    // as in `encode`).
    let mut scratch = Graph::empty();
    for node in &diff.upsert_nodes {
        put_node(&mut scratch, encode_node(node, None)?);
    }
    let mut bind = Vec::new();
    for edge in &diff.upsert_edges {
        let (target, bindings) = edge_bindings(edge);
        match node_mut(&mut scratch, &target) {
            Some(node) => node.bind.extend(bindings),
            None => bind.extend(bindings.into_iter().map(|(port, source)| Binding {
                node: target,
                port,
                source,
            })),
        }
    }

    // Upserted nodes are removed then set again; `Graph::apply` removes before
    // it sets, so this clears each upserted node's stale bindings and the
    // bindings reading it first.
    let remove: Vec<Uuid> = diff
        .remove_nodes
        .iter()
        .map(|id| node_id_uuid(id))
        .chain(diff.upsert_nodes.iter().map(|node| node_id_uuid(&node.id)))
        .collect();
    let unbind: Vec<Port> = diff
        .remove_edges
        .iter()
        .map(|target| Port::new(node_id_uuid(&target.node_id), in_slot(&target.input)))
        .collect();

    Ok(GraphDiff {
        remove,
        set: scratch.nodes,
        unbind,
        bind,
        root: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Each slot binding of the graph — a node's binding on a slot it names,
    /// or on the metadata slot — with the port it binds; names aside.
    fn slots(graph: &Graph) -> Vec<(Port, &Source)> {
        graph
            .nodes
            .iter()
            .flat_map(|node| {
                node.bind
                    .iter()
                    .filter(|(slot, _)| **slot == meta_slot() || slot_name(node, **slot).is_some())
                    .map(|(slot, source)| (Port::new(node.id, *slot), source))
            })
            .collect()
    }

    /// A spec with an input, a literal-param constant, a pure-data node with
    /// edges, and a sink — encodes to a structural graph and decodes back
    /// identically (compared as node/edge sets, since node order is not carried).
    #[test]
    fn round_trips_a_representative_spec() {
        let spec: GraphSpec = serde_json::from_value(json!({
            "nodes": [
                { "id": "sensor", "type": "input",    "params": { "path": "sensor/x" } },
                { "id": "gain",   "type": "constant", "params": { "value": { "f32": 2.0 } } },
                { "id": "mul",    "type": "multiply" },
                { "id": "sink",   "type": "output",   "params": { "path": "actuator/y" } }
            ],
            "edges": [
                { "from": { "node_id": "sensor", "output": "out" }, "to": { "node_id": "mul", "input": "lhs" } },
                { "from": { "node_id": "gain",   "output": "out" }, "to": { "node_id": "mul", "input": "rhs" } },
                { "from": { "node_id": "mul",    "output": "out" }, "to": { "node_id": "sink", "input": "in" } }
            ]
        }))
        .expect("valid spec");

        let graph = encode(&spec).expect("encode");
        let decoded = decode(&graph).expect("decode");

        assert_eq!(canonical(&spec), canonical(&decoded));
    }

    /// One shared node per Vizij node, each bound to its kind's function id, and
    /// one link per parameter/edge.
    #[test]
    fn encode_produces_one_node_per_vizij_node() {
        let spec: GraphSpec = serde_json::from_value(json!({
            "nodes": [
                { "id": "a", "type": "constant", "params": { "value": { "f32": 1.0 } } },
                { "id": "b", "type": "output",   "params": { "path": "out/b" } }
            ],
            "edges": [
                { "from": { "node_id": "a", "output": "out" }, "to": { "node_id": "b", "input": "in" } }
            ]
        }))
        .expect("valid spec");

        let graph = encode(&spec).expect("encode");
        let vizij: Vec<&Node> = graph.nodes.iter().filter(|n| !is_composite(n)).collect();
        assert_eq!(vizij.len(), 2);
        // The root is a flow holding them in their listed order.
        let root = graph.node(&graph.root.expect("a root")).expect("the root");
        assert_eq!(root.function, flow_function());
        assert_eq!(
            root.children,
            Some(vec![node_id_uuid("a"), node_id_uuid("b")])
        );
        // constant's `value` param and the a->b edge are both bindings; `value`
        // is bound to a value, the edge to a port.
        let literals = slots(&graph)
            .iter()
            .filter(|(_, source)| matches!(source, Source::Value(_)))
            .count();
        let ports = slots(&graph)
            .iter()
            .filter(|(_, source)| matches!(source, Source::Port(_)))
            .count();
        assert_eq!(ports, 1, "one edge");
        assert!(literals >= 1, "at least the constant value param");
    }

    fn ids(spec: &GraphSpec) -> Vec<&str> {
        spec.nodes.iter().map(|node| node.id.as_str()).collect()
    }

    /// Decoding lists the nodes in the order the spec listed them, whatever
    /// their ids.
    #[test]
    fn decoding_keeps_the_listed_order() {
        let spec: GraphSpec = serde_json::from_value(json!({
            "nodes": [
                { "id": "z", "type": "constant", "params": { "value": { "f32": 1.0 } } },
                { "id": "a", "type": "constant", "params": { "value": { "f32": 2.0 } } },
                { "id": "m", "type": "constant", "params": { "value": { "f32": 3.0 } } }
            ],
            "edges": []
        }))
        .expect("valid spec");
        let decoded = decode(&encode(&spec).expect("encode")).expect("decode");
        assert_eq!(ids(&decoded), ["z", "a", "m"]);
    }

    /// A node no composite holds comes after every placed one, by id; a child
    /// the graph no longer has is skipped.
    #[test]
    fn unplaced_nodes_come_after_the_placed_ones_by_id() {
        let spec: GraphSpec = serde_json::from_value(json!({
            "nodes": [
                { "id": "z", "type": "constant", "params": { "value": { "f32": 1.0 } } },
                { "id": "a", "type": "constant", "params": { "value": { "f32": 2.0 } } }
            ],
            "edges": []
        }))
        .expect("valid spec");
        let mut graph = encode(&spec).expect("encode");
        let added: GraphSpec = serde_json::from_value(json!({
            "nodes": [
                { "id": "y", "type": "constant", "params": { "value": { "f32": 3.0 } } },
                { "id": "b", "type": "constant", "params": { "value": { "f32": 4.0 } } }
            ],
            "edges": []
        }))
        .expect("valid spec");
        let diff = GraphSpecDiff {
            upsert_nodes: added.nodes,
            remove_nodes: vec!["z".to_string()],
            ..GraphSpecDiff::default()
        };
        graph
            .apply(spec_diff_to_graph_diff(&diff).expect("translate"))
            .expect("apply");
        assert_eq!(ids(&decode(&graph).expect("decode")), ["a", "b", "y"]);
    }

    /// An edge selector round-trips through `Source::Select`.
    #[test]
    fn edge_selector_round_trips_through_select() {
        let mut spec: GraphSpec = serde_json::from_value(json!({
            "nodes": [
                { "id": "a", "type": "constant", "params": { "value": { "f32": 1.0 } } },
                { "id": "b", "type": "output",   "params": { "path": "out/b" } }
            ],
            "edges": [
                { "from": { "node_id": "a", "output": "out" }, "to": { "node_id": "b", "input": "in" } }
            ]
        }))
        .expect("valid spec");
        spec.edges[0].selector = Some(vec![
            SelectorSegment::Field("weights".into()),
            SelectorSegment::Index(1),
        ]);

        let graph = encode(&spec).expect("encode");
        // The edge source is a `Select` over the `Port`, carrying the path.
        assert!(slots(&graph)
            .iter()
            .any(|(_, source)| matches!(source, Source::Select { .. })));

        // encode -> decode recovers the selector.
        let decoded = decode(&graph).expect("decode");
        assert_eq!(canonical(&spec), canonical(&decoded));
    }

    /// An inline input default rides a value on the input slot and comes back
    /// as an `input_default` — the same round trip as a parameter.
    #[test]
    fn inline_input_default_round_trips() {
        let spec: GraphSpec = serde_json::from_value(json!({
            "nodes": [
                {
                    "id": "shift",
                    "type": "add",
                    "input_defaults": { "operand_2": { "value": { "f32": 1.0 } } }
                }
            ],
            "edges": []
        }))
        .expect("valid spec");

        let graph = encode(&spec).expect("encode");
        let decoded = decode(&graph).expect("decode");
        assert_eq!(canonical(&spec), canonical(&decoded));
    }

    /// An edge into an input shadows that input's inline default: only the edge
    /// is encoded (matching `input_connections`), and it decodes as an edge.
    #[test]
    fn an_edge_shadows_an_inline_default_on_the_same_input() {
        let spec: GraphSpec = serde_json::from_value(json!({
            "nodes": [
                { "id": "src", "type": "constant", "params": { "value": { "f32": 3.0 } } },
                {
                    "id": "add",
                    "type": "add",
                    "input_defaults": { "operand_1": { "value": { "f32": 9.0 } } }
                }
            ],
            "edges": [
                { "from": { "node_id": "src", "output": "out" }, "to": { "node_id": "add", "input": "operand_1" } }
            ]
        }))
        .expect("valid spec");

        let graph = encode(&spec).expect("encode");
        // add/operand_1 is bound to the edge (a port), not the shadowed
        // default (a value).
        let into_operand_1: Vec<_> = slots(&graph)
            .into_iter()
            .filter(|(port, _)| *port == Port::new(node_id_uuid("add"), in_slot("operand_1")))
            .collect();
        assert_eq!(into_operand_1.len(), 1);
        assert!(matches!(into_operand_1[0].1, Source::Port(_)));

        // Decoding drops the shadowed default (it never evaluates); the edge
        // survives.
        let decoded = decode(&graph).expect("decode");
        let add = decoded.nodes.iter().find(|n| n.id == "add").unwrap();
        assert!(add.input_defaults.is_empty());
        assert_eq!(decoded.edges.len(), 1);
    }

    /// Declared shape metadata (`output_shapes` and an input default's `shape`)
    /// rides the reserved metadata slot and survives a round trip.
    #[test]
    fn declared_shape_metadata_round_trips() {
        let spec: GraphSpec = serde_json::from_value(json!({
            "nodes": [
                {
                    "id": "n",
                    "type": "constant",
                    "params": { "value": { "f32": 1.0 } },
                    "output_shapes": { "out": { "id": { "id": "Vec3" } } },
                    "input_defaults": {
                        "seed": { "value": { "f32": 0.0 }, "shape": { "id": { "id": "Scalar" } } }
                    }
                }
            ],
            "edges": []
        }))
        .expect("valid spec");

        let graph = encode(&spec).expect("encode");
        // The metadata rides one reserved slot as a JSON string value.
        assert!(slots(&graph).iter().any(|(port, source)| *port
            == Port::new(node_id_uuid("n"), meta_slot())
            && matches!(source, Source::Value(Value::String(_)))));

        let decoded = decode(&graph).expect("decode");
        assert_eq!(canonical(&spec), canonical(&decoded));
    }

    /// A spec diff (change a param, add a node, keep the incident edge) applied
    /// to an encoded graph decodes to the edited spec.
    #[test]
    fn a_spec_diff_edits_the_encoded_graph() {
        let base: GraphSpec = serde_json::from_value(json!({
            "nodes": [
                { "id": "c",   "type": "constant", "params": { "value": { "f32": 1.0 } } },
                { "id": "out", "type": "output",   "params": { "path": "a/b" } }
            ],
            "edges": [
                { "from": { "node_id": "c", "output": "out" }, "to": { "node_id": "out", "input": "in" } }
            ]
        }))
        .expect("base spec");
        let mut graph = encode(&base).expect("encode base");

        // Change `c`'s value and add a new constant `d`. `c` is upserted, so its
        // incident edge is included (the `GraphSpecDiff` contract).
        let diff = GraphSpecDiff {
            upsert_nodes: serde_json::from_value(json!([
                { "id": "c", "type": "constant", "params": { "value": { "f32": 2.0 } } },
                { "id": "d", "type": "constant", "params": { "value": { "f32": 5.0 } } }
            ]))
            .unwrap(),
            upsert_edges: serde_json::from_value(json!([
                { "from": { "node_id": "c", "output": "out" }, "to": { "node_id": "out", "input": "in" } }
            ]))
            .unwrap(),
            ..Default::default()
        };
        graph
            .apply(spec_diff_to_graph_diff(&diff).expect("translate"))
            .expect("apply");

        let expected: GraphSpec = serde_json::from_value(json!({
            "nodes": [
                { "id": "c",   "type": "constant", "params": { "value": { "f32": 2.0 } } },
                { "id": "d",   "type": "constant", "params": { "value": { "f32": 5.0 } } },
                { "id": "out", "type": "output",   "params": { "path": "a/b" } }
            ],
            "edges": [
                { "from": { "node_id": "c", "output": "out" }, "to": { "node_id": "out", "input": "in" } }
            ]
        }))
        .expect("expected spec");
        assert_eq!(
            canonical(&expected),
            canonical(&decode(&graph).expect("decode"))
        );
    }

    /// A spec diff that removes a node drops the node and its edges.
    #[test]
    fn a_spec_diff_removes_a_node() {
        let base: GraphSpec = serde_json::from_value(json!({
            "nodes": [
                { "id": "c",   "type": "constant", "params": { "value": { "f32": 1.0 } } },
                { "id": "d",   "type": "constant", "params": { "value": { "f32": 5.0 } } },
                { "id": "out", "type": "output",   "params": { "path": "a/b" } }
            ],
            "edges": [
                { "from": { "node_id": "c", "output": "out" }, "to": { "node_id": "out", "input": "in" } }
            ]
        }))
        .expect("base spec");
        let mut graph = encode(&base).expect("encode base");

        let diff = GraphSpecDiff {
            remove_nodes: vec!["d".to_string()],
            ..Default::default()
        };
        graph
            .apply(spec_diff_to_graph_diff(&diff).expect("translate"))
            .expect("apply");

        let decoded = decode(&graph).expect("decode");
        assert!(decoded.nodes.iter().all(|n| n.id != "d"));
        assert_eq!(decoded.nodes.len(), 2);
        assert_eq!(decoded.edges.len(), 1);
    }

    /// Every real authored fixture (`fixtures/node_graphs/*.json`) round-trips
    /// through `encode`/`decode` unchanged — the structural encoding is faithful
    /// for the graphs the apps actually run (selectors, inline defaults, ~90
    /// node kinds), not just hand-written specs.
    #[test]
    fn round_trips_every_real_fixture() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../fixtures/node_graphs");
        let mut count = 0;
        for entry in std::fs::read_dir(dir).expect("read fixtures dir") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = std::fs::read_to_string(&path).expect("read fixture");
            let file: serde_json::Value = serde_json::from_str(&raw).expect("fixture json");
            // Fixtures wrap the spec under a "spec" key; fall back to the whole file.
            let spec_json = file.get("spec").cloned().unwrap_or(file);
            let spec = crate::parse_spec(&spec_json.to_string())
                .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));

            let graph = encode(&spec).unwrap_or_else(|e| panic!("encode {}: {e}", path.display()));
            let decoded =
                decode(&graph).unwrap_or_else(|e| panic!("decode {}: {e}", path.display()));
            assert_eq!(
                canonical(&spec),
                canonical(&decoded),
                "round-trip mismatch for {}",
                path.display()
            );
            count += 1;
        }
        assert!(count >= 15, "expected the real fixtures, found {count}");
    }

    /// Compare two specs as order-independent node/edge sets via canonical JSON.
    fn canonical(spec: &GraphSpec) -> serde_json::Value {
        let mut nodes: Vec<serde_json::Value> = spec
            .nodes
            .iter()
            .map(|n| serde_json::to_value(n).expect("node json"))
            .collect();
        nodes.sort_by_key(|v| v["id"].as_str().unwrap_or_default().to_string());
        let mut edges: Vec<serde_json::Value> = spec
            .edges
            .iter()
            .map(|e| serde_json::to_value(e).expect("edge json"))
            .collect();
        edges.sort_by_key(|v| v.to_string());
        json!({ "nodes": nodes, "edges": edges })
    }
}
