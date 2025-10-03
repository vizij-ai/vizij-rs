use bevy::prelude::*;
use bevy_vizij_animation::{VizijAnimationPlugin, VizijEngine, VizijTargetRoot};

/// it should apply constant translation from fixture to a bound entity deterministically
#[test]
fn integration_parity_const_translation_applied() {
    // Build app with the plugin
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(VizijAnimationPlugin);

    // Spawn a root and a child entity named "node" with a Transform.
    // BindingIndex will register "node/Transform.translation" etc.
    let root = app.world_mut().spawn(VizijTargetRoot).id();
    let node = app
        .world_mut()
        .spawn((
            Name::new("node"),
            Transform::default(),
            GlobalTransform::default(),
        ))
        .id();
    // Make node a child of root so traversal under VizijTargetRoot finds it.
    app.world_mut().entity_mut(root).add_child(node);

    // Load the constant Vec3 animation fixture into the core engine and add an instance.
    {
        let json_str = vizij_test_fixtures::animations::json("constant-vec3")
            .expect("load constant-vec3 fixture");
        let anim = vizij_animation_core::parse_stored_animation_json(&json_str)
            .expect("parse constant-vec3 fixture");

        let mut eng = app.world_mut().resource_mut::<VizijEngine>();
        let aid = eng.0.load_animation(anim);
        let pid = eng.0.create_player("p");
        let _iid = eng.0.add_instance(
            pid,
            aid,
            vizij_animation_core::engine::InstanceCfg::default(),
        );
    }

    // Run Update once to build the binding index and prebind.
    app.world_mut().run_schedule(Update);

    // Run FixedUpdate once to compute and apply outputs.
    app.world_mut().run_schedule(FixedUpdate);

    // Verify the Transform.translation was set to [1, 2, 3] from the fixture.
    let tf = app
        .world()
        .get::<Transform>(node)
        .expect("Transform exists");
    let expected = Vec3::new(1.0, 2.0, 3.0);
    assert!(
        (tf.translation - expected).length() <= 1e-5,
        "expected translation {:?}, got {:?}",
        expected,
        tf.translation
    );
}
