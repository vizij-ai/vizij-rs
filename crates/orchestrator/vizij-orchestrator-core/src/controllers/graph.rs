use anyhow::{anyhow, Result};
use indexmap::IndexSet;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

use vizij_api_core::{TypedPath, Value, WriteBatch};
use vizij_graph_core::eval::{evaluate_all, GraphRuntime};
use vizij_graph_core::types::{GraphSpec, NodeType, Selector};

use crate::blackboard::Blackboard;

/// Subscriptions specify which blackboard paths a graph consumes/produces.
/// Only subscribed input paths will be staged into the GraphRuntime to reduce
/// unnecessary work and keep evaluation deterministic.
#[derive(Debug, Clone)]
pub struct Subscriptions {
    pub inputs: Vec<TypedPath>,
    pub outputs: Vec<TypedPath>,
    /// Mirror the full controller write batch into the blackboard even when `outputs`
    /// restrict which paths are surfaced to consumers.
    ///
    /// When enabled, the orchestrator still returns only the filtered writes in
    /// `merged_writes`, but the blackboard receives every write produced by the graph so
    /// downstream passes have access to the internal state.
    pub mirror_writes: bool,
}

impl Default for Subscriptions {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            mirror_writes: true,
        }
    }
}

/// Lightweight config for registering a graph with the orchestrator.
#[derive(Debug, Clone)]
pub struct GraphControllerConfig {
    pub id: String,
    pub spec: GraphSpec,
    /// Optional subscriptions to restrict staging/publishing.
    pub subs: Subscriptions,
}

