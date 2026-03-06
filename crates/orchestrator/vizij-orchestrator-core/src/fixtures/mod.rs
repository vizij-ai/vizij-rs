//! Helpers for turning shared fixture descriptors into orchestrator-ready configs.

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::controllers::graph::{GraphControllerConfig, Subscriptions};
use vizij_api_core::{json::normalize_graph_spec_value, TypedPath};
use vizij_graph_core::types::GraphSpec;
use vizij_test_fixtures::{animations, node_graphs, orchestrations};

#[derive(Debug, Deserialize, Clone)]
pub struct GraphFixture {
    #[serde(skip_deserializing, default)]
    pub key: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    pub spec: serde_json::Value,
    #[serde(default)]
    pub subs: serde_json::Value,
    #[serde(default)]
    pub mirror_writes: bool,
    #[serde(skip)]
    pub stage: Vec<InputFixture>,
}

#[derive(Debug, Clone)]
pub struct AnimationFixture {
    pub key: Option<String>,
    pub id: Option<String>,
    pub setup: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct InputFixture {
    pub path: String,
    pub value: serde_json::Value,
    pub shape: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct MergedGraphFixture {
    pub id: String,
    pub graphs: Vec<GraphFixture>,
    pub options: crate::controllers::graph::GraphMergeOptions,
}

impl GraphFixture {
    pub fn controller_config(&self) -> GraphControllerConfig {
        let mut spec_value = self.spec.clone();
        normalize_graph_spec_value(&mut spec_value).expect("normalize graph spec");
        let spec: GraphSpec = serde_json::from_value::<GraphSpec>(spec_value)
            .unwrap_or_else(|e| panic!("graph spec json invalid: {e}"))
            .with_cache();

        let subs_json = &self.subs;
        let parse_paths = |key: &str| -> Vec<TypedPath> {
            subs_json
                .get(key)
                .and_then(|value| value.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|entry| {
                            let path = entry
                                .as_str()
                                .unwrap_or_else(|| panic!("{key} entry must be string"));
                            TypedPath::parse(path)
                                .unwrap_or_else(|_| panic!("invalid typed path {path}"))
                        })
                        .collect::<Vec<TypedPath>>()
                })
                .unwrap_or_default()
        };

        let mirror = subs_json
            .get("mirrorWrites")
            .and_then(|v| v.as_bool())
            .or_else(|| subs_json.get("mirror_writes").and_then(|v| v.as_bool()))
            .unwrap_or(self.mirror_writes);

        let id = self
            .id
            .clone()
            .or_else(|| self.key.clone())
            .unwrap_or_else(|| "graph-fixture".to_string());

        GraphControllerConfig {
            id,
            spec,
            subs: Subscriptions {
                inputs: parse_paths("inputs"),
                outputs: parse_paths("outputs"),
                mirror_writes: mirror,
            },
        }
    }
}

impl MergedGraphFixture {
    pub fn controller_config(&self) -> GraphControllerConfig {
        let configs: Vec<GraphControllerConfig> = self
            .graphs
            .iter()
            .map(|graph| graph.controller_config())
            .collect();
        GraphControllerConfig::merged_with_options(self.id.clone(), configs, self.options)
            .expect("merged graph fixture should merge")
    }
}

#[derive(Debug, Clone)]
pub struct StepFixture {
    pub delta: f64,
    pub expect: Vec<(String, serde_json::Value)>,
}

impl StepFixture {
    pub fn expected(&self, path: &str) -> Option<&serde_json::Value> {
        self.expect
            .iter()
            .find_map(|(p, v)| if p == path { Some(v) } else { None })
    }
}

#[derive(Debug, Clone)]
pub struct DemoFixture {
    pub description: Option<String>,
    pub schedule: Option<String>,
    pub graph: GraphFixture,
    pub graphs: Vec<GraphFixture>,
    pub merged_graphs: Vec<MergedGraphFixture>,
    pub animation: AnimationFixture,
    pub animations: Vec<AnimationFixture>,
    pub initial_inputs: Vec<InputFixture>,
    pub steps: Vec<StepFixture>,
}

impl DemoFixture {
    pub fn graph_spec_json(&self) -> &serde_json::Value {
        &self.graph.spec
    }

    pub fn graph_subscriptions(&self) -> &serde_json::Value {
        &self.graph.subs
    }

    pub fn graphs(&self) -> &[GraphFixture] {
        &self.graphs
    }

