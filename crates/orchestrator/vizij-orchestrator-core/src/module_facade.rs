//! JSON request/response facade for module-shaped orchestrator execution.
//!
//! This facade is intentionally host-neutral. Arora modules, wasm bindings, and browser
//! wrappers can all dispatch the same string payloads while the actual graph, animation,
//! blackboard, and scheduling semantics remain in the core orchestrator.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use vizij_api_core::{json as api_json, TypedPath, WriteBatch};
use vizij_graph_core::types::GraphSpec;

use crate::controllers::animation::AnimationControllerConfig;
use crate::controllers::graph::{
    GraphController, GraphControllerConfig, GraphMergeOptions, OutputConflictStrategy,
    Subscriptions,
};
use crate::{Orchestrator, OrchestratorFrame, Schedule};

/// Version of the module-facade JSON contract.
pub const MODULE_FACADE_VERSION: u32 = 1;

/// Stateful facade around a single Vizij orchestrator runtime.
#[derive(Debug, Default)]
pub struct VizijModuleFacade {
    runtime: Option<Orchestrator>,
    runtime_handle: Option<String>,
    runtime_counter: u64,
    graph_counter: u32,
    anim_counter: u32,
    output_version: u64,
    last_version: u64,
    last_writes: WriteBatch,
}

impl VizijModuleFacade {
    /// Construct an empty facade. Call `runtime.create` before registering controllers.
    pub fn new() -> Self {
        Self::default()
    }

    /// Dispatch one JSON request and return a JSON response string.
    ///
    /// This method never panics for malformed host input. Errors are encoded as
    /// `{ ok: false, error, version }` so module hosts can forward the response unchanged.
    pub fn dispatch_json(&mut self, request_json: &str) -> String {
        let request: FacadeRequest = match serde_json::from_str(request_json) {
            Ok(request) => request,
            Err(error) => {
                return FacadeResponse::error(None, format!("invalid facade request: {error}"))
                    .to_json_string()
            }
        };
        self.dispatch(request).to_json_string()
    }

    /// Dispatch a parsed facade request.
    pub fn dispatch(&mut self, request: FacadeRequest) -> FacadeResponse {
        let request_id = request.request_id.clone();
        match self.dispatch_inner(request) {
            Ok(result) => FacadeResponse::ok(request_id, result),
            Err(error) => FacadeResponse::error(request_id, error.to_string()),
        }
    }

    fn dispatch_inner(&mut self, request: FacadeRequest) -> Result<JsonValue> {
        match request.call.as_str() {
            "runtime.create" => self.create_runtime(request.args),
            "graph.normalize" | "graph.normalizeSpec" | "graph.normalize_spec" => {
                self.normalize_graph(request.args)
            }
            "runtime.dispose" => {
                self.validate_runtime_handle(request.runtime_handle.as_deref())?;
                self.dispose_runtime()
            }
            "controllers.list" | "runtime.controllers" => {
                self.validate_runtime_handle(request.runtime_handle.as_deref())?;
                self.list_controllers()
            }
            "graph.register" | "graph.load" => {
                self.validate_runtime_handle(request.runtime_handle.as_deref())?;
                self.register_graph(request.args)
            }
            "graph.replace" => {
                self.validate_runtime_handle(request.runtime_handle.as_deref())?;
                self.replace_graph(request.args)
            }
            "graph.merge" | "graph.registerMerged" | "graph.register_merged" => {
                self.validate_runtime_handle(request.runtime_handle.as_deref())?;
                self.register_merged_graph(request.args)
            }
            "graph.remove" => {
                self.validate_runtime_handle(request.runtime_handle.as_deref())?;
                self.remove_graph(request.args)
            }
            "animation.register" | "animation.load" => {
                self.validate_runtime_handle(request.runtime_handle.as_deref())?;
                self.register_animation(request.args)
            }
            "animation.remove" => {
                self.validate_runtime_handle(request.runtime_handle.as_deref())?;
                self.remove_animation(request.args)
            }
            "input.set" => {
                self.validate_runtime_handle(request.runtime_handle.as_deref())?;
                self.set_input(request.args)
            }
            "input.remove" => {
                self.validate_runtime_handle(request.runtime_handle.as_deref())?;
                self.remove_input(request.args)
            }
            "orchestrator.step" => {
                self.validate_runtime_handle(request.runtime_handle.as_deref())?;
                self.step(request.args)
            }
            "orchestrator.stepDelta" | "orchestrator.step_delta" => {
                self.validate_runtime_handle(request.runtime_handle.as_deref())?;
                self.step_delta(request.args)
            }
            other => Err(anyhow!("unknown facade call '{other}'")),
        }
    }

