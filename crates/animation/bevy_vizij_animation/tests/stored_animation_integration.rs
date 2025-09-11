use bevy::prelude::*;
use bevy_vizij_animation::{VizijAnimationPlugin, VizijEngine, VizijTargetRoot};
use serde_json::json;
use vizij_animation_core::parse_stored_animation_json;

/// it should load a StoredAnimation (new format) and apply translation to a bound entity
#[test]
fn new_format_stored_animation_applies_translation() {
    // Minimal StoredAnimation JSON with one Vec3 track targeting Bevy's canonical translation path.
    // Duration=1000ms, stamps 0..1 -> constant [1,2,3].
    let stored_anim = json!({
      "id": "anim-const-vec3",
      "name": "ConstVec3",
      "tracks": [{
        "id": "t0",
        "name": "Translation",
        "animatableId": "node/Transform.translation",
        "points": [
          { "id": "k0", "stamp": 0.0, "value": { "x": 1, "y": 2, "z": 3 } },
          { "id": "k1", "stamp": 1.0, "value": { "x": 1, "y": 2, "z": 3 } }
        ],
        "settings": { "color": "#ffffff" }
      }],
      "groups": {},
      "transitions": {},
      "duration": 1000
    })
    .to_string();

    // Build app with the plugin
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(VizijAnimationPlugin);

    // Spawn a root and a child entity named "node" with a Transform.
    let root = app.world_mut().spawn(VizijTargetRoot).id();
    let node = app
        .world_mut()
        .spawn((
            Name::new("node"),
            Transform::default(),
            GlobalTransform::default(),
        ))
        .id();
    app.world_mut().entity_mut(root).add_child(node);

    // Parse StoredAnimation -> AnimationData and load into core engine, add instance.
    {
        let anim = parse_stored_animation_json(&stored_anim).expect("parse stored animation");
        let mut eng = app.world_mut().resource_mut::<VizijEngine>();
        let aid = eng.0.load_animation(anim);
        let pid = eng.0.create_player("p");
        let _iid = eng.0.add_instance(
            pid,
            aid,
            vizij_animation_core::engine::InstanceCfg::default(),
        );
    }

    // Run Update once to build binding index and prebind.
    app.world_mut().run_schedule(Update);
    // Run FixedUpdate to compute and apply outputs.
    app.world_mut().run_schedule(FixedUpdate);

    // Verify the Transform.translation was set to [1, 2, 3]
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
