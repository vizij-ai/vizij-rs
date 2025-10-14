use serde_json::json;
use vizij_api_core::{TypedPath, Value};
use vizij_graph_core::types::GraphSpec;
use vizij_orchestrator::{GraphControllerConfig, Orchestrator, Schedule, Subscriptions};

fn main() -> anyhow::Result<()> {
    // Graph A: produce a constant value and expose it via an output node.
    let producer_spec: GraphSpec = serde_json::from_value(json!({
        "nodes": [
            {
                "id": "constant_one",
                "type": "constant",
                "params": { "value": { "type": "float", "data": 1.0 } }
            },
            {
                "id": "publish",
                "type": "output",
                "params": { "path": "shared/value" }
            }
        ],
        "links": [
            { "from": { "node_id": "constant_one" }, "to": { "node_id": "publish", "input": "in" } }
        ]
    }))?;

    // Graph B: consume the shared value, double it, and emit another output.
    let consumer_spec: GraphSpec = serde_json::from_value(json!({
        "nodes": [
            {
                "id": "shared_input",
                "type": "input",
                "params": { "path": "shared/value" }
            },
            {
                "id": "scale",
                "type": "multiply",
                "input_defaults": {
                    "rhs": { "value": { "type": "float", "data": 2.0 } }
                }
            },
            {
                "id": "result",
                "type": "output",
                "params": { "path": "shared/doubled" }
            }
        ],
        "links": [
            { "from": { "node_id": "shared_input" }, "to": { "node_id": "scale", "input": "lhs" } },
            { "from": { "node_id": "scale" }, "to": { "node_id": "result", "input": "in" } }
        ]
    }))?;

    let shared_value_path = TypedPath::parse("shared/value").expect("typed path");
    let shared_doubled_path = TypedPath::parse("shared/doubled").expect("typed path");

    let producer_cfg = GraphControllerConfig {
        id: "producer".into(),
        spec: producer_spec,
        subs: Subscriptions {
            inputs: Vec::new(),
            outputs: vec![shared_value_path.clone()],
            mirror_writes: true,
        },
    };

    let consumer_cfg = GraphControllerConfig {
        id: "consumer".into(),
        spec: consumer_spec,
        subs: Subscriptions {
            inputs: vec![shared_value_path.clone()],
            outputs: vec![shared_doubled_path.clone()],
            mirror_writes: true,
        },
    };

    // Merge both graphs so the orchestrator can execute them in a single pass.
    let orch = Orchestrator::new(Schedule::SinglePass)
        .with_merged_graph("merged-graph", vec![producer_cfg, consumer_cfg])?;

    // Step once; the merged graph should propagate 1.0 -> doubled output (2.0).
    let mut orch = orch;
    let frame = orch.step(1.0 / 60.0)?;
    for op in frame.merged_writes.iter() {
        println!("{} = {:?}", op.path, op.value);
    }

    if let Some(entry) = orch.blackboard.get(&shared_doubled_path.to_string()) {
        println!("Blackboard {} = {:?}", shared_doubled_path, entry.value);
        assert_eq!(entry.value, Value::Float(2.0));
    }

    Ok(())
}
