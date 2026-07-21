//! The Vizij graph spec carried as a shared behavior [`Graph`].
//!
//! Arora's one behavior-edition seam is the interpreter module: a `Call` to
//! its LOAD function reaches [`BehaviorInterpreter::load`] through the
//! engine's normal dispatch, carrying an [`arora_behavior::Graph`]. Vizij's
//! authored model is a `vizij-graph-core` spec, whose node vocabulary the
//! shared model does not (yet) mirror structurally — so the spec rides the
//! shared model **opaquely**: one node bound to the [`GRAPH_PROGRAM`]
//! function, whose single input is fed the normalized spec JSON as a literal.
//! [`ProcessingGraph::load`](crate::ProcessingGraph) recognizes exactly that
//! shape and installs the spec in place.
//!
//! A structural mapping (Vizij node kinds as shared-model functions, edited
//! per-node through `GraphDiff`) would make Vizij graphs editable by any
//! shared-model tool; until then [`GraphDiff`](arora_behavior::GraphDiff)
//! edition is rejected and every edit is a whole-spec load.
//!
//! [`BehaviorInterpreter::load`]: arora_behavior::BehaviorInterpreter::load

use arora_behavior::graph::{Io, Link, LinkSource, Node, Port};
use arora_behavior::{interpreter_module, Graph};
use arora_types::call::Call;
use arora_types::value::Value;
use uuid::{uuid, Uuid};

/// Function id of the one carrier node: "run this Vizij graph spec".
/// Self-identifying like the vizij type ids: the ASCII bytes of "vizij" lead
/// the UUID, the behavior-carrier group tails it.
pub const GRAPH_PROGRAM: Uuid = uuid!("76697a69-6a00-0000-0000-000000400001");

/// Slot id of the carrier node's one input: the normalized spec JSON, fed by
/// a literal link.
pub const GRAPH_SPEC_SLOT: Uuid = uuid!("76697a69-6a00-0000-0000-000000400002");

/// Wrap `spec_json` (a Vizij graph spec, in any form the spec normalizer
/// accepts) into the one-node carrier [`Graph`].
pub fn encode(spec_json: &str) -> Graph {
    let mut graph = Graph::empty();
    graph.nodes.insert(
        GRAPH_PROGRAM,
        Node {
            id: GRAPH_PROGRAM,
            function: GRAPH_PROGRAM,
            inputs: vec![Io::new(GRAPH_SPEC_SLOT)],
            ..Node::default()
        },
    );
    graph.links.push(Link::new(
        Port::new(GRAPH_PROGRAM, GRAPH_SPEC_SLOT),
        LinkSource::Literal(Value::String(spec_json.to_string())),
    ));
    graph.root = Some(GRAPH_PROGRAM);
    graph
}

/// Read the spec JSON back out of a carrier [`Graph`]. Errors when the graph
/// is not the one-node carrier shape [`encode`] produces.
pub fn decode(graph: &Graph) -> Result<String, String> {
    let root = graph
        .root
        .ok_or_else(|| "the graph names no root node".to_string())?;
    let node = graph
        .node(&root)
        .ok_or_else(|| format!("the root node {root} is not in the graph"))?;
    if node.function != GRAPH_PROGRAM {
        return Err(format!(
            "the root node is bound to {}, not the Vizij graph-program function",
            node.function
        ));
    }
    match graph.link_to(&Port::new(node.id, GRAPH_SPEC_SLOT)) {
        Some(Link {
            source: LinkSource::Literal(Value::String(json)),
            ..
        }) => Ok(json.clone()),
        Some(_) => Err("the graph-spec slot is not fed a literal string".to_string()),
        None => Err("the graph-spec slot is not fed".to_string()),
    }
}

/// Build the interpreter-module LOAD [`Call`] that installs `spec_json` as
/// the running behavior — what an embedder dispatches (through an
/// `arora::Caller` or `Arora::call`) to swap the Vizij graph in place.
pub fn encode_load_call(spec_json: &str) -> Call {
    interpreter_module::encode_load(&encode(spec_json))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_round_trips_the_spec_json() {
        let json = r#"{"nodes":[],"edges":[]}"#;
        assert_eq!(decode(&encode(json)).unwrap(), json);
    }

    #[test]
    fn decode_rejects_non_carrier_graphs() {
        // No root.
        assert!(decode(&Graph::empty()).is_err());

        // A root bound to some other function.
        let mut graph = encode("{}");
        graph.nodes.get_mut(&GRAPH_PROGRAM).unwrap().function = Uuid::from_u128(7);
        assert!(decode(&graph).is_err());

        // The spec slot unfed.
        let mut graph = encode("{}");
        graph.links.clear();
        assert!(decode(&graph).is_err());
    }

    #[test]
    fn load_call_round_trips_through_the_interpreter_module() {
        let call = encode_load_call(r#"{"nodes":[]}"#);
        let graph = interpreter_module::decode_load(&call).unwrap();
        assert_eq!(decode(&graph).unwrap(), r#"{"nodes":[]}"#);
    }
}
