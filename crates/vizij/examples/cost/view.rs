//! The view as the page runs it: one App on one thread, no pipelined
//! rendering (the browser has none), drawing offscreen.

use std::sync::mpsc::Sender;
use std::time::Duration;

use bevy::prelude::App;
use vizij::view::meta::FaceMeta;
use vizij::view::ViewEvent;
use vizij_arora_hal::RigHal;

/// The size of the offscreen target the view draws.
pub const WIDTH: u32 = 763;
pub const HEIGHT: u32 = 760;

/// The view with each of `faces` (its meta and the rig its device drives)
/// loaded from `glb` and ready, side by side when there are several, and the
/// sender of its events. Every system runs on the calling thread, as in the
/// browser, so that thread's count is the whole frame's.
pub fn view_app(glb: &[u8], faces: &[(&FaceMeta, &RigHal)]) -> (App, Sender<ViewEvent>) {
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::asset::{AssetMetaCheck, AssetPlugin};
    use bevy::prelude::*;
    use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
    use bevy::render::render_resource::TextureUsages;
    use bevy::render::RenderPlugin;
    use bevy::window::ExitCondition;
    use bevy::winit::WinitPlugin;
    use vizij::view::snapshot::{ensure_ready, FORMAT};
    use vizij::view::{self, FaceAssets, OffscreenTarget, ViewEvents, ViewOptions, ViewPlugin};

    let (tx, rx) = std::sync::mpsc::channel();
    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                ..default()
            })
            .set(AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<PipelinedRenderingPlugin>()
            .disable::<bevy::log::LogPlugin>(),
    )
    .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    let mut target = Image::new_target_texture(WIDTH, HEIGHT, FORMAT, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let handle = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    app.insert_resource(OffscreenTarget(handle))
        .insert_resource(ViewEvents(std::sync::Mutex::new(rx)))
        .insert_resource(ViewOptions {
            background: Color::BLACK,
            fit: view::Fit::Contain,
            zoom: Vec2::ONE,
            ambient: std::f32::consts::FRAC_PI_2,
            unlit: false,
        })
        .add_plugins(ViewPlugin);
    for (i, (meta, rig)) in faces.iter().enumerate() {
        tx.send(ViewEvent::LoadFace {
            face_id: format!("face{i}"),
            meta: Box::new((*meta).clone()),
            glb: glb.to_vec(),
            rig: (*rig).clone(),
        })
        .unwrap();
        if faces.len() > 1 {
            let width = WIDTH as i32 / faces.len() as i32;
            tx.send(ViewEvent::PlaceFace {
                face_id: format!("face{i}"),
                rect: Some(IRect::new(
                    i as i32 * width,
                    0,
                    (i as i32 + 1) * width,
                    HEIGHT as i32,
                )),
            })
            .unwrap();
        }
    }
    ensure_ready(&mut app);
    for _ in 0..600 {
        app.update();
        if view::all_faces_ready(&mut app) {
            break;
        }
    }
    assert!(view::all_faces_ready(&mut app), "faces never got ready");
    let single = |world: &mut World| {
        for (_, schedule) in world.resource_mut::<Schedules>().iter_mut() {
            schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
        }
    };
    single(app.world_mut());
    single(app.sub_app_mut(bevy::render::RenderApp).world_mut());
    (app, tx)
}
