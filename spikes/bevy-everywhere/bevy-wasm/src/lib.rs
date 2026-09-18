//! Bevy 0.19.1 on wasm32: a Vizij face GLB (with morph targets) rendered into
//! a page canvas. Mirrors vizij-rs/crates/vizij/src/view.rs: Z-up faces in the
//! XY plane, an orthographic camera fitted to the authored rootBounds, no
//! tonemapping, materials unlit with the ambient-Lambert factor baked into the
//! albedo, double-sided.
//!
//! Entry points (wasm-bindgen):
//! - `start(canvas, glb)`: one app, one camera, into `canvas`.
//! - `start_overlay(canvas, glb, n)`: one app, `n` viewport cameras into one
//!   transparent canvas; `set_rects` positions them (the three.js "multiple
//!   elements" trick).

use std::sync::Mutex;

use bevy::camera::{Projection, ScalingMode, Viewport};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::gltf::GltfAssetLabel;
use bevy::mesh::morph::MorphWeights;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use wasm_bindgen::prelude::*;

/// Authored rootBounds (center x, center y, width, height) of the two demo
/// faces, read from their RobotData extension (the `Scene` node).
fn root_bounds(glb: &str) -> (f32, f32, f32, f32) {
    if glb.contains("Toasty") {
        (0.6399, -0.0098, 3.2482, 2.2399)
    } else {
        (0.053917, 0.491355, 22.579290, 14.406971)
    }
}

#[derive(Resource, Clone)]
struct FaceOptions {
    glb: String,
    bounds: (f32, f32, f32, f32),
    /// three.js-style ambient intensity (the web renderer uses π/2).
    ambient: f32,
    cameras: usize,
    overlay: bool,
}

impl FaceOptions {
    fn albedo_factor(&self) -> f32 {
        self.ambient / std::f32::consts::PI
    }
}

/// Marks a mesh whose material has been made unique and shaded.
#[derive(Component)]
struct Shaded;

/// A view camera, by index (one per page element in overlay mode).
#[derive(Component)]
struct ViewCamera(usize);

/// CSS-pixel rects [x, y, w, h] from the page, one per camera (overlay mode).
static RECTS: Mutex<Vec<[f32; 4]>> = Mutex::new(Vec::new());

#[wasm_bindgen]
pub fn set_rects(rects: &[f32]) {
    let mut guard = RECTS.lock().unwrap();
    guard.clear();
    for r in rects.chunks_exact(4) {
        guard.push([r[0], r[1], r[2], r[3]]);
    }
}

#[wasm_bindgen]
pub fn start(canvas: String, glb: String) {
    run(canvas, glb, 1, false);
}

#[wasm_bindgen]
pub fn start_overlay(canvas: String, glb: String, cameras: usize) {
    run(canvas, glb, cameras, true);
}

fn run(canvas: String, glb: String, cameras: usize, overlay: bool) {
    let options = FaceOptions {
        bounds: root_bounds(&glb),
        glb,
        ambient: std::f32::consts::FRAC_PI_2,
        cameras,
        overlay,
    };
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Vizij face (Bevy wasm spike)".into(),
            canvas: Some(canvas),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: false,
            transparent: overlay,
            ..default()
        }),
        ..default()
    }))
    .insert_resource(options)
    // With viewport cameras, Bevy clears the window outside every viewport
    // with the global `ClearColor` (default srgb_u8(43, 44, 47)); the overlay
    // needs that region transparent.
    .insert_resource(ClearColor(if overlay { Color::NONE } else { Color::BLACK }))
    .add_systems(Startup, (setup_scene, setup_cameras))
    .add_systems(Update, (shade_materials, oscillate_morphs, apply_rects));
    // On the web the winit runner spawns the loop and returns at once.
    let exit = app.run();
    info!("App::run returned: {exit:?}");
}

fn setup_scene(mut commands: Commands, options: Res<FaceOptions>, asset_server: Res<AssetServer>) {
    commands.spawn(WorldAssetRoot(
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(options.glb.clone())),
    ));
}

