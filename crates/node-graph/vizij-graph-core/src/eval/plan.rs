use crate::schema::{registry, NodeSignature};
use crate::types::{
    GraphSpec, InputConnection, InputDefault, NodeSpec, NodeType, Selector, SelectorSegment,
};
use hashbrown::{HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

use super::value_layout::PortValue;
use super::variadic::{compare_variadic_keys, parse_variadic_key};

/// Continuous span inside a variadic group.
#[derive(Clone, Copy, Debug, Default)]
pub struct VariadicRange {
    /// Starting slot index for the group.
    pub start: usize,
    /// Number of slots in the group.
    pub len: usize,
}

/// Input/output slot layout for a node.
#[derive(Clone, Debug, Default)]
pub struct PortLayout {
    /// Slot names in stable order.
    pub slots: Vec<String>,
    name_to_slot: HashMap<String, usize>,
    variadics: HashMap<String, VariadicRange>,
}

impl PortLayout {
    /// Lookup a slot index by name.
    pub fn slot(&self, name: &str) -> Option<usize> {
        self.name_to_slot.get(name).copied()
    }

    /// Lookup a slot name by index.
    pub fn slot_name(&self, slot: usize) -> Option<&str> {
        self.slots.get(slot).map(String::as_str)
    }

    /// Lookup a variadic group range.
    pub fn variadic_range(&self, group: &str) -> Option<VariadicRange> {
        self.variadics.get(group).copied()
    }

    fn insert_slot(&mut self, name: String) {
        if !self.name_to_slot.contains_key(&name) {
            let slot = self.slots.len();
            self.slots.push(name.clone());
            self.name_to_slot.insert(name, slot);
        }
    }
}

/// Input/output layouts for a node.
#[derive(Clone, Debug, Default)]
pub struct NodeLayout {
    /// Input layout for the node.
    pub inputs: PortLayout,
    /// Output layout for the node.
    pub outputs: PortLayout,
}

/// Resolved input source for a bound port.
#[derive(Clone, Debug)]
pub struct ResolvedInputSource {
    /// Index of the source node in the spec.
    pub node_idx: usize,
    /// Slot index within the source node's outputs.
    pub slot: usize,
    /// Output port name on the source node.
    pub output_name: String,
    /// Optional selector applied to the source output.
    pub selector: Option<Selector>,
}

/// Resolved input binding with optional defaults.
#[derive(Clone, Debug)]
pub struct InputBinding {
    /// Optional resolved source input.
    pub source: Option<ResolvedInputSource>,
    /// Optional default value when no source is connected.
    pub default: Option<PortValue>,
}

/// Cached, topology-ready view of a [`GraphSpec`] for reuse across frames.
///
/// The cache stores stable port layouts and input bindings so graph evaluation can avoid
/// rebuilding topology on every frame.
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
            // Currently Split is the only variadic-output node.
            if matches!(node.kind, NodeType::Split) {
                let sizes_len = node.params.sizes.as_ref().map(|v| v.len()).unwrap_or(0);
                let mut max_ref = 0usize;
                for name in referenced.iter() {
                    if let Some(stripped) = name.strip_prefix("part") {
                        if let Ok(idx) = stripped.parse::<usize>() {
                            max_ref = max_ref.max(idx);
                        }
                    }
                }
                let count = std::cmp::max(1, std::cmp::max(sizes_len, max_ref));
                let start = layout.slots.len();
                for i in 0..count {
                    layout.insert_slot(format!("part{}", i + 1));
                }
                let range = VariadicRange { start, len: count };
                layout.variadics.insert("part".to_string(), range);
                layout.variadics.insert(var_out.id.to_string(), range);
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

/// Compute a structural fingerprint for plan cache invalidation.
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
