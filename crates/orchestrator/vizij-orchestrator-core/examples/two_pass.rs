use serde_json::to_string_pretty;
use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};
use vizij_graph_core::types::GraphSpec;
use vizij_orchestrator::{GraphControllerConfig, Orchestrator, Schedule, Subscriptions};

fn main() -> anyhow::Result<()> {
    let mut orch = Orchestrator::new(Schedule::TwoPass);

    // Graph controllers
    let g1 = GraphControllerConfig {
        id: "g1".into(),
        spec: GraphSpec::default(),
        subs: Subscriptions::default(),
    };
    let g2 = GraphControllerConfig {
        id: "g2".into(),
        spec: GraphSpec::default(),
        subs: Subscriptions::default(),
    };
    orch = orch.with_graph(g1).with_graph(g2);

    // Inject writes into runtimes to simulate outputs across passes
    let tp1 = TypedPath::parse("robot/a").unwrap();
    let mut b1 = WriteBatch::new();
    b1.push(WriteOp::new(tp1.clone(), Value::Float(1.0)));
    orch.graphs.get_mut("g1").unwrap().rt.writes = b1;

    let tp2 = TypedPath::parse("robot/b").unwrap();
    let mut b2 = WriteBatch::new();
    b2.push(WriteOp::new(tp2.clone(), Value::Float(2.0)));
    orch.graphs.get_mut("g2").unwrap().rt.writes = b2;

    // Step orchestrator
    let frame = orch.step(1.0 / 60.0)?;

    println!(
        "Frame merged writes:\n{}",
        to_string_pretty(&frame.merged_writes)?
    );
    if let Some(e) = orch.blackboard.get(&tp1.to_string()) {
        println!("Blackboard {} = {:?}", tp1, e.value);
    }
    if let Some(e) = orch.blackboard.get(&tp2.to_string()) {
        println!("Blackboard {} = {:?}", tp2, e.value);
    }

    Ok(())
}