    fn create_runtime(&mut self, args: JsonValue) -> Result<JsonValue> {
        let args = parse_args::<RuntimeCreateArgs>(args)?;
        let schedule = parse_schedule(args.schedule.as_deref())?;
        let handle = args.runtime_handle.unwrap_or_else(|| {
            let handle = format!("runtime:{}", self.runtime_counter);
            self.runtime_counter = self.runtime_counter.wrapping_add(1);
            handle
        });

        self.runtime = Some(Orchestrator::new(schedule));
        self.runtime_handle = Some(handle.clone());
        self.graph_counter = 0;
        self.anim_counter = 0;
        self.output_version = 0;
        self.last_version = 0;
        self.last_writes = WriteBatch::default();

        Ok(json!({
            "runtimeHandle": handle,
            "schedule": schedule_name(schedule),
        }))
    }

    fn dispose_runtime(&mut self) -> Result<JsonValue> {
        let disposed = self.runtime.take().is_some();
        self.runtime_handle = None;
        self.output_version = 0;
        self.last_version = 0;
        self.last_writes = WriteBatch::default();
        Ok(json!({ "disposed": disposed }))
    }

    fn list_controllers(&mut self) -> Result<JsonValue> {
        let runtime = self.runtime_mut()?;
        let graphs: Vec<String> = runtime.graphs.keys().cloned().collect();
        let anims: Vec<String> = runtime.anims.keys().cloned().collect();
        Ok(json!({ "graphs": graphs, "anims": anims }))
    }

    fn register_graph(&mut self, args: JsonValue) -> Result<JsonValue> {
        let fallback_id = self.next_graph_id();
        let cfg =
            build_graph_controller_config(parse_args::<GraphRegistrationArgs>(args)?, fallback_id)?;
        let id = cfg.id.clone();
        let controller = GraphController::new(cfg);
        self.runtime_mut()?.graphs.insert(id.clone(), controller);
        self.reset_delta_baseline();
        Ok(json!({ "graphId": id }))
    }

    fn normalize_graph(&mut self, args: JsonValue) -> Result<JsonValue> {
        let mut spec = parse_args::<GraphNormalizeArgs>(args)?.spec;
        api_json::normalize_graph_spec_value(&mut spec)
            .map_err(|error| anyhow!("normalize graph spec error: {error}"))?;
        Ok(spec)
    }

    fn replace_graph(&mut self, args: JsonValue) -> Result<JsonValue> {
        let cfg = build_graph_controller_config(
            parse_args::<GraphRegistrationArgs>(args)?,
            self.next_graph_id(),
        )?;
        let id = cfg.id.clone();
        let runtime = self.runtime_mut()?;
        let controller = runtime
            .graphs
            .get_mut(&id)
            .ok_or_else(|| anyhow!("graph '{id}' is not registered"))?;
        controller.replace_config(cfg);
        self.reset_delta_baseline();
        Ok(json!({ "graphId": id, "replaced": true }))
    }