    pub fn merged_graphs(&self) -> &[MergedGraphFixture] {
        &self.merged_graphs
    }

    pub fn animation_setup(&self) -> &serde_json::Value {
        &self.animation.setup
    }

    pub fn animations(&self) -> &[AnimationFixture] {
        &self.animations
    }

    pub fn schedule(&self) -> Option<&str> {
        self.schedule.as_deref()
    }

    pub fn initial_inputs(&self) -> &[InputFixture] {
        &self.initial_inputs
    }

    pub fn steps(&self) -> &[StepFixture] {
        &self.steps
    }
}

#[derive(Debug, Deserialize)]
struct PipelineDescriptor {
    description: Option<String>,
    #[serde(default)]
    schedule: Option<String>,
    #[serde(default)]
    animations: Vec<AnimationSeed>,
    #[serde(default)]
    graphs: Vec<GraphSeed>,
    #[serde(default, rename = "merged_graphs")]
    merged_graphs: Vec<MergedGraphSeed>,
    #[serde(default, rename = "initial_inputs")]
    initial_inputs: Vec<InputSeed>,
    #[serde(default)]
    steps: Vec<StepSeed>,
}

#[derive(Debug, Deserialize)]
struct AnimationSeed {
    fixture: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    player: Option<serde_json::Value>,
    #[serde(default)]
    instance: Option<serde_json::Value>,
    #[serde(default)]
    setup: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
struct GraphSeed {
    fixture: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    subs: Option<serde_json::Value>,
    #[serde(default)]
    mirror_writes: Option<bool>,
    #[serde(default)]
    stage: Vec<InputSeed>,
}

#[derive(Debug, Deserialize)]
struct MergeStrategySeed {
    #[serde(default)]
    outputs: Option<String>,
    #[serde(default)]
    intermediate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MergedGraphSeed {
    id: String,
    graphs: Vec<GraphSeed>,
    #[serde(default)]
    strategy: Option<MergeStrategySeed>,
}

#[derive(Debug, Deserialize, Clone)]
struct InputSeed {
    path: String,
    value: serde_json::Value,
    #[serde(default)]
    shape: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct StepSeed {
    delta: f64,
    expect: Map<String, Value>,
}

impl From<InputSeed> for InputFixture {
    fn from(seed: InputSeed) -> Self {
        Self {
            path: seed.path,
            value: seed.value,
            shape: seed.shape,
        }
    }
}

impl From<StepSeed> for StepFixture {
    fn from(seed: StepSeed) -> Self {
        Self {
            delta: seed.delta,
            expect: seed.expect.into_iter().collect(),
        }
    }
}

fn parse_strategy(
    value: Option<String>,
    field: &str,
    name: &str,
) -> crate::controllers::graph::OutputConflictStrategy {
    use crate::controllers::graph::OutputConflictStrategy::*;
    match value.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("namespace") => Namespace,
        Some("blend") | Some("blend-equal-weights") => BlendEqualWeights,
        Some("add") | Some("sum") | Some("blend-sum") | Some("additive") => Add,
        Some("default-blend") | Some("blend-default") | Some("blend-weights") | Some("weights") => {
            DefaultBlend
        }
        Some("error") | None => Error,
        Some(other) => {
            panic!("orchestration '{name}' provided unknown merge strategy '{other}' for {field}")
        }
    }
}

fn materialize_graph_fixture(seed: GraphSeed, name: &str) -> GraphFixture {
    let fixture_key = seed.fixture.clone();
    let mut graph: GraphFixture = node_graphs::spec(&fixture_key)
        .unwrap_or_else(|_| panic!("load shared graph fixture '{}' for {name}", fixture_key));
    graph.key = Some(fixture_key);
    if let Some(id) = seed.id {
        graph.id = Some(id);
    }
    if let Some(subs) = seed.subs {
        graph.subs = subs;
    }
    graph.mirror_writes = seed.mirror_writes.unwrap_or(false);
    graph.stage = seed.stage.into_iter().map(InputFixture::from).collect();
    graph
}

fn materialize_animation_fixture(idx: usize, seed: AnimationSeed, name: &str) -> AnimationFixture {
    let animation_json: serde_json::Value = animations::load(&seed.fixture).unwrap_or_else(|_| {
        panic!(
            "load shared animation fixture '{}' for {name}",
            seed.fixture
        )
    });

    let setup = if let Some(custom) = seed.setup {
        custom
    } else {
        let default_player_name = if idx == 0 {
            "fixture-player".to_string()
        } else {
            format!("fixture-player-{}", idx)
        };
        let default_player = json!({
            "name": default_player_name,
            "loop_mode": "loop"
        });
        let mut payload = Map::new();
        payload.insert("animation".to_string(), animation_json);
        payload.insert("player".to_string(), seed.player.unwrap_or(default_player));
        if let Some(instance_value) = seed.instance {
            payload.insert("instance".to_string(), instance_value);
        }
        serde_json::Value::Object(payload)
    };

    AnimationFixture {
        key: Some(seed.fixture),
        id: seed.id,
        setup,
    }
}

fn materialize_merged_graph_fixture(merged: MergedGraphSeed, name: &str) -> MergedGraphFixture {
    use crate::controllers::graph::GraphMergeOptions;

    if merged.graphs.is_empty() {
        panic!(
            "orchestration '{name}' merged graph '{}' did not include component graphs",
            merged.id
        );
    }

    let graphs: Vec<GraphFixture> = merged
        .graphs
        .into_iter()
        .map(|seed| materialize_graph_fixture(seed, name))
        .collect();

    let options = if let Some(strategy) = merged.strategy {
        GraphMergeOptions {
            output_conflicts: parse_strategy(strategy.outputs, "outputs", name),
            intermediate_conflicts: parse_strategy(strategy.intermediate, "intermediate", name),
        }
    } else {
        GraphMergeOptions::default()
    };

    MergedGraphFixture {
        id: merged.id,
        graphs,
        options,
    }
}

fn pipeline_fixture(name: &str) -> DemoFixture {
    let descriptor: PipelineDescriptor =
        orchestrations::load(name).unwrap_or_else(|_| panic!("load {name} descriptor"));

    if descriptor.animations.is_empty() {
        panic!("orchestration '{name}' did not specify any animation fixtures");
    }

    if descriptor.graphs.is_empty() && descriptor.merged_graphs.is_empty() {
        panic!("orchestration '{name}' did not specify any graph fixtures");
    }

    let graphs: Vec<GraphFixture> = descriptor
        .graphs
        .into_iter()
        .map(|seed| materialize_graph_fixture(seed, name))
        .collect();

    let merged_graphs: Vec<MergedGraphFixture> = descriptor
        .merged_graphs
        .into_iter()
        .map(|seed| materialize_merged_graph_fixture(seed, name))
        .collect();

    let animations: Vec<AnimationFixture> = descriptor
        .animations
        .into_iter()
        .enumerate()
        .map(|(idx, seed)| materialize_animation_fixture(idx, seed, name))
        .collect();

    let mut initial_inputs: Vec<InputFixture> = descriptor
        .initial_inputs
        .into_iter()
        .map(InputFixture::from)
        .collect();

    for graph in graphs
        .iter()
        .chain(merged_graphs.iter().flat_map(|merged| merged.graphs.iter()))
    {
        initial_inputs.extend(graph.stage.iter().cloned());
    }

    let graph_for_compat = graphs
        .first()
        .cloned()
        .or_else(|| {
            merged_graphs
                .first()
                .and_then(|merged| merged.graphs.first().cloned())
        })
        .expect("at least one graph fixture available");

    let animation_for_compat = animations
        .first()
        .cloned()
        .expect("at least one animation fixture available");

    DemoFixture {
        description: descriptor.description,
        schedule: descriptor.schedule,
        graph: graph_for_compat.clone(),
        graphs,
        merged_graphs,
        animation: animation_for_compat.clone(),
        animations,
        initial_inputs,
        steps: descriptor
            .steps
            .into_iter()
            .map(StepFixture::from)
            .collect(),
    }
}

pub fn demo_single_pass() -> DemoFixture {
    pipeline_fixture("scalar-ramp-pipeline")
}

pub fn blend_pose_pipeline() -> DemoFixture {
    pipeline_fixture("blend-pose-pipeline")
}

pub fn load_pipeline(name: &str) -> DemoFixture {
    pipeline_fixture(name)
}

pub fn graph_controller_config_from_fixture(name: &str) -> GraphControllerConfig {
    let mut graph: GraphFixture =
        node_graphs::spec(name).unwrap_or_else(|_| panic!("load graph fixture {name}"));
    graph.key = Some(name.to_string());
    graph.controller_config()
}
