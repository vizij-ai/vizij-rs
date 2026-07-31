//! A small JSON graph-spec builder shared by the crate's generated assets
//! (the ROS4HRI profile, the skill fragments): node/edge lists in the exact
//! form `normalize_graph_spec_value` accepts, with scratch ids for the
//! anonymous math in between.

use serde_json::{json, Value as Json};

/// Incrementally builds a graph spec's node/edge lists.
pub(crate) struct GraphBuilder {
    pub(crate) nodes: Vec<Json>,
    pub(crate) edges: Vec<Json>,
    scratch: u32,
}

impl GraphBuilder {
    pub(crate) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            scratch: 0,
        }
    }

    pub(crate) fn node(&mut self, id: &str, ty: &str, params: Json) -> String {
        self.nodes
            .push(json!({ "id": id, "type": ty, "params": params }));
        id.to_string()
    }

    pub(crate) fn edge(&mut self, from: &str, to: &str, input: &str) {
        self.edges
            .push(json!({ "from": { "node_id": from }, "to": { "node_id": to, "input": input } }));
    }

    /// A fresh constant node holding `value`.
    pub(crate) fn constant(&mut self, value: f64) -> String {
        self.scratch += 1;
        let id = format!("c{}", self.scratch);
        self.node(&id, "constant", json!({ "value": value }))
    }

    /// A scratch node wired from `inputs` (port name → source node).
    pub(crate) fn op(&mut self, ty: &str, params: Json, inputs: &[(&str, &str)]) -> String {
        self.scratch += 1;
        let id = format!("n{}", self.scratch);
        self.node(&id, ty, params);
        for (port, from) in inputs {
            self.edge(from, &id, port);
        }
        id
    }

    pub(crate) fn sub(&mut self, lhs: &str, rhs: &str) -> String {
        self.op("subtract", json!({}), &[("lhs", lhs), ("rhs", rhs)])
    }
    pub(crate) fn mul(&mut self, a: &str, b: &str) -> String {
        self.op("multiply", json!({}), &[("operand_0", a), ("operand_1", b)])
    }
    pub(crate) fn div(&mut self, lhs: &str, rhs: &str) -> String {
        self.op("divide", json!({}), &[("lhs", lhs), ("rhs", rhs)])
    }
    pub(crate) fn add2(&mut self, a: &str, b: &str) -> String {
        self.op("add", json!({}), &[("operand_0", a), ("operand_1", b)])
    }
    pub(crate) fn max2(&mut self, a: &str, b: &str) -> String {
        self.op("max", json!({}), &[("operand_0", a), ("operand_1", b)])
    }

    /// Exponential smoothing with the given half-life (seconds).
    pub(crate) fn damp(&mut self, from: &str, half_life: f64) -> String {
        self.op("damp", json!({ "half_life": half_life }), &[("in", from)])
    }

    /// An input node reading `path`, defaulting to `value` until staged.
    pub(crate) fn input(&mut self, id: &str, path: &str, value: Json) -> String {
        self.node(id, "input", json!({ "path": path, "value": value }))
    }

    /// An output node writing `path`.
    pub(crate) fn output(&mut self, from: &str, path: String) {
        self.scratch += 1;
        let id = format!("o{}", self.scratch);
        self.node(&id, "output", json!({ "path": path }));
        self.edge(from, &id, "in");
    }
}
