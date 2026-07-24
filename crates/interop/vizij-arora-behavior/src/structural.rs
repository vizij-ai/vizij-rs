//! Structural encoding of a Vizij graph spec as an Arora shared [`Graph`].
//!
//! Where [`spec_graph`](crate::spec_graph) rides the whole spec opaquely on one
//! carrier node, this maps a [`GraphSpec`] **structurally**: one shared-model
//! [`Node`] per Vizij node, so every node, edge, and parameter is individually
//! addressable — the precondition for editing a Vizij graph through
//! `apply(GraphDiff)` instead of re-loading the whole spec (VIZ-79).
//!
//! # Conventions
//!
//! The shared [`Node`] has no parameter bag — behavior is a bare
//! `function: Uuid` and values arrive on input slots via [`Link`]s. So:
//!
//! - **kind → `Node.function`**: a stable per-kind id, `gen_uuid_from_str("vizij/graph/kind/<kind>")`.
//! - **edge → [`Link`]**: `source = LinkSource::Port(from)`, `target = to`, with
//!   port slots `gen_uuid_from_str("vizij/graph/{in,out}/<port>")`.
//! - **each present [`NodeParams`] field → its own literal slot**: an input
//!   [`Io`] at `gen_uuid_from_str("vizij/graph/param/<field>")` fed
//!   `LinkSource::Literal(value)`. The field's value is carried through
//!   [`arora_types::value_serde`], so the mapping is generic over every kind —
//!   no per-node-kind code — while keeping each parameter a distinct, typed slot.
//!
//! `gen_uuid_from_str` is one-way, so the names needed to rebuild the
//! `GraphSpec` (node ids, the kind string, port and parameter keys) are carried
//! in the graph's [`variables`](Graph) table (`uuid -> name`), which Vizij
//! graphs otherwise leave unused. Decoding is then a reverse lookup: a link with
//! a `Literal` source is a parameter; a link with a `Port` source is an edge.
//!
//! # Not yet mapped
//!
//! `NodeSpec::output_shapes`, `NodeSpec::input_defaults`, and an edge's
//! `selector` have no shared-model home yet; [`encode`] returns an error rather
//! than dropping them silently. Giving them structural homes (or an arora-sdk
//! `Link.selector`) is part of VIZ-79.

use std::collections::HashMap;

use arora_behavior::graph::{Io, Link, LinkSource, Node, Port};
use arora_behavior::Graph;
use arora_types::{gen_uuid_from_str, value_serde};
use uuid::Uuid;
use vizij_graph_core::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeSpec, NodeType,
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

/// The present (non-null) fields of a [`NodeParams`], as `(field, json)` pairs.
///
/// `NodeParams` is a flat serde struct; a field is "present" when it serializes
/// to something other than JSON `null` (i.e. a set `Option`, or a
/// `#[serde(default)]` scalar). Each becomes its own parameter slot.
fn present_params(params: &NodeParams) -> Result<Vec<(String, serde_json::Value)>, String> {
    let json = serde_json::to_value(params).map_err(|e| format!("serialize node params: {e}"))?;
    let map = match json {
        serde_json::Value::Object(map) => map,
        other => return Err(format!("node params did not serialize to an object: {other}")),
    };
    Ok(map.into_iter().filter(|(_, v)| !v.is_null()).collect())
}

/// Encode a Vizij [`GraphSpec`] as an Arora shared [`Graph`] (see module docs).
///
/// Errors if the spec uses a feature with no shared-model home yet
/// (`output_shapes`, `input_defaults`, or an edge `selector`).
pub fn encode(spec: &GraphSpec) -> Result<Graph, String> {
    let mut graph = Graph::empty();

    for node in &spec.nodes {
        if !node.output_shapes.is_empty() {
            return Err(format!("node '{}' has output_shapes, not yet mapped", node.id));
        }
        if !node.input_defaults.is_empty() {
            return Err(format!("node '{}' has input_defaults, not yet mapped", node.id));
        }

        let nid = node_id_uuid(&node.id);
        graph.variables.insert(nid, node.id.clone());

        let (function, kind) = kind_function(&node.kind)?;
        graph.variables.insert(function, kind);

        let mut inputs = Vec::new();
        for (field, value) in present_params(&node.params)? {
            let slot = param_slot(&field);
            graph.variables.insert(slot, field);
            inputs.push(Io::new(slot));
            let literal =
                value_serde::to_value(&value).map_err(|e| format!("param to value: {e}"))?;
            graph
                .links
                .push(Link::new(Port::new(nid, slot), LinkSource::Literal(literal)));
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
    }

    for edge in &spec.edges {
        if edge.selector.is_some() {
            return Err("edge selector is not yet mapped to the shared model".to_string());
        }
        let from = node_id_uuid(&edge.from.node_id);
        let to = node_id_uuid(&edge.to.node_id);
        let src_slot = out_slot(&edge.from.output);
        let dst_slot = in_slot(&edge.to.input);
        graph.variables.insert(src_slot, edge.from.output.clone());
        graph.variables.insert(dst_slot, edge.to.input.clone());

        declare_io(&mut graph, from, src_slot, false);
        declare_io(&mut graph, to, dst_slot, true);

        graph.links.push(Link::new(
            Port::new(to, dst_slot),
            LinkSource::Port(Port::new(from, src_slot)),
        ));
    }

    Ok(graph)
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

    // Node id -> its accumulating param JSON object.
    let mut param_objs: HashMap<Uuid, serde_json::Map<String, serde_json::Value>> = graph
        .nodes
        .keys()
        .map(|id| (*id, serde_json::Map::new()))
        .collect();
    let mut edges: Vec<EdgeSpec> = Vec::new();

    for link in &graph.links {
        let target = &link.target;
        match &link.source {
            LinkSource::Literal(value) => {
                let field = name(&target.port)?;
                let json: serde_json::Value = value_serde::from_value(value.clone())
                    .map_err(|e| format!("value to param json: {e}"))?;
                param_objs
                    .get_mut(&target.node)
                    .ok_or_else(|| format!("literal targets unknown node {}", target.node))?
                    .insert(field, json);
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
        nodes.push(NodeSpec {
            id: name(&node.id)?,
            kind,
            params,
            output_shapes: Default::default(),
            input_defaults: Default::default(),
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

    /// The homeless features are refused rather than silently dropped.
    #[test]
    fn encode_rejects_edge_selectors() {
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
        spec.edges[0].selector = Some(vec![vizij_graph_core::types::SelectorSegment::Index(0)]);

        assert!(encode(&spec).is_err());
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
