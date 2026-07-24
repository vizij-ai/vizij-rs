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
//! `function: Uuid` and values arrive on input slots via [`Link`]s. So:
//!
//! - **kind → `Node.function`**: a stable per-kind id, `gen_uuid_from_str("vizij/graph/kind/<kind>")`.
//! - **edge → [`Link`]**: `source = LinkSource::Port(from)`, `target = to`, with
//!   port slots `gen_uuid_from_str("vizij/graph/{in,out}/<port>")`. An edge
//!   `selector` wraps the source in `LinkSource::Select { source, path }`, the
//!   `Key` attribute path the runtime reads through [`Key::select`].
//! - **each present [`NodeParams`] field → its own literal slot**: an input
//!   [`Io`] at `gen_uuid_from_str("vizij/graph/param/<field>")` fed
//!   `LinkSource::Literal(value)`. The field's value is carried through
//!   [`arora_types::value_serde`], so the mapping is generic over every kind —
//!   no per-node-kind code — while keeping each parameter a distinct, typed slot.
//! - **each inline [`input default`](NodeSpec::input_defaults) → an input
//!   literal slot**: a default value on an *unconnected* input is the same
//!   `Literal`-fed-slot mechanism as a parameter, on the input slot
//!   `gen_uuid_from_str("vizij/graph/in/<port>")`. An input that also carries an
//!   edge takes the edge — the default is shadowed and never evaluates (this is
//!   [`GraphSpec::input_connections`], where an edge replaces a default), so
//!   structural encodes only the edge.
//!
//! `gen_uuid_from_str` is one-way, so the names needed to rebuild the
//! `GraphSpec` (node ids, the kind string, port and parameter keys) are carried
//! in the graph's [`variables`](Graph) table (`uuid -> name`), which Vizij
//! graphs otherwise leave unused. Decoding is then a reverse lookup on each
//! link's target slot, disambiguated by recomputing the slot id from the
//! recorded name: a `Literal` on a `param` slot is a parameter, on an `in` slot
//! an inline default; a `Port`/`Select` source on an `in` slot is an edge.
//!
//! # Declared-shape metadata
//!
//! [`output_shapes`](NodeSpec::output_shapes) and an input default's
//! [`shape`](vizij_graph_core::types::InputDefault::shape) are declared *type*
//! metadata, not values, so they have no per-value slot. They ride one reserved
//! `Literal(JSON)` slot per node (`meta_slot`), present only when a node
//! actually declares a shape (no real authored graph does — the runtime derives
//! shapes — but the eval path honors them where present, so they must survive a
//! round trip). This is the one carrier that is not a per-value slot.
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

use arora_behavior::graph::{GraphDiff, Io, Link, LinkSource, Node, Port};
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

fn node_id_uuid(id: &str) -> Uuid {
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
/// on decode, so it needs no `variables` entry.
fn meta_slot() -> Uuid {
    gen_uuid_from_str("vizij/graph/meta")
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
/// (one shared node, its parameter/input-default/metadata literal slots) and
/// `encode_edge` (one link, with a `Select` for a selector).
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
        encode_node(&mut graph, node, edged.get(node.id.as_str()))?;
    }
    for edge in &spec.edges {
        encode_edge(&mut graph, edge);
    }

    Ok(graph)
}

