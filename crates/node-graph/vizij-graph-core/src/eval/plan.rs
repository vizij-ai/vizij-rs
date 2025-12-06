use crate::types::{GraphSpec, InputConnection, SelectorSegment};
use hashbrown::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

/// Cached, topology-ready view of a [`GraphSpec`] for reuse across frames.
#[derive(Debug, Default)]
pub struct PlanCache {
    fingerprint: u64,
    /// Node indices in topological order.
    pub order: Vec<usize>,
    /// Per-node input bindings keyed by declared input name.
    pub inputs: Vec<HashMap<String, InputConnection>>,
}

impl PlanCache {
    /// Ensure the cache matches the provided spec; rebuild on structural change.
    pub fn ensure(&mut self, spec: &GraphSpec) -> Result<(), String> {
        let fp = fingerprint(spec);
        if fp == self.fingerprint && self.inputs.len() == spec.nodes.len() {
            return Ok(());
        }
        self.rebuild(spec, fp)
    }

    fn rebuild(&mut self, spec: &GraphSpec, fingerprint: u64) -> Result<(), String> {
        let inputs_map = spec.input_connections()?;
        let order_ids = crate::topo::topo_order(&spec.nodes, &spec.edges)?;
        let node_index: HashMap<&str, usize> = spec
            .nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| (node.id.as_str(), idx))
            .collect();

        let mut order = Vec::with_capacity(order_ids.len());
        for id in order_ids {
            let idx = node_index
                .get(id.as_str())
                .copied()
                .ok_or_else(|| format!("topo order referenced missing node '{}'", id))?;
            order.push(idx);
        }

        let mut inputs = vec![HashMap::new(); spec.nodes.len()];
        for (node_id, conns) in inputs_map {
            if let Some(&idx) = node_index.get(node_id.as_str()) {
                inputs[idx] = conns;
            }
        }

        self.order = order;
        self.inputs = inputs;
        self.fingerprint = fingerprint;
        Ok(())
    }
}

fn fingerprint(spec: &GraphSpec) -> u64 {
    let mut hasher = DefaultHasher::new();
    hasher.write_usize(spec.nodes.len());
    for node in &spec.nodes {
        hasher.write(node.id.as_bytes());
        hasher.write_usize(node.input_defaults.len());
        for (k, _) in &node.input_defaults {
            hasher.write(k.as_bytes());
        }
        hasher.write_usize(node.output_shapes.len());
        for (k, _) in &node.output_shapes {
            hasher.write(k.as_bytes());
        }
    }

    hasher.write_usize(spec.edges.len());
    for edge in &spec.edges {
        hasher.write(edge.from.node_id.as_bytes());
        hasher.write(edge.from.output.as_bytes());
        hasher.write(edge.to.node_id.as_bytes());
        hasher.write(edge.to.input.as_bytes());
        if let Some(selector) = &edge.selector {
            hasher.write_u8(1);
            for seg in selector {
                match seg {
                    SelectorSegment::Field(f) => {
                        hasher.write_u8(0);
                        hasher.write(f.as_bytes());
                    }
                    SelectorSegment::Index(i) => {
                        hasher.write_u8(1);
                        hasher.write_usize(*i);
                    }
                }
            }
        } else {
            hasher.write_u8(0);
        }
    }

    hasher.finish()
}
