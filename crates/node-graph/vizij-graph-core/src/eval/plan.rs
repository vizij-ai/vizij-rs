//! Execution-plan construction and cache management for graph evaluation.

use crate::schema::{registry, NodeSignature};
use crate::types::{
    GraphSpec, InputConnection, InputDefault, NodeSpec, NodeType, Selector, SelectorSegment,
};
use hashbrown::{HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

use super::value_layout::PortValue;
use super::variadic::{compare_variadic_keys, parse_variadic_key};

#[derive(Clone, Copy, Debug, Default)]
pub struct VariadicRange {
    pub start: usize,
    pub len: usize,
}

#[derive(Clone, Debug, Default)]
pub struct PortLayout {
    pub slots: Vec<String>,
    name_to_slot: HashMap<String, usize>,
    variadics: HashMap<String, VariadicRange>,
}

impl PortLayout {
    pub fn slot(&self, name: &str) -> Option<usize> {
        self.name_to_slot.get(name).copied()
    }

    pub fn slot_name(&self, slot: usize) -> Option<&str> {
        self.slots.get(slot).map(String::as_str)
    }

    pub fn variadic_range(&self, group: &str) -> Option<VariadicRange> {
        self.variadics.get(group).copied()
    }

    pub fn variadic_groups(&self) -> &HashMap<String, VariadicRange> {
        &self.variadics
    }

    fn insert_slot(&mut self, name: String) {
        if !self.name_to_slot.contains_key(&name) {
            let slot = self.slots.len();
            self.slots.push(name.clone());
            self.name_to_slot.insert(name, slot);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NodeLayout {
    pub inputs: PortLayout,
    pub outputs: PortLayout,
}

#[derive(Clone, Debug)]
pub struct ResolvedInputSource {
    pub node_idx: usize,
    pub slot: usize,
    pub output_name: String,
    pub selector: Option<Selector>,
}

#[derive(Clone, Debug)]
pub struct InputBinding {
    pub source: Option<ResolvedInputSource>,
    pub default: Option<PortValue>,
}

/// Cached, topology-ready view of a [`GraphSpec`] for reuse across frames.
#[derive(Debug, Default)]
pub struct PlanCache {
    fingerprint: u64,
    version: u64,
    /// Node indices in topological order.
    pub order: Vec<usize>,
    /// Per-node input bindings keyed by slot index.
    pub input_bindings: Vec<Vec<InputBinding>>,
    /// Precomputed input/output layouts for each node.
    pub layouts: Vec<NodeLayout>,
    /// Map from node id -> stable index into spec.nodes / outputs_vec.
    pub node_index: HashMap<String, usize>,
}

impl PlanCache {
    /// Ensure the cache matches the provided spec; rebuild on structural change.
    pub fn ensure(&mut self, spec: &GraphSpec) -> Result<(), String> {
        if spec.version > 0 {
            self.ensure_versioned(spec)
        } else {
            let fp = fingerprint_spec(spec);
            if fp == self.fingerprint && self.layouts.len() == spec.nodes.len() {
                return Ok(());
            }
            self.rebuild(spec, fp, 0)
        }
    }

    /// Extend the plan to `spec`, whose nodes from `first` on form a component
    /// the earlier nodes neither feed nor read — what grafting a run adds.
    /// The plan for the nodes before `first` stands; the component's own is
    /// built from its nodes and edges and placed after it, which is the
    /// order a rebuild gives, since [`topo_order`](crate::topo::topo_order)
    /// runs a later-listed component after all of the earlier ones. The plan
    /// then serves `spec`'s version.
    ///
    /// Errors, leaving the plan unchanged, when the plan does not cover
    /// exactly the nodes before `first` or an edge crosses `first`: the
    /// caller rebuilds.
    pub fn append_component(&mut self, spec: &GraphSpec, first: usize) -> Result<(), String> {
        if self.layouts.len() != first || first > spec.nodes.len() {
            return Err(format!(
                "the plan covers {} nodes, not the {first} before the component",
                self.layouts.len()
            ));
        }
        let component_ids: HashSet<&str> = spec.nodes[first..]
            .iter()
            .map(|node| node.id.as_str())
            .collect();
        let mut edges = Vec::new();
        for edge in &spec.edges {
            let from = component_ids.contains(edge.from.node_id.as_str());
            let to = component_ids.contains(edge.to.node_id.as_str());
            match (from, to) {
                (true, true) => edges.push(edge.clone()),
                (false, false) => {}
                _ => {
                    return Err(format!(
                        "edge {} -> {} crosses into the component",
                        edge.from.node_id, edge.to.node_id
                    ))
                }
            }
        }
        let component = GraphSpec {
            nodes: spec.nodes[first..].to_vec(),
            edges,
            ..GraphSpec::default()
        };
        let mut plan = PlanCache::default();
        plan.rebuild(&component, 0, 0)?;

        self.order
            .extend(plan.order.into_iter().map(|idx| idx + first));
        for mut bindings in plan.input_bindings {
            for binding in &mut bindings {
                if let Some(source) = &mut binding.source {
                    source.node_idx += first;
                }
            }
            self.input_bindings.push(bindings);
        }
        self.layouts.extend(plan.layouts);
        self.node_index.extend(
            plan.node_index
                .into_iter()
                .map(|(id, idx)| (id, idx + first)),
        );
        self.fingerprint = spec.fingerprint;
        self.version = spec.version;
        Ok(())
    }

    /// Shrink the plan to `spec`: the spec it served minus the nodes at
    /// `removed` (indices into that spec), which no remaining node reads —
    /// what pruning a run takes out. Nothing is rebuilt: the remaining
    /// layouts, bindings and order keep theirs, their indices closed up over
    /// the gap. The plan then serves `spec`'s version.
    ///
    /// Errors, leaving the plan unchanged, when the counts do not add up or
    /// a remaining node reads a removed one: the caller rebuilds.
    pub fn remove_nodes(&mut self, spec: &GraphSpec, removed: &[usize]) -> Result<(), String> {
        let before = self.layouts.len();
        let mut gone = vec![false; before];
        for &idx in removed {
            match gone.get_mut(idx) {
                Some(slot) if !*slot => *slot = true,
                _ => return Err(format!("{idx} is not a node the plan covers, once")),
            }
        }
        if before != spec.nodes.len() + removed.len() {
            return Err(format!(
                "the plan covers {before} nodes, not {} kept and {} removed",
                spec.nodes.len(),
                removed.len()
            ));
        }
        let mut remap: Vec<Option<usize>> = Vec::with_capacity(before);
        let mut next = 0;
        for gone in gone {
            if gone {
                remap.push(None);
            } else {
                remap.push(Some(next));
                next += 1;
            }
        }
        let reads_removed = self
            .input_bindings
            .iter()
            .enumerate()
            .filter(|(idx, _)| remap[*idx].is_some())
            .flat_map(|(_, bindings)| bindings)
            .filter_map(|binding| binding.source.as_ref())
            .any(|source| remap[source.node_idx].is_none());
        if reads_removed {
            return Err("a remaining node reads a removed one".into());
        }

        let kept = |idx: &usize| remap[*idx].is_some();
        let layouts = std::mem::take(&mut self.layouts);
        self.layouts = layouts
            .into_iter()
            .enumerate()
            .filter(|(idx, _)| kept(idx))
            .map(|(_, layout)| layout)
            .collect();
        let input_bindings = std::mem::take(&mut self.input_bindings);
        self.input_bindings = input_bindings
            .into_iter()
            .enumerate()
            .filter(|(idx, _)| kept(idx))
            .map(|(_, mut bindings)| {
                for binding in &mut bindings {
                    if let Some(source) = &mut binding.source {
                        source.node_idx = remap[source.node_idx].expect("checked above");
                    }
                }
                bindings
            })
            .collect();
        self.order = self.order.iter().filter_map(|&idx| remap[idx]).collect();
        self.node_index.retain(|_, idx| remap[*idx].is_some());
        for idx in self.node_index.values_mut() {
            *idx = remap[*idx].expect("retained above");
        }
        self.fingerprint = spec.fingerprint;
        self.version = spec.version;
        Ok(())
    }

    /// Version-aware fast path: compare caller-managed version for O(1) steady-state checks.
    pub fn ensure_versioned(&mut self, spec: &GraphSpec) -> Result<(), String> {
        debug_assert!(
            spec.version == 0 || spec.fingerprint == 0 || spec.fingerprint == fingerprint_spec(spec),
            "spec fingerprint does not match contents; caller likely forgot to bump version/fingerprint"
        );

        if self.version == spec.version && self.layouts.len() == spec.nodes.len() {
            return Ok(());
        }
        let fp = if spec.fingerprint != 0 {
            spec.fingerprint
        } else {
            fingerprint_spec(spec)
        };
        self.rebuild(spec, fp, spec.version)
    }

    fn rebuild(&mut self, spec: &GraphSpec, fingerprint: u64, version: u64) -> Result<(), String> {
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
                .ok_or_else(|| format!("plan referenced missing node '{}'", id))?;
            order.push(idx);
        }

        let signatures = signature_map();
        let referenced_outputs = gather_referenced_outputs(spec, &node_index);
        let empty_connections: HashMap<String, InputConnection> = HashMap::new();

        let mut layouts = Vec::with_capacity(spec.nodes.len());
        for (idx, node) in spec.nodes.iter().enumerate() {
            let signature = signatures.get(&node.kind);
            let connections = inputs_map.get(&node.id).unwrap_or(&empty_connections);
            let inputs_layout = build_input_layout(node, signature, connections);
            let outputs_layout = build_output_layout(
                node,
                signature,
                referenced_outputs.get(idx).unwrap_or(&HashSet::new()),
            );
            layouts.push(NodeLayout {
                inputs: inputs_layout,
                outputs: outputs_layout,
            });
        }

        let mut input_bindings = Vec::with_capacity(spec.nodes.len());
        for (idx, node) in spec.nodes.iter().enumerate() {
            let bindings = build_input_bindings(
                idx,
                inputs_map.get(&node.id).unwrap_or(&empty_connections),
                &node_index,
                &layouts,
            );
            input_bindings.push(bindings);
        }

        self.order = order;
        self.layouts = layouts;
        self.input_bindings = input_bindings;
        self.fingerprint = fingerprint;
        self.version = version;
        self.node_index = node_index
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        Ok(())
    }
}

fn build_input_layout(
    _node: &NodeSpec,
    signature: Option<&NodeSignature>,
    connections: &HashMap<String, InputConnection>,
) -> PortLayout {
    let mut layout = PortLayout::default();

    // Fixed inputs from the signature come first to keep indices stable.
    if let Some(sig) = signature {
        for port in &sig.inputs {
            layout.insert_slot(port.id.to_string());
        }
    }

    let variadic_id = signature.and_then(|sig| sig.variadic_inputs.as_ref().map(|v| v.id));
    let mut variadic_keys: Vec<(String, Option<usize>)> = Vec::new();

    for key in connections.keys() {
        if let Some(var_name) = variadic_id {
            let (prefix, _) = parse_variadic_key(key);
            if prefix == var_name {
                let (_, idx) = parse_variadic_key(key);
                variadic_keys.push((key.clone(), idx));
                continue;
            }
        }

        layout.insert_slot(key.clone());
    }

    if let Some(var_name) = variadic_id {
        variadic_keys.sort_by(|(a, _), (b, _)| compare_variadic_keys(a, b));
        let start = layout.slots.len();
        for (key, _) in &variadic_keys {
            layout.insert_slot(key.clone());
        }
        layout.variadics.insert(
            var_name.to_string(),
            VariadicRange {
                start,
                len: variadic_keys.len(),
            },
        );
    }

    layout
}

fn build_output_layout(
    node: &NodeSpec,
    signature: Option<&NodeSignature>,
    referenced: &HashSet<String>,
) -> PortLayout {
    let mut layout = PortLayout::default();

    if let Some(sig) = signature {
        for port in &sig.outputs {
            layout.insert_slot(port.id.to_string());
        }
    }

    // Respect any explicit output_shapes hints.
    for key in node.output_shapes.keys() {
        layout.insert_slot(key.clone());
    }

    if let Some(sig) = signature {
        if let Some(var_out) = &sig.variadic_outputs {
            let var_id = var_out.id;

            // Detect canonical {id}_{N} references via parse_variadic_key.
            // The frontend uses 0-indexed naming: elements_0, elements_1, ...
            let mut has_canonical = false;
            let mut max_canonical_idx: Option<usize> = None;
            for name in referenced.iter() {
                let (prefix, idx_opt) = parse_variadic_key(name);
                if prefix == var_id {
                    if let Some(idx) = idx_opt {
                        has_canonical = true;
                        max_canonical_idx =
                            Some(max_canonical_idx.map_or(idx, |prev: usize| prev.max(idx)));
                    }
                }
            }

            // Detect legacy concatenated {stem}{N} references (e.g. "part1" for id "parts").
            let stem = var_id.strip_suffix('s').unwrap_or(var_id);
            let mut max_legacy = 0usize;
            if stem != var_id {
                for name in referenced.iter() {
                    if let Some(tail) = name.strip_prefix(stem) {
                        if let Ok(idx) = tail.parse::<usize>() {
                            max_legacy = max_legacy.max(idx);
                        }
                    }
                }
            }

            // For canonical refs, count = max_index + 1 (0-indexed).
            // For legacy refs, count = max_index (1-indexed, so max IS the count).
            let canonical_count = max_canonical_idx.map(|m| m + 1).unwrap_or(0);
            let max_ref = std::cmp::max(canonical_count, max_legacy);

            // Node-type-specific param-based minimum count.
            let min_from_params = match node.kind {
                NodeType::Split => node.params.sizes.as_ref().map(|v| v.len()).unwrap_or(0),
                NodeType::ReadRecord | NodeType::TaskRun => node
                    .params
                    .record_keys
                    .as_ref()
                    .map(|v| v.len())
                    .unwrap_or(0),
                NodeType::FromVector => node.params.sizes.as_ref().map(|v| v.len()).unwrap_or(0),
                _ => 0,
            };

            let count = std::cmp::max(1, std::cmp::max(min_from_params, max_ref));
            let start = layout.slots.len();

            if var_out.keyed || has_canonical {
                // Frontend convention: 0-indexed {id}_{N} (elements_0, elements_1, ...).
                // Also used for keyed variadics (e.g. ReadRecord fields_0, fields_1).
                for i in 0..count {
                    layout.insert_slot(format!("{}_{}", var_id, i));
                }
            } else {
                // Legacy concatenated convention: {stem}{N} (1-indexed).
                for i in 0..count {
                    layout.insert_slot(format!("{}{}", stem, i + 1));
                }
            }

            let range = VariadicRange { start, len: count };
            layout.variadics.insert(var_id.to_string(), range);
            // Also register under the stem for backward eval-function compat.
            if stem != var_id {
                layout.variadics.insert(stem.to_string(), range);
            }
        }
    }

    // Add any referenced outputs we haven't seen yet to keep slot lookups stable.
    for name in referenced {
        layout.insert_slot(name.clone());
    }

    layout
}

fn build_input_bindings(
    node_idx: usize,
    connections: &HashMap<String, InputConnection>,
    node_index: &HashMap<&str, usize>,
    layouts: &[NodeLayout],
) -> Vec<InputBinding> {
    let mut bindings = Vec::with_capacity(layouts[node_idx].inputs.slots.len());
    let input_layout = &layouts[node_idx].inputs;

    for name in input_layout.slots.iter() {
        if let Some(conn) = connections.get(name) {
            let default = connection_default_port(conn);
            let source = if let Some(src_id) = conn.node_id.as_ref() {
                if let Some(&idx) = node_index.get(src_id.as_str()) {
                    let slot = layouts[idx].outputs.slot(&conn.output_key);
                    slot.map(|slot| ResolvedInputSource {
                        node_idx: idx,
                        slot,
                        output_name: conn.output_key.clone(),
                        selector: conn.selector.clone(),
                    })
                } else {
                    None
                }
            } else {
                None
            };
            bindings.push(InputBinding { source, default });
        } else {
            bindings.push(InputBinding {
                source: None,
                default: None,
            });
        }
    }

    bindings
}

fn connection_default_port(conn: &InputConnection) -> Option<PortValue> {
    conn.default_value.as_ref().map(|value| {
        if let Some(shape) = &conn.default_shape {
            PortValue::with_shape(value.clone(), shape.clone())
        } else {
            PortValue::new(value.clone())
        }
    })
}

fn gather_referenced_outputs(
    spec: &GraphSpec,
    node_index: &HashMap<&str, usize>,
) -> Vec<HashSet<String>> {
    let mut referenced = vec![HashSet::new(); spec.nodes.len()];
    for edge in &spec.edges {
        if let Some(&idx) = node_index.get(edge.from.node_id.as_str()) {
            referenced[idx].insert(edge.from.output.clone());
        }
    }
    referenced
}

fn signature_map() -> HashMap<NodeType, NodeSignature> {
    registry()
        .nodes
        .into_iter()
        .map(|sig| (sig.type_id.clone(), sig))
        .collect()
}

pub fn fingerprint_spec(spec: &GraphSpec) -> u64 {
    let mut hasher = DefaultHasher::new();
    hasher.write_usize(spec.nodes.len());
    for node in &spec.nodes {
        hasher.write(node.id.as_bytes());
        hasher.write_usize(node.input_defaults.len());
        // Sort defaults to ensure deterministic hashing.
        let mut defaults: Vec<(&String, &InputDefault)> = node.input_defaults.iter().collect();
        defaults.sort_by(|a, b| a.0.cmp(b.0));
        for (k, default) in defaults {
            hasher.write(k.as_bytes());
            // Include default value and shape so plan cache updates when defaults change.
            if let Ok(json) = serde_json::to_string(&default.value) {
                hasher.write(json.as_bytes());
            }
            if let Some(shape) = &default.shape {
                if let Ok(json) = serde_json::to_string(shape) {
                    hasher.write(json.as_bytes());
                }
            }
        }
        // Node kind impacts layout; include serialized enum to stay forward-compatible with tags.
        if let Ok(kind_json) = serde_json::to_string(&node.kind) {
            hasher.write(kind_json.as_bytes());
        }

        // Include params that alter layout (currently Split sizes / index can adjust output slots).
        if let Some(index) = node.params.index {
            hasher.write_u64(index.to_bits() as u64);
        }
        if let Some(sizes) = &node.params.sizes {
            hasher.write_usize(sizes.len());
            for size in sizes {
                hasher.write_u64(size.to_bits() as u64);
            }
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

    // Include connection default values/shapes so plan cache rebuilds when they change.
    if let Ok(connections) = spec.input_connections() {
        let mut entries: Vec<(String, String, InputConnection)> = Vec::new();
        for (node_id, inputs) in connections {
            for (input_key, conn) in inputs {
                entries.push((node_id.clone(), input_key.clone(), conn));
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, _, conn) in entries {
            if let Some(value) = &conn.default_value {
                if let Ok(json) = serde_json::to_string(value) {
                    hasher.write(json.as_bytes());
                }
            }
            if let Some(shape) = &conn.default_shape {
                if let Ok(json) = serde_json::to_string(shape) {
                    hasher.write(json.as_bytes());
                }
            }
        }
    }

    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, NodeParams, NodeSpec};
    use vizij_api_core::value::float;

    fn constant(id: &str) -> NodeSpec {
        NodeSpec {
            id: id.into(),
            kind: NodeType::Constant,
            params: NodeParams {
                value: Some(float(1.0)),
                ..Default::default()
            },
            output_shapes: Default::default(),
            input_defaults: Default::default(),
        }
    }

    fn add(id: &str) -> NodeSpec {
        NodeSpec {
            kind: NodeType::Add,
            params: NodeParams::default(),
            ..constant(id)
        }
    }

    fn edge(from: &str, to: &str, input: &str) -> EdgeSpec {
        EdgeSpec {
            from: EdgeOutputEndpoint {
                node_id: from.into(),
                output: "out".into(),
            },
            to: EdgeInputEndpoint {
                node_id: to.into(),
                input: input.into(),
            },
            selector: None,
        }
    }

    /// A component of `prefix`: two constants summed.
    fn component(prefix: &str) -> (Vec<NodeSpec>, Vec<EdgeSpec>) {
        let (a, b, sum) = (
            format!("{prefix}/a"),
            format!("{prefix}/b"),
            format!("{prefix}/sum"),
        );
        (
            vec![constant(&a), constant(&b), add(&sum)],
            vec![edge(&a, &sum, "operand_0"), edge(&b, &sum, "operand_1")],
        )
    }

    fn spec_of(parts: &[(Vec<NodeSpec>, Vec<EdgeSpec>)]) -> GraphSpec {
        GraphSpec {
            nodes: parts.iter().flat_map(|(n, _)| n.clone()).collect(),
            edges: parts.iter().flat_map(|(_, e)| e.clone()).collect(),
            ..GraphSpec::default()
        }
        .with_cache()
    }

    /// Everything a plan says, in comparable form.
    fn shape(plan: &PlanCache) -> Vec<String> {
        let mut index: Vec<String> = plan
            .node_index
            .iter()
            .map(|(id, idx)| format!("index {id}={idx}"))
            .collect();
        index.sort();
        let mut out = vec![format!("order {:?}", plan.order)];
        out.extend(index);
        for (idx, layout) in plan.layouts.iter().enumerate() {
            out.push(format!(
                "layout {idx}: in {:?} out {:?}",
                layout.inputs.slots, layout.outputs.slots
            ));
        }
        for (idx, bindings) in plan.input_bindings.iter().enumerate() {
            for binding in bindings {
                out.push(format!(
                    "binding {idx}: {:?} default {}",
                    binding
                        .source
                        .as_ref()
                        .map(|s| (s.node_idx, s.slot, s.output_name.clone())),
                    binding.default.is_some()
                ));
            }
        }
        out
    }

    fn rebuilt(spec: &GraphSpec) -> PlanCache {
        let mut plan = PlanCache::default();
        plan.ensure_versioned(spec).expect("the spec plans");
        plan
    }

    /// Appending a component gives the plan a rebuild gives, and serves the
    /// spec's version without rebuilding.
    #[test]
    fn appending_a_component_matches_a_rebuild() {
        let base = spec_of(&[component("base"), component("more")]);
        let mut plan = rebuilt(&base);

        let mut grown = spec_of(&[component("base"), component("more"), component("run")]);
        grown.version = base.version + 1;
        plan.append_component(&grown, base.nodes.len())
            .expect("the run is a component");
        assert_eq!(shape(&plan), shape(&rebuilt(&grown)));
        assert!(plan.ensure_versioned(&grown).is_ok());
        assert_eq!(plan.version, grown.version);
    }

    /// Removing a component from the middle gives the plan a rebuild of what
    /// remains gives.
    #[test]
    fn removing_a_component_matches_a_rebuild() {
        let full = spec_of(&[component("base"), component("run1"), component("run2")]);
        let mut plan = rebuilt(&full);

        let mut kept = spec_of(&[component("base"), component("run2")]);
        kept.version = full.version + 1;
        plan.remove_nodes(&kept, &[3, 4, 5])
            .expect("run1 is read by nothing");
        assert_eq!(shape(&plan), shape(&rebuilt(&kept)));
        assert_eq!(plan.version, kept.version);
    }

    /// Anything that is not a separate component is refused, the plan
    /// untouched: the caller rebuilds.
    #[test]
    fn a_crossing_edge_is_refused_and_the_plan_kept() {
        let base = spec_of(&[component("base")]);
        let mut plan = rebuilt(&base);
        let before = shape(&plan);

        let (mut nodes, mut edges) = component("run");
        nodes.push(add("run/reads_base"));
        edges.push(edge("base/sum", "run/reads_base", "operand_0"));
        let crossing = spec_of(&[component("base"), (nodes, edges)]);
        assert!(plan.append_component(&crossing, base.nodes.len()).is_err());
        assert_eq!(shape(&plan), before);

        // A remaining node reading a removed one blocks the removal.
        let full = spec_of(&[component("base")]);
        let mut plan = rebuilt(&full);
        let kept = GraphSpec {
            nodes: vec![full.nodes[2].clone()],
            ..GraphSpec::default()
        };
        assert!(plan.remove_nodes(&kept, &[0, 1]).is_err());
        assert_eq!(shape(&plan), shape(&rebuilt(&full)));
    }
}
