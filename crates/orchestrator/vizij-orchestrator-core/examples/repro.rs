use vizij_api_core::TypedPath;
use vizij_orchestrator::{
    controllers::animation::AnimationControllerConfig, controllers::graph::GraphControllerConfig,
    controllers::Subscriptions, Orchestrator, Schedule,
};

fn main() {
    let mut orchestrator = Orchestrator::new(Schedule::SinglePass);

    let graph_spec_json = serde_json::json!({
        "nodes": [
            {
                "id": "input",
                "type": "input",
                "params": {
                    "path": "demo/input/value",
                    "value": { "type": "float", "data": 0.0 }
                }
            },
            {
                "id": "gain",
                "type": "constant",
                "params": { "value": { "type": "float", "data": 1.5 } }
            },
            {
                "id": "scaled",
                "type": "multiply",
                "inputs": {
                    "a": { "node_id": "input" },
                    "b": { "node_id": "gain" }
                }
            },
            {
                "id": "offset_constant",
                "type": "constant",
                "params": { "value": { "type": "float", "data": 0.25 } }
            },
            {
                "id": "output_sum",
                "type": "add",
                "inputs": {
                    "lhs": { "node_id": "scaled" },
                    "rhs": { "node_id": "offset_constant" }
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
        inputs: vec![TypedPath::parse("demo/input/value").unwrap()],
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
            "duration": 2000,
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
            "demo/input/value",
            serde_json::json!({ "type": "float", "data": 0.5 }),
            None,
        )
        .unwrap();
    let frame = orchestrator.step(0.016).unwrap();
    println!("frame epoch {}", frame.epoch);
    println!("writes: {:?}", frame.merged_writes);
}
