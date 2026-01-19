//! WebAssembly bindings for `vizij-orchestrator-core`.
//!
//! The exported `VizijOrchestrator` mirrors the Rust API but exchanges data via
//! JSON-friendly values. Graph specs and write batches are normalized to match
//! `vizij-api-core` conventions.

use js_sys::Function;
use serde::Deserialize;
use serde_wasm_bindgen as swb;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsError;

use vizij_api_core::{json, TypedPath, WriteBatch};
use vizij_graph_core::types::GraphSpec;
use vizij_orchestrator::{
    blackboard::ConflictLog,
    controllers::animation::AnimationControllerConfig,
    controllers::graph::{GraphControllerConfig, GraphMergeOptions, OutputConflictStrategy},
    scheduler::Schedule,
    Orchestrator,
};
mod normalize;

/// Minimal options accepted by the JS constructor.
#[derive(Default, Deserialize)]
struct OrchestratorOptions {
    pub schedule: Option<String>,
}

impl VizijOrchestrator {
    fn next_graph_id(&mut self) -> String {
        let id = format!("graph:{}", self.graph_counter);
        self.graph_counter = self.graph_counter.wrapping_add(1);
        id
    }
}

/// Simple JS resolver wrapper that calls a JS Function resolver(path) -> string|number|null
struct JsResolver {
    f: Function,
}

impl vizij_animation_core::TargetResolver for JsResolver {
    fn resolve(&mut self, path: &str) -> Option<String> {
        let arg = JsValue::from_str(path);
        match self.f.call1(&JsValue::UNDEFINED, &arg) {
            Ok(val) => {
                if val.is_undefined() || val.is_null() {
                    return None;
                }
                if let Some(s) = val.as_string() {
                    return Some(s);
                }
                if let Some(n) = val.as_f64() {
                    return Some(if n.fract() == 0.0 {
                        format!("{}", n as i64)
                    } else {
                        format!("{}", n)
                    });
                }
                // Last-resort: try serde conversion to String
                swb::from_value::<String>(val).ok()
            }
            Err(_) => None,
        }
    }
}

#[derive(Deserialize)]
struct JsGraphConfig {
    #[serde(default)]
    id: Option<String>,
    spec: serde_json::Value,
    #[serde(default)]
    subs: Option<JsGraphSubscriptions>,
}

#[derive(Deserialize)]
struct JsMergedGraphConfig {
    #[serde(default)]
    id: Option<String>,
    graphs: Vec<JsGraphConfig>,
    #[serde(default)]
    strategy: Option<JsMergeStrategy>,
}

#[derive(Default, Deserialize)]
struct JsGraphSubscriptions {
    #[serde(default)]
    inputs: Vec<String>,
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default)]
    #[serde(rename = "mirrorWrites", alias = "mirror_writes")]
    mirror_writes: Option<bool>,
}

#[derive(Default, Deserialize)]
struct JsMergeStrategy {
    #[serde(default)]
    outputs: Option<String>,
    #[serde(default)]
    intermediate: Option<String>,
}

fn map_graph_subscriptions(
    cfg: Option<JsGraphSubscriptions>,
) -> Result<vizij_orchestrator::controllers::graph::Subscriptions, String> {
    let mut subs = vizij_orchestrator::controllers::graph::Subscriptions::default();
    if let Some(conf) = cfg {
        subs.inputs = conf
            .inputs
            .into_iter()
            .map(|s| {
                TypedPath::parse(&s)
                    .map_err(|e| format!("invalid input subscription '{}': {}", s, e))
            })
            .collect::<Result<_, _>>()?;
        subs.outputs = conf
            .outputs
            .into_iter()
            .map(|s| {
                TypedPath::parse(&s)
                    .map_err(|e| format!("invalid output subscription '{}': {}", s, e))
            })
            .collect::<Result<_, _>>()?;
        if let Some(mirror) = conf.mirror_writes {
            subs.mirror_writes = mirror;
        }
    }
    Ok(subs)
}

fn parse_conflict_strategy(value: &str) -> Result<OutputConflictStrategy, String> {
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
        other => Err(format!(
            "unknown merge conflict strategy '{}'; expected 'error', 'namespace', 'blend', 'add', or 'default-blend'",
            other
        )),
    }
}

