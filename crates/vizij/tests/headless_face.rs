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
use std::time::Duration;

use bevy::prelude::*;
use uuid::Uuid;
use vizij::face::{FaceConfig, ProgramSelect};
use vizij::native::{start, BridgeConfig, Mode, FACE};
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
        speech: None,
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
    let mut faces = app.world_mut().query::<&Face>();
    let face = faces.single(app.world()).expect("one face");
    assert_eq!(face.id, FACE);
    assert_eq!(face.slot, 0, "the first face takes the first slot");
    let pose = rig.pose();
    assert!(!pose.is_empty(), "the rig holds no pose");
    let bound = pose
        .iter()
        .filter(|(path, _)| {
            path.to_string()
                .parse::<Uuid>()
                .is_ok_and(|id| face.bindings.by_uuid.contains_key(&id))
        })
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
        .find(|bound| bound.feature == FeatureKind::Translation)
        .map(|bound| bound.entity)
        .expect("a translation binding");
    assert!(
        app.world().get::<Transform>(translated).is_some(),
        "the bound entity carries no transform"
    );
    let element_ids: std::collections::HashSet<Uuid> =
        face.meta.elements.iter().map(|e| e.id).collect();
    assert!(
        face.bindings.elements.values().any(|e| e.mesh.is_some()),
        "no element has a mesh"
    );
    for element_id in face.bindings.elements.keys() {
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
    use arora_hal::Hal;
    use arora_types::data::{Key, StateChange};
    use vizij::view::{Shading, Surface, ViewEvent};
    use vizij_api_core::value::{float, vec3};

    let (x, y, yaw, color) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let glb = studio_face_glb(x, y, yaw, color);
    let meta = vizij::view::meta::FaceMeta::from_glb_bytes(&glb).expect("reads");
    assert_eq!(meta.root_bounds, Some((0.0, 0.0, 1.0, 1.0)));

    let rig = vizij_arora_hal::RigHal::new();
    let mut pose = StateChange::new();
    for (id, value) in [
        (x, float(0.25)),
        (y, float(-0.125)),
        (yaw, float(0.3)),
        (color, vec3([1.0, 0.0, 0.0])),
    ] {
        pose.set.insert(Key::from(id.to_string()), Some(value));
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

    let mut faces = app.world_mut().query::<&Face>();
    let face = faces.single(app.world()).expect("one face");
    assert_eq!(face.bindings.by_uuid.len(), 4, "every animatable binds");
    let targets = *face.bindings.elements.values().next().expect("the plate");
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

/// The GLB of a one-plate face as Semio Studio exports it: an unnamed node
/// carrying a unit square, a `phong` material with three's exporter's
/// metallic factor of 0.5, and RobotData features per axis — `translation.x`
/// and `translation.y` and `rotation.y` (yaw) each on an animatable of its
/// own, `color.r/g/b` on the components of the compound animatable `color`.
fn studio_face_glb(x: Uuid, y: Uuid, yaw: Uuid, color: Uuid) -> Vec<u8> {
    let axis = |id: String| serde_json::json!({ "animated": true, "value": { "id": id, "type": "number", "default": 0 } });
    let fixed = |value: f32| serde_json::json!({ "animated": false, "value": value });
    let json = serde_json::json!({
        "asset": { "version": "2.0" },
        "extensionsUsed": ["RobotData"],
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{
            "mesh": 0,
            "extensions": { "RobotData": {
                "id": Uuid::new_v4().to_string(),
                "name": "Plate",
                "type": "shape",
                "material": "phong",
                "features": {
                    "translation.x": axis(x.to_string()),
                    "translation.y": axis(y.to_string()),
                    "translation.z": fixed(0.0),
                    "rotation.r": fixed(0.0),
                    "rotation.p": fixed(0.0),
                    "rotation.y": axis(yaw.to_string()),
                    "color.r": axis(format!("{color}.r")),
                    "color.g": axis(format!("{color}.g")),
                    "color.b": axis(format!("{color}.b")),
                    "shininess": fixed(30.0),
                }
            } }
        }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 }, "indices": 1, "material": 0 }] }],
        "materials": [{ "pbrMetallicRoughness": { "metallicFactor": 0.5, "roughnessFactor": 0.5 } }],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3",
              "min": [-0.5, -0.5, 0.0], "max": [0.5, 0.5, 0.0] },
            { "bufferView": 1, "componentType": 5123, "count": 6, "type": "SCALAR" },
        ],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": 48 },
            { "buffer": 0, "byteOffset": 48, "byteLength": 12 },
        ],
        "buffers": [{ "byteLength": 60 }],
    });
    let mut bin = Vec::new();
    for p in [
        [-0.5f32, -0.5, 0.0],
        [0.5, -0.5, 0.0],
        [0.5, 0.5, 0.0],
        [-0.5, 0.5, 0.0],
    ] {
        for c in p {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    for i in [0u16, 1, 2, 0, 2, 3] {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    glb(&json, &bin)
}

/// A GLB container of `json` and its binary chunk `bin`.
fn glb(json: &serde_json::Value, bin: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(json).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(bin);
    glb
}

/// A RobotData ellipse on a node carrying no mesh draws as its unit circle,
/// sized by its `width` and `height` and filled by its `fillColor`; its
/// stroke is not drawn.
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

    let mut faces = app.world_mut().query::<&Face>();
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
        speech: None,
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
        speech: None,
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
