use bevy::prelude::*;
use bevy_vizij_animation::{VizijAnimationPlugin, VizijEngine};

#[test]
fn plugin_inserts_engine_resource() {
    let mut app = App::new();
    // it should insert VizijEngine when the plugin is added
    app.add_plugins(MinimalPlugins)
        .add_plugins(VizijAnimationPlugin);

    assert!(app.world().get_resource::<VizijEngine>().is_some());
}

/// it should tick a FixedUpdate system that calls Engine::update without panicking
#[test]
fn fixedupdate_ticks_engine() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(VizijAnimationPlugin);

    #[derive(Resource, Default)]
    struct Ticks(u32);
    app.world_mut().insert_resource(Ticks::default());

    // System running in FixedUpdate that advances the engine and counts ticks
    fn fixed_sys(mut eng: ResMut<VizijEngine>, mut ticks: ResMut<Ticks>) {
        let _ = eng
            .0
            .update_values(1.0 / 60.0, vizij_animation_core::inputs::Inputs::default());
        ticks.0 += 1;
    }

    app.add_systems(FixedUpdate, fixed_sys);

    // Drive the FixedUpdate schedule explicitly a few times
    for _ in 0..10 {
        app.world_mut().run_schedule(FixedUpdate);
    }

    // We don't assert exact count (dependent on Bevy's fixed timestep settings) but ensure it ran.
    let ticks = app.world().get_resource::<Ticks>().unwrap();
    assert!(ticks.0 > 0);
}
