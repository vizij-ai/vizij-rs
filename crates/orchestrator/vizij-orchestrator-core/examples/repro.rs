use vizij_api_core::{TypedPath, Value};
use vizij_orchestrator::{
    controllers::animation::AnimationControllerConfig, controllers::graph::GraphControllerConfig,
    controllers::graph::Subscriptions, Orchestrator, Schedule,
};

fn main() {
    let mut orchestrator = Orchestrator::new(Schedule::SinglePass);

    let graph_spec_json = serde_json::json!({
        "nodes": [
            {
                "id": "anim_input",
                "type": "input",
                "params": {
                    "path": "demo/animation.value",
                    "value": { "type": "float", "data": 0 }
                }
            },
            {
                "id": "gain_input",
                "type": "input",
                "params": {
                    "path": "demo/graph/gain",
                    "value": { "type": "float", "data": 1.5 }
                }
            },
            {
                "id": "offset_input",
                "type": "input",
                "params": {
                    "path": "demo/graph/offset",
                    "value": { "type": "float", "data": 0.25 }
                }
            },
            {
                "id": "scaled",
                "type": "multiply",
                "inputs": {
                    "lhs": { "node_id": "anim_input" },
                    "rhs": { "node_id": "gain_input" }
                }
            },
            {
                "id": "output_sum",
                "type": "add",
                "inputs": {
                    "lhs": { "node_id": "scaled" },
                    "rhs": { "node_id": "offset_input" }
                }
            },
            {
                "id": "out",
                "type": "output",
                "params": { "path": "demo/output/value" },
                "inputs": { "in": { "node_id": "output_sum" } }
            }
        ]
    });

    let graph_spec: vizij_graph_core::types::GraphSpec =
        serde_json::from_value(graph_spec_json).expect("graph spec json");
    let subs = Subscriptions {
        inputs: vec![
            TypedPath::parse("demo/animation.value").unwrap(),
            TypedPath::parse("demo/graph/gain").unwrap(),
            TypedPath::parse("demo/graph/offset").unwrap(),
        ],
        outputs: vec![TypedPath::parse("demo/output/value").unwrap()],
        mirror_writes: true,
    };
    orchestrator = orchestrator.with_graph(GraphControllerConfig {
        id: "graph:0".into(),
        spec: graph_spec,
        subs,
    });

    let animation_setup = serde_json::json!({
        "animation": {
            "id": "demo-ramp",
            "name": "Demo Ramp",
            "duration": 2001,
            "groups": [],
            "tracks": [
                {
                    "id": "ramp-track",
                    "name": "Ramp Value",
                    "animatableId": "demo/animation.value",
                    "points": [
                        { "id": "start", "stamp": 0.0, "value": 0.0 },
                        { "id": "end", "stamp": 1.0, "value": 1.0 }
                    ]
                }
            ]
        },
        "player": {
            "name": "demo-player",
            "loop_mode": "loop"
        }
    });

    orchestrator = orchestrator.with_animation(AnimationControllerConfig {
        id: "anim:0".into(),
        setup: animation_setup,
    });

    orchestrator
        .set_input(
            "demo/graph/gain",
            serde_json::json!({ "type": "float", "data": 1.5 }),
            None,
        )
        .unwrap();
    orchestrator
        .set_input(
            "demo/graph/offset",
            serde_json::json!({ "type": "float", "data": 0.25 }),
            None,
        )
        .unwrap();

    let frame0 = orchestrator.step(0.0).unwrap();
    println!("frame0 writes {:?}", frame0.merged_writes);
    let frame1 = orchestrator.step(1.0).unwrap();
    println!("frame1 writes {:?}", frame1.merged_writes);
    let frame2 = orchestrator.step(1.0).unwrap();
    println!("frame2 writes {:?}", frame2.merged_writes);

    println!("\nBlackboard snapshot:");
    for (path, entry) in orchestrator.blackboard.iter() {
        let value_desc = match &entry.value {
            Value::Float(f) => format!("Float({f})"),
            other => format!("{:?}", other),
        };
        println!("{} => {}", path, value_desc);
    }
}
