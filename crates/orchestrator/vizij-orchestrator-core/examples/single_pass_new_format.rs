use serde_json::to_string_pretty;
use vizij_animation_core::{parse_stored_animation_json, InstanceCfg};
use vizij_graph_core::types::GraphSpec;
use vizij_orchestrator::{
    AnimationControllerConfig, GraphControllerConfig, Orchestrator, Schedule, Subscriptions,
};
use vizij_test_fixtures::animations;

fn main() -> anyhow::Result<()> {
    // Create orchestrator with single-pass schedule
    let mut orch = Orchestrator::new(Schedule::SinglePass);

    // Add a graph controller (empty for this example)
    let gcfg = GraphControllerConfig {
        id: "g".into(),
        spec: GraphSpec::default(),
        subs: Subscriptions::default(),
    };
    orch = orch.with_graph(gcfg);

    // Add an animation controller and obtain mutable reference to its engine
    let acfg = AnimationControllerConfig {
        id: "anim".into(),
        setup: serde_json::json!({}),
    };
    orch = orch.with_animation(acfg);

    // Load shared animation fixture (vector-pose-combo)
    let json = animations::json("vector-pose-combo")?;
    let anim_data = parse_stored_animation_json(&json).map_err(|e| anyhow::anyhow!(e))?;

    // Register animation with the engine inside the animation controller
    let anim_ctrl = orch
        .anims
        .get_mut("anim")
        .expect("animation controller present");
    let anim_id = anim_ctrl.engine.load_animation(anim_data);

    // Create a player and add an instance
    let player = anim_ctrl.engine.create_player("example-player");
    let inst_cfg = InstanceCfg::default();
    let _inst = anim_ctrl.engine.add_instance(player, anim_id, inst_cfg);

    // Step the orchestrator a few frames
    for i in 0..3 {
        let frame = orch.step(1.0 / 60.0)?;
        println!(
            "Frame {} merged_writes:\n{}",
            i,
            to_string_pretty(&frame.merged_writes)?
        );
    }

    Ok(())
}
