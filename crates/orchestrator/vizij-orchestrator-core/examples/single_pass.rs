use serde_json::to_string_pretty;
use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};
use vizij_graph_core::types::GraphSpec;
use vizij_orchestrator::{
    AnimationControllerConfig, GraphControllerConfig, Orchestrator, Schedule, Subscriptions,
};

fn main() -> anyhow::Result<()> {
    let mut orch = Orchestrator::new(Schedule::SinglePass);

    // Graph controller
    let gcfg = GraphControllerConfig {
        id: "g".into(),
        spec: GraphSpec::default(),
        subs: Subscriptions::default(),
    };
    orch = orch.with_graph(gcfg);

    // Animation controller (stub engine)
    let acfg = AnimationControllerConfig {
        id: "anim".into(),
        setup: serde_json::json!({}),
    };
    orch = orch.with_animation(acfg);

    // Inject a graph write for the graph to produce
    let tp = TypedPath::parse("robot/pos").unwrap();
    let mut wb = WriteBatch::new();
    wb.push(WriteOp::new(tp.clone(), Value::Vec3([0.1, 0.2, 0.3])));
    orch.graphs.get_mut("g").unwrap().rt.writes = wb;

    // Step single frame
    let frame = orch.step(1.0 / 60.0)?;

    println!(
        "Frame merged writes:\n{}",
        to_string_pretty(&frame.merged_writes)?
    );
    Ok(())
}