    fn register_merged_graph(&mut self, args: JsonValue) -> Result<JsonValue> {
        let args = parse_args::<MergedGraphRegistrationArgs>(args)?;
        if args.graphs.is_empty() {
            return Err(anyhow!("graph.merge requires at least one graph"));
        }
        let merged_id = args.id.unwrap_or_else(|| self.next_graph_id());
        let options = map_merge_options(args.strategy)?;
        let mut configs = Vec::with_capacity(args.graphs.len());
        for (idx, graph) in args.graphs.into_iter().enumerate() {
            let fallback_id = format!("{merged_id}::{idx}");
            configs.push(build_graph_controller_config(graph, fallback_id)?);
        }
        let merged_cfg =
            GraphControllerConfig::merged_with_options(merged_id.clone(), configs, options)?;
        self.runtime_mut()?
            .graphs
            .insert(merged_id.clone(), GraphController::new(merged_cfg));
        self.reset_delta_baseline();
        Ok(json!({ "graphId": merged_id }))
    }

    fn remove_graph(&mut self, args: JsonValue) -> Result<JsonValue> {
        let args = parse_args::<RemoveControllerArgs>(args)?;
        let removed = self.runtime_mut()?.graphs.shift_remove(&args.id).is_some();
        if removed {
            self.reset_delta_baseline();
        }
        Ok(json!({ "removed": removed }))
    }

    fn register_animation(&mut self, args: JsonValue) -> Result<JsonValue> {
        let args = parse_args::<AnimationRegistrationArgs>(args)?;
        let id = args.id.unwrap_or_else(|| self.next_anim_id());
        let cfg = AnimationControllerConfig {
            id: id.clone(),
            setup: args.setup.unwrap_or(JsonValue::Null),
        };
        let controller = crate::controllers::animation::AnimationController::try_new(cfg)?;
        self.runtime_mut()?.anims.insert(id.clone(), controller);
        self.reset_delta_baseline();
        Ok(json!({ "animationId": id }))
    }

    fn remove_animation(&mut self, args: JsonValue) -> Result<JsonValue> {
        let args = parse_args::<RemoveControllerArgs>(args)?;
        let removed = self.runtime_mut()?.anims.shift_remove(&args.id).is_some();
        if removed {
            self.reset_delta_baseline();
        }
        Ok(json!({ "removed": removed }))
    }

    fn set_input(&mut self, args: JsonValue) -> Result<JsonValue> {
        let args = parse_args::<SetInputArgs>(args)?;
        self.runtime_mut()?
            .set_input(&args.path, args.value, args.shape)?;
        Ok(json!({ "path": args.path }))
    }

    fn remove_input(&mut self, args: JsonValue) -> Result<JsonValue> {
        let args = parse_args::<RemoveInputArgs>(args)?;
        let removed = self.runtime_mut()?.blackboard.remove(&args.path).is_some();
        Ok(json!({ "removed": removed }))
    }

    fn step(&mut self, args: JsonValue) -> Result<JsonValue> {
        let args = parse_args::<StepArgs>(args)?;
        let frame = self.step_runtime(args.dt)?;
        serde_json::to_value(frame).map_err(Into::into)
    }

    fn step_delta(&mut self, args: JsonValue) -> Result<JsonValue> {
        let args = parse_args::<StepDeltaArgs>(args)?;
        let frame = self.step_runtime(args.dt)?;
        self.output_version = self.output_version.saturating_add(1);
        let version = self.output_version;
        let since = args.since_version.unwrap_or(0);

        let mut merged_writes = frame.merged_writes.clone();
        if since == self.last_version && frame.merged_writes == self.last_writes {
            merged_writes = WriteBatch::default();
        }

        self.last_version = version;
        self.last_writes = frame.merged_writes;

        Ok(json!({
            "version": version,
            "epoch": frame.epoch,
            "dt": frame.dt,
            "merged_writes": merged_writes,
            "conflicts": frame.conflicts,
            "events": frame.events,
            "timings_ms": frame.timings_ms,
        }))
    }

    fn step_runtime(&mut self, dt: f32) -> Result<OrchestratorFrame> {
        if !dt.is_finite() || dt < 0.0 {
            return Err(anyhow!("dt must be finite and non-negative"));
        }
        self.runtime_mut()?.step(dt)
    }

    fn runtime_mut(&mut self) -> Result<&mut Orchestrator> {
        self.runtime
            .as_mut()
            .ok_or_else(|| anyhow!("runtime is not created; call runtime.create first"))
    }

