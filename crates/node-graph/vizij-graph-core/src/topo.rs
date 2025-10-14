use crate::types::{LinkSpec, NodeId, NodeSpec};
use std::collections::{HashMap, VecDeque};

pub fn topo_order(nodes: &[NodeSpec], links: &[LinkSpec]) -> Result<Vec<NodeId>, String> {
    let mut indeg: HashMap<NodeId, usize> = HashMap::new();
    let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

    for n in nodes {
        indeg.entry(n.id.clone()).or_insert(0);
    }

    for link in links {
        if !indeg.contains_key(&link.from.node_id) {
            return Err(format!(
                "topo_order: missing source node '{}'",
                link.from.node_id
            ));
        }
        if !indeg.contains_key(&link.to.node_id) {
            return Err(format!(
                "topo_order: missing target node '{}'",
                link.to.node_id
            ));
        }
        adj.entry(link.from.node_id.clone())
            .or_default()
            .push(link.to.node_id.clone());
        *indeg.entry(link.to.node_id.clone()).or_insert(0) += 1;
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
        GraphSpec, LinkInputEndpoint, LinkOutputEndpoint, LinkSpec, NodeParams, NodeType,
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
            links: vec![LinkSpec {
                from: LinkOutputEndpoint {
                    node_id: "a".into(),
                    output: "out".into(),
                },
                to: LinkInputEndpoint {
                    node_id: "b".into(),
                    input: "lhs".into(),
                },
                selector: None,
            }],
        };
        let order = topo_order(&g.nodes, &g.links).unwrap();
        assert_eq!(order.len(), 2);
    }
}
