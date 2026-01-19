use crate::types::{EdgeSpec, NodeId, NodeSpec};
use std::collections::{HashMap, VecDeque};

/// Return node ids in topological order.
///
/// # Examples
///
/// ```
/// use vizij_graph_core::topo::topo_order;
/// use vizij_graph_core::types::{
///     EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeSpec, NodeType,
/// };
/// use vizij_api_core::Value;
///
/// let spec = GraphSpec {
///     nodes: vec![
///         NodeSpec {
///             id: "a".into(),
///             kind: NodeType::Constant,
///             params: NodeParams {
///                 value: Some(Value::Float(1.0)),
///                 ..Default::default()
///             },
///             output_shapes: Default::default(),
///             input_defaults: Default::default(),
///         },
///         NodeSpec {
///             id: "b".into(),
///             kind: NodeType::Add,
///             params: Default::default(),
///             output_shapes: Default::default(),
///             input_defaults: Default::default(),
///         },
///     ],
///     edges: vec![EdgeSpec {
///         from: EdgeOutputEndpoint {
///             node_id: "a".into(),
///             output: "out".into(),
///         },
///         to: EdgeInputEndpoint {
///             node_id: "b".into(),
///             input: "lhs".into(),
///         },
///         selector: None,
///     }],
///     ..Default::default()
/// };
///
/// let order = topo_order(&spec.nodes, &spec.edges).unwrap();
/// assert_eq!(order.len(), 2);
/// ```
///
/// # Errors
///
/// Returns an error when an edge references a missing node or when the graph contains cycles.
pub fn topo_order(nodes: &[NodeSpec], edges: &[EdgeSpec]) -> Result<Vec<NodeId>, String> {
    let mut indeg: HashMap<NodeId, usize> = HashMap::new();
    let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

    for n in nodes {
        indeg.entry(n.id.clone()).or_insert(0);
    }

    for edge in edges {
        if !indeg.contains_key(&edge.from.node_id) {
            return Err(format!(
                "topo_order: missing source node '{}'",
                edge.from.node_id
            ));
        }
        if !indeg.contains_key(&edge.to.node_id) {
            return Err(format!(
                "topo_order: missing target node '{}'",
                edge.to.node_id
            ));
        }
        adj.entry(edge.from.node_id.clone())
            .or_default()
            .push(edge.to.node_id.clone());
        *indeg.entry(edge.to.node_id.clone()).or_insert(0) += 1;
    }

    let mut q: VecDeque<NodeId> = indeg
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(k, _)| k.clone())
        .collect();

    let mut order = Vec::new();
    while let Some(u) = q.pop_front() {
        order.push(u.clone());
        if let Some(vs) = adj.get(&u) {
            for v in vs {
                if let Some(d) = indeg.get_mut(v) {
                    *d -= 1;
                    if *d == 0 {
                        q.push_back(v.clone());
                    }
                }
            }
        }
    }

    if order.len() != indeg.len() {
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
    use vizij_api_core::Value;
    #[test]
    fn simple_topo() {
        let g = GraphSpec {
            nodes: vec![
                NodeSpec {
                    id: "a".into(),
                    kind: NodeType::Constant,
                    params: NodeParams {
                        value: Some(Value::Float(1.0)),
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
}
