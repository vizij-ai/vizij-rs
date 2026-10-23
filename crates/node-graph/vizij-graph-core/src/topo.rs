//! Topological ordering helpers for graph execution planning.

use crate::types::{EdgeSpec, NodeId, NodeSpec};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// The nodes in an order every edge respects, and among the orders that do,
/// the one that always takes the earliest-listed node ready to run — so the
/// order is a function of the graph alone. Where two nodes write the same
/// path the later writer wins, and that must not depend on anything else:
/// the order is the graph's precedence. A component listed after the rest
/// therefore runs after all of it.
pub fn topo_order(nodes: &[NodeSpec], edges: &[EdgeSpec]) -> Result<Vec<NodeId>, String> {
    let index: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    if index.len() != nodes.len() {
        return Err("topo_order: two nodes share an id".into());
    }
    let mut indeg = vec![0usize; nodes.len()];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for edge in edges {
        let from = *index
            .get(edge.from.node_id.as_str())
            .ok_or_else(|| format!("topo_order: missing source node '{}'", edge.from.node_id))?;
        let to = *index
            .get(edge.to.node_id.as_str())
            .ok_or_else(|| format!("topo_order: missing target node '{}'", edge.to.node_id))?;
        adj[from].push(to);
        indeg[to] += 1;
    }

    let mut ready: BinaryHeap<Reverse<usize>> = indeg
        .iter()
        .enumerate()
        .filter(|(_, d)| **d == 0)
        .map(|(i, _)| Reverse(i))
        .collect();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(Reverse(u)) = ready.pop() {
        order.push(nodes[u].id.clone());
        for &v in &adj[u] {
            indeg[v] -= 1;
            if indeg[v] == 0 {
                ready.push(Reverse(v));
            }
        }
    }

    if order.len() != nodes.len() {
        return Err("cycle detected in graph".into());
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeType,
    };
    use vizij_api_core::value::float;
    #[test]
    fn simple_topo() {
        let g = GraphSpec {
            nodes: vec![
                NodeSpec {
                    id: "a".into(),
                    kind: NodeType::Constant,
                    params: NodeParams {
                        value: Some(float(1.0)),
                        ..Default::default()
                    },
                    output_shapes: Default::default(),
                    input_defaults: Default::default(),
                },
                NodeSpec {
                    id: "b".into(),
                    kind: NodeType::Add,
                    params: Default::default(),
                    output_shapes: Default::default(),
                    input_defaults: Default::default(),
                },
            ],
            edges: vec![EdgeSpec {
                from: EdgeOutputEndpoint {
                    node_id: "a".into(),
                    output: "out".into(),
                },
                to: EdgeInputEndpoint {
                    node_id: "b".into(),
                    input: "lhs".into(),
                },
                selector: None,
            }],
            ..Default::default()
        }
        .with_cache();
        let order = topo_order(&g.nodes, &g.edges).unwrap();
        assert_eq!(order.len(), 2);
    }

    fn node(id: &str) -> NodeSpec {
        NodeSpec {
            id: id.into(),
            kind: NodeType::Add,
            params: Default::default(),
            output_shapes: Default::default(),
            input_defaults: Default::default(),
        }
    }

    fn edge(from: &str, to: &str) -> EdgeSpec {
        EdgeSpec {
            from: EdgeOutputEndpoint {
                node_id: from.into(),
                output: "out".into(),
            },
            to: EdgeInputEndpoint {
                node_id: to.into(),
                input: "lhs".into(),
            },
            selector: None,
        }
    }

    /// The order is the graph's alone: every call gives the same one, and
    /// a component listed after another runs after all of it, however
    /// shallow it is — so a later writer of a shared path is the later one.
    #[test]
    fn the_order_is_the_listed_precedence() {
        // `base` is a chain three deep; `late` is one node, listed after it.
        let nodes = vec![node("base/a"), node("base/b"), node("base/c"), node("late")];
        let edges = vec![edge("base/a", "base/b"), edge("base/b", "base/c")];
        let order = topo_order(&nodes, &edges).unwrap();
        assert_eq!(order, ["base/a", "base/b", "base/c", "late"]);
        for _ in 0..32 {
            assert_eq!(topo_order(&nodes, &edges).unwrap(), order);
        }
        // Independent nodes keep their listed order.
        let nodes = vec![node("z"), node("a"), node("m")];
        assert_eq!(topo_order(&nodes, &[]).unwrap(), ["z", "a", "m"]);
    }

    #[test]
    fn two_nodes_sharing_an_id_are_refused() {
        assert!(topo_order(&[node("a"), node("a")], &[]).is_err());
    }
}
