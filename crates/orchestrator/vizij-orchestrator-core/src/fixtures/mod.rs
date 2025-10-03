use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GraphFixture {
    pub spec: serde_json::Value,
    #[serde(default)]
    pub subs: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct AnimationFixture {
    pub setup: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct DemoFixture {
    pub graph: GraphFixture,
    pub animation: AnimationFixture,
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
}

pub fn demo_single_pass() -> DemoFixture {
    serde_json::from_str(include_str!("../../fixtures/demo_single_pass.json"))
        .expect("valid demo_single_pass fixture")
}