/// Encode one Vizij node into its shared [`Node`] plus the `Literal` links
/// feeding its parameter, inline-default, and metadata slots, recording the
/// names needed to decode it. `edged_inputs` names the node's inputs that carry
/// an edge (whose inline default is shadowed and skipped). The whole-graph
/// [`encode`] and a `GraphDiff` node upsert share this.
fn encode_node(
    graph: &mut Graph,
    node: &NodeSpec,
    edged_inputs: Option<&HashSet<&str>>,
) -> Result<(), String> {
    let nid = node_id_uuid(&node.id);
    graph.variables.insert(nid, node.id.clone());
    let (function, kind) = kind_function(&node.kind)?;
    graph.variables.insert(function, kind);

    let mut inputs = Vec::new();

    // Parameters: each present field is its own Literal-fed slot.
    for (field, value) in present_params(&node.params)? {
        let slot = param_slot(&field);
        graph.variables.insert(slot, field);
        inputs.push(Io::new(slot));
        let literal = value_serde::to_value(&value).map_err(|e| format!("param to value: {e}"))?;
        graph.links.push(Link::new(
            Port::new(nid, slot),
            LinkSource::Literal(literal),
        ));
    }

    // Inline input defaults: a default value on an unconnected input, on the
    // input slot. An edge on the same input takes precedence (default shadowed).
    for (port, default) in &node.input_defaults {
        if edged_inputs.is_some_and(|set| set.contains(port.as_str())) {
            continue;
        }
        let slot = in_slot(port);
        graph.variables.insert(slot, port.clone());
        inputs.push(Io::new(slot));
        graph.links.push(Link::new(
            Port::new(nid, slot),
            LinkSource::Literal(default.value.clone()),
        ));
    }

    // Declared-shape metadata, when present: one reserved Literal(JSON) slot.
    if let Some(meta) = node_meta_json(node)? {
        inputs.push(Io::new(meta_slot()));
        graph.links.push(Link::new(
            Port::new(nid, meta_slot()),
            LinkSource::Literal(Value::String(meta)),
        ));
    }

    graph.nodes.insert(
        nid,
        Node {
            id: nid,
            function,
            inputs,
            ..Node::default()
        },
    );
    Ok(())
}

