use crate::types::{NodeId, NodeSpec};
use std::collections::{HashMap, VecDeque};

pub fn topo_order(nodes: &[NodeSpec]) -> Result<Vec<NodeId>, String> {
    let mut indeg: HashMap<NodeId, usize> = HashMap::new();
    let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

    for n in nodes {
        indeg.entry(n.id.clone()).or_insert(0);
        for inp_conn in n.inputs.values() {
            adj.entry(inp_conn.node_id.clone())
                .or_default()
                .push(n.id.clone());
            *indeg.entry(n.id.clone()).or_default() += 1;
        }
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
                    inputs: HashMap::new(),
                },
                NodeSpec {
                    id: "b".into(),
                    kind: NodeType::Add,
                    params: Default::default(),
                    inputs: [(
                        "a".to_string(),
                        InputConnection {
                            node_id: "a".into(),
                            output_key: "out".to_string(),
                        },
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                },
            ],
        };
        let order = topo_order(&g.nodes).unwrap();
        assert_eq!(order.len(), 2);
    }
}
