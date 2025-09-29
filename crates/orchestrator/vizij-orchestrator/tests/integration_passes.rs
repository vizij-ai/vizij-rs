use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};
use vizij_graph_core::types::GraphSpec;
use vizij_orchestrator::{GraphControllerConfig, Orchestrator, Schedule, Subscriptions};

#[test]
fn single_pass_applies_graph_writes_and_merges() {
    // Setup orchestrator with single-pass schedule
    let mut orch = Orchestrator::new(Schedule::SinglePass);

    // Register a graph controller with default subscriptions
    let cfg = GraphControllerConfig {
        id: "g".into(),
        spec: GraphSpec::default(),
        subs: Subscriptions::default(),
    };
    orch = orch.with_graph(cfg);

    // Prepare a write produced by the graph runtime and attach it
    let tp = TypedPath::parse("robot/x").unwrap();
    let mut batch = WriteBatch::new();
    batch.push(WriteOp::new(tp.clone(), Value::Float(0.5)));

    // Inject the batch into the graph runtime writes so evaluate() will yield it
    let gc = orch.graphs.get_mut("g").expect("graph exists");
    gc.rt.writes = batch.clone();

    // Step orchestrator
    let frame = orch.step(0.016).expect("step ok");

    // merged_writes should contain the write
    let found = frame
        .merged_writes
        .iter()
        .any(|op| op.path.to_string() == tp.to_string() && op.value == Value::Float(0.5));
    assert!(found, "merged_writes must contain the graph write");

    // Blackboard should have the applied value
    let be = orch
        .blackboard
        .get(&tp.to_string())
        .expect("blackboard entry present");
    assert_eq!(be.value, Value::Float(0.5));
}

#[test]
fn two_pass_applies_graph_then_anim_then_graph_writes_and_merges() {
    // Two-pass schedule: graphs -> anims -> graphs
    let mut orch = Orchestrator::new(Schedule::TwoPass);

    // Register a graph controller that will produce a write in pass1
    let cfg = GraphControllerConfig {
        id: "g1".into(),
        spec: GraphSpec::default(),
        subs: Subscriptions::default(),
    };
    orch = orch.with_graph(cfg);

    // Register another graph controller that will produce a write in pass2
    let cfg2 = GraphControllerConfig {
        id: "g2".into(),
        spec: GraphSpec::default(),
        subs: Subscriptions::default(),
    };
    orch = orch.with_graph(cfg2);

    // Prepare writes for both graphs (they'll be consumed when evaluate() is called)
    let tp1 = TypedPath::parse("robot/a").unwrap();
    let mut b1 = WriteBatch::new();
    b1.push(WriteOp::new(tp1.clone(), Value::Float(1.0)));
    orch.graphs.get_mut("g1").unwrap().rt.writes = b1;

    let tp2 = TypedPath::parse("robot/b").unwrap();
    let mut b2 = WriteBatch::new();
    b2.push(WriteOp::new(tp2.clone(), Value::Float(2.0)));
    orch.graphs.get_mut("g2").unwrap().rt.writes = b2;

    // Step orchestrator
    let frame = orch.step(0.016).expect("step ok");

    // merged_writes should contain writes from both graphs in deterministic order
    let mut found_a = false;
    let mut found_b = false;
    for op in frame.merged_writes.iter() {
        if op.path.to_string() == tp1.to_string() && op.value == Value::Float(1.0) {
            found_a = true;
        }
        if op.path.to_string() == tp2.to_string() && op.value == Value::Float(2.0) {
            found_b = true;
        }
    }
    assert!(
        found_a && found_b,
        "merged_writes must include both graph writes"
    );

    // Blackboard should have both entries applied
    let be_a = orch.blackboard.get(&tp1.to_string()).expect("entry a");
    assert_eq!(be_a.value, Value::Float(1.0));
    let be_b = orch.blackboard.get(&tp2.to_string()).expect("entry b");
    assert_eq!(be_b.value, Value::Float(2.0));
}
