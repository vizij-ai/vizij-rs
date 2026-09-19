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
use vizij::device::native::{start, BridgeConfig, Mode, FACE};
use vizij::device::{FaceConfig, ProgramSelect};
use vizij::view::meta::FeatureKind;
use vizij::view::snapshot::{capture, SnapshotPlugin};
use vizij::view::{self, Face, FaceAssets, ViewEvents, ViewOptions, ViewPlugin};

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

    // The view half: the face the device queued, served from memory and
    // rendered offscreen.
    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(SnapshotPlugin {
        width: WIDTH,
        height: HEIGHT,
    })
    .insert_resource(ViewEvents(std::sync::Mutex::new(device.events)))
    .insert_resource(ViewOptions {
        background: Color::BLACK,
        fit: view::Fit::Contain,
        zoom: Vec2::ONE,
        ambient: std::f32::consts::FRAC_PI_2,
        unlit: false,
    })
    .add_plugins(ViewPlugin);

    let image = capture(&mut app, WIDTH, HEIGHT, 60, view::all_faces_ready).expect("a frame");

    // The face is the device's slot, on a layer of its own, with its scene
    // joined: every animatable is a pose entry the rig holds and a binding
    // the view indexed, one of them a transform the scene carries, and every
    // mesh maps back to the RobotData element a pick would report.
    let mut faces = app.world_mut().query::<&Face>();
    let face = faces.single(app.world()).expect("one face");
    assert_eq!(face.id, FACE);
    assert_eq!(face.slot, 0, "the first face takes the first slot");
    let pose = rig.pose();
    assert!(!pose.is_empty(), "the rig holds no pose");
    let bound = pose
        .iter()
        .filter(|(path, _)| face.bindings.by_uuid.contains_key(&path.to_string()))
        .count();
    assert_eq!(
        bound,
        face.meta.animatables.len(),
        "every animatable is driven and bound"
    );
    let translated = face
        .bindings
        .by_uuid
        .values()
        .find(|(_, feature, _, _)| *feature == FeatureKind::Translation)
        .map(|(entity, _, _, _)| *entity)
        .expect("a translation binding");
    assert!(
        app.world().get::<Transform>(translated).is_some(),
        "the bound entity carries no transform"
    );
    let element_ids: std::collections::HashSet<&str> =
        face.meta.elements.iter().map(|e| e.id.as_str()).collect();
    assert!(
        !face.bindings.element_of.is_empty(),
        "no mesh maps to an element"
    );
    for element_id in face.bindings.element_of.values() {
        assert!(
            element_ids.contains(element_id.as_str()),
            "{element_id} is no element id"
        );
    }

    // And something was drawn: the frame is not the flat clear colour.
    let distinct: std::collections::HashSet<[u8; 4]> = image.pixels().map(|p| p.0).collect();
    assert!(
        distinct.len() > 8,
        "the frame is flat ({} distinct colours)",
        distinct.len()
    );
}

/// Two faces in one App, each in its own slot with its own camera, placed
/// side by side on one target: each half shows its face and nothing of the
/// other, and unloading one leaves the other.
#[test]
#[ignore = "renders on a GPU/lavapipe; run in the snapshot-regression CI job with VIZIJ_FIXTURES set"]
fn two_faces_share_one_target_each_in_its_own_viewport() {
    use vizij::view::ViewEvent;

    let Some(fixtures) = std::env::var_os("VIZIJ_FIXTURES").map(PathBuf::from) else {
        eprintln!("VIZIJ_FIXTURES unset — skipping the two-face test");
        return;
    };
    let config = || FaceConfig {
        wanted: ["rig", "pose-driver", "pose", "standard-adaptation"]
            .map(String::from)
            .to_vec(),
        program: ProgramSelect::None,
        stage_neutral: true,
        ros4hri: true,
    };
    let quori = std::fs::read(fixtures.join("Quori_Current_Extended.glb")).expect("read Quori");
    let toasty = std::fs::read(fixtures.join("Toasty_Current.glb")).expect("read Toasty");
    let left = start(&quori, config(), BridgeConfig::default(), Mode::Quiet).expect("quori");
    let right = start(&toasty, config(), BridgeConfig::default(), Mode::Quiet).expect("toasty");

    // One channel for the view; each device queued its face on its own, so
    // they are re-sent here under the ids of the two slots.
    let (events_tx, events_rx) = std::sync::mpsc::channel();
    for (id, device) in [("left", &left), ("right", &right)] {
        let Ok(ViewEvent::LoadFace { meta, glb, rig, .. }) = device.events.try_recv() else {
            panic!("the device queued no face");
        };
        events_tx
            .send(ViewEvent::LoadFace {
                face_id: id.to_string(),
                meta,
                glb,
                rig,
            })
            .unwrap();
    }
    let half = WIDTH;
    for (id, x) in [("left", 0), ("right", half)] {
        events_tx
            .send(ViewEvent::PlaceFace {
                face_id: id.to_string(),
                rect: Some([x, 0, half, HEIGHT]),
            })
            .unwrap();
    }

    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(SnapshotPlugin {
        width: 2 * half,
        height: HEIGHT,
    })
    .insert_resource(ViewEvents(std::sync::Mutex::new(events_rx)))
    .insert_resource(ViewOptions {
        background: Color::BLACK,
        fit: view::Fit::Contain,
        zoom: Vec2::ONE,
        ambient: std::f32::consts::FRAC_PI_2,
        unlit: false,
    })
    .add_plugins(ViewPlugin);

    let image = capture(&mut app, 2 * half, HEIGHT, 60, view::all_faces_ready).expect("a frame");
    let mut faces = app.world_mut().query::<&Face>();
    let slots: Vec<usize> = faces.iter(app.world()).map(|face| face.slot).collect();
    assert_eq!(slots.len(), 2);
    assert_ne!(slots[0], slots[1], "two faces, two slots");

    let colours = |x0: u32, x1: u32| -> std::collections::HashSet<[u8; 4]> {
        (x0..x1)
            .flat_map(|x| (0..HEIGHT).map(move |y| (x, y)))
            .map(|(x, y)| image.get_pixel(x, y).0)
            .collect()
    };
    let (left_colours, right_colours) = (colours(0, half), colours(half, 2 * half));
    assert!(left_colours.len() > 8, "the left half is flat");
    assert!(right_colours.len() > 8, "the right half is flat");
    // Toasty's stripes are its own; Quori's palette has none of them.
    assert!(
        left_colours.symmetric_difference(&right_colours).count() > 8,
        "the two halves show the same thing"
    );

    // Unloading one face leaves the other in place.
    events_tx
        .send(ViewEvent::UnloadFace {
            face_id: "right".into(),
        })
        .unwrap();
    for _ in 0..3 {
        app.update();
    }
    let remaining: Vec<String> = faces
        .iter(app.world())
        .map(|face| face.id.clone())
        .collect();
    assert_eq!(remaining, vec!["left".to_string()]);
}