#[derive(Debug, Error)]
pub enum GraphMergeError {
    #[error("no graphs provided for merge")]
    Empty,
    #[error("output path '{path}' is produced by multiple graphs: {graphs:?}")]
    ConflictingOutputs { path: String, graphs: Vec<String> },
    #[error("output node '{node_id}' in graph '{graph}' is missing an upstream connection")]
    OutputMissingUpstream { node_id: String, graph: String },
    #[error("namespacing intermediate outputs for path '{path}' is not supported")]
    NamespaceIntermediateUnsupported { path: String },
    #[error("merged output node '{node_id}' is missing a TypedPath")]
    OutputPathUnavailable { node_id: String },
    #[error("failed to construct namespaced path '{path}': {reason}")]
    InvalidNamespacedPath { path: String, reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputConflictStrategy {
    Error,
    Namespace,
    BlendEqualWeights,
}

#[derive(Debug, Clone, Copy)]
pub struct GraphMergeOptions {
    pub output_conflicts: OutputConflictStrategy,
    pub intermediate_conflicts: OutputConflictStrategy,
}

impl Default for GraphMergeOptions {
    fn default() -> Self {
        Self {
            output_conflicts: OutputConflictStrategy::Error,
            intermediate_conflicts: OutputConflictStrategy::Error,
        }
    }
}

impl GraphControllerConfig {
    /// Merge multiple graph controller configs into a single config with combined graph/spec state.
    ///
    /// Nodes are namespaced to avoid id conflicts. Outputs with matching paths must be unique, or
    /// the merge returns [`GraphMergeError::ConflictingOutputs`]. Inputs that previously sourced
    /// their values from another graph via the blackboard are rewired to the upstream node so the
    /// combined graph can execute in a single pass.
    pub fn merged(
        id: impl Into<String>,
        graphs: Vec<GraphControllerConfig>,
    ) -> Result<Self, GraphMergeError> {
        Self::merged_with_options(id, graphs, GraphMergeOptions::default())
    }

    pub fn merged_with_options(
        id: impl Into<String>,
        graphs: Vec<GraphControllerConfig>,
        options: GraphMergeOptions,
    ) -> Result<Self, GraphMergeError> {
        let merged_id = id.into();
        if graphs.is_empty() {
            return Err(GraphMergeError::Empty);
        }

        #[derive(Clone)]
        struct InputNodeInfo {
            node_id: String,
            path: Option<TypedPath>,
        }

        #[derive(Clone)]
        struct OutputNodeInfo {
            node_id: String,
            path: Option<TypedPath>,
            graph_label: String,
        }

        #[derive(Clone)]
        struct OutputBinding {
            source_node_id: String,
            source_port: String,
            selector: Option<Selector>,
            graph_label: String,
        }

        let mut merged_nodes = Vec::new();
        let mut merged_links = Vec::new();
        let mut existing_ids: HashSet<String> = HashSet::new();
        let mut input_nodes: Vec<InputNodeInfo> = Vec::new();
        let mut output_nodes: Vec<OutputNodeInfo> = Vec::new();

        let mut merged_input_paths: IndexSet<TypedPath> = IndexSet::new();
        let mut merged_output_paths: IndexSet<TypedPath> = IndexSet::new();
        let mut mirror_writes = false;

        for (index, cfg) in graphs.into_iter().enumerate() {
            let GraphControllerConfig { id, spec, subs } = cfg;
            mirror_writes |= subs.mirror_writes;
            for tp in subs.inputs {
                merged_input_paths.insert(tp);
            }
            for tp in subs.outputs {
                merged_output_paths.insert(tp);
            }

            let namespace = make_namespace(index, &id);
            let mut id_map: HashMap<String, String> = HashMap::new();

            for mut node in spec.nodes {
                let original_id = node.id.clone();
                let mut candidate = format!("{}::{}", namespace, original_id);
                let mut counter = 1;
                while existing_ids.contains(&candidate) {
                    candidate = format!("{}::{}__{}", namespace, original_id, counter);
                    counter += 1;
                }
                existing_ids.insert(candidate.clone());
                id_map.insert(original_id, candidate.clone());
                node.id = candidate.clone();

                match node.kind {
                    NodeType::Input => {
                        let path = node.params.path.clone();
                        input_nodes.push(InputNodeInfo {
                            node_id: candidate.clone(),
                            path,
                        });
                    }
                    NodeType::Output => {
                        let path = node.params.path.clone();
                        output_nodes.push(OutputNodeInfo {
                            node_id: candidate.clone(),
                            path,
                            graph_label: id.clone(),
                        });
                    }
                    _ => {}
                }

                merged_nodes.push(node);
            }

            for mut link in spec.links {
                if let Some(new_id) = id_map.get(&link.from.node_id) {
                    link.from.node_id = new_id.clone();
                }
                if let Some(new_id) = id_map.get(&link.to.node_id) {
                    link.to.node_id = new_id.clone();
                }
                merged_links.push(link);
            }
        }

        // Build lookup maps for outputs and inputs.
        let output_lookup: HashMap<String, OutputNodeInfo> = output_nodes
            .iter()
            .map(|info| {
                (
                    info.node_id.clone(),
                    OutputNodeInfo {
                        node_id: info.node_id.clone(),
                        path: info.path.clone(),
                        graph_label: info.graph_label.clone(),
                    },
                )
            })
            .collect();

        let mut input_nodes_by_path: HashMap<String, Vec<InputNodeInfo>> = HashMap::new();
        for info in &input_nodes {
            if let Some(path) = info.path.as_ref() {
                input_nodes_by_path
                    .entry(path.to_string())
                    .or_default()
                    .push(InputNodeInfo {
                        node_id: info.node_id.clone(),
                        path: info.path.clone(),
                    });
            }
        }

        let mut output_nodes_by_path: HashMap<String, Vec<OutputNodeInfo>> = HashMap::new();
        for info in &output_nodes {
            if let Some(path) = info.path.as_ref() {
                output_nodes_by_path
                    .entry(path.to_string())
                    .or_default()
                    .push(OutputNodeInfo {
                        node_id: info.node_id.clone(),
                        path: info.path.clone(),
                        graph_label: info.graph_label.clone(),
                    });
            }
        }

        let mut bindings_by_path: HashMap<String, Vec<OutputBinding>> = HashMap::new();
        for link in &merged_links {
            if let Some(info) = output_lookup.get(&link.to.node_id) {
                if let Some(path) = info.path.as_ref() {
                    let binding = OutputBinding {
                        source_node_id: link.from.node_id.clone(),
                        source_port: link.from.output.clone(),
                        selector: link.selector.clone(),
                        graph_label: info.graph_label.clone(),
                    };
                    bindings_by_path
                        .entry(path.to_string())
                        .or_default()
                        .push(binding);
                }
            }
        }

        for info in &output_nodes {
            if let Some(path) = info.path.as_ref() {
                if !bindings_by_path.contains_key(&path.to_string()) {
                    return Err(GraphMergeError::OutputMissingUpstream {
                        node_id: info.node_id.clone(),
                        graph: info.graph_label.clone(),
                    });
                }
            }
        }

        let mut unique_bindings: HashMap<String, OutputBinding> = HashMap::new();
        let mut nodes_to_remove: HashSet<String> = HashSet::new();
        let mut inputs_to_remove: HashSet<String> = HashSet::new();
        let mut removed_input_paths: HashSet<String> = HashSet::new();

        for (path, bindings) in bindings_by_path.into_iter() {
            let consumers = input_nodes_by_path.remove(&path).unwrap_or_default();
            let outputs_for_path = output_nodes_by_path.get(&path).cloned().unwrap_or_default();

            if bindings.len() <= 1 {
                if let Some(binding) = bindings.into_iter().next() {
                    unique_bindings.insert(path.clone(), binding);
                }
                continue;
            }

            let has_consumers = !consumers.is_empty();
            let strategy = if has_consumers {
                options.intermediate_conflicts
            } else {
                options.output_conflicts
            };

            match strategy {
                OutputConflictStrategy::Error => {
                    let graphs: Vec<String> =
                        bindings.iter().map(|b| b.graph_label.clone()).collect();
                    return Err(GraphMergeError::ConflictingOutputs { path, graphs });
                }
                OutputConflictStrategy::Namespace => {
                    if has_consumers {
                        return Err(GraphMergeError::NamespaceIntermediateUnsupported { path });
                    }
                    for output in outputs_for_path {
                        let original_path = output.path.clone().ok_or_else(|| {
                            GraphMergeError::OutputPathUnavailable {
                                node_id: output.node_id.clone(),
                            }
                        })?;
                        let namespaced_string = format!("{}/{}", output.graph_label, original_path);
                        let namespaced_path =
                            TypedPath::parse(&namespaced_string).map_err(|e| {
                                GraphMergeError::InvalidNamespacedPath {
                                    path: namespaced_string.clone(),
                                    reason: e,
                                }
                            })?;
                        if let Some(node) = find_node_mut(&mut merged_nodes, &output.node_id) {
                            node.params.path = Some(namespaced_path.clone());
                        }
                        merged_output_paths.shift_remove(&original_path);
                        merged_output_paths.insert(namespaced_path);
                    }
                }
                OutputConflictStrategy::BlendEqualWeights => {
                    let blend_node_id = unique_node_id(
                        &format!("blend_{}", sanitize_identifier(&path)),
                        &mut existing_ids,
                    );
                    merged_nodes.push(vizij_graph_core::types::NodeSpec {
                        id: blend_node_id.clone(),
                        kind: NodeType::DefaultBlend,
                        params: vizij_graph_core::types::NodeParams::default(),
                        output_shapes: Default::default(),
                        input_defaults: Default::default(),
                    });

                    for (idx, binding) in bindings.iter().enumerate() {
                        merged_links.push(vizij_graph_core::types::LinkSpec {
                            from: vizij_graph_core::types::LinkOutputEndpoint {
                                node_id: binding.source_node_id.clone(),
                                output: binding.source_port.clone(),
                            },
                            to: vizij_graph_core::types::LinkInputEndpoint {
                                node_id: blend_node_id.clone(),
                                input: format!("target_{}", idx + 1),
                            },
                            selector: binding.selector.clone(),
                        });
                    }

                    let weights_node_id = unique_node_id(
                        &format!("blend_weights_{}", sanitize_identifier(&path)),
                        &mut existing_ids,
                    );
                    let weight = 1.0f32 / bindings.len() as f32;
                    merged_nodes.push(vizij_graph_core::types::NodeSpec {
                        id: weights_node_id.clone(),
                        kind: NodeType::Constant,
                        params: vizij_graph_core::types::NodeParams {
                            value: Some(Value::Vector(vec![weight; bindings.len()])),
                            ..Default::default()
                        },
                        output_shapes: Default::default(),
                        input_defaults: Default::default(),
                    });

                    merged_links.push(vizij_graph_core::types::LinkSpec {
                        from: vizij_graph_core::types::LinkOutputEndpoint {
                            node_id: weights_node_id.clone(),
                            output: "out".to_string(),
                        },
                        to: vizij_graph_core::types::LinkInputEndpoint {
                            node_id: blend_node_id.clone(),
                            input: "weights".to_string(),
                        },
                        selector: None,
                    });

                    if has_consumers {
                        for output in &outputs_for_path {
                            let original_path = output.path.clone().ok_or_else(|| {
                                GraphMergeError::OutputPathUnavailable {
                                    node_id: output.node_id.clone(),
                                }
                            })?;
                            let sanitized_prefix = sanitize_identifier(&output.graph_label);
                            let namespaced_string =
                                format!("{}/{}", sanitized_prefix, original_path);
                            let namespaced_path =
                                TypedPath::parse(&namespaced_string).map_err(|reason| {
                                    GraphMergeError::InvalidNamespacedPath {
                                        path: namespaced_string.clone(),
                                        reason,
                                    }
                                })?;
                            if let Some(node) = find_node_mut(&mut merged_nodes, &output.node_id) {
                                node.params.path = Some(namespaced_path.clone());
                            }
                            let _ = merged_output_paths.shift_remove(&original_path);
                            merged_output_paths.insert(namespaced_path);
                        }

                        let blend_output_path = format!("blend/{}", path);
                        let blend_output_typed =
                            TypedPath::parse(&blend_output_path).map_err(|reason| {
                                GraphMergeError::InvalidNamespacedPath {
                                    path: blend_output_path.clone(),
                                    reason,
                                }
                            })?;
                        let blend_output_node_id = unique_node_id(
                            &format!("blend_output_{}", sanitize_identifier(&path)),
                            &mut existing_ids,
                        );
                        merged_nodes.push(vizij_graph_core::types::NodeSpec {
                            id: blend_output_node_id.clone(),
                            kind: NodeType::Output,
                            params: vizij_graph_core::types::NodeParams {
                                path: Some(blend_output_typed.clone()),
                                ..Default::default()
                            },
                            output_shapes: Default::default(),
                            input_defaults: Default::default(),
                        });
                        merged_links.push(vizij_graph_core::types::LinkSpec {
                            from: vizij_graph_core::types::LinkOutputEndpoint {
                                node_id: blend_node_id.clone(),
                                output: "out".to_string(),
                            },
                            to: vizij_graph_core::types::LinkInputEndpoint {
                                node_id: blend_output_node_id.clone(),
                                input: "in".to_string(),
                            },
                            selector: None,
                        });
                        merged_output_paths.insert(blend_output_typed);

                        for consumer in consumers {
                            for link in merged_links.iter_mut() {
                                if link.from.node_id == consumer.node_id {
                                    link.from.node_id = blend_node_id.clone();
                                    link.from.output = "out".to_string();
                                }
                            }
                            inputs_to_remove.insert(consumer.node_id.clone());
                            if let Some(tp) = consumer.path {
                                removed_input_paths.insert(tp.to_string());
                            }
                        }
                    } else {
                        for output in &outputs_for_path {
                            nodes_to_remove.insert(output.node_id.clone());
                        }

                        let representative = outputs_for_path
                            .first()
                            .expect("outputs_for_path non-empty when blending final outputs");
                        let original_path = representative.path.clone().ok_or_else(|| {
                            GraphMergeError::OutputPathUnavailable {
                                node_id: representative.node_id.clone(),
                            }
                        })?;

                        let output_node_id = unique_node_id(
                            &format!("output_{}", sanitize_identifier(&path)),
                            &mut existing_ids,
                        );

                        merged_nodes.push(vizij_graph_core::types::NodeSpec {
                            id: output_node_id.clone(),
                            kind: NodeType::Output,
                            params: vizij_graph_core::types::NodeParams {
                                path: Some(original_path.clone()),
                                ..Default::default()
                            },
                            output_shapes: Default::default(),
                            input_defaults: Default::default(),
                        });

                        merged_links.push(vizij_graph_core::types::LinkSpec {
                            from: vizij_graph_core::types::LinkOutputEndpoint {
                                node_id: blend_node_id.clone(),
                                output: "out".to_string(),
                            },
                            to: vizij_graph_core::types::LinkInputEndpoint {
                                node_id: output_node_id.clone(),
                                input: "in".to_string(),
                            },
                            selector: None,
                        });
                    }
                }
            }
        }

        // Rewire any remaining single-source inputs.
        for input in &input_nodes {
            if inputs_to_remove.contains(&input.node_id) {
                continue;
            }
            if let Some(path) = input.path.as_ref() {
                if let Some(binding) = unique_bindings.get(&path.to_string()) {
                    for link in merged_links.iter_mut() {
                        if link.from.node_id == input.node_id {
                            link.from.node_id = binding.source_node_id.clone();
                            link.from.output = binding.source_port.clone();
                            if let Some(binding_selector) = binding.selector.as_ref() {
                                link.selector = match link.selector.take() {
                                    Some(existing) => {
                                        let mut composed = binding_selector.clone();
                                        composed.extend(existing);
                                        Some(composed)
                                    }
                                    None => Some(binding_selector.clone()),
                                };
                            }
                        }
                    }
                    inputs_to_remove.insert(input.node_id.clone());
                    removed_input_paths.insert(path.to_string());
                }
            }
        }

        // Drop nodes/links scheduled for removal.
        merged_nodes.retain(|node| {
            !nodes_to_remove.contains(&node.id) && !inputs_to_remove.contains(&node.id)
        });
        merged_links.retain(|link| {
            !nodes_to_remove.contains(&link.from.node_id)
                && !nodes_to_remove.contains(&link.to.node_id)
                && !inputs_to_remove.contains(&link.from.node_id)
                && !inputs_to_remove.contains(&link.to.node_id)
        });

        for node in &merged_nodes {
            if matches!(node.kind, NodeType::Input) {
                if let Some(path) = node.params.path.as_ref() {
                    merged_input_paths.insert(path.clone());
                }
            }
        }

        let inputs: Vec<TypedPath> = merged_input_paths
            .into_iter()
            .filter(|tp| !removed_input_paths.contains(&tp.to_string()))
            .collect();
        let outputs: Vec<TypedPath> = merged_output_paths.into_iter().collect();

        let merged_spec = GraphSpec {
            nodes: merged_nodes,
            links: merged_links,
        };

        Ok(GraphControllerConfig {
            id: merged_id,
            spec: merged_spec,
            subs: Subscriptions {
                inputs,
                outputs,
                mirror_writes,
            },
        })
    }
}

/// Controller owning a persistent GraphRuntime for evaluations.
#[derive(Debug)]
pub struct GraphController {
    pub id: String,
    pub spec: GraphSpec,
    pub rt: GraphRuntime,
    pub subs: Subscriptions,
}

impl GraphController {
    pub fn new(cfg: GraphControllerConfig) -> Self {
        Self {
            id: cfg.id,
            spec: cfg.spec,
            rt: GraphRuntime::default(),
            subs: cfg.subs,
        }
    }

    /// Evaluate the graph given the current blackboard state and epoch.
    ///
    /// Behavior:
    ///  - Advance the GraphRuntime epoch so newly staged inputs become visible.
    ///  - Stage subscribed Blackboard inputs into the runtime (only inputs listed in Subscriptions).
    ///  - Call evaluate_all(runtime, &spec)
    ///  - Collect runtime.writes and return as WriteBatch.
    pub fn evaluate(&mut self, bb: &mut Blackboard, _epoch: u64, dt: f32) -> Result<WriteBatch> {
        // Update runtime timekeeping so transition/time nodes observe advancing time.
        let delta = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let prev_t = if self.rt.t.is_finite() {
            self.rt.t
        } else {
            0.0
        };
        self.rt.dt = delta;
        self.rt.t = prev_t + delta;

        // Stage only subscribed blackboard entries into the graph runtime.
        for tp in &self.subs.inputs {
            if let Some(entry) = bb.get(&tp.to_string()) {
                let path = tp.clone();
                let value = entry.value.clone();
                let shape = entry.shape.clone();
                self.rt.set_input(path, value, shape);
            }
        }

        // Preserve any pre-populated writes (e.g., injected by tests or external tooling)
        let mut combined = WriteBatch::new();
        combined.append(std::mem::take(&mut self.rt.writes));

        // Call into graph evaluation
        evaluate_all(&mut self.rt, &self.spec).map_err(|e| anyhow!("evaluate_all error: {}", e))?;

        // Collect new writes produced during evaluation and append to combined batch.
        let new_writes: WriteBatch = std::mem::take(&mut self.rt.writes);
        combined.append(new_writes);

        Ok(combined)
    }
}

fn make_namespace(index: usize, id: &str) -> String {
    let mut sanitized = id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        sanitized = format!("graph_{index}");
    }
    format!("g{index}_{sanitized}")
}

fn find_node_mut<'a>(
    nodes: &'a mut [vizij_graph_core::types::NodeSpec],
    id: &str,
) -> Option<&'a mut vizij_graph_core::types::NodeSpec> {
    nodes.iter_mut().find(|node| node.id == id)
}

