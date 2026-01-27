use anyhow::{anyhow, Result};
use indexmap::IndexSet;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

use vizij_api_core::{TypedPath, Value, WriteBatch};
use vizij_graph_core::eval::{evaluate_all, evaluate_all_cached, GraphRuntime};
use vizij_graph_core::types::{GraphSpec, NodeType, Selector};

use crate::blackboard::Blackboard;

/// Subscriptions specify which blackboard paths a graph consumes and produces.
///
/// Only subscribed input paths are staged into the `GraphRuntime` to reduce
/// unnecessary work and keep evaluation deterministic.
#[derive(Debug, Clone, Default)]
pub struct Subscriptions {
    /// Blackboard paths staged into the runtime before evaluation.
    pub inputs: Vec<TypedPath>,
    /// Blackboard paths published to consumers (empty means publish all).
    pub outputs: Vec<TypedPath>,
    /// Mirror the full controller write batch into the blackboard even when `outputs`
    /// restrict which paths are surfaced to consumers.
    ///
    /// When enabled, the orchestrator still returns only the filtered writes in
    /// `merged_writes`, but the blackboard receives every write produced by the graph so
    /// downstream passes have access to the internal state.
    pub mirror_writes: bool,
}

/// Configuration for registering a graph with the orchestrator.
///
/// Use [`GraphControllerConfig::merged`] or [`GraphControllerConfig::merged_with_options`]
/// to collapse multiple graph specs into one controller.
#[derive(Debug, Clone)]
pub struct GraphControllerConfig {
    /// Controller identifier (used in conflict logs and diagnostics).
    pub id: String,
    /// Graph definition to evaluate each step.
    pub spec: GraphSpec,
    /// Optional subscriptions to restrict staging/publishing.
    pub subs: Subscriptions,
}

/// Errors emitted while merging multiple graphs into a single spec.
///
/// Most merge errors indicate conflicting output paths or invalid node wiring.
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

/// Strategy for resolving conflicting output paths when merging graphs.
///
/// Strategies affect both direct output nodes and intermediate outputs when
/// multiple graphs feed the same downstream path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputConflictStrategy {
    /// Reject conflicting output paths.
    Error,
    /// Namespace conflicting outputs under their graph id.
    ///
    /// This strategy is only valid for final outputs; intermediate conflicts return an error.
    Namespace,
    /// Blend conflicting outputs with equal weights.
    ///
    /// The blend uses a constant weight vector with equal contribution per producer.
    BlendEqualWeights,
    /// Sum conflicting outputs with a variadic add node.
    ///
    /// This inserts a variadic add node and rewires downstream consumers.
    Add,
    /// Blend conflicting outputs with per-producer weight inputs.
    ///
    /// The merged graph inserts a default-blend node and exposes weight inputs at
    /// `blend_weights/<path>/<graph>` for host tuning.
    DefaultBlend,
}

/// Conflict resolution options used during graph merges.
#[derive(Debug, Clone, Copy)]
pub struct GraphMergeOptions {
    /// Strategy for conflicting final outputs.
    pub output_conflicts: OutputConflictStrategy,
    /// Strategy for conflicting intermediate outputs consumed by another graph.
    ///
    /// `Namespace` is only supported for final outputs; intermediate conflicts
    /// return [`GraphMergeError::NamespaceIntermediateUnsupported`].
    pub intermediate_conflicts: OutputConflictStrategy,
}

impl Default for GraphMergeOptions {
    /// Creates a new instance.
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
    ///
    /// The merged config inherits the union of subscription paths and sets `mirror_writes` if any
    /// input graph had it enabled.
    ///
    /// Node ids are namespaced using a deterministic `g{index}_{graph_id}` prefix; collisions
    /// append `__{n}`.
    ///
    /// # Errors
    /// Returns [`GraphMergeError`] when the merge cannot be completed.
    pub fn merged(
        id: impl Into<String>,
        graphs: Vec<GraphControllerConfig>,
    ) -> Result<Self, GraphMergeError> {
        Self::merged_with_options(id, graphs, GraphMergeOptions::default())
    }

