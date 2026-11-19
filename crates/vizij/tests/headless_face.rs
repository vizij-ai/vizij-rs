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

mod common;

use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use uuid::Uuid;
use vizij::face::{FaceConfig, ProgramSelect};
use vizij::native::{start, BridgeConfig, Mode, FACE};
use vizij::view::meta::FeatureKind;
use vizij::view::snapshot::{capture, SnapshotPlugin};
use vizij::view::{self, EntityInstance, Face, FaceAssets, ViewEvents, ViewOptions, ViewPlugin};

use common::{glb, plate_face_glb, studio_face_glb};

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
        #[cfg(feature = "studio")]
        studio: false,
        speech: None,
        frame_rate: None,
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
    // element a pick would report is a RobotData element.
    let mut faces = app.world_mut().query::<(&EntityInstance, &Face)>();
    let (instance, face) = faces.single(app.world()).expect("one face");
    assert_eq!(instance.id, FACE);
    assert_eq!(face.slot, 0, "the first face takes the first slot");
    let pose = rig.pose();
    assert!(!pose.is_empty(), "the rig holds no pose");
    let bound = pose
        .iter()
        .filter(|(path, _)| {
            instance
                .bindings
                .by_id
                .contains_key(path.to_string().as_str())
        })
        .count();
    assert_eq!(
        bound,
        instance.meta.animatables.len(),
        "every animatable is driven and bound"
    );
    let translated = instance
        .bindings
        .by_id
        .values()
        .flatten()
        .find(|bound| bound.feature == FeatureKind::Translation)
        .map(|bound| bound.entity)
        .expect("a translation binding");
    assert!(
        app.world().get::<Transform>(translated).is_some(),
        "the bound entity carries no transform"
    );
    // InnerFace and the four lids share a material, and its colour.
    assert_eq!(
        instance.bindings.by_id["c627bc47-95bb-4a89-a67c-e6a6e76bef8d"].len(),
        5,
        "the shared colour lands on each element that shares it"
    );
    let element_ids: std::collections::HashSet<&String> =
        instance.meta.elements.iter().map(|e| &e.id).collect();
    assert!(
        instance
            .bindings
            .elements
            .values()
            .any(|e| e.mesh.is_some()),
        "no element has a mesh"
    );
    for element_id in instance.bindings.elements.keys() {
        assert!(
            element_ids.contains(element_id),
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

/// A face exported from Semio Studio: RobotData features per axis, on an
/// unnamed node, with a `phong` material, and no `rootBounds`. Its axes move
/// the node one component at a time, a compound animatable split per axis
/// colors it whole, and the phong material is shaded without the metallic
/// factor its GLB carries.
#[test]
#[ignore = "renders on a GPU/lavapipe; run in the snapshot-regression CI job"]
fn a_studio_exported_face_renders_its_per_axis_pose() {
    let element = Uuid::new_v4().to_string();
    let ids = [(); 4].map(|_| Uuid::new_v4().to_string());
    renders_its_per_axis_pose(&element, ids);
}

/// The same face with RobotData ids that are no UUIDs — an element `plate`,
/// animatables `plate_x` … and the compound `plate_color` split per axis —
/// loads, binds every animatable under its id and draws its pose.
#[test]
#[ignore = "renders on a GPU/lavapipe; run in the snapshot-regression CI job"]
fn a_face_whose_ids_are_plain_strings_renders_its_pose() {
    let ids = ["plate_x", "plate_y", "plate_yaw", "plate_color"].map(String::from);
    renders_its_per_axis_pose("plate", ids);
}

/// The plate face with element id `element` and animatable ids
/// `[x, y, yaw, color]`, its pose driven through them, rendered.
fn renders_its_per_axis_pose(element: &str, [x, y, yaw, color]: [String; 4]) {
    use arora_hal::Hal;
    use arora_types::data::{Key, StateChange};
    use vizij::view::{Shading, Surface, ViewEvent};
    use vizij_api_core::value::{float, vec3};

    let glb = plate_face_glb(
        element,
        [x.clone(), y.clone(), yaw.clone(), color.clone()],
        None,
    );
    let meta = vizij::view::meta::FaceMeta::from_glb_bytes(&glb).expect("reads");
    assert_eq!(
        meta.elements
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        [element]
    );
    assert_eq!(meta.root_bounds, Some((0.0, 0.0, 1.0, 1.0)));

    let rig = vizij_arora_hal::RigHal::new();
    let mut pose = StateChange::new();
    for (id, value) in [
        (&x, float(0.25)),
        (&y, float(-0.125)),
        (&yaw, float(0.3)),
        (&color, vec3([1.0, 0.0, 0.0])),
    ] {
        pose.set.insert(Key::from(id.as_str()), Some(value));
    }
    rig.try_send(&pose);

    let (events_tx, events_rx) = std::sync::mpsc::channel();
    events_tx
        .send(ViewEvent::LoadFace {
            face_id: "studio".into(),
            meta: Box::new(meta),
            glb,
            rig,
        })
        .unwrap();
    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(SnapshotPlugin {
        width: WIDTH,
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
    let image = capture(&mut app, WIDTH, HEIGHT, 60, view::all_faces_ready).expect("a frame");

    let mut faces = app.world_mut().query::<&EntityInstance>();
    let face = faces.single(app.world()).expect("one face");
    assert_eq!(face.bindings.by_id.len(), 4, "every animatable binds");
    for id in [&x, &y, &yaw, &color] {
        assert!(face.bindings.by_id.contains_key(id.as_str()), "{id} binds");
    }
    let targets = *face.bindings.elements.get(element).expect("the plate");
    let transform = app.world().get::<Transform>(targets.node).expect("a node");
    assert_eq!(transform.translation, Vec3::new(0.25, -0.125, 0.0));
    assert!(transform
        .rotation
        .abs_diff_eq(Quat::from_rotation_z(0.3), 1e-6));
    let surface = app
        .world()
        .get::<Surface>(targets.mesh.expect("a mesh"))
        .expect("a surface");
    assert_eq!(surface.shading, Shading::Diffuse);
    assert_eq!(surface.base, [1.0, 0.0, 0.0]);

    // Diffuse red under the ambient factor: linear 0.5, sRGB 188.
    let red = image
        .pixels()
        .filter(|p| p.0[0].abs_diff(188) <= 2 && p.0[1] < 8 && p.0[2] < 8)
        .count();
    assert!(red > 100, "the plate is not drawn red ({red} pixels)");
}

/// A RobotData ellipse on a node carrying no mesh draws as its unit circle,
/// sized by its `width` and `height` and filled by its `fillColor`; its
/// stroke is not drawn.
/// A face whose camera frames none of its meshes has nothing to wait for:
/// it is ready as soon as it is indexed and drawn, its target showing the
/// background.
#[test]
#[ignore = "renders on a GPU/lavapipe; run in the snapshot-regression CI job"]
fn a_face_framing_none_of_its_meshes_is_ready() {
    use vizij::view::{FaceView, ViewEvent};

    let glb = studio_face_glb(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        None,
    );
    let meta = vizij::view::meta::FaceMeta::from_glb_bytes(&glb).expect("reads");
    let (events_tx, events_rx) = std::sync::mpsc::channel();
    // The framed rectangle lies far above the unit plate, inside the face's
    // slot.
    events_tx
        .send(ViewEvent::SetFaceView {
            face_id: "away".into(),
            view: FaceView {
                bounds: Some((0.0, 400.0, 1.0, 1.0)),
                ..Default::default()
            },
        })
        .unwrap();
    events_tx
        .send(ViewEvent::LoadFace {
            face_id: "away".into(),
            meta: Box::new(meta),
            glb,
            rig: vizij_arora_hal::RigHal::new(),
        })
        .unwrap();
    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(SnapshotPlugin {
        width: WIDTH,
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
    vizij::view::snapshot::ensure_ready(&mut app);

    // Well under the frames after which a face held back is made ready.
    for _ in 0..60 {
        app.update();
        if view::all_faces_ready(&mut app) {
            return;
        }
    }
    panic!("a face with no mesh in view was not ready within 60 frames");
}

#[test]
#[ignore = "renders on a GPU/lavapipe; run in the snapshot-regression CI job"]
fn an_ellipse_draws_as_its_unit_circle() {
    use vizij::view::{Shading, Surface, ViewEvent};

    let fixed = |value: serde_json::Value| serde_json::json!({ "animated": false, "value": value });
    let json = serde_json::json!({
        "asset": { "version": "2.0" },
        "extensionsUsed": ["RobotData"],
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{
            "name": "Eye",
            "extensions": { "RobotData": {
                "id": Uuid::new_v4().to_string(),
                "name": "Eye",
                "type": "ellipse",
                "features": {
                    "translation": fixed(serde_json::json!({ "x": 0, "y": 0, "z": 0 })),
                    "rotation": fixed(serde_json::json!({ "x": 0, "y": 0, "z": 0 })),
                    "width": fixed(serde_json::json!(0.8)),
                    "height": fixed(serde_json::json!(0.4)),
                    "fillColor": fixed(serde_json::json!({ "r": 1, "g": 0, "b": 0 })),
                    "strokeColor": fixed(serde_json::json!({ "r": 0, "g": 1, "b": 0 })),
                    "strokeWidth": fixed(serde_json::json!(4)),
                }
            } }
        }],
    });
    let glb = glb(&json, &[]);
    let meta = vizij::view::meta::FaceMeta::from_glb_bytes(&glb).expect("reads");

    let (events_tx, events_rx) = std::sync::mpsc::channel();
    events_tx
        .send(ViewEvent::LoadFace {
            face_id: "ellipse".into(),
            meta: Box::new(meta),
            glb,
            rig: vizij_arora_hal::RigHal::new(),
        })
        .unwrap();
    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(SnapshotPlugin {
        width: WIDTH,
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
    let image = capture(&mut app, WIDTH, HEIGHT, 60, view::all_faces_ready).expect("a frame");

    let mut faces = app.world_mut().query::<&EntityInstance>();
    let face = faces.single(app.world()).expect("one face");
    let targets = *face.bindings.elements.values().next().expect("the eye");
    let scale = app
        .world()
        .get::<Transform>(targets.node)
        .expect("a node")
        .scale;
    assert_eq!(scale, Vec3::new(0.8, 0.4, 1.0));
    let mesh = targets.mesh.expect("a unit circle");
    let surface = app.world().get::<Surface>(mesh).expect("a surface");
    assert_eq!(surface.shading, Shading::Standard);
    assert_eq!(surface.base, [1.0, 0.0, 0.0]);

    // The default 5×4 bounds on 320×200 draw 50 px a unit: the ellipse is
    // 80×40 px, about 2500 px of diffuse red (linear 0.5, sRGB 188), and
    // nothing green.
    let red = image
        .pixels()
        .filter(|p| p.0[0].abs_diff(188) <= 2 && p.0[1] < 8 && p.0[2] < 8)
        .count();
    assert!((2300..2700).contains(&red), "{red} red pixels");
    assert!(image.pixels().all(|p| p.0[1] < 8), "a stroke is drawn");
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
        #[cfg(feature = "studio")]
        studio: false,
        speech: None,
        frame_rate: None,
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
                rect: Some(IRect::new(x as i32, 0, (x + half) as i32, HEIGHT as i32)),
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
    let mut faces = app.world_mut().query::<(&EntityInstance, &Face)>();
    let slots: Vec<usize> = faces.iter(app.world()).map(|(_, face)| face.slot).collect();
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
        .map(|(instance, _)| instance.id.clone())
        .collect();
    assert_eq!(remaining, vec!["left".to_string()]);
}

/// Loading and unloading a face over and over leaves nothing behind: once
/// the cycles are done and the App has settled, it holds no more entities,
/// meshes, images or materials than the cycles' floor. A cycle's census can
/// read high — a texture the asset thread has yet to drop, the first
/// cycle's face still being torn down behind the loader's own work — never
/// low, so the floor over the cycles is the steady state, and a leak is what
/// the end holds above it.
#[test]
#[ignore = "renders on a GPU/lavapipe; run in the snapshot-regression CI job with VIZIJ_FIXTURES set"]
fn load_unload_cycles_leave_nothing_behind() {
    use vizij::view::ViewEvent;

    let Some(fixtures) = std::env::var_os("VIZIJ_FIXTURES").map(PathBuf::from) else {
        eprintln!("VIZIJ_FIXTURES unset — skipping the cycle test");
        return;
    };
    let glb = std::fs::read(fixtures.join("Quori_Current_Extended.glb")).expect("read Quori");
    let config = FaceConfig {
        wanted: ["rig", "pose-driver", "pose", "standard-adaptation"]
            .map(String::from)
            .to_vec(),
        program: ProgramSelect::None,
        stage_neutral: true,
        ros4hri: true,
        #[cfg(feature = "studio")]
        studio: false,
        speech: None,
        frame_rate: None,
    };
    let device = start(&glb, config, BridgeConfig::default(), Mode::Quiet).expect("start");
    let (events_tx, events_rx) = std::sync::mpsc::channel();

    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(SnapshotPlugin {
        width: WIDTH,
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
    vizij::view::snapshot::ensure_ready(&mut app);

    fn census(app: &mut App) -> (u32, usize, usize, usize) {
        let world = app.world_mut();
        (
            world.entities().len(),
            world.resource::<Assets<Mesh>>().len(),
            world.resource::<Assets<Image>>().len(),
            world.resource::<Assets<StandardMaterial>>().len(),
        )
    }
    /// Frames until the census has held still for ten frames and `quiet`
    /// (bounded by `bound`, so a leak fails the comparison, not the wait).
    fn settle(app: &mut App, quiet: Duration, bound: Duration) -> (u32, usize, usize, usize) {
        let mut now = census(app);
        let mut still = 0;
        let mut changed = std::time::Instant::now();
        let start = std::time::Instant::now();
        while start.elapsed() < bound {
            app.update();
            let next = census(app);
            if next == now {
                still += 1;
            } else {
                still = 0;
                changed = std::time::Instant::now();
            }
            now = next;
            if still >= 10 && changed.elapsed() >= quiet {
                break;
            }
        }
        now
    }
    let mut floor: Option<(u32, usize, usize, usize)> = None;
    for cycle in 0..25 {
        events_tx
            .send(ViewEvent::LoadFace {
                face_id: "cycle".into(),
                meta: Box::new(device.meta.clone()),
                glb: glb.clone(),
                rig: device.rig.clone(),
            })
            .unwrap();
        for _ in 0..600 {
            app.update();
            if view::all_faces_ready(&mut app) {
                break;
            }
        }
        assert!(
            view::all_faces_ready(&mut app),
            "cycle {cycle}: the face never got ready"
        );
        events_tx
            .send(ViewEvent::UnloadFace {
                face_id: "cycle".into(),
            })
            .unwrap();
        // Despawns and asset drops settle over frames; the census is read
        // once it has held still for ten frames and a quarter second
        // (bounded), and kept for the record.
        let now = settle(&mut app, Duration::from_millis(250), Duration::from_secs(5));
        eprintln!("cycle {cycle}: entities/meshes/images/materials {now:?}");
        floor = Some(match floor {
            None => now,
            Some(f) => (
                f.0.min(now.0),
                f.1.min(now.1),
                f.2.min(now.2),
                f.3.min(now.3),
            ),
        });
    }
    // The end: still for a full second before the count that matters.
    let last = settle(&mut app, Duration::from_secs(1), Duration::from_secs(15));
    assert_eq!(last, floor.unwrap(), "the cycles left something behind");
}

/// A face comes into view only once it is indexed, so a load or a reload
/// never draws the GLB as it spawned, unposed and unshaded; and it draws on
/// the target only once it is ready, its camera under the clear until then,
/// so the target never shows a face whose pipelines are still compiling. A
/// reload's scene spawns behind the face it replaces and takes the target in
/// the frame that face retires; every frame in between the target shows the
/// old face alone.
#[test]
#[ignore = "renders on a GPU/lavapipe; run in the snapshot-regression CI job with VIZIJ_FIXTURES set"]
fn a_face_draws_on_its_target_only_once_ready() {
    use vizij::view::{Superseded, ViewEvent};

    let Some(fixtures) = std::env::var_os("VIZIJ_FIXTURES").map(PathBuf::from) else {
        eprintln!("VIZIJ_FIXTURES unset — skipping the indexed-draw test");
        return;
    };
    let glb = std::fs::read(fixtures.join("Quori_Current_Extended.glb")).expect("read Quori");
    let config = FaceConfig {
        wanted: ["rig", "pose-driver", "pose", "standard-adaptation"]
            .map(String::from)
            .to_vec(),
        program: ProgramSelect::None,
        stage_neutral: true,
        ros4hri: true,
        #[cfg(feature = "studio")]
        studio: false,
        speech: None,
        frame_rate: None,
    };
    let device = start(&glb, config, BridgeConfig::default(), Mode::Quiet).expect("start");
    let (events_tx, events_rx) = std::sync::mpsc::channel();

    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(SnapshotPlugin {
        width: WIDTH,
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
    vizij::view::snapshot::ensure_ready(&mut app);

    let load = || ViewEvent::LoadFace {
        face_id: "face".into(),
        meta: Box::new(device.meta.clone()),
        glb: glb.clone(),
        rig: device.rig.clone(),
    };
    /// One face as a frame drew it.
    #[derive(Debug)]
    struct Drawn {
        indexed: bool,
        ready: bool,
        superseded: bool,
        visible: bool,
        /// Whether its camera draws over the clear: on the target.
        on_target: bool,
    }
    fn drawn(app: &mut App) -> Vec<Drawn> {
        let world = app.world_mut();
        let mut faces = world.query::<(&EntityInstance, Has<Superseded>, &InheritedVisibility)>();
        let mut cameras = world.query::<&Camera>();
        faces
            .iter(world)
            .map(|(face, superseded, visible)| Drawn {
                indexed: face.bindings.indexed,
                ready: face.ready,
                superseded,
                visible: visible.get(),
                on_target: cameras.get(world, face.camera).unwrap().order > view::CLEAR_ORDER,
            })
            .collect()
    }
    // Frames until one face is left and it is ready, checking each frame
    // that only an indexed face was in view, only a ready one on the
    // target, and the old face alone on it while its successor was not
    // ready. The frames it took: until the new face was indexed, and until
    // it was ready.
    let frames_until_swapped = |app: &mut App, what: &str| {
        let mut indexed = None;
        for frame in 0..600 {
            app.update();
            let faces = drawn(app);
            if indexed.is_none() && faces.iter().any(|face| face.indexed && !face.ready) {
                indexed = Some(frame);
            }
            for face in &faces {
                assert!(
                    face.indexed || !face.visible,
                    "{what}, frame {frame}: a face was drawn before it was indexed ({faces:?})"
                );
                assert_eq!(
                    face.on_target, face.ready,
                    "{what}, frame {frame}: a face was on the target unless ready ({faces:?})"
                );
                if face.superseded {
                    assert!(
                        face.visible && face.on_target,
                        "{what}, frame {frame}: the face being replaced left the target"
                    );
                }
            }
            if let [Drawn {
                ready: true,
                superseded: false,
                visible: true,
                ..
            }] = faces[..]
            {
                return (indexed.unwrap_or(frame), frame);
            }
        }
        panic!("{what}: the face never came into view alone");
    };

    events_tx.send(load()).unwrap();
    let (indexed, ready) = frames_until_swapped(&mut app, "the load");
    eprintln!("the load: indexed after {indexed} frames, ready after {ready}");
    for reload in 0..3 {
        events_tx.send(load()).unwrap();
        let (indexed, ready) = frames_until_swapped(&mut app, &format!("reload {reload}"));
        eprintln!("reload {reload}: indexed after {indexed} frames, ready after {ready}");
    }
}

/// A caption written to the face standard's caption keys shows over the
/// face, at the bottom of its rectangle: someone else's in white, the face's
/// own (`__system__`) in its blue once the first has had its time.
#[cfg(feature = "captions")]
#[test]
#[ignore = "renders on a GPU/lavapipe; run in the snapshot-regression CI job"]
fn a_caption_shows_over_the_face() {
    use arora_hal::Hal;
    use arora_types::data::{Key, StateChange};
    use vizij::view::ViewEvent;
    use vizij_api_core::Value;
    use vizij_arora_host::standard::{CAPTION_SPEAKER, CAPTION_TEXT};

    const W: u32 = 640;
    const H: u32 = 400;
    let ids = [(); 4].map(|_| Uuid::new_v4());
    let glb = studio_face_glb(ids[0], ids[1], ids[2], ids[3], None);
    let meta = vizij::view::meta::FaceMeta::from_glb_bytes(&glb).expect("reads");
    let rig = vizij_arora_hal::RigHal::new();
    let (events_tx, events_rx) = std::sync::mpsc::channel();
    events_tx
        .send(ViewEvent::LoadFace {
            face_id: "captioned".into(),
            meta: Box::new(meta),
            glb,
            rig: rig.clone(),
        })
        .unwrap();
    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(SnapshotPlugin {
        width: W,
        height: H,
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

    // The pixels of the caption band (the bottom quarter) close to `rgb`.
    let in_band = |image: &image::RgbaImage, rgb: [u8; 3]| {
        image
            .enumerate_pixels()
            .filter(|(_, y, p)| *y > H * 3 / 4 && (0..3).all(|c| p.0[c].abs_diff(rgb[c]) <= 24))
            .count()
    };
    let caption = |text: &str, speaker: &str| {
        let mut change = StateChange::new();
        change
            .set
            .insert(Key::from(CAPTION_TEXT), Some(Value::String(text.into())));
        change.set.insert(
            Key::from(CAPTION_SPEAKER),
            Some(Value::String(speaker.into())),
        );
        rig.try_send(&change);
    };
    const WHITE: [u8; 3] = [255, 255, 255];
    const BLUE: [u8; 3] = [0, 102, 179];

    let bare = capture(&mut app, W, H, 10, view::all_faces_ready).expect("a frame");
    assert_eq!(in_band(&bare, WHITE), 0, "no caption yet");

    caption("Hello, face.", "person_1");
    let shown = capture(&mut app, W, H, 10, view::all_faces_ready).expect("a frame");
    let white = in_band(&shown, WHITE);
    assert!(
        white > 50,
        "the caption is not drawn ({white} white pixels)"
    );
    let above: usize = shown
        .enumerate_pixels()
        .filter(|(_, y, p)| *y <= H * 3 / 4 && p.0[..3] == WHITE)
        .count();
    assert_eq!(above, 0, "the caption stays in its band");

    // Queued behind the first, which holds for 2 s plus its last-chunk 1.5 s.
    // The captions advance on the virtual clock, which caps a frame's step at
    // `max_delta`: a slow renderer's frames take longer than that, so the wait
    // counts the virtual clock's seconds, not the wall clock's.
    caption("And hello back.", "__system__");
    let virtual_now = |app: &App| app.world().resource::<Time<Virtual>>().elapsed();
    let start = virtual_now(&app);
    while virtual_now(&app) - start < Duration::from_millis(3700) {
        app.update();
    }
    let own = capture(&mut app, W, H, 2, view::all_faces_ready).expect("a frame");
    let blue = in_band(&own, BLUE);
    assert!(
        blue > 50,
        "the face's own caption is not blue ({blue} pixels)"
    );
    assert_eq!(in_band(&own, WHITE), 0, "the first caption is gone");
}

/// A selected element of Quori is outlined in the selection red around it
/// and nowhere else, and a face with nothing selected — before any selection
/// and once it is cleared — draws no outline at all. The same holds for the
/// face placed on a rectangle reaching past the target's corner, where its
/// camera draws into a viewport through a sub-view: the outline's mask is
/// drawn through the same lens, so the line stays on the element.
#[test]
#[ignore = "renders on a GPU/lavapipe; run in the snapshot-regression CI job with VIZIJ_FIXTURES set"]
fn a_selected_element_is_outlined_around_it_alone() {
    use vizij::view::ViewEvent;

    let Some(fixtures) = std::env::var_os("VIZIJ_FIXTURES").map(PathBuf::from) else {
        eprintln!("VIZIJ_FIXTURES unset — skipping the outline test");
        return;
    };
    let glb = std::fs::read(fixtures.join("Quori_Current_Extended.glb")).expect("read Quori");
    let meta = vizij::view::meta::FaceMeta::from_glb_bytes(&glb).expect("reads");
    let (events_tx, events_rx) = std::sync::mpsc::channel();
    events_tx
        .send(ViewEvent::LoadFace {
            face_id: "quori".into(),
            meta: Box::new(meta),
            glb,
            rig: vizij_arora_hal::RigHal::new(),
        })
        .unwrap();
    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(SnapshotPlugin {
        width: OUTLINED_W,
        height: OUTLINED_H,
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
    let select = |elements: Vec<String>| {
        events_tx
            .send(ViewEvent::SetSelection {
                face_id: "quori".into(),
                elements,
            })
            .unwrap();
    };

    // Over the whole target.
    let bare = outlined_capture(&mut app);
    let (element, rect) = mid_sized_element(&mut app);
    select(vec![element.clone()]);
    let selected = outlined_capture(&mut app);
    if let Some(out) = std::env::var_os("VIZIJ_OUTLINE_PNG") {
        vizij::view::snapshot::save_png(&selected, std::path::Path::new(&out)).expect("saved");
    }
    assert_outlined_alone(&bare, &selected, &element, rect);
    select(Vec::new());
    let cleared = outlined_capture(&mut app);
    assert_eq!(
        changed_pixels(&bare, &cleared, Rect::EMPTY).1.len(),
        0,
        "a cleared selection leaves the frame changed"
    );

    // Placed past the target's top and right edges: a viewport and a
    // sub-view of the whole rectangle's framing.
    events_tx
        .send(ViewEvent::PlaceFace {
            face_id: "quori".into(),
            rect: Some(IRect::new(140, -70, 140 + 420, -70 + 260)),
        })
        .unwrap();
    let bare = outlined_capture(&mut app);
    let (element, rect) = mid_sized_element(&mut app);
    select(vec![element.clone()]);
    let selected = outlined_capture(&mut app);
    if let Some(out) = std::env::var_os("VIZIJ_OUTLINE_PLACED_PNG") {
        vizij::view::snapshot::save_png(&selected, std::path::Path::new(&out)).expect("saved");
    }
    assert_outlined_alone(&bare, &selected, &element, rect);
}

/// A click on a face picks the element under it, over the face's own
/// bindings, and a click on the face's empty space is its miss.
///
/// The pick takes only cameras drawing to the window the pointer is in, so
/// once the face is ready its camera is pointed at a window entity of the
/// image's size — one with no surface, which draws nothing — and the press
/// and release are delivered to that window as Bevy's picking delivers them.
#[test]
#[ignore = "renders on a GPU/lavapipe; run in the snapshot-regression CI job with VIZIJ_FIXTURES set"]
fn a_click_on_a_face_picks_the_element_under_it() {
    use bevy::camera::{NormalizedRenderTarget, RenderTarget};
    use bevy::picking::backend::HitData;
    use bevy::picking::events::{Pointer, PointerPress, PointerRelease};
    use bevy::picking::pointer::{Location, PointerButton, PointerId};
    use bevy::window::{WindowRef, WindowResolution};
    use vizij::view::{Picked, ViewEvent};

    let Some(fixtures) = std::env::var_os("VIZIJ_FIXTURES").map(PathBuf::from) else {
        eprintln!("VIZIJ_FIXTURES unset — skipping the pick test");
        return;
    };
    let glb = std::fs::read(fixtures.join("Quori_Current_Extended.glb")).expect("read Quori");
    let meta = vizij::view::meta::FaceMeta::from_glb_bytes(&glb).expect("reads");
    let (events_tx, events_rx) = std::sync::mpsc::channel();
    events_tx
        .send(ViewEvent::LoadFace {
            face_id: "quori".into(),
            meta: Box::new(meta),
            glb,
            rig: vizij_arora_hal::RigHal::new(),
        })
        .unwrap();
    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(SnapshotPlugin {
        width: OUTLINED_W,
        height: OUTLINED_H,
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
    outlined_capture(&mut app);
    let (element, rect) = mid_sized_element(&mut app);
    // The point nearest the element's middle that is clear of every smaller
    // element drawn over it — an eye's highlight in front of the eye.
    let others = drawn_elements(&mut app);
    let mut points: Vec<Vec2> = (1..10)
        .flat_map(|i| (1..10).map(move |j| Vec2::new(i as f32, j as f32) / 10.0))
        .map(|at| rect.min + at * rect.size())
        .collect();
    points.sort_by(|a, b| {
        let centre = rect.center();
        a.distance(centre).total_cmp(&b.distance(centre))
    });
    let on_element = points
        .into_iter()
        .find(|point| {
            others.iter().all(|(id, other)| {
                *id == element
                    || other.size().length() >= rect.size().length()
                    || !other.inflate(2.0).contains(*point)
            })
        })
        .expect("a point on the element alone");

    let window = app
        .world_mut()
        .spawn(Window {
            resolution: WindowResolution::new(OUTLINED_W, OUTLINED_H)
                .with_scale_factor_override(1.0),
            ..default()
        })
        .id();
    let mut faces = app.world_mut().query::<&EntityInstance>();
    let camera = faces.single(app.world()).expect("one face").camera;
    app.world_mut()
        .entity_mut(camera)
        .insert(RenderTarget::Window(WindowRef::Entity(window)));
    app.update();
    app.update();

    let target = NormalizedRenderTarget::Window(
        WindowRef::Entity(window).normalize(None).expect("a window"),
    );
    let at = |position: Vec2| {
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: target.clone(),
                position,
            },
        )
    };
    let click = |app: &mut App, position: Vec2| -> Vec<Picked> {
        let world = app.world_mut();
        world.trigger(PointerPress {
            entity: window,
            pointer: at(position),
            button: PointerButton::Primary,
            hit: HitData::new(window, 0.0, None, None),
            count: 1,
        });
        world.trigger(PointerRelease {
            entity: window,
            pointer: at(position),
            button: PointerButton::Primary,
            hit: HitData::new(window, 0.0, None, None),
        });
        world.flush();
        world.resource_mut::<Messages<Picked>>().drain().collect()
    };

    let picked = click(&mut app, on_element);
    assert_eq!(picked.len(), 1, "one click, one pick: {picked:?}");
    assert_eq!(picked[0].instance_id, "quori");
    assert_eq!(picked[0].element_id.as_deref(), Some(element.as_str()));

    let missed = click(&mut app, Vec2::new(4.0, 4.0));
    assert_eq!(missed.len(), 1, "one click, one miss: {missed:?}");
    assert_eq!(missed[0].instance_id, "quori");
    assert_eq!(missed[0].element_id, None, "a click on no element");
}

const OUTLINED_W: u32 = 480;
const OUTLINED_H: u32 = 300;

fn outlined_capture(app: &mut App) -> image::RgbaImage {
    capture(app, OUTLINED_W, OUTLINED_H, 5, view::all_faces_ready).expect("a frame")
}

/// The first element, by id, whose mesh is drawn mid-sized and well inside
/// the image, so an outline has room on every side and the rest of the image
/// is plainly elsewhere; with where it is drawn, in image pixels.
fn mid_sized_element(app: &mut App) -> (String, Rect) {
    let image = Vec2::new(OUTLINED_W as f32, OUTLINED_H as f32);
    let mut candidates: Vec<(String, Rect)> = drawn_elements(app)
        .into_iter()
        .filter(|(_, rect)| {
            let size = rect.size();
            size.cmpgt(Vec2::splat(20.0)).all()
                && size.cmplt(image / 3.0).all()
                && rect.min.cmpgt(Vec2::splat(12.0)).all()
                && rect.max.cmplt(image - 12.0).all()
        })
        .collect();
    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    candidates.into_iter().next().expect("a mid-sized element")
}

/// Every element with a mesh of its own, with where its mesh's bounding box
/// is drawn, in image pixels.
fn drawn_elements(app: &mut App) -> Vec<(String, Rect)> {
    use bevy::camera::primitives::Aabb;

    let mut faces = app.world_mut().query::<&EntityInstance>();
    let (face_camera, elements) = {
        let face = faces.single(app.world()).expect("one face");
        (face.camera, face.bindings.elements.clone())
    };
    let mut cameras = app.world_mut().query::<(&Camera, &GlobalTransform)>();
    let (camera, at) = cameras.get(app.world(), face_camera).expect("a camera");
    // The mesh's bounding box carried through the face's camera.
    let on_image = |mesh: Entity| -> Option<Rect> {
        let aabb = app.world().get::<Aabb>(mesh)?;
        let world = app.world().get::<GlobalTransform>(mesh)?.affine();
        let (centre, half) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
        let mut rect: Option<Rect> = None;
        for corner in 0..8 {
            let sign = Vec3::new(
                if corner & 1 == 0 { -1.0 } else { 1.0 },
                if corner & 2 == 0 { -1.0 } else { 1.0 },
                if corner & 4 == 0 { -1.0 } else { 1.0 },
            );
            let point = world.transform_point3(centre + half * sign);
            let seen = camera.world_to_viewport(at, point).ok()?;
            rect = Some(match rect {
                Some(rect) => rect.union_point(seen),
                None => Rect::from_corners(seen, seen),
            });
        }
        rect
    };
    elements
        .iter()
        .filter_map(|(id, targets)| Some((id.clone(), on_image(targets.mesh?)?)))
        .collect()
}

/// The pixels `after` changed from `before`: how many of those within
/// `reach` are the selection red (sRGB 255, 16, 16 at 0.9, over whatever is
/// under it), and where every other changed pixel is.
fn changed_pixels(
    before: &image::RgbaImage,
    after: &image::RgbaImage,
    reach: Rect,
) -> (usize, Vec<(u32, u32)>) {
    let red = |p: &image::Rgba<u8>| p.0[0] > 150 && p.0[1] < 90 && p.0[2] < 90;
    let mut around = 0;
    let mut elsewhere = Vec::new();
    for (x, y, now) in after.enumerate_pixels() {
        let was = before.get_pixel(x, y);
        if (0..3).all(|channel| was.0[channel].abs_diff(now.0[channel]) <= 8) {
            continue;
        }
        if reach.contains(Vec2::new(x as f32 + 0.5, y as f32 + 0.5)) {
            around += usize::from(red(now));
        } else {
            elsewhere.push((x, y));
        }
    }
    (around, elsewhere)
}

/// `selected` differs from `bare` by an outline in the selection red around
/// `rect`, where `element` is drawn, and by nothing anywhere else.
fn assert_outlined_alone(
    bare: &image::RgbaImage,
    selected: &image::RgbaImage,
    element: &str,
    rect: Rect,
) {
    // The line is 3 logical pixels wide at a scale factor of 1; its soft
    // outer edge and the box's rounding take a pixel or two more.
    let (around, elsewhere) = changed_pixels(bare, selected, rect.inflate(6.0));
    assert!(
        around > 40,
        "{element} is not outlined ({around} red pixels around {rect:?})"
    );
    assert!(
        elsewhere.is_empty(),
        "the outline of {element} reaches beyond it: {} pixels, first {:?}",
        elsewhere.len(),
        elsewhere.first()
    );
}