/// Encode one Vizij edge into its [`Link`], declaring the source-output and
/// target-input slots on the endpoints it connects. A selector wraps the `Port`
/// source in a [`Select`](LinkSource::Select) carrying the `Key` path. The link
/// replaces any existing one on the target input (at most one link per input,
/// matching [`Graph::apply`] and `GraphSpec::input_connections`). Shared by the
/// whole-graph [`encode`] and a `GraphDiff` edge upsert.
fn encode_edge(graph: &mut Graph, edge: &EdgeSpec) {
    let from = node_id_uuid(&edge.from.node_id);
    let to = node_id_uuid(&edge.to.node_id);
    let src_slot = out_slot(&edge.from.output);
    let dst_slot = in_slot(&edge.to.input);
    graph.variables.insert(src_slot, edge.from.output.clone());
    graph.variables.insert(dst_slot, edge.to.input.clone());

    declare_io(graph, from, src_slot, false);
    declare_io(graph, to, dst_slot, true);

    // A selector on the edge reads a sub-path of the source output: wrap the
    // `Port` source in a `Select` carrying the path.
    let mut source = LinkSource::Port(Port::new(from, src_slot));
    if let Some(selector) = &edge.selector {
        source = LinkSource::Select {
            source: Box::new(source),
            path: selector_to_key(selector),
        };
    }
    let target = Port::new(to, dst_slot);
    graph.links.retain(|l| l.target != target);
    graph.links.push(Link::new(target, source));
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

/// Ensure `node` declares a slot `slot` in its inputs (`is_input`) or outputs.
fn declare_io(graph: &mut Graph, node: Uuid, slot: Uuid, is_input: bool) {
    if let Some(n) = graph.nodes.get_mut(&node) {
        let list = if is_input {
            &mut n.inputs
        } else {
            &mut n.outputs
        };
        if !list.iter().any(|io| io.id == slot) {
            list.push(Io::new(slot));
        }
    }
}

/// Decode an Arora shared [`Graph`] produced by [`encode`] back into a
/// [`GraphSpec`]. Nodes are ordered by id (the shared model stores them
/// unordered); evaluation is order-independent (the plan topo-sorts).
pub fn decode(graph: &Graph) -> Result<GraphSpec, String> {
    let name = |id: &Uuid| -> Result<String, String> {
        graph
            .variables
            .get(id)
            .cloned()
            .ok_or_else(|| format!("no name recorded for id {id}"))
    };

    // Per-node accumulators: parameter JSON, input-default value JSON, parsed
    // shape metadata. Edges collect as they are read.
    let mut param_objs: HashMap<Uuid, serde_json::Map<String, serde_json::Value>> = graph
        .nodes
        .keys()
        .map(|id| (*id, serde_json::Map::new()))
        .collect();
    // Inline-default values, kept as the literal `Value` (not round-tripped
    // through JSON), keyed by node then port.
    let mut default_values: HashMap<Uuid, Vec<(String, Value)>> = HashMap::new();
    // The reserved metadata slot's parsed JSON blob per node (see [`meta_slot`]).
    let mut meta_objs: HashMap<Uuid, serde_json::Value> = HashMap::new();
    let mut edges: Vec<EdgeSpec> = Vec::new();
    let meta = meta_slot();

    for link in &graph.links {
        let target = &link.target;
        match &link.source {
            LinkSource::Literal(value) => {
                // The reserved metadata slot: a JSON blob of declared shapes.
                if target.port == meta {
                    let json = match value {
                        Value::String(json) => json,
                        _ => return Err("the metadata slot is not fed a JSON string".to_string()),
                    };
                    let parsed: serde_json::Value = serde_json::from_str(json)
                        .map_err(|e| format!("node metadata json: {e}"))?;
                    meta_objs.insert(target.node, parsed);
                    continue;
                }
                let field = name(&target.port)?;
                // A literal on a `param` slot is a parameter; on an `in` slot an
                // inline default. Recompute the slot id from the recorded name
                // to tell which (the namespaces differ, so the ids differ).
                if target.port == param_slot(&field) {
                    let json: serde_json::Value = value_serde::from_value(value.clone())
                        .map_err(|e| format!("value to json: {e}"))?;
                    param_objs
                        .get_mut(&target.node)
                        .ok_or_else(|| format!("literal targets unknown node {}", target.node))?
                        .insert(field, json);
                } else if target.port == in_slot(&field) {
                    default_values
                        .entry(target.node)
                        .or_default()
                        .push((field, value.clone()));
                } else {
                    return Err(format!(
                        "literal on unrecognized slot {} of node {}",
                        target.port, target.node
                    ));
                }
            }
            LinkSource::Port(source) => {
                edges.push(EdgeSpec {
                    from: EdgeOutputEndpoint {
                        node_id: name(&source.node)?,
                        output: name(&source.port)?,
                    },
                    to: EdgeInputEndpoint {
                        node_id: name(&target.node)?,
                        input: name(&target.port)?,
                    },
                    selector: None,
                });
            }
            // A `Select` over a `Port` is an edge that reads a sub-path of the
            // source output; recover the Vizij selector from its `Key` path.
            LinkSource::Select { source, path } => {
                let source = match source.as_ref() {
                    LinkSource::Port(port) => port,
                    _ => return Err("structural decode expects a Select over a Port".to_string()),
                };
                edges.push(EdgeSpec {
                    from: EdgeOutputEndpoint {
                        node_id: name(&source.node)?,
                        output: name(&source.port)?,
                    },
                    to: EdgeInputEndpoint {
                        node_id: name(&target.node)?,
                        input: name(&target.port)?,
                    },
                    selector: Some(key_to_selector(path)),
                });
            }
            LinkSource::Variable(_) => {
                return Err("structural decode does not expect variable link sources".to_string());
            }
        }
    }

    let mut nodes: Vec<NodeSpec> = Vec::with_capacity(graph.nodes.len());
    for node in graph.nodes.values() {
        let params_obj = param_objs.remove(&node.id).unwrap_or_default();
        let params: NodeParams = serde_json::from_value(serde_json::Value::Object(params_obj))
            .map_err(|e| format!("rebuild node params: {e}"))?;
        let kind: NodeType =
            serde_json::from_value(serde_json::Value::String(name(&node.function)?))
                .map_err(|e| format!("rebuild node kind: {e}"))?;

        let node_meta = meta_objs
            .remove(&node.id)
            .unwrap_or(serde_json::Value::Null);
        let output_shapes = match node_meta.get("output_shapes") {
            Some(shapes) if !shapes.is_null() => serde_json::from_value(shapes.clone())
                .map_err(|e| format!("rebuild output_shapes: {e}"))?,
            _ => Default::default(),
        };
        let input_shapes = node_meta.get("input_shapes").and_then(|v| v.as_object());
        // Recombine each inline default's value (the literal `Value` from its
        // `in` slot) with its declared shape (from the metadata blob).
        let mut input_defaults: std::collections::HashMap<String, InputDefault> =
            std::collections::HashMap::new();
        for (port, value) in default_values.remove(&node.id).unwrap_or_default() {
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
            id: name(&node.id)?,
            kind,
            params,
            output_shapes,
            input_defaults: input_defaults.into_iter().collect(),
        });
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

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
/// An upserted node is **removed and re-added**, which clears any stale
/// parameter/default links (e.g. a parameter the edit dropped). So **every edge
/// incident to an upserted node must appear in `upsert_edges`** — otherwise the
/// node removal drops it and it is not restored. The editor gathers those edges
/// from the new spec; this keeps the translation context-free (it needs no view
/// of the running graph).
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphSpecDiff {
    /// Nodes to insert or replace (each removed then re-added).
    #[serde(default)]
    pub upsert_nodes: Vec<NodeSpec>,
    /// Node ids to remove (with their incident links).
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
/// removed then re-added (see [`GraphSpecDiff`]), so the caller must include
/// every edge incident to an upserted node in `upsert_edges`.
pub fn spec_diff_to_graph_diff(diff: &GraphSpecDiff) -> Result<GraphDiff, String> {
    // Encode the upserted nodes and edges into a scratch graph. Nodes first,
    // edges after, so an edge's link replaces any inline-default link on the
    // same input (the edge wins — as in `encode`).
    let mut scratch = Graph::empty();
    for node in &diff.upsert_nodes {
        encode_node(&mut scratch, node, None)?;
    }
    for edge in &diff.upsert_edges {
        encode_edge(&mut scratch, edge);
    }

    // Upserted nodes are removed then re-added; `Graph::apply` removes before it
    // adds, so this clears each upserted node's stale links first.
    let remove_nodes: Vec<Uuid> = diff
        .remove_nodes
        .iter()
        .map(|id| node_id_uuid(id))
        .chain(diff.upsert_nodes.iter().map(|node| node_id_uuid(&node.id)))
        .collect();
    let remove_links: Vec<Port> = diff
        .remove_edges
        .iter()
        .map(|target| Port::new(node_id_uuid(&target.node_id), in_slot(&target.input)))
        .collect();

    Ok(GraphDiff {
        add_nodes: scratch.nodes.into_values().collect(),
        add_links: scratch.links,
        remove_nodes,
        remove_links,
        variables: scratch.variables,
        ..GraphDiff::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        assert_eq!(graph.nodes.len(), 2);
        // constant's `value` param and the a->b edge are both links; `value`
        // rides a Literal source, the edge a Port source.
        let literals = graph
            .links
            .iter()
            .filter(|l| matches!(l.source, LinkSource::Literal(_)))
            .count();
        let ports = graph
            .links
            .iter()
            .filter(|l| matches!(l.source, LinkSource::Port(_)))
            .count();
        assert_eq!(ports, 1, "one edge");
        assert!(literals >= 1, "at least the constant value param");
    }

    /// An edge selector round-trips through `LinkSource::Select`.
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
        assert!(graph
            .links
            .iter()
            .any(|link| matches!(link.source, LinkSource::Select { .. })));

        // encode -> decode recovers the selector.
        let decoded = decode(&graph).expect("decode");
        assert_eq!(canonical(&spec), canonical(&decoded));
    }

    /// An inline input default rides a `Literal` on the input slot and comes back
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
        // Exactly one link targets add/operand_1, and it is the edge (a Port),
        // not the shadowed default (a Literal).
        let into_operand_1: Vec<_> = graph
            .links
            .iter()
            .filter(|l| l.target == Port::new(node_id_uuid("add"), in_slot("operand_1")))
            .collect();
        assert_eq!(into_operand_1.len(), 1);
        assert!(matches!(into_operand_1[0].source, LinkSource::Port(_)));

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
        // The metadata rides one reserved slot as a JSON string literal.
        assert!(graph
            .links
            .iter()
            .any(|l| l.target == Port::new(node_id_uuid("n"), meta_slot())
                && matches!(l.source, LinkSource::Literal(Value::String(_)))));

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