    fn validate_runtime_handle(&self, requested: Option<&str>) -> Result<()> {
        let Some(requested) = requested else {
            return Ok(());
        };
        let Some(current) = self.runtime_handle.as_deref() else {
            return Err(anyhow!("runtime is not created; call runtime.create first"));
        };
        if requested != current {
            return Err(anyhow!(
                "runtime handle mismatch: request targeted '{requested}' but active runtime is '{current}'"
            ));
        }
        Ok(())
    }

    fn next_graph_id(&mut self) -> String {
        let id = format!("graph:{}", self.graph_counter);
        self.graph_counter = self.graph_counter.wrapping_add(1);
        id
    }

    fn next_anim_id(&mut self) -> String {
        let id = format!("anim:{}", self.anim_counter);
        self.anim_counter = self.anim_counter.wrapping_add(1);
        id
    }

    fn reset_delta_baseline(&mut self) {
        self.last_version = 0;
        self.last_writes = WriteBatch::default();
    }
}

/// JSON request accepted by [`VizijModuleFacade`].
#[derive(Debug, Clone, Deserialize)]
pub struct FacadeRequest {
    pub call: String,
    #[serde(default, rename = "runtimeHandle", alias = "runtime_handle")]
    pub runtime_handle: Option<String>,
    #[serde(default, rename = "requestId", alias = "request_id")]
    pub request_id: Option<String>,
    #[serde(default)]
    pub args: JsonValue,
}

/// JSON response returned by [`VizijModuleFacade`].
#[derive(Debug, Clone, Serialize)]
pub struct FacadeResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none", rename = "requestId")]
    pub request_id: Option<String>,
}

impl FacadeResponse {
    fn ok(request_id: Option<String>, result: JsonValue) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
            version: MODULE_FACADE_VERSION,
            request_id,
        }
    }

    fn error(request_id: Option<String>, error: String) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(error),
            version: MODULE_FACADE_VERSION,
            request_id,
        }
    }

    fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|error| {
            format!(
                "{{\"ok\":false,\"error\":\"failed to serialize facade response: {error}\",\"version\":{MODULE_FACADE_VERSION}}}"
            )
        })
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCreateArgs {
    #[serde(default)]
    schedule: Option<String>,
    #[serde(default, alias = "runtime_handle")]
    runtime_handle: Option<String>,
}

#[derive(Deserialize)]
struct GraphRegistrationArgs {
    #[serde(default)]
    id: Option<String>,
    spec: JsonValue,
    #[serde(default)]
    subs: Option<GraphSubscriptionsArgs>,
}

#[derive(Deserialize)]
struct GraphNormalizeArgs {
    spec: JsonValue,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphSubscriptionsArgs {
    #[serde(default)]
    inputs: Vec<String>,
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default, alias = "mirror_writes")]
    mirror_writes: Option<bool>,
}

#[derive(Deserialize)]
struct MergedGraphRegistrationArgs {
    #[serde(default)]
    id: Option<String>,
    graphs: Vec<GraphRegistrationArgs>,
    #[serde(default)]
    strategy: Option<MergeStrategyArgs>,
}

#[derive(Default, Deserialize)]
struct MergeStrategyArgs {
    #[serde(default)]
    outputs: Option<String>,
    #[serde(default)]
    intermediate: Option<String>,
}

#[derive(Deserialize)]
struct AnimationRegistrationArgs {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    setup: Option<JsonValue>,
}

#[derive(Deserialize)]
struct RemoveControllerArgs {
    id: String,
}

#[derive(Deserialize)]
struct SetInputArgs {
    path: String,
    value: JsonValue,
    #[serde(default)]
    shape: Option<JsonValue>,
}

#[derive(Deserialize)]
struct RemoveInputArgs {
    path: String,
}

