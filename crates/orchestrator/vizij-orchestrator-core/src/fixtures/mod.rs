use serde::Deserialize;
use serde_json::{json, Map, Value};

use vizij_test_fixtures::{animations, node_graphs, orchestrations};

#[derive(Debug, Deserialize, Clone)]
pub struct GraphFixture {
    pub spec: serde_json::Value,
    #[serde(default)]
    pub subs: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct AnimationFixture {
    pub setup: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct InputFixture {
    pub path: String,
    pub value: serde_json::Value,
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
    pub graph: GraphFixture,
    pub animation: AnimationFixture,
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

    pub fn animation_setup(&self) -> &serde_json::Value {
        &self.animation.setup
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
    animation: String,
    graph: String,
    #[serde(default, rename = "initial_inputs")]
    initial_inputs: Vec<InputSeed>,
    #[serde(default)]
    steps: Vec<StepSeed>,
}

#[derive(Debug, Deserialize)]
struct InputSeed {
    path: String,
    value: serde_json::Value,
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

fn pipeline_fixture(name: &str) -> DemoFixture {
    let descriptor: PipelineDescriptor =
        orchestrations::load(name).unwrap_or_else(|_| panic!("load {name} descriptor"));

    let graph: GraphFixture = node_graphs::spec(&descriptor.graph)
        .unwrap_or_else(|_| panic!("load shared graph fixture for {name}"));

    let animation_json: serde_json::Value = animations::load(&descriptor.animation)
        .unwrap_or_else(|_| panic!("load shared animation fixture for {name}"));

    let animation_setup = json!({
        "animation": animation_json,
        "player": {
            "name": "fixture-player",
            "loop_mode": "loop"
        }
    });

    DemoFixture {
        description: descriptor.description,
        graph,
        animation: AnimationFixture {
            setup: animation_setup,
        },
        initial_inputs: descriptor
            .initial_inputs
            .into_iter()
            .map(InputFixture::from)
            .collect(),
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