    /// Merge multiple graph configs using explicit conflict resolution options.
    ///
    /// The merge uses graph ids to namespace node identifiers, avoiding collisions across specs.
    ///
    /// # Errors
    /// Returns [`GraphMergeError`] when inputs are missing, outputs conflict, or namespace
    /// generation fails.
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
            has_default_in: bool,
        }

        #[derive(Clone)]
        struct OutputBinding {
            source_node_id: String,
            source_port: String,
            selector: Option<Selector>,
            graph_label: String,
        }

        let mut merged_nodes = Vec::new();
        let mut merged_edges = Vec::new();
        let mut existing_ids: HashSet<String> = HashSet::new();
        let mut input_nodes: Vec<InputNodeInfo> = Vec::new();
        let mut output_nodes: Vec<OutputNodeInfo> = Vec::new();

        let mut merged_input_paths: IndexSet<TypedPath> = IndexSet::new();
        let mut merged_output_paths: IndexSet<TypedPath> = IndexSet::new();
        let mut mirror_writes = false;

        for (index, cfg) in graphs.into_iter().enumerate() {
            let GraphControllerConfig { id, spec, subs } = cfg;
            let spec = spec.with_cache();
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
                        let has_default_in = node.input_defaults.contains_key("in");
                        output_nodes.push(OutputNodeInfo {
                            node_id: candidate.clone(),
                            path,
                            graph_label: id.clone(),
                            has_default_in,
                        });
                    }
                    _ => {}
                }

                merged_nodes.push(node);
            }

            for mut edge in spec.edges {
                if let Some(new_id) = id_map.get(&edge.from.node_id) {
                    edge.from.node_id = new_id.clone();
                }
                if let Some(new_id) = id_map.get(&edge.to.node_id) {
                    edge.to.node_id = new_id.clone();
                }
                merged_edges.push(edge);
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
                        has_default_in: info.has_default_in,
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
                        has_default_in: info.has_default_in,
                    });
            }
        }

        let mut bindings_by_path: HashMap<String, Vec<OutputBinding>> = HashMap::new();
        for edge in &merged_edges {
            if let Some(info) = output_lookup.get(&edge.to.node_id) {
                if let Some(path) = info.path.as_ref() {
                    let binding = OutputBinding {
                        source_node_id: edge.from.node_id.clone(),
                        source_port: edge.from.output.clone(),
                        selector: edge.selector.clone(),
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
                if !bindings_by_path.contains_key(&path.to_string()) && !info.has_default_in {
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

        struct ResolutionContext<'a> {
            merged_nodes: &'a mut Vec<vizij_graph_core::types::NodeSpec>,
            merged_edges: &'a mut Vec<vizij_graph_core::types::EdgeSpec>,
            merged_output_paths: &'a mut IndexSet<TypedPath>,
            inputs_to_remove: &'a mut HashSet<String>,
            removed_input_paths: &'a mut HashSet<String>,
            nodes_to_remove: &'a mut HashSet<String>,
            existing_ids: &'a mut HashSet<String>,
        }

        /// Internal helper for `attach_conflict_resolution`.
        fn attach_conflict_resolution(
            path: &str,
            source_node_id: &str,
            source_output_port: &str,
            has_consumers: bool,
            outputs_for_path: &[OutputNodeInfo],
            consumers: Vec<InputNodeInfo>,
            ctx: &mut ResolutionContext<'_>,
        ) -> Result<(), GraphMergeError> {
            let source_node_id = source_node_id.to_string();
            let source_output_port = source_output_port.to_string();

            if has_consumers {
                for output in outputs_for_path {
                    let original_path = output.path.clone().ok_or_else(|| {
                        GraphMergeError::OutputPathUnavailable {
                            node_id: output.node_id.clone(),
                        }
                    })?;
                    let sanitized_prefix = sanitize_identifier(&output.graph_label);
                    let namespaced_string = format!("{}/{}", sanitized_prefix, original_path);
                    let namespaced_path =
                        TypedPath::parse(&namespaced_string).map_err(|reason| {
                            GraphMergeError::InvalidNamespacedPath {
                                path: namespaced_string.clone(),
                                reason,
                            }
                        })?;
                    if let Some(node) = find_node_mut(ctx.merged_nodes, &output.node_id) {
                        node.params.path = Some(namespaced_path.clone());
                    }
                    let _ = ctx.merged_output_paths.shift_remove(&original_path);
                    ctx.merged_output_paths.insert(namespaced_path);
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
                    &format!("blend_output_{}", sanitize_identifier(path)),
                    ctx.existing_ids,
                );
                ctx.merged_nodes.push(vizij_graph_core::types::NodeSpec {
                    id: blend_output_node_id.clone(),
                    kind: NodeType::Output,
                    params: vizij_graph_core::types::NodeParams {
                        path: Some(blend_output_typed.clone()),
                        ..Default::default()
                    },
                    output_shapes: Default::default(),
                    input_defaults: Default::default(),
                });
                ctx.merged_edges.push(vizij_graph_core::types::EdgeSpec {
                    from: vizij_graph_core::types::EdgeOutputEndpoint {
                        node_id: source_node_id.clone(),
                        output: source_output_port.clone(),
                    },
                    to: vizij_graph_core::types::EdgeInputEndpoint {
                        node_id: blend_output_node_id.clone(),
                        input: "in".to_string(),
                    },
                    selector: None,
                });
                ctx.merged_output_paths.insert(blend_output_typed);

                for consumer in consumers {
                    for edge in ctx.merged_edges.iter_mut() {
                        if edge.from.node_id == consumer.node_id {
                            edge.from.node_id = source_node_id.clone();
                            edge.from.output = source_output_port.clone();
                        }
                    }
                    ctx.inputs_to_remove.insert(consumer.node_id.clone());
                    if let Some(tp) = consumer.path {
                        ctx.removed_input_paths.insert(tp.to_string());
                    }
                }
            } else {
                for output in outputs_for_path {
                    ctx.nodes_to_remove.insert(output.node_id.clone());
                }

                let representative = outputs_for_path
                    .first()
                    .expect("outputs_for_path non-empty when resolving final outputs");
                let original_path = representative.path.clone().ok_or_else(|| {
                    GraphMergeError::OutputPathUnavailable {
                        node_id: representative.node_id.clone(),
                    }
                })?;

                let output_node_id = unique_node_id(
                    &format!("output_{}", sanitize_identifier(path)),
                    ctx.existing_ids,
                );

                ctx.merged_nodes.push(vizij_graph_core::types::NodeSpec {
                    id: output_node_id.clone(),
                    kind: NodeType::Output,
                    params: vizij_graph_core::types::NodeParams {
                        path: Some(original_path.clone()),
                        ..Default::default()
                    },
                    output_shapes: Default::default(),
                    input_defaults: Default::default(),
                });

                ctx.merged_edges.push(vizij_graph_core::types::EdgeSpec {
                    from: vizij_graph_core::types::EdgeOutputEndpoint {
                        node_id: source_node_id,
                        output: source_output_port,
                    },
                    to: vizij_graph_core::types::EdgeInputEndpoint {
                        node_id: output_node_id,
                        input: "in".to_string(),
                    },
                    selector: None,
                });
            }

            Ok(())
        }

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
                    let sanitized_path = sanitize_identifier(&path);
                    let blend_node_id =
                        unique_node_id(&format!("blend_{}", sanitized_path), &mut existing_ids);
                    merged_nodes.push(vizij_graph_core::types::NodeSpec {
                        id: blend_node_id.clone(),
                        kind: NodeType::DefaultBlend,
                        params: vizij_graph_core::types::NodeParams::default(),
                        output_shapes: Default::default(),
                        input_defaults: Default::default(),
                    });

                    for (idx, binding) in bindings.iter().enumerate() {
                        merged_edges.push(vizij_graph_core::types::EdgeSpec {
                            from: vizij_graph_core::types::EdgeOutputEndpoint {
                                node_id: binding.source_node_id.clone(),
                                output: binding.source_port.clone(),
                            },
                            to: vizij_graph_core::types::EdgeInputEndpoint {
                                node_id: blend_node_id.clone(),
                                input: format!("operand_{}", idx + 1),
                            },
                            selector: binding.selector.clone(),
                        });
                    }

                    let weights_node_id = unique_node_id(
                        &format!("blend_weights_{}", sanitized_path),
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

                    merged_edges.push(vizij_graph_core::types::EdgeSpec {
                        from: vizij_graph_core::types::EdgeOutputEndpoint {
                            node_id: weights_node_id,
                            output: "out".to_string(),
                        },
                        to: vizij_graph_core::types::EdgeInputEndpoint {
                            node_id: blend_node_id.clone(),
                            input: "weights".to_string(),
                        },
                        selector: None,
                    });

                    let mut ctx = ResolutionContext {
                        merged_nodes: &mut merged_nodes,
                        merged_edges: &mut merged_edges,
                        merged_output_paths: &mut merged_output_paths,
                        inputs_to_remove: &mut inputs_to_remove,
                        removed_input_paths: &mut removed_input_paths,
                        nodes_to_remove: &mut nodes_to_remove,
                        existing_ids: &mut existing_ids,
                    };
                    attach_conflict_resolution(
                        &path,
                        &blend_node_id,
                        "out",
                        has_consumers,
                        &outputs_for_path,
                        consumers,
                        &mut ctx,
                    )?;
                }
                OutputConflictStrategy::Add => {
                    let sanitized_path = sanitize_identifier(&path);
                    let add_node_id =
                        unique_node_id(&format!("sum_{}", sanitized_path), &mut existing_ids);
                    merged_nodes.push(vizij_graph_core::types::NodeSpec {
                        id: add_node_id.clone(),
                        kind: NodeType::Add,
                        params: vizij_graph_core::types::NodeParams::default(),
                        output_shapes: Default::default(),
                        input_defaults: Default::default(),
                    });

                    for (idx, binding) in bindings.iter().enumerate() {
                        merged_edges.push(vizij_graph_core::types::EdgeSpec {
                            from: vizij_graph_core::types::EdgeOutputEndpoint {
                                node_id: binding.source_node_id.clone(),
                                output: binding.source_port.clone(),
                            },
                            to: vizij_graph_core::types::EdgeInputEndpoint {
                                node_id: add_node_id.clone(),
                                input: format!("operand_{}", idx + 1),
                            },
                            selector: binding.selector.clone(),
                        });
                    }

                    let mut ctx = ResolutionContext {
                        merged_nodes: &mut merged_nodes,
                        merged_edges: &mut merged_edges,
                        merged_output_paths: &mut merged_output_paths,
                        inputs_to_remove: &mut inputs_to_remove,
                        removed_input_paths: &mut removed_input_paths,
                        nodes_to_remove: &mut nodes_to_remove,
                        existing_ids: &mut existing_ids,
                    };
                    attach_conflict_resolution(
                        &path,
                        &add_node_id,
                        "out",
                        has_consumers,
                        &outputs_for_path,
                        consumers,
                        &mut ctx,
                    )?;
                }
                OutputConflictStrategy::DefaultBlend => {
                    let sanitized_path = sanitize_identifier(&path);
                    let blend_node_id = unique_node_id(
                        &format!("blend_default_{}", sanitized_path),
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
                        merged_edges.push(vizij_graph_core::types::EdgeSpec {
                            from: vizij_graph_core::types::EdgeOutputEndpoint {
                                node_id: binding.source_node_id.clone(),
                                output: binding.source_port.clone(),
                            },
                            to: vizij_graph_core::types::EdgeInputEndpoint {
                                node_id: blend_node_id.clone(),
                                input: format!("operand_{}", idx + 1),
                            },
                            selector: binding.selector.clone(),
                        });
                    }

                    let join_node_id = unique_node_id(
                        &format!("blend_weight_join_{}", sanitized_path),
                        &mut existing_ids,
                    );
                    merged_nodes.push(vizij_graph_core::types::NodeSpec {
                        id: join_node_id.clone(),
                        kind: NodeType::Join,
                        params: vizij_graph_core::types::NodeParams::default(),
                        output_shapes: Default::default(),
                        input_defaults: Default::default(),
                    });

                    for (idx, binding) in bindings.iter().enumerate() {
                        let weight_input_id = unique_node_id(
                            &format!("blend_weight_input_{}_{}", sanitized_path, idx + 1),
                            &mut existing_ids,
                        );
                        let sanitized_label = sanitize_identifier(&binding.graph_label);
                        let weight_path_string =
                            format!("blend_weights/{}/{}", sanitized_path, sanitized_label);
                        let weight_path =
                            TypedPath::parse(&weight_path_string).map_err(|reason| {
                                GraphMergeError::InvalidNamespacedPath {
                                    path: weight_path_string.clone(),
                                    reason,
                                }
                            })?;
                        merged_nodes.push(vizij_graph_core::types::NodeSpec {
                            id: weight_input_id.clone(),
                            kind: NodeType::Input,
                            params: vizij_graph_core::types::NodeParams {
                                value: Some(Value::Float(1.0)),
                                path: Some(weight_path),
                                ..Default::default()
                            },
                            output_shapes: Default::default(),
                            input_defaults: Default::default(),
                        });
                        merged_edges.push(vizij_graph_core::types::EdgeSpec {
                            from: vizij_graph_core::types::EdgeOutputEndpoint {
                                node_id: weight_input_id,
                                output: "out".to_string(),
                            },
                            to: vizij_graph_core::types::EdgeInputEndpoint {
                                node_id: join_node_id.clone(),
                                input: format!("operand_{}", idx + 1),
                            },
                            selector: None,
                        });
                    }

                    merged_edges.push(vizij_graph_core::types::EdgeSpec {
                        from: vizij_graph_core::types::EdgeOutputEndpoint {
                            node_id: join_node_id,
                            output: "out".to_string(),
                        },
                        to: vizij_graph_core::types::EdgeInputEndpoint {
                            node_id: blend_node_id.clone(),
                            input: "weights".to_string(),
                        },
                        selector: None,
                    });

                    let mut ctx = ResolutionContext {
                        merged_nodes: &mut merged_nodes,
                        merged_edges: &mut merged_edges,
                        merged_output_paths: &mut merged_output_paths,
                        inputs_to_remove: &mut inputs_to_remove,
                        removed_input_paths: &mut removed_input_paths,
                        nodes_to_remove: &mut nodes_to_remove,
                        existing_ids: &mut existing_ids,
                    };
                    attach_conflict_resolution(
                        &path,
                        &blend_node_id,
                        "out",
                        has_consumers,
                        &outputs_for_path,
                        consumers,
                        &mut ctx,
                    )?;
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
                    for edge in merged_edges.iter_mut() {
                        if edge.from.node_id == input.node_id {
                            edge.from.node_id = binding.source_node_id.clone();
                            edge.from.output = binding.source_port.clone();
                            if let Some(binding_selector) = binding.selector.as_ref() {
                                edge.selector = match edge.selector.take() {
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

        // Drop nodes/edges scheduled for removal.
        merged_nodes.retain(|node| {
            !nodes_to_remove.contains(&node.id) && !inputs_to_remove.contains(&node.id)
        });
        merged_edges.retain(|edge| {
            !nodes_to_remove.contains(&edge.from.node_id)
                && !nodes_to_remove.contains(&edge.to.node_id)
                && !inputs_to_remove.contains(&edge.from.node_id)
                && !inputs_to_remove.contains(&edge.to.node_id)
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
            edges: merged_edges,
            ..Default::default()
        }
        .with_cache();

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

/// Controller owning a persistent `GraphRuntime` for evaluations.
#[derive(Debug)]
pub struct GraphController {
    /// Controller identifier.
    pub id: String,
    /// Cached graph spec (includes layout cache).
    pub spec: GraphSpec,
    /// Persistent runtime state (inputs, outputs, cached plan).
    pub rt: GraphRuntime,
    /// Subscription filter applied during evaluation.
    pub subs: Subscriptions,
    plan_ready: bool,
}

impl GraphController {
    /// Create a graph controller from a configuration.
    pub fn new(cfg: GraphControllerConfig) -> Self {
        Self {
            id: cfg.id,
            spec: cfg.spec,
            rt: GraphRuntime::default(),
            subs: cfg.subs,
            plan_ready: false,
        }
    }

    /// Replace the graph configuration and invalidate the cached plan.
    pub fn replace_config(&mut self, cfg: GraphControllerConfig) {
        // Structural edits require invalidating the cached plan. Always re-apply `with_cache()`
        // at the boundary so versioned plan caching cannot reuse stale layouts.
        self.spec = cfg.spec.with_cache();
        self.subs = cfg.subs;
        self.plan_ready = false;
    }

    /// Evaluate the graph using the current blackboard state.
    ///
    /// Behavior:
    /// - Advance `GraphRuntime.t`/`dt` so time-based nodes observe the new step.
    /// - Stage subscribed blackboard inputs into the runtime (only inputs listed in `Subscriptions`).
    /// - Call `evaluate_all` (or `evaluate_all_cached` when a plan is ready).
    /// - Collect runtime writes and return them as a `WriteBatch`.
    ///
    /// # Errors
    /// Returns an error if the graph evaluation fails.
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
            if let Some(entry) = bb.get_tp(tp) {
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
        let res = if self.plan_ready {
            evaluate_all_cached(&mut self.rt, &self.spec)
        } else {
            evaluate_all(&mut self.rt, &self.spec)
        };
        match res {
            Ok(_) => self.plan_ready = true,
            Err(e) => {
                self.plan_ready = false;
                return Err(anyhow!("evaluate_all error: {}", e));
            }
        }

        // Collect new writes produced during evaluation and append to combined batch.
        let new_writes: WriteBatch = std::mem::take(&mut self.rt.writes);
        combined.append(new_writes);

        Ok(combined)
    }
}

/// Internal helper for `make_namespace`.
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

/// Internal helper for `find_node_mut`.
fn find_node_mut<'a>(
    nodes: &'a mut [vizij_graph_core::types::NodeSpec],
    id: &str,
) -> Option<&'a mut vizij_graph_core::types::NodeSpec> {
    nodes.iter_mut().find(|node| node.id == id)
}

/// Internal helper for `sanitize_identifier`.
fn sanitize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

/// Internal helper for `unique_node_id`.
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

    /// Internal helper for `cfg_from_json`.
    fn cfg_from_json(id: &str, spec_json: serde_json::Value) -> GraphControllerConfig {
        let mut spec_json = spec_json;
        vizij_api_core::json::normalize_graph_spec_value(&mut spec_json)
            .expect("normalize graph spec");
        GraphControllerConfig {
            id: id.to_string(),
            spec: serde_json::from_value(spec_json).expect("graph spec json"),
            subs: Subscriptions::default(),
        }
    }

    #[test]
    /// Internal helper for `merged_graph_errors_on_empty`.
    fn merged_graph_errors_on_empty() {
        let err =
            GraphControllerConfig::merged("merged", Vec::new()).expect_err("merge should fail");
        assert!(matches!(err, GraphMergeError::Empty));
    }

    #[test]
    /// Internal helper for `merged_graph_errors_when_output_missing_source`.
    fn merged_graph_errors_when_output_missing_source() {
        let spec = json!({
            "nodes": [
                { "id": "orphan", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": []
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
    /// Internal helper for `merged_graph_allows_output_defaults_without_edges`.
    fn merged_graph_allows_output_defaults_without_edges() {
        let spec = json!({
            "nodes": [
                {
                    "id": "constant_out",
                    "type": "output",
                    "params": { "path": "shared/value" },
                    "input_defaults": {
                        "in": {
                            "value": { "type": "float", "data": 1.0 }
                        }
                    }
                }
            ],
            "edges": []
        });

        let cfg = cfg_from_json("solo", spec);
        let merged = GraphControllerConfig::merged("merged", vec![cfg]).expect("merge ok");

        let output_node = merged
            .spec
            .nodes
            .iter()
            .find(|node| matches!(node.kind, NodeType::Output))
            .expect("output node present");
        assert!(
            output_node.input_defaults.contains_key("in"),
            "output defaults should be preserved"
        );
    }

    #[test]
    /// Internal helper for `merged_graph_namespaces_node_ids`.
    fn merged_graph_namespaces_node_ids() {
        let producer = json!({
            "nodes": [
                { "id": "shared", "type": "constant", "params": { "value": { "type": "float", "data": 1.0 } } },
                { "id": "out", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
                { "from": { "node_id": "shared" }, "to": { "node_id": "out", "input": "in" } }
            ]
        });
        let consumer = json!({
            "nodes": [
                { "id": "shared", "type": "input", "params": { "path": "shared/value" } },
                { "id": "consumer", "type": "add" }
            ],
            "edges": [
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
    /// Internal helper for `merged_graph_preserves_unmatched_inputs`.
    fn merged_graph_preserves_unmatched_inputs() {
        let producer = json!({
            "nodes": [
                { "id": "value", "type": "constant", "params": { "value": { "type": "float", "data": 1.0 } } },
                { "id": "out", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
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
            "edges": [
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
    /// Internal helper for `merged_graph_add_strategy_sums_outputs`.
    fn merged_graph_add_strategy_sums_outputs() {
        let producer_a = json!({
            "nodes": [
                { "id": "a_constant", "type": "constant", "params": { "value": { "type": "float", "data": 1.0 } } },
                { "id": "a_out", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
                { "from": { "node_id": "a_constant" }, "to": { "node_id": "a_out", "input": "in" } }
            ]
        });
        let producer_b = json!({
            "nodes": [
                { "id": "b_constant", "type": "constant", "params": { "value": { "type": "float", "data": 2.0 } } },
                { "id": "b_out", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
                { "from": { "node_id": "b_constant" }, "to": { "node_id": "b_out", "input": "in" } }
            ]
        });

        let mut cfg_a = cfg_from_json("rig_a", producer_a);
        cfg_a.subs.outputs = vec![TypedPath::parse("shared/value").expect("typed path")];
        let mut cfg_b = cfg_from_json("rig_b", producer_b);
        cfg_b.subs.outputs = vec![TypedPath::parse("shared/value").expect("typed path")];

        let merged = GraphControllerConfig::merged_with_options(
            "merged",
            vec![cfg_a, cfg_b],
            GraphMergeOptions {
                output_conflicts: OutputConflictStrategy::Add,
                intermediate_conflicts: OutputConflictStrategy::Error,
            },
        )
        .expect("merge ok");

        let mut add_nodes = merged
            .spec
            .nodes
            .iter()
            .filter(|node| matches!(node.kind, NodeType::Add));
        let add_node = add_nodes
            .next()
            .expect("addition node inserted for conflicting outputs");
        assert!(
            add_nodes.next().is_none(),
            "only one additive node expected"
        );

        let output_nodes: Vec<&vizij_graph_core::types::NodeSpec> = merged
            .spec
            .nodes
            .iter()
            .filter(|node| matches!(node.kind, NodeType::Output))
            .collect();
        assert_eq!(
            output_nodes.len(),
            1,
            "final outputs collapsed to a single node"
        );
        let final_output = output_nodes[0];
        let final_path = final_output
            .params
            .path
            .as_ref()
            .expect("final output path present")
            .to_string();
        assert_eq!(
            final_path, "shared/value",
            "sum strategy retains the original output path"
        );

        let has_sum_edge = merged
            .spec
            .edges
            .iter()
            .any(|edge| edge.from.node_id == add_node.id && edge.to.node_id == final_output.id);
        assert!(
            has_sum_edge,
            "summed value should route into the final output node"
        );

        let operand_edges: Vec<_> = merged
            .spec
            .edges
            .iter()
            .filter(|edge| edge.to.node_id == add_node.id)
            .collect();
        assert_eq!(
            operand_edges.len(),
            2,
            "both sources should connect into the additive node"
        );
    }

    #[test]
    /// Internal helper for `merged_graph_default_blend_strategy_inserts_weight_inputs`.
    fn merged_graph_default_blend_strategy_inserts_weight_inputs() {
        let producer_a = json!({
            "nodes": [
                { "id": "a_constant", "type": "constant", "params": { "value": { "type": "float", "data": 1.0 } } },
                { "id": "a_out", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
                { "from": { "node_id": "a_constant" }, "to": { "node_id": "a_out", "input": "in" } }
            ]
        });
        let producer_b = json!({
            "nodes": [
                { "id": "b_constant", "type": "constant", "params": { "value": { "type": "float", "data": 3.0 } } },
                { "id": "b_out", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
                { "from": { "node_id": "b_constant" }, "to": { "node_id": "b_out", "input": "in" } }
            ]
        });
        let consumer = json!({
            "nodes": [
                { "id": "shared_in", "type": "input", "params": { "path": "shared/value" } },
                { "id": "forward", "type": "output", "params": { "path": "final/value" } }
            ],
            "edges": [
                { "from": { "node_id": "shared_in" }, "to": { "node_id": "forward", "input": "in" } }
            ]
        });

        let mut cfg_a = cfg_from_json("rig_a", producer_a);
        cfg_a.subs.outputs = vec![TypedPath::parse("shared/value").expect("typed path")];
        let mut cfg_b = cfg_from_json("rig_b", producer_b);
        cfg_b.subs.outputs = vec![TypedPath::parse("shared/value").expect("typed path")];
        let mut cfg_consumer = cfg_from_json("consumer", consumer);
        cfg_consumer.subs.inputs = vec![TypedPath::parse("shared/value").expect("typed path")];
        cfg_consumer.subs.outputs = vec![TypedPath::parse("final/value").expect("typed path")];

        let merged = GraphControllerConfig::merged_with_options(
            "merged",
            vec![cfg_a, cfg_b, cfg_consumer],
            GraphMergeOptions {
                output_conflicts: OutputConflictStrategy::Error,
                intermediate_conflicts: OutputConflictStrategy::DefaultBlend,
            },
        )
        .expect("merge ok");

        let blend_node = merged
            .spec
            .nodes
            .iter()
            .find(|node| matches!(node.kind, NodeType::DefaultBlend))
            .expect("default blend node injected for intermediate conflict");

        assert!(
            merged
                .spec
                .nodes
                .iter()
                .any(|node| matches!(node.kind, NodeType::Join)),
            "weight join node should be present"
        );

        let weight_inputs: Vec<&vizij_graph_core::types::NodeSpec> = merged
            .spec
            .nodes
            .iter()
            .filter(|node| {
                matches!(node.kind, NodeType::Input)
                    && node
                        .params
                        .path
                        .as_ref()
                        .map(|tp| tp.to_string().starts_with("blend_weights/"))
                        .unwrap_or(false)
            })
            .collect();
        assert_eq!(
            weight_inputs.len(),
            2,
            "weight control inputs should be created for each producer"
        );
        for input in &weight_inputs {
            assert_eq!(
                input.params.value,
                Some(Value::Float(1.0)),
                "weight inputs should default to 1.0"
            );
        }

        let subscription_paths: Vec<String> =
            merged.subs.inputs.iter().map(|tp| tp.to_string()).collect();
        for input in &weight_inputs {
            let path_str = input.params.path.as_ref().unwrap().to_string();
            assert!(
                subscription_paths.contains(&path_str),
                "subscriptions should include weight control path {}",
                path_str
            );
        }

        assert!(
            merged
                .spec
                .nodes
                .iter()
                .any(|node| matches!(node.kind, NodeType::Output)
                    && node
                        .params
                        .path
                        .as_ref()
                        .map(|tp| tp.to_string() == "blend/shared/value")
                        .unwrap_or(false)),
            "blend output should surface aggregated value for tooling"
        );

        let consumer_output = merged
            .spec
            .nodes
            .iter()
            .find(|node| {
                matches!(node.kind, NodeType::Output)
                    && node
                        .params
                        .path
                        .as_ref()
                        .map(|tp| tp.to_string() == "final/value")
                        .unwrap_or(false)
            })
            .expect("consumer output present");

        let rewired = merged.spec.edges.iter().any(|edge| {
            edge.from.node_id == blend_node.id && edge.to.node_id == consumer_output.id
        });
        assert!(
            rewired,
            "consumer should read from the default blend output"
        );
    }

    #[test]
    /// Internal helper for `merged_graph_composes_selectors`.
    fn merged_graph_composes_selectors() {
        let producer = json!({
            "nodes": [
                { "id": "source", "type": "constant", "params": { "value": { "type": "vector", "data": [0, 1, 2, 3] } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
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
            "edges": [
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

        let edge = merged
            .spec
            .edges
            .iter()
            .find(|edge| edge.to.node_id.ends_with("consumer::extract"))
            .expect("edge to extract");
        assert!(
            edge.from.node_id.ends_with("producer::source"),
            "rewired edge should originate from producer source"
        );
        let selector = edge.selector.as_ref().expect("composed selector");
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
    /// Internal helper for `merged_graph_blends_final_outputs`.
    fn merged_graph_blends_final_outputs() {
        let producer_a = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 1.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let producer_b = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 3.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
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
    /// Internal helper for `merged_graph_blends_intermediate_outputs`.
    fn merged_graph_blends_intermediate_outputs() {
        let producer_a = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 2.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let producer_b = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 4.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let consumer = json!({
            "nodes": [
                { "id": "shared_input", "type": "input", "params": { "path": "shared/value" } },
                { "id": "publish", "type": "output", "params": { "path": "result/value" } }
            ],
            "edges": [
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
    /// Internal helper for `merged_graph_namespaces_final_outputs`.
    fn merged_graph_namespaces_final_outputs() {
        let producer_a = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 1.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let producer_b = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 2.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
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
    /// Internal helper for `merged_graph_namespace_intermediate_errors`.
    fn merged_graph_namespace_intermediate_errors() {
        let producer_a = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 1.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let producer_b = json!({
            "nodes": [
                { "id": "const", "type": "constant", "params": { "value": { "float": 2.0 } } },
                { "id": "publish", "type": "output", "params": { "path": "shared/value" } }
            ],
            "edges": [
                { "from": { "node_id": "const" }, "to": { "node_id": "publish", "input": "in" } }
            ]
        });
        let consumer = json!({
            "nodes": [
                { "id": "input", "type": "input", "params": { "path": "shared/value" } },
                { "id": "publish", "type": "output", "params": { "path": "result/value" } }
            ],
            "edges": [
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