fn sanitize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn unique_node_id(base: &str, existing: &mut HashSet<String>) -> String {
    let mut candidate = base.to_string();
    let mut counter = 1usize;
    while existing.contains(&candidate) {
        candidate = format!("{}_{}", base, counter);
        counter += 1;
    }
    existing.insert(candidate.clone());
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;
    use vizij_api_core::Value;
    use vizij_graph_core::eval::{evaluate_all, GraphRuntime};
    use vizij_graph_core::types::SelectorSegment;

    fn cfg_from_json(id: &str, spec_json: serde_json::Value) -> GraphControllerConfig {
        let mut spec_json = spec_json;
        vizij_api_core::json::normalize_graph_spec_value(&mut spec_json);
        GraphControllerConfig {
            id: id.to_string(),
            spec: serde_json::from_value(spec_json).expect("graph spec json"),
            subs: Subscriptions::default(),
        }
    }

    #[test]
    fn merged_graph_errors_on_empty() {
        let err =
            GraphControllerConfig::merged("merged", Vec::new()).expect_err("merge should fail");
        assert!(matches!(err, GraphMergeError::Empty));
    }

    #[test]
    fn merged_graph_errors_when_output_missing_source() {
        let spec = json!({
            "nodes": [
                { "id": "orphan", "type": "output", "params": { "path": "shared/value" } }
            ],
            "links": []
        });
        let cfg = cfg_from_json("solo", spec);
        let err =
            GraphControllerConfig::merged("merged", vec![cfg]).expect_err("merge should fail");
        assert!(matches!(
            err,
            GraphMergeError::OutputMissingUpstream { node_id, .. }
            if node_id.contains("orphan")
        ));
    }

    #[test]
    fn merged_graph_namespaces_node_ids() {
        let producer = json!({
            "nodes": [
                { "id": "shared", "type": "constant", "params": { "value": { "type": "float", "data": 1.0 } } },
                { "id": "out", "type": "output", "params": { "path": "shared/value" } }
            ],
            "links": [
                { "from": { "node_id": "shared" }, "to": { "node_id": "out", "input": "in" } }
            ]
        });
        let consumer = json!({
            "nodes": [
                { "id": "shared", "type": "input", "params": { "path": "shared/value" } },
                { "id": "consumer", "type": "add" }
            ],
            "links": [
                { "from": { "node_id": "shared" }, "to": { "node_id": "consumer", "input": "lhs" } }
            ]
        });
        let mut producer_cfg = cfg_from_json("producer", producer);
        producer_cfg.subs.outputs = vec![TypedPath::parse("shared/value").expect("typed path")];

        let mut consumer_cfg = cfg_from_json("consumer", consumer);
        consumer_cfg.subs.inputs = vec![
            TypedPath::parse("shared/value").expect("typed path"),
            TypedPath::parse("external/drive").expect("typed path"),
        ];
        consumer_cfg.subs.outputs = vec![TypedPath::parse("result/value").expect("typed path")];

        let merged = GraphControllerConfig::merged("merged", vec![producer_cfg, consumer_cfg])
            .expect("merge ok");

        let mut ids = HashSet::new();
        for node in &merged.spec.nodes {
            assert!(ids.insert(node.id.clone()), "duplicate node id {}", node.id);
            assert!(
                node.id.starts_with("g0_producer") || node.id.starts_with("g1_consumer"),
                "unexpected namespace for {}",
                node.id
            );
        }
    }

    #[test]
    fn merged_graph_preserves_unmatched_inputs() {
        let producer = json!({
            "nodes": [
                { "id": "value", "type": "constant", "params": { "value": { "type": "float", "data": 1.0 } } },
                { "id": "out", "type": "output", "params": { "path": "shared/value" } }
            ],
            "links": [
                { "from": { "node_id": "value" }, "to": { "node_id": "out", "input": "in" } }
            ]
        });
        let consumer = json!({
            "nodes": [
                { "id": "shared_input", "type": "input", "params": { "path": "shared/value" } },
                { "id": "external_input", "type": "input", "params": { "path": "external/drive" } },
                { "id": "sum", "type": "add" },
                { "id": "publish", "type": "output", "params": { "path": "result/value" } }
            ],
            "links": [
                { "from": { "node_id": "shared_input" }, "to": { "node_id": "sum", "input": "lhs" } },
                { "from": { "node_id": "external_input" }, "to": { "node_id": "sum", "input": "rhs" } },
                { "from": { "node_id": "sum" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let mut producer_cfg = cfg_from_json("producer", producer);
        producer_cfg.subs.outputs = vec![TypedPath::parse("shared/value").expect("typed path")];

        let mut consumer_cfg = cfg_from_json("consumer", consumer);
        consumer_cfg.subs.inputs = vec![
            TypedPath::parse("shared/value").expect("typed path"),
            TypedPath::parse("external/drive").expect("typed path"),
        ];
        consumer_cfg.subs.outputs = vec![TypedPath::parse("result/value").expect("typed path")];

        let merged = GraphControllerConfig::merged("merged", vec![producer_cfg, consumer_cfg])
            .expect("merge ok");

        let external_node = merged
            .spec
            .nodes
            .iter()
            .find(|n| {
                n.params.path.as_ref().map(|p| p.to_string()) == Some("external/drive".to_string())
            })
            .expect("external input retained");
        assert!(
            external_node.id.contains("external_input"),
            "external input id preserved"
        );
        let input_paths: Vec<String> = merged.subs.inputs.iter().map(|tp| tp.to_string()).collect();
        assert!(
            input_paths.contains(&"external/drive".to_string()),
            "subscriptions retain external path"
        );
    }

    #[test]
    fn merged_graph_composes_selectors() {
        let producer = json!({
            "nodes": [
                { "id": "source", "type": "constant", "params": { "value": { "type": "vector", "data": [0, 1, 2, 3] } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "links": [
                {
                    "from": { "node_id": "source" },
                    "to": { "node_id": "publish", "input": "in" },
                    "selector": [ { "field": "vector" } ]
                }
            ]
        });
        let consumer = json!({
            "nodes": [
                { "id": "shared_input", "type": "input", "params": { "path": "shared/value" } },
                { "id": "extract", "type": "split" }
            ],
            "links": [
                {
                    "from": { "node_id": "shared_input" },
                    "to": { "node_id": "extract", "input": "in" },
                    "selector": [ { "index": 2 } ]
                }
            ]
        });

        let merged = GraphControllerConfig::merged(
            "merged",
            vec![
                cfg_from_json("producer", producer),
                cfg_from_json("consumer", consumer),
            ],
        )
        .expect("merge ok");

        let link = merged
            .spec
            .links
            .iter()
            .find(|l| l.to.node_id.ends_with("consumer::extract"))
            .expect("link to extract");
        assert!(
            link.from.node_id.ends_with("producer::source"),
            "rewired link should originate from producer source"
        );
        let selector = link.selector.as_ref().expect("composed selector");
        assert_eq!(
            selector,
            &vec![
                SelectorSegment::Field("vector".to_string()),
                SelectorSegment::Index(2)
            ],
            "selectors should compose in order"
        );
    }

    #[test]
    fn merged_graph_blends_final_outputs() {
        let producer_a = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 1.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "links": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let producer_b = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 3.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "links": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });

        let options = GraphMergeOptions {
            output_conflicts: OutputConflictStrategy::BlendEqualWeights,
            intermediate_conflicts: OutputConflictStrategy::Error,
        };

        let merged = GraphControllerConfig::merged_with_options(
            "merged",
            vec![
                cfg_from_json("producer_a", producer_a),
                cfg_from_json("producer_b", producer_b),
            ],
            options,
        )
        .expect("merge ok");

        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &merged.spec).expect("evaluation succeeds");

        let mut writes = rt.writes.iter();
        let write = writes.next().expect("write present");
        assert_eq!(write.path.to_string(), "shared/value");
        match &write.value {
            Value::Float(v) => assert!((*v - 2.0).abs() < 1e-6),
            other => panic!("expected float write, got {:?}", other),
        }
        assert!(writes.next().is_none(), "only one blended write expected");
    }

    #[test]
    fn merged_graph_blends_intermediate_outputs() {
        let producer_a = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 2.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "links": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let producer_b = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 4.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "links": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let consumer = json!({
            "nodes": [
                { "id": "shared_input", "type": "input", "params": { "path": "shared/value" } },
                { "id": "publish", "type": "output", "params": { "path": "result/value" } }
            ],
            "links": [
                { "from": { "node_id": "shared_input" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });

        let options = GraphMergeOptions {
            output_conflicts: OutputConflictStrategy::Error,
            intermediate_conflicts: OutputConflictStrategy::BlendEqualWeights,
        };

        let merged = GraphControllerConfig::merged_with_options(
            "merged",
            vec![
                cfg_from_json("producer_a", producer_a),
                cfg_from_json("producer_b", producer_b),
                cfg_from_json("consumer", consumer),
            ],
            options,
        )
        .expect("merge ok");

        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &merged.spec).expect("evaluation succeeds");
        let result_write = rt
            .writes
            .iter()
            .find(|w| w.path.to_string() == "result/value")
            .expect("blended consumer output present");
        match &result_write.value {
            Value::Float(v) => assert!((*v - 3.0).abs() < 1e-6),
            other => panic!("expected float write, got {:?}", other),
        }
    }

    #[test]
    fn merged_graph_namespaces_final_outputs() {
        let producer_a = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 1.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "links": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let producer_b = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 2.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "links": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });

        let options = GraphMergeOptions {
            output_conflicts: OutputConflictStrategy::Namespace,
            intermediate_conflicts: OutputConflictStrategy::Error,
        };

        let merged = GraphControllerConfig::merged_with_options(
            "merged",
            vec![
                cfg_from_json("producer_a", producer_a),
                cfg_from_json("producer_b", producer_b),
            ],
            options,
        )
        .expect("merge ok");

        let output_paths: Vec<String> = merged
            .spec
            .nodes
            .iter()
            .filter(|node| matches!(node.kind, NodeType::Output))
            .filter_map(|node| node.params.path.as_ref().map(|p| p.to_string()))
            .collect();

        assert!(
            output_paths.contains(&"producer_a/shared/value".to_string()),
            "namespaced path missing for producer_a"
        );
        assert!(
            output_paths.contains(&"producer_b/shared/value".to_string()),
            "namespaced path missing for producer_b"
        );
        assert!(
            !output_paths.contains(&"shared/value".to_string()),
            "original path should be replaced when namespacing"
        );
    }

    #[test]
    fn merged_graph_namespace_intermediate_errors() {
        let producer_a = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 1.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "links": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let producer_b = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 2.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "links": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let consumer = json!({
            "nodes": [
                { "id": "input", "type": "input", "params": { "path": "shared/value" } },
                { "id": "publish", "type": "output", "params": { "path": "result/value" } }
            ],
            "links": [
                { "from": { "node_id": "input" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });

        let options = GraphMergeOptions {
            output_conflicts: OutputConflictStrategy::Namespace,
            intermediate_conflicts: OutputConflictStrategy::Namespace,
        };

        let err = GraphControllerConfig::merged_with_options(
            "merged",
            vec![
                cfg_from_json("producer_a", producer_a),
                cfg_from_json("producer_b", producer_b),
                cfg_from_json("consumer", consumer),
            ],
            options,
        )
        .expect_err("namespace strategy should fail for intermediate paths");

        assert!(matches!(
            err,
            GraphMergeError::NamespaceIntermediateUnsupported { .. }
        ));
    }
}