#[derive(Deserialize)]
struct StepArgs {
    dt: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StepDeltaArgs {
    dt: f32,
    #[serde(default, alias = "since_version")]
    since_version: Option<u64>,
}

fn parse_args<T: for<'de> Deserialize<'de>>(args: JsonValue) -> Result<T> {
    serde_json::from_value(args).map_err(|error| anyhow!("invalid facade args: {error}"))
}

fn parse_schedule(schedule: Option<&str>) -> Result<Schedule> {
    match schedule {
        None | Some("SinglePass") | Some("singlePass") | Some("single_pass") => {
            Ok(Schedule::SinglePass)
        }
        Some("TwoPass") | Some("twoPass") | Some("two_pass") => Ok(Schedule::TwoPass),
        Some("RateDecoupled") | Some("rateDecoupled") | Some("rate_decoupled") => {
            Ok(Schedule::RateDecoupled)
        }
        Some(other) => Err(anyhow!("unknown schedule option '{other}'")),
    }
}

fn schedule_name(schedule: Schedule) -> &'static str {
    match schedule {
        Schedule::SinglePass => "SinglePass",
        Schedule::TwoPass => "TwoPass",
        Schedule::RateDecoupled => "RateDecoupled",
    }
}

fn build_graph_controller_config(
    mut graph: GraphRegistrationArgs,
    fallback_id: String,
) -> Result<GraphControllerConfig> {
    api_json::normalize_graph_spec_value(&mut graph.spec)
        .map_err(|error| anyhow!("normalize graph spec error: {error}"))?;
    let spec: GraphSpec = serde_json::from_value::<GraphSpec>(graph.spec)
        .map_err(|error| anyhow!("graph spec deserialize error: {error}"))?
        .with_cache();
    Ok(GraphControllerConfig {
        id: graph.id.unwrap_or(fallback_id),
        spec,
        subs: map_graph_subscriptions(graph.subs)?,
    })
}

fn map_graph_subscriptions(cfg: Option<GraphSubscriptionsArgs>) -> Result<Subscriptions> {
    let mut subs = Subscriptions::default();
    if let Some(conf) = cfg {
        subs.inputs = conf
            .inputs
            .into_iter()
            .map(|input| {
                TypedPath::parse(&input)
                    .map_err(|error| anyhow!("invalid input subscription '{input}': {error}"))
            })
            .collect::<Result<_>>()?;
        subs.outputs = conf
            .outputs
            .into_iter()
            .map(|output| {
                TypedPath::parse(&output)
                    .map_err(|error| anyhow!("invalid output subscription '{output}': {error}"))
            })
            .collect::<Result<_>>()?;
        if let Some(mirror) = conf.mirror_writes {
            subs.mirror_writes = mirror;
        }
    }
    Ok(subs)
}

fn map_merge_options(cfg: Option<MergeStrategyArgs>) -> Result<GraphMergeOptions> {
    let mut options = GraphMergeOptions::default();
    if let Some(strategy) = cfg {
        if let Some(outputs) = strategy.outputs {
            options.output_conflicts = parse_conflict_strategy(&outputs)?;
        }
        if let Some(intermediate) = strategy.intermediate {
            options.intermediate_conflicts = parse_conflict_strategy(&intermediate)?;
        }
    }
    Ok(options)
}

fn parse_conflict_strategy(value: &str) -> Result<OutputConflictStrategy> {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" => Ok(OutputConflictStrategy::Error),
        "namespace" => Ok(OutputConflictStrategy::Namespace),
        "blend" | "blend_equal" | "blend_equal_weights" => {
            Ok(OutputConflictStrategy::BlendEqualWeights)
        }
        "add" | "sum" | "blend_sum" | "blend-sum" | "additive" => {
            Ok(OutputConflictStrategy::Add)
        }
        "default_blend"
        | "default-blend"
        | "blend-default"
        | "blend_weights"
        | "blend-weights"
        | "weights" => Ok(OutputConflictStrategy::DefaultBlend),
        other => Err(anyhow!(
            "unknown merge conflict strategy '{other}'; expected 'error', 'namespace', 'blend', 'add', or 'default-blend'"
        )),
    }
}
