//! The library's two halves composed headless, the way an entry point other
//! than the desktop binary composes them: a device started from GLB bytes on
//! its worker thread, the view served the same bytes from memory and rendered
//! offscreen through `SnapshotPlugin`, the pose read through `RigHal` and
//! seen in the scene.
//!
//! `#[ignore]`d — it renders on a GPU (lavapipe in CI), so the plain
//! `cargo test` never runs it. CI's `snapshot-regression` job runs it with
//! `--ignored` and `VIZIJ_FIXTURES` pointing at the face GLBs; without
//! `VIZIJ_FIXTURES` it skips.

use std::path::PathBuf;

use bevy::prelude::*;
use vizij::device::native::{start, BridgeConfig, Mode};
use vizij::device::{FaceConfig, ProgramSelect};
use vizij::view::meta::FeatureKind;
use vizij::view::snapshot::{capture, SnapshotPlugin};
use vizij::view::{self, BindingIndex, DeviceRes, Face, FaceAssets, ViewOptions, ViewPlugin};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;

#[test]
#[ignore = "renders on a GPU/lavapipe; run in the snapshot-regression CI job with VIZIJ_FIXTURES set"]
fn a_face_composed_from_bytes_renders_the_devices_pose() {
    let Some(fixtures) = std::env::var_os("VIZIJ_FIXTURES").map(PathBuf::from) else {
        eprintln!("VIZIJ_FIXTURES unset — skipping the headless composition test");
        return;
    };
    let glb = std::fs::read(fixtures.join("Quori_Current_Extended.glb")).expect("read Quori");

    // The device half: quiet (no bridge, no front end), the neutral pose
    // staged, no program so the pose is the authored neutral.
    let config = FaceConfig {
        wanted: ["rig", "pose-driver", "pose", "standard-adaptation"]
            .map(String::from)
            .to_vec(),
        program: ProgramSelect::None,
        stage_neutral: true,
        ros4hri: true,
    };
    let device = start(&glb, config, BridgeConfig::default(), Mode::Quiet).expect("start");
    let rig = device.rig.clone();

    // The view half: the same bytes served from memory, rendered offscreen.
    let mut app = App::new();
    let assets = FaceAssets::register(&mut app);
    let asset_path = assets.push(view::face_name(&device.meta), glb);
    app.add_plugins(SnapshotPlugin {
        width: WIDTH,
        height: HEIGHT,
    })
    .insert_resource(Face {
        meta: device.meta.clone(),
        asset_path,
    })
    .insert_resource(DeviceRes {
        rig: device.rig.clone(),
    })
    .insert_resource(ViewOptions {
        background: Color::BLACK,
        fit: view::Fit::Contain,
        zoom: Vec2::ONE,
        ambient: std::f32::consts::FRAC_PI_2,
        unlit: false,
    })
    .add_plugins(ViewPlugin);

    let image = capture(&mut app, WIDTH, HEIGHT, 60, |app| {
        app.world()
            .get_resource::<BindingIndex>()
            .map(|index| index.ready)
            .unwrap_or(false)
    })
    .expect("a frame");

    // The device drove a pose: every animatable the rig holds is a binding
    // the view indexed, and at least one is a transform the scene carries.
    let pose = rig.pose();
    assert!(!pose.is_empty(), "the rig holds no pose");
    let index = app.world().resource::<BindingIndex>();
    let bound = pose
        .iter()
        .filter(|(path, _)| index.by_uuid.contains_key(&path.to_string()))
        .count();
    assert!(bound > 0, "no pose entry maps onto a scene binding");
    let translated = index
        .by_uuid
        .values()
        .find(|(_, feature, _, _)| *feature == FeatureKind::Translation)
        .map(|(entity, _, _, _)| *entity)
        .expect("a translation binding");
    assert!(
        app.world().get::<Transform>(translated).is_some(),
        "the bound entity carries no transform"
    );

    // And something was drawn: the frame is not the flat clear colour.
    let distinct: std::collections::HashSet<[u8; 4]> = image.pixels().map(|p| p.0).collect();
    assert!(
        distinct.len() > 8,
        "the frame is flat ({} distinct colours)",
        distinct.len()
    );
}
