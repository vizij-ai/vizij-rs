use serde_json::to_string_pretty;
use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};
use vizij_graph_core::types::GraphSpec;
use vizij_orchestrator::{GraphControllerConfig, Orchestrator, Schedule, Subscriptions};

fn main() -> anyhow::Result<()> {
    // Create an orchestrator with the default single-pass schedule.
    let mut orch = Orchestrator::new(Schedule::SinglePass);

    // Register a graph controller (empty GraphSpec for this minimal example).
    let cfg = GraphControllerConfig {
        id: "example-graph".into(),
        spec: GraphSpec::default(),
        subs: Subscriptions::default(),
    };
    orch = orch.with_graph(cfg);

    // Inject a write into the graph runtime to simulate a node producing an output.
    let tp = TypedPath::parse("robot/Arm/Joint1.angle").unwrap();
    let mut batch = WriteBatch::new();
    batch.push(WriteOp::new(tp.clone(), Value::Float(0.42)));

    // Place the batch into the controller runtime so evaluate() will return it.
    if let Some(g) = orch.graphs.get_mut("example-graph") {
        g.rt.writes = batch;
    }

    // Step the orchestrator
    let frame = orch.step(1.0 / 60.0)?;

    println!("Orchestrator frame epoch: {}", frame.epoch);
    println!(
        "Merged writes (pretty):\n{}",
        to_string_pretty(&frame.merged_writes)?
    );

    // Inspect blackboard entry (if applied)
    if let Some(entry) = orch.blackboard.get(&tp.to_string()) {
        println!("Blackboard[{}] = {:?}", tp, entry.value);
    } else {
        println!("No blackboard entry for {}", tp);
    }

    Ok(())
}