fn setup_cameras(mut commands: Commands, options: Res<FaceOptions>) {
    // Lighting is baked into the materials; no scene light.
    let (cx, cy, bw, bh) = options.bounds;
    let clear = if options.overlay {
        Color::NONE
    } else {
        Color::srgb_u8(0x10, 0x11, 0x14)
    };
    for i in 0..options.cameras {
        commands.spawn((
            Camera3d::default(),
            Camera {
                clear_color: ClearColorConfig::Custom(clear),
                order: i as isize,
                ..default()
            },
            Projection::Orthographic(OrthographicProjection {
                // view.rs's `Fit::Contain`.
                scaling_mode: ScalingMode::AutoMin {
                    min_width: bw,
                    min_height: bh,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_xyz(cx, cy, 100.0).looking_at(Vec3::new(cx, cy, 0.0), Vec3::Y),
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Sample4,
            ViewCamera(i),
        ));
    }
}

/// The web renderer's material conventions, applied once per spawned mesh:
/// unique material, double-sided, unlit with the ambient factor in the albedo.
fn shade_materials(
    meshes: Query<(Entity, &MeshMaterial3d<StandardMaterial>), (With<Mesh3d>, Without<Shaded>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    options: Res<FaceOptions>,
    mut commands: Commands,
) {
    let factor = options.albedo_factor();
    for (entity, material) in &meshes {
        let mut mat = materials
            .get(&material.0)
            .cloned()
            .unwrap_or_else(StandardMaterial::default);
        mat.double_sided = true;
        mat.cull_mode = None;
        mat.unlit = true;
        let lin = mat.base_color.to_linear();
        mat.base_color = Color::LinearRgba(LinearRgba {
            red: lin.red * factor,
            green: lin.green * factor,
            blue: lin.blue * factor,
            alpha: lin.alpha,
        });
        let handle = materials.add(mat);
        commands.entity(entity).insert((MeshMaterial3d(handle), Shaded));
    }
}

/// Oscillates one morph weight per morphed node: `jawud` on the Mouth, the
/// first target elsewhere (the lids' `lidcurve` on Quori).
fn oscillate_morphs(
    time: Res<Time>,
    meshes: Res<Assets<Mesh>>,
    mut morphs: Query<(Option<&Name>, &mut MorphWeights)>,
) {
    let t = time.elapsed_secs();
    let osc = 0.5 * (1.0 + (t * 2.0).sin());
    for (name, mut weights) in &mut morphs {
        let index = meshes
            .get(weights.first_mesh().unwrap_or(&Handle::default()))
            .and_then(|mesh| mesh.morph_target_names())
            .and_then(|names| names.iter().position(|n| n == "jawud"))
            .filter(|_| name.map(|n| n.as_str() == "Mouth").unwrap_or(false))
            .unwrap_or(0);
        if let Some(slot) = weights.weights_mut().get_mut(index) {
            *slot = osc;
        }
    }
}

/// Overlay mode: each camera's viewport follows its page element's rect
/// (CSS px from `set_rects`, scaled to physical px, clamped to the canvas).
fn apply_rects(
    options: Res<FaceOptions>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&ViewCamera, &mut Camera)>,
) {
    if !options.overlay {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let scale = window.scale_factor();
    let (pw, ph) = (window.physical_width() as f32, window.physical_height() as f32);
    let rects = RECTS.lock().unwrap().clone();
    for (ViewCamera(i), mut camera) in &mut cameras {
        let Some([x, y, w, h]) = rects.get(*i).copied() else {
            camera.is_active = false;
            continue;
        };
        let x0 = (x * scale).clamp(0.0, pw);
        let y0 = (y * scale).clamp(0.0, ph);
        let x1 = ((x + w) * scale).clamp(0.0, pw);
        let y1 = ((y + h) * scale).clamp(0.0, ph);
        if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
            camera.is_active = false;
            continue;
        }
        camera.is_active = true;
        camera.viewport = Some(Viewport {
            physical_position: UVec2::new(x0 as u32, y0 as u32),
            physical_size: UVec2::new((x1 - x0) as u32, (y1 - y0) as u32),
            ..default()
        });
    }
}
