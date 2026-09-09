//! A small JSON graph-spec builder shared by the crate's generated assets
//! (the ROS4HRI mapping, the skill fragments): node/edge lists in the exact
//! form `normalize_graph_spec_value` accepts.
//!
//! Every node is named by its author. The assets are read by people as often
//! as by the runtime — diffed in review, opened in the authoring app — so an
//! id says what the node computes (`gaze/left/yaw/atan`), and a section
//! prefix groups the nodes of one channel. Scalar constants are shared: one
//! node per distinct value, named for it (`const/0.28`).

use std::collections::BTreeMap;

use serde_json::{json, Value as Json};

/// Incrementally builds a graph spec's node/edge lists.
pub(crate) struct GraphBuilder {
    pub(crate) nodes: Vec<Json>,
    pub(crate) edges: Vec<Json>,
    /// Shared scalar constants, node id by rendered value.
    constants: BTreeMap<String, String>,
}

impl GraphBuilder {
    pub(crate) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            constants: BTreeMap::new(),
        }
    }

    pub(crate) fn node(&mut self, id: &str, ty: &str, params: Json) -> String {
        debug_assert!(
            !self.nodes.iter().any(|n| n["id"] == id),
            "node id {id:?} declared twice"
        );
        self.nodes
            .push(json!({ "id": id, "type": ty, "params": params }));
        id.to_string()
    }

    pub(crate) fn edge(&mut self, from: &str, to: &str, input: &str) {
        self.edges
            .push(json!({ "from": { "node_id": from }, "to": { "node_id": to, "input": input } }));
    }

    /// The shared constant node holding `value` — created on first use, then
    /// reused, under the id `const/<value>`.
    pub(crate) fn constant(&mut self, value: f64) -> String {
        let id = format!("const/{value}");
        if !self.constants.contains_key(&id) {
            self.node(&id, "constant", json!({ "value": value }));
            self.constants.insert(id.clone(), id.clone());
        }
        id
    }

    /// A named constant holding a float vector (an arora `f32s`), the layout
    /// `join` produces — so the two can meet in vector arithmetic.
    pub(crate) fn vector_constant(&mut self, id: &str, values: &[f64]) -> String {
        self.node(id, "constant", json!({ "value": { "f32s": values } }))
    }

    /// A named node wired from `inputs` (port name → source node).
    pub(crate) fn op(
        &mut self,
        id: &str,
        ty: &str,
        params: Json,
        inputs: &[(&str, &str)],
    ) -> String {
        self.node(id, ty, params);
        for (port, from) in inputs {
            self.edge(from, id, port);
        }
        id.to_string()
    }

    pub(crate) fn sub(&mut self, id: &str, lhs: &str, rhs: &str) -> String {
        self.op(id, "subtract", json!({}), &[("lhs", lhs), ("rhs", rhs)])
    }
    pub(crate) fn mul(&mut self, id: &str, a: &str, b: &str) -> String {
        self.op(
            id,
            "multiply",
            json!({}),
            &[("operand_0", a), ("operand_1", b)],
        )
    }
    pub(crate) fn div(&mut self, id: &str, lhs: &str, rhs: &str) -> String {
        self.op(id, "divide", json!({}), &[("lhs", lhs), ("rhs", rhs)])
    }
    pub(crate) fn add(&mut self, id: &str, a: &str, b: &str) -> String {
        self.op(id, "add", json!({}), &[("operand_0", a), ("operand_1", b)])
    }
    pub(crate) fn max(&mut self, id: &str, a: &str, b: &str) -> String {
        self.op(id, "max", json!({}), &[("operand_0", a), ("operand_1", b)])
    }

    /// `in` clamped to `[lo, hi]`.
    pub(crate) fn clamp(&mut self, id: &str, from: &str, lo: f64, hi: f64) -> String {
        let lo = self.constant(lo);
        let hi = self.constant(hi);
        self.op(
            id,
            "clamp",
            json!({}),
            &[("in", from), ("min", &lo), ("max", &hi)],
        )
    }

    /// `in` mapped linearly from `[in_lo, in_hi]` onto `[out_lo, out_hi]`,
    /// clamped to the output range.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn remap(
        &mut self,
        id: &str,
        from: &str,
        (in_lo, in_hi): (f64, f64),
        (out_lo, out_hi): (f64, f64),
    ) -> String {
        let in_lo = self.constant(in_lo);
        let in_hi = self.constant(in_hi);
        let out_lo = self.constant(out_lo);
        let out_hi = self.constant(out_hi);
        self.op(
            id,
            "remap",
            json!({}),
            &[
                ("in", from),
                ("in_min", &in_lo),
                ("in_max", &in_hi),
                ("out_min", &out_lo),
                ("out_max", &out_hi),
            ],
        )
    }

    /// `then` while `cond` holds, else `otherwise`.
    pub(crate) fn select(&mut self, id: &str, cond: &str, then: &str, otherwise: &str) -> String {
        self.op(
            id,
            "if",
            json!({}),
            &[("cond", cond), ("then", then), ("else", otherwise)],
        )
    }

    /// Exponential smoothing with the given half-life (seconds).
    pub(crate) fn damp(&mut self, id: &str, from: &str, half_life: f64) -> String {
        self.op(
            id,
            "damp",
            json!({ "half_life": half_life }),
            &[("in", from)],
        )
    }

    /// An input node reading `path`, defaulting to `value` until staged.
    pub(crate) fn input(&mut self, id: &str, path: &str, value: Json) -> String {
        self.node(id, "input", json!({ "path": path, "value": value }))
    }

    /// An output node writing `path`.
    pub(crate) fn output(&mut self, id: &str, from: &str, path: String) {
        self.node(id, "output", json!({ "path": path }));
        self.edge(from, id, "in");
    }
}