fn map_merge_options(cfg: Option<JsMergeStrategy>) -> Result<GraphMergeOptions, String> {
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

fn build_graph_controller_config(
    mut graph: JsGraphConfig,
    fallback_id: String,
) -> Result<GraphControllerConfig, String> {
    json::normalize_graph_spec_value(&mut graph.spec)
        .map_err(|e| format!("normalize graph spec error: {}", e))?;
    let spec: GraphSpec = serde_json::from_value::<GraphSpec>(graph.spec)
        .map_err(|e| format!("graph spec deserialize error: {}", e))?
        .with_cache();
    let subs = map_graph_subscriptions(graph.subs)?;
    Ok(GraphControllerConfig {
        id: graph.id.unwrap_or(fallback_id),
        spec,
        subs,
    })
}

/// Orchestrator wrapper for JS/wasm hosts.
#[wasm_bindgen]
pub struct VizijOrchestrator {
    core: Orchestrator,
    // counters for autogenerated ids
    graph_counter: u32,
    anim_counter: u32,
    output_version: u64,
    last_version: u64,
    last_writes: WriteBatch,
    last_conflicts: Vec<ConflictLog>,
    last_events: Vec<serde_json::Value>,
    last_timings: std::collections::HashMap<String, f32>,
}

#[wasm_bindgen]
impl VizijOrchestrator {
    /// Create a new orchestrator.
    ///
    /// Accepts an optional options object with `{ schedule }`, where schedule is
    /// `"SinglePass"`, `"TwoPass"`, or `"RateDecoupled"`.
    ///
    /// # Errors
    /// Returns an error if the options payload cannot be decoded or if the
    /// schedule string is invalid.
    #[wasm_bindgen(constructor)]
    pub fn new(opts: JsValue) -> Result<VizijOrchestrator, JsError> {
        #[cfg(feature = "console_error")]
        console_error_panic_hook::set_once();

        let options: OrchestratorOptions = if opts.is_undefined() || opts.is_null() {
            OrchestratorOptions::default()
        } else {
            swb::from_value(opts).map_err(|e| JsError::new(&format!("config parse error: {e}")))?
        };

        let schedule = match options.schedule.as_deref() {
            Some("SinglePass") | None => Schedule::SinglePass,
            Some("TwoPass") => Schedule::TwoPass,
            Some("RateDecoupled") => Schedule::RateDecoupled,
            Some(other) => {
                return Err(JsError::new(&format!("unknown schedule option: {}", other)))
            }
        };

        Ok(VizijOrchestrator {
            core: Orchestrator::new(schedule),
            graph_counter: 0,
            anim_counter: 0,
            output_version: 0,
            last_version: 0,
            last_writes: WriteBatch::default(),
            last_conflicts: Vec::new(),
            last_events: Vec::new(),
            last_timings: std::collections::HashMap::new(),
        })
    }

    /// Register a graph controller.
    ///
    /// Accepts either:
    ///  - a string containing the GraphSpec JSON, or
    ///  - an object { id?: string, spec: object } where spec is a GraphSpec-compatible object.
    ///
    /// Returns the controller id.
    ///
    /// # Errors
    /// Returns an error if the graph spec cannot be decoded or normalized.
    #[wasm_bindgen(js_name = register_graph)]
    pub fn register_graph(&mut self, cfg: JsValue) -> Result<String, JsError> {
        if cfg.is_undefined() || cfg.is_null() {
            return Err(JsError::new("register_graph: cfg required"));
        }

        // If cfg is a string, parse it as JSON text into serde_json::Value
        let (id, mut spec_val, subs_val) = if cfg.is_string() {
            // Treat as raw JSON string
            let s = cfg.as_string().unwrap();
            let v: serde_json::Value = serde_json::from_str(&s)
                .map_err(|e| JsError::new(&format!("graph json parse error: {}", e)))?;
            (None, v, None)
        } else {
            // Treat as object { id?: string, spec: object }
            let obj: JsGraphConfig = swb::from_value(cfg)
                .map_err(|e| JsError::new(&format!("graph cfg parse error: {}", e)))?;
            (obj.id, obj.spec, obj.subs)
        };

        json::normalize_graph_spec_value(&mut spec_val)
            .map_err(|e| JsError::new(&format!("normalize graph spec error: {}", e)))?;

        // Deserialize GraphSpec
        let spec: vizij_graph_core::types::GraphSpec = serde_json::from_value(spec_val)
            .map_err(|e| JsError::new(&format!("graph spec deserialize error: {}", e)))?;

        let subs = map_graph_subscriptions(subs_val)
            .map_err(|e| JsError::new(&format!("graph subscriptions error: {e}")))?;

        // Generate id if needed
        let id = id.unwrap_or_else(|| self.next_graph_id());

        // Build controller config and insert into orchestrator
        let cfg = GraphControllerConfig {
            id: id.clone(),
            spec: spec.with_cache(),
            subs,
        };
        let controller = vizij_orchestrator::controllers::graph::GraphController::new(cfg);
        self.core.graphs.insert(id.clone(), controller);

        Ok(id)
    }

    /// Replace an existing graph controller's spec/subscriptions.
    ///
    /// Accepts an object { id: string, spec: object, subs?: ... }.
    /// (String JSON is intentionally not supported; the id is required.)
    ///
    /// This is the supported way to apply structural edits at runtime. The spec is normalized and
    /// `.with_cache()` is applied so the versioned plan cache cannot reuse stale layouts.
    ///
    /// # Errors
    /// Returns an error if the config payload is invalid or if the graph id is not registered.
    #[wasm_bindgen(js_name = replace_graph)]
    pub fn replace_graph(&mut self, cfg: JsValue) -> Result<(), JsError> {
        if cfg.is_undefined() || cfg.is_null() {
            return Err(JsError::new("replace_graph: cfg required"));
        }
        if cfg.is_string() {
            return Err(JsError::new(
                "replace_graph: expected object { id, spec, subs? } (string form is only supported for register_graph)",
            ));
        }

        let obj: JsGraphConfig = swb::from_value(cfg)
            .map_err(|e| JsError::new(&format!("graph cfg parse error: {e}")))?;

        let id = obj
            .id
            .ok_or_else(|| JsError::new("replace_graph: id required"))?;

        let mut spec_val = obj.spec;
        json::normalize_graph_spec_value(&mut spec_val)
            .map_err(|e| JsError::new(&format!("normalize graph spec error: {e}")))?;
        let spec: vizij_graph_core::types::GraphSpec = serde_json::from_value(spec_val)
            .map_err(|e| JsError::new(&format!("graph spec deserialize error: {e}")))?;

        let subs = map_graph_subscriptions(obj.subs)
            .map_err(|e| JsError::new(&format!("graph subscriptions error: {e}")))?;

        let controller = self.core.graphs.get_mut(&id).ok_or_else(|| {
            JsError::new(&format!("replace_graph: graph '{id}' is not registered"))
        })?;

        controller.replace_config(GraphControllerConfig {
            id,
            spec: spec.with_cache(),
            subs,
        });

        Ok(())
    }

    /// Export a graph spec as a JS object for inspection.
    ///
    /// # Errors
    /// Returns an error if the graph id is unknown or serialization fails.
    #[wasm_bindgen(js_name = export_graph)]
    pub fn export_graph(&self, id: &str) -> Result<JsValue, JsError> {
        let value = self
            .core
            .export_graph_json(id)
            .map_err(|e| JsError::new(&format!("export_graph error: {e}")))?;
        swb::to_value(&value).map_err(|e| JsError::new(&format!("serialize graph spec error: {e}")))
    }

    /// Register multiple graph specs as a single merged controller.
    ///
    /// Accepts an object { id?: string, graphs: GraphRegistrationConfig[] } mirroring the single
    /// controller shape. Each entry supports the same `spec` and optional `subs` fields.
    ///
    /// # Errors
    /// Returns an error if any graph spec is invalid or the merge fails.
    #[wasm_bindgen(js_name = register_merged_graph)]
    pub fn register_merged_graph(&mut self, cfg: JsValue) -> Result<String, JsError> {
        if cfg.is_undefined() || cfg.is_null() {
            return Err(JsError::new("register_merged_graph: cfg required"));
        }
        let obj: JsMergedGraphConfig = swb::from_value(cfg)
            .map_err(|e| JsError::new(&format!("merged graph cfg parse error: {e}")))?;
        if obj.graphs.is_empty() {
            return Err(JsError::new(
                "register_merged_graph: expected at least one graph",
            ));
        }

        let options = map_merge_options(obj.strategy)
            .map_err(|e| JsError::new(&format!("merge strategy error: {e}")))?;

        let merged_id = obj.id.unwrap_or_else(|| self.next_graph_id());
        let mut configs = Vec::with_capacity(obj.graphs.len());
        for (idx, graph_cfg) in obj.graphs.into_iter().enumerate() {
            let fallback_id = format!("{}::{}", merged_id, idx);
            let cfg = build_graph_controller_config(graph_cfg, fallback_id)
                .map_err(|e| JsError::new(&format!("graph cfg error: {}", e)))?;
            configs.push(cfg);
        }

        let merged_cfg =
            GraphControllerConfig::merged_with_options(merged_id.clone(), configs, options)
                .map_err(|e| JsError::new(&format!("graph merge error: {}", e)))?;
        let controller = vizij_orchestrator::controllers::graph::GraphController::new(merged_cfg);
        self.core.graphs.insert(merged_id.clone(), controller);

        Ok(merged_id)
    }

    /// Register an animation controller.
    ///
    /// Accepts an object { id?: string, setup?: any } where setup is forwarded to the
    /// AnimationControllerConfig.setup field.
    /// Returns the controller id.
    ///
    /// # Errors
    /// Returns an error if the setup payload is invalid or the animation cannot be initialized.
    #[wasm_bindgen(js_name = register_animation)]
    pub fn register_animation(&mut self, cfg: JsValue) -> Result<String, JsError> {
        if cfg.is_undefined() || cfg.is_null() {
            return Err(JsError::new("register_animation: cfg required"));
        }
        let obj: serde_json::Value = swb::from_value(cfg)
            .map_err(|e| JsError::new(&format!("animation cfg parse error: {}", e)))?;
        let id_opt = obj
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let setup = obj.get("setup").cloned().unwrap_or(serde_json::Value::Null);

        let id = match id_opt {
            Some(s) => s,
            None => {
                let aid = format!("anim:{}", self.anim_counter);
                self.anim_counter = self.anim_counter.wrapping_add(1);
                aid
            }
        };

        let cfg = AnimationControllerConfig {
            id: id.clone(),
            setup,
        };
        let controller =
            vizij_orchestrator::controllers::animation::AnimationController::try_new(cfg)
                .map_err(|e| JsError::new(&format!("animation setup error: {e}")))?;
        self.core.anims.insert(id.clone(), controller);
        Ok(id)
    }

    /// Resolve canonical target paths using a JS resolver function.
    ///
    /// The resolver should be `function(path: string): string|number|null|undefined`.
    /// For each registered animation controller we call engine.prebind(&mut JsResolver).
    #[wasm_bindgen]
    pub fn prebind(&mut self, resolver: Function) {
        let mut js_resolver = JsResolver { f: resolver };
        for (_id, anim) in self.core.anims.iter_mut() {
            // Engine::prebind expects &mut dyn TargetResolver
            // swallow errors from JS resolver as the animation wasm does
            anim.engine.prebind(&mut js_resolver);
        }
    }

    /// Set a blackboard input value (convenience).
    ///
    /// `value_json` and `shape_json` should be JS objects compatible with the core Value/Shape JSON shapes.
    ///
    /// # Errors
    /// Returns an error if the path is invalid or JSON payloads cannot be decoded.
    #[wasm_bindgen(js_name = set_input)]
    pub fn set_input(
        &mut self,
        path: &str,
        value_json: JsValue,
        shape_json: JsValue,
    ) -> Result<(), JsError> {
        let value: serde_json::Value = swb::from_value(value_json)
            .map_err(|e| JsError::new(&format!("value parse error: {}", e)))?;
        let shape_opt: Option<serde_json::Value> =
            if shape_json.is_null() || shape_json.is_undefined() {
                None
            } else {
                Some(
                    swb::from_value(shape_json)
                        .map_err(|e| JsError::new(&format!("shape parse error: {}", e)))?,
                )
            };
        self.core
            .blackboard
            .set(
                path.to_string(),
                value,
                shape_opt,
                self.core.epoch,
                "host".to_string(),
            )
            .map_err(|e| JsError::new(&format!("blackboard set error: {}", e)))?;
        Ok(())
    }

    /// Remove an input key from the blackboard.
    ///
    /// Returns `true` if a value was removed.
    #[wasm_bindgen(js_name = remove_input)]
    pub fn remove_input(&mut self, path: &str) -> bool {
        self.core.blackboard.remove(path).is_some()
    }

    /// Step the orchestrator by `dt` seconds and return an OrchestratorFrame as a JS value.
    ///
    /// # Errors
    /// Returns an error if evaluation fails or the frame cannot be serialized.
    #[wasm_bindgen]
    pub fn step(&mut self, dt: f32) -> Result<JsValue, JsError> {
        let frame = self
            .core
            .step(dt)
            .map_err(|e| JsError::new(&format!("step error: {}", e)))?;
        self.output_version = self.output_version.saturating_add(1);
        self.last_version = self.output_version;
        self.last_writes = frame.merged_writes.clone();
        self.last_conflicts = frame.conflicts.clone();
        self.last_events = frame.events.clone();
        self.last_timings = frame.timings_ms.clone();
        swb::to_value(&frame).map_err(|e| JsError::new(&format!("serialize frame error: {}", e)))
    }

    /// Step and return only changes since the caller's version.
    ///
    /// If `since_version` does not match the last snapshot, a full frame payload
    /// is returned instead.
    ///
    /// # Errors
    /// Returns an error if evaluation fails or the payload cannot be serialized.
    #[wasm_bindgen(js_name = step_delta)]
    pub fn step_delta(&mut self, dt: f32, since_version: Option<u64>) -> Result<JsValue, JsError> {
        let frame = self
            .core
            .step(dt)
            .map_err(|e| JsError::new(&format!("step error: {}", e)))?;
        self.output_version = self.output_version.saturating_add(1);
        let version = self.output_version;
        let since = since_version.unwrap_or(0);

        let (mut merged_writes, conflicts, events, timings_ms) = (
            frame.merged_writes.clone(),
            frame.conflicts.clone(),
            frame.events.clone(),
            frame.timings_ms.clone(),
        );

        // If the caller's baseline matches ours, send only the fields that changed.
        if since == self.last_version {
            if frame.merged_writes == self.last_writes {
                merged_writes = WriteBatch::default();
            }
            // Conflicts/events/timings are typically small; always send full payloads for now.
        } else {
            // Caller baseline is stale; treat this as a full snapshot.
        }

        // Update caches to the latest frame.
        self.last_version = version;
        self.last_writes = frame.merged_writes;
        self.last_conflicts = frame.conflicts;
        self.last_events = frame.events;
        self.last_timings = frame.timings_ms;

        let payload = serde_json::json!({
            "version": version,
            "epoch": frame.epoch,
            "dt": frame.dt,
            "merged_writes": merged_writes,
            "conflicts": conflicts,
            "events": events,
            "timings_ms": timings_ms,
        });

        swb::to_value(&payload).map_err(|e| JsError::new(&format!("serialize delta error: {}", e)))
    }

    /// List registered controller ids.
    ///
    /// # Errors
    /// Returns an error if the response cannot be serialized.
    #[wasm_bindgen(js_name = list_controllers)]
    pub fn list_controllers(&self) -> Result<JsValue, JsError> {
        let graphs: Vec<String> = self.core.graphs.keys().cloned().collect();
        let anims: Vec<String> = self.core.anims.keys().cloned().collect();
        let mut out = serde_json::Map::new();
        out.insert(
            "graphs".to_string(),
            serde_json::Value::Array(graphs.into_iter().map(serde_json::Value::String).collect()),
        );
        out.insert(
            "anims".to_string(),
            serde_json::Value::Array(anims.into_iter().map(serde_json::Value::String).collect()),
        );
        swb::to_value(&serde_json::Value::Object(out))
            .map_err(|e| JsError::new(&format!("list_controllers serialize: {}", e)))
    }

    /// Remove a registered graph controller by id.
    ///
    /// Returns `true` if a controller was removed.
    #[wasm_bindgen(js_name = remove_graph)]
    pub fn remove_graph(&mut self, id: &str) -> bool {
        self.core.graphs.shift_remove(id).is_some()
    }

    /// Remove a registered animation controller by id.
    ///
    /// Returns `true` if a controller was removed.
    #[wasm_bindgen(js_name = remove_animation)]
    pub fn remove_animation(&mut self, id: &str) -> bool {
        self.core.anims.shift_remove(id).is_some()
    }
}

/// Normalize a graph spec JSON string into canonical form.
///
/// The return value is a JSON string with shorthand expanded.
///
/// # Errors
/// Returns an error if the JSON cannot be parsed or normalized.
#[wasm_bindgen(js_name = normalize_graph_spec_json)]
pub fn normalize_graph_spec_json(json: &str) -> Result<JsValue, JsError> {
    match crate::normalize::normalize_graph_spec_json(json) {
        Ok(s) => Ok(JsValue::from_str(&s)),
        Err(e) => Err(JsError::new(&format!(
            "normalize_graph_spec_json error: {}",
            e
        ))),
    }
}

/// ABI version for compatibility checks.
#[wasm_bindgen]
pub fn abi_version() -> u32 {
    2
}
