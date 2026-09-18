//! The head: Bevy systems rendering the face and applying the device's pose.
//!
//! Scene model matches the web renderer (`@vizij/render`): Z-up world, faces
//! in the XY plane layered along Z, orthographic camera fit to the authored
//! `rootBounds`, ambient-only lighting, sRGB output with no tonemapping,
//! double-sided materials, opacity-driven alpha blending.
//!
//! A face enters as GLB bytes: [`meta`] reads the bindings and the bundle
//! from them, and the scene loads them through the in-memory asset source
//! ([`FaceAssets`]), so the view never touches a file system — the same path
//! on desktop, in the browser and on Android. [`snapshot`] renders offscreen
//! and reads the pixels back; [`frames`] publishes what is rendered into the
//! device's store.

pub mod frames;
pub mod meta;
pub mod snapshot;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::AssetSourceBuilder;
use bevy::asset::AssetApp;
use bevy::camera::{
    CameraProjection, Projection, RenderTarget, ScalingMode, SubCameraView, Viewport,
};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::gltf::GltfAssetLabel;
use bevy::math::Vec3A;
use bevy::mesh::morph::MorphWeights;
use bevy::picking::events::{Click, Pointer};
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::prelude::*;
use vizij_api_core::value::{as_bool, as_color_rgba, as_float, as_vec3, as_vector};
use vizij_api_core::Value;

use meta::{Binding, FaceMeta, FeatureKind};

/// The in-memory asset source the faces' GLBs are served from: a loaded
/// face's bytes live here under a path of their own until the face is
/// unloaded, and its scene loads `mem://<path>`. Registered before the asset
/// plugin ([`FaceAssets::register`]).
#[derive(Resource, Clone)]
pub struct FaceAssets {
    dir: Dir,
    /// Paths are never reused: the asset server caches a handle by its path,
    /// and a face pushed under a path another face had would load stale.
    pushes: Arc<AtomicU32>,
}

impl FaceAssets {
    /// The asset source's name, the scheme of every face's asset path.
    pub const SOURCE: &'static str = "mem";

    /// Register the `mem` source on `app` — before `DefaultPlugins`, which the
    /// asset plugin's construction requires — and keep the handle as a
    /// resource. The returned clone pushes the first face.
    pub fn register(app: &mut App) -> Self {
        let assets = Self {
            dir: Dir::default(),
            pushes: Arc::new(AtomicU32::new(0)),
        };
        let dir = assets.dir.clone();
        app.register_asset_source(
            Self::SOURCE,
            AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
        );
        app.insert_resource(assets.clone());
        assets
    }

    /// Serve `glb` under a fresh path named after `name`; the path to load
    /// the scene from.
    pub fn push(&self, name: &str, glb: Vec<u8>) -> String {
        let n = self.pushes.fetch_add(1, Ordering::SeqCst);
        let path = format!("{name}-{n}.glb");
        self.dir.insert_asset(Path::new(&path), glb);
        path
    }

    /// Stop serving the GLB at `path`.
    pub fn remove(&self, path: &str) {
        self.dir.remove_asset(Path::new(path));
    }

    /// The scene asset of the GLB served at `path`.
    fn scene(path: &str) -> bevy::asset::AssetPath<'static> {
        GltfAssetLabel::Scene(0).from_asset(format!("{}://{path}", Self::SOURCE))
    }
}

/// The face the view shows, as a Bevy resource./// A face shown by the view: an entity carrying the metadata, the GLB served
/// from memory, the device's rig feed, its slot and the joins to the spawned
/// scene once indexed. Its scene is a child; its camera is its own
/// entity ([`FaceCamera`]).
#[derive(Component)]
pub struct Face {
    /// The slot name the face was loaded under — what a front end names it
    /// by, and what a pick reports.
    pub id: String,
    pub meta: FaceMeta,
    /// Where its GLB is served from ([`FaceAssets::push`]).
    pub asset_path: String,
    /// The device driving it; the view reads its pose each frame.
    pub rig: vizij_arora_hal::RigHal,
    /// The slot the face occupies in the world: faces stand [`SLOT_STRIDE`]
    /// apart along X, each with its camera over it, so a camera sees only
    /// its own face and a pointer ray from it meets no other.
    pub slot: usize,
    /// The joins to the spawned scene, once every element's node exists.
    pub bindings: Bindings,
    /// Its camera.
    pub camera: Entity,
}

/// The joins from a face's animatables to its spawned scene: built once the
/// GLB scene has spawned, empty until then.
#[derive(Default)]
pub struct Bindings {
    /// uuid → (target entity, feature, morph index, material shade factor).
    pub by_uuid: HashMap<String, (Entity, FeatureKind, Option<usize>, f32)>,
    /// mesh entity → the id of the element it belongs to, for picks.
    pub element_of: HashMap<Entity, String>,
    pub ready: bool,
}

/// A face's camera: orthographic over the face's authored bounds, over the
/// face's slot.
#[derive(Component)]
pub struct FaceCamera(pub Entity);

/// A change the view applies, sent by whatever fronts the device.
pub enum ViewEvent {
    /// Show a face under `face_id`: its metadata, its GLB bytes and the rig
    /// feed of the device driving it. A face already shown under that id is
    /// replaced — the device restarted on new graphs, the view swaps over.
    LoadFace {
        face_id: String,
        meta: Box<FaceMeta>,
        glb: Vec<u8>,
        rig: vizij_arora_hal::RigHal,
    },
    /// Take the face down: its scene, its camera, its GLB.
    UnloadFace { face_id: String },
    /// Confine the face's camera to a rectangle of the target, in physical
    /// pixels (`[x, y, width, height]`, origin top-left); `None` gives it the
    /// whole target again. How several faces share one window or canvas.
    PlaceFace {
        face_id: String,
        rect: Option<[u32; 4]>,
    },
    /// A new background color, as sRGB bytes.
    Background([u8; 3]),
}

/// The front ends' requests, drained each frame ([`ViewEvent`]).
#[derive(Resource)]
pub struct ViewEvents(pub Mutex<Receiver<ViewEvent>>);

/// An element a pointer clicked, by the ids its face's RobotData declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picked {
    pub face_id: String,
    pub element_id: String,
}

/// The picks since a consumer last drained them.
#[derive(Resource, Default)]
pub struct Picks(pub Vec<Picked>);

/// Where each face's camera draws, by face id ([`ViewEvent::PlaceFace`]);
/// a face without an entry, or with `None`, draws over the whole target. Kept
/// apart from the faces so a placement survives the face's reload.
#[derive(Resource, Default)]
struct Placements(HashMap<String, Option<[u32; 4]>>);

/// Whether every requested face is shown and indexed — and at least one is.
/// What a snapshot waits for before it captures.
pub fn all_faces_ready(app: &mut App) -> bool {
    let mut faces = app.world_mut().query::<&Face>();
    let mut any = false;
    for face in faces.iter(app.world()) {
        if !face.bindings.ready {
            return false;
        }
        any = true;
    }
    any
}

/// How the camera fits the face's authored rootBounds into the viewport.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "desktop", derive(clap::ValueEnum))]
pub enum Fit {
    /// The whole bounds stay visible; the window's excess axis shows
    /// background — the web renderer's behavior.
    Contain,
    /// The bounds fill the window; the excess axis is cropped — for a full
    /// screen whose aspect ratio the face does not control.
    Cover,
    /// The bounds fill the window on both axes, each scaled on its own — the
    /// face distorts to the window's aspect ratio instead of being cropped or
    /// letterboxed.
    Stretch,
}

/// View options (the CLI's rendering knobs).
#[derive(Resource, Clone)]
pub struct ViewOptions {
    /// Background clear color.
    pub background: Color,
    /// How the face fits the window.
    pub fit: Fit,
    /// Magnification applied after the fit, per axis (x = width, y = height),
    /// about the face's center; 1 is the bare fit. Positive.
    pub zoom: Vec2,
    /// three.js-style ambient light intensity (the web uses π/2). Materials
    /// render unlit with their albedo scaled by `intensity/π` in linear space
    /// — exactly the web's ambient-Lambert pipeline (verified pixel-exact
    /// against the web renderer on Quori).
    pub ambient: f32,
    /// Render pure albedo (equivalent to ambient = π).
    pub unlit: bool,
}

impl ViewOptions {
    /// The linear-space albedo factor implementing the ambient model.
    pub fn albedo_factor(&self) -> f32 {
        if self.unlit {
            1.0
        } else {
            self.ambient / std::f32::consts::PI
        }
    }
}

/// Scales a color's linear RGB by the ambient factor, keeping alpha.
fn shade(color: Color, factor: f32) -> Color {
    let lin = color.to_linear();
    Color::LinearRgba(LinearRgba {
        red: lin.red * factor,
        green: lin.green * factor,
        blue: lin.blue * factor,
        alpha: lin.alpha,
    })
}

/// When present, the view camera renders into this offscreen image instead of
/// a window (snapshot mode).
/// When present, every face camera renders into this offscreen image
/// instead of a window (snapshot and headless modes).
#[derive(Resource, Clone)]
pub struct OffscreenTarget(pub Handle<Image>);

/// The distance between two faces' slots along X. A face spans a few units
/// (its authored bounds), so slots this far apart never share a camera's
/// frustum or a pointer ray.
///
/// Slots, not render layers: a mesh carrying `RenderLayers` renders its
/// morph targets at zero weight on Bevy 0.19.1's storage-buffer morph path
/// (every desktop GPU; WebGL2 takes the uniform path and does not show it),
/// which closes Toasty's eyes. Spacing the faces apart needs nothing of the
/// renderer.
pub const SLOT_STRIDE: f32 = 1000.0;

/// The slots in use, so an unloaded face's slot goes back to the pool.
#[derive(Resource, Default)]
struct Slots(Vec<usize>);

impl Slots {
    fn take(&mut self) -> usize {
        let slot = (0..)
            .find(|slot| !self.0.contains(slot))
            .expect("a free slot");
        self.0.push(slot);
        slot
    }

    fn release(&mut self, slot: usize) {
        self.0.retain(|used| *used != slot);
    }
}

/// Where a slot's face stands.
fn slot_origin(slot: usize) -> Vec3 {
    Vec3::new(slot as f32 * SLOT_STRIDE, 0.0, 0.0)
}

pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Slots>()
            .init_resource::<Placements>()
            .init_resource::<Picks>()
            .add_plugins(MeshPickingPlugin)
            .add_observer(on_click)
            .add_systems(
                Update,
                (apply_view_events, place_cameras, index_faces, apply_poses).chain(),
            );
    }
}

/// Apply the front ends' requests: load, replace or unload a face, or
/// recolor the background of every face camera.
#[allow(clippy::too_many_arguments)]
fn apply_view_events(
    events: Option<Res<ViewEvents>>,
    assets: Res<FaceAssets>,
    options: Res<ViewOptions>,
    offscreen: Option<Res<OffscreenTarget>>,
    mut frame_config: Option<ResMut<frames::FrameConfig>>,
    mut slots: ResMut<Slots>,
    mut placements: ResMut<Placements>,
    mut commands: Commands,
    faces: Query<(Entity, &Face)>,
    mut cameras: Query<&mut Camera, With<FaceCamera>>,
    asset_server: Res<AssetServer>,
) {
    let Some(events) = events else { return };
    let Ok(receiver) = events.0.lock() else {
        return;
    };
    while let Ok(event) = receiver.try_recv() {
        match event {
            ViewEvent::Background([r, g, b]) => {
                let background = Color::srgb_u8(r, g, b);
                for mut camera in &mut cameras {
                    if let ClearColorConfig::Custom(color) = &mut camera.clear_color {
                        *color = background;
                    }
                }
            }
            ViewEvent::LoadFace {
                face_id,
                meta,
                glb,
                rig,
            } => {
                unload(&face_id, &faces, &assets, &mut slots, &mut commands);
                let slot = slots.take();
                let asset_path = assets.push(&face_id, glb);
                // The published frames are stamped in the loaded face's own
                // frame, so they follow the face.
                if let Some(config) = frame_config.as_mut() {
                    config.face_frame_id = frames::default_frame_id(&meta);
                }
                log::info!(
                    "face {face_id}: loading {} ({} elements) in slot {slot}",
                    meta.bundle.face_id.as_deref().unwrap_or("an unnamed face"),
                    meta.elements.len()
                );
                let camera = commands
                    .spawn(camera_for(
                        &meta,
                        &options,
                        slot,
                        offscreen.as_deref(),
                        faces.is_empty(),
                    ))
                    .id();
                let face = commands
                    .spawn((
                        Face {
                            id: face_id,
                            meta: *meta,
                            asset_path: asset_path.clone(),
                            rig,
                            slot,
                            bindings: Bindings::default(),
                            camera,
                        },
                        Transform::from_translation(slot_origin(slot)),
                        Visibility::default(),
                        children![WorldAssetRoot(
                            asset_server.load(FaceAssets::scene(&asset_path))
                        )],
                    ))
                    .id();
                commands.entity(camera).insert(FaceCamera(face));
            }
            ViewEvent::UnloadFace { face_id } => {
                unload(&face_id, &faces, &assets, &mut slots, &mut commands);
            }
            ViewEvent::PlaceFace { face_id, rect } => {
                placements.0.insert(face_id, rect);
            }
        }
    }
}

/// Confine each face's camera to its placement, when it has one.
fn place_cameras(
    placements: Res<Placements>,
    faces: Query<&Face>,
    mut cameras: Query<&mut Camera, With<FaceCamera>>,
) {
    for face in &faces {
        let Ok(mut camera) = cameras.get_mut(face.camera) else {
            continue;
        };
        let wanted = placements
            .0
            .get(&face.id)
            .copied()
            .flatten()
            .map(|[x, y, width, height]| Viewport {
                physical_position: UVec2::new(x, y),
                physical_size: UVec2::new(width.max(1), height.max(1)),
                ..default()
            });
        let same = match (&camera.viewport, &wanted) {
            (None, None) => true,
            (Some(current), Some(wanted)) => {
                current.physical_position == wanted.physical_position
                    && current.physical_size == wanted.physical_size
            }
            _ => false,
        };
        if !same {
            camera.viewport = wanted;
        }
    }
}

/// Take the face shown under `face_id` down, if any: its scene, its camera,
/// its GLB, its slot.
fn unload(
    face_id: &str,
    faces: &Query<(Entity, &Face)>,
    assets: &FaceAssets,
    slots: &mut Slots,
    commands: &mut Commands,
) {
    for (entity, face) in faces {
        if face.id == face_id {
            commands.entity(face.camera).despawn();
            commands.entity(entity).despawn();
            assets.remove(&face.asset_path);
            slots.release(face.slot);
            log::info!("face {face_id}: unloaded");
        }
    }
}

/// A face's camera: the fit over its authored bounds, over its slot; the
/// first camera clears the target with the background, the others draw over
/// it.
fn camera_for(
    meta: &FaceMeta,
    options: &ViewOptions,
    slot: usize,
    offscreen: Option<&OffscreenTarget>,
    clears: bool,
) -> impl Bundle {
    // Lighting is baked into the materials (see `ViewOptions::albedo_factor`);
    // no scene light is spawned.
    let (projection, mut transform) = camera_fit(meta, options);
    transform.translation += slot_origin(slot);
    (
        Camera3d::default(),
        Camera {
            clear_color: if clears {
                ClearColorConfig::Custom(options.background)
            } else {
                ClearColorConfig::None
            },
            order: slot as isize,
            ..default()
        },
        // The render target is its own component since Bevy 0.17.
        match offscreen {
            Some(target) => RenderTarget::Image(target.0.clone().into()),
            None => RenderTarget::default(),
        },
        projection,
        transform,
        Tonemapping::None,
        DebandDither::Disabled,
        Msaa::Sample4,
    )
}

fn camera_fit(meta: &FaceMeta, options: &ViewOptions) -> (Projection, Transform) {
    let (cx, cy, bw, bh) = meta.root_bounds.unwrap_or((0.0, 0.0, 5.0, 4.0));
    let projection = Projection::custom(FitProjection {
        bounds: Vec2::new(bw, bh),
        fit: options.fit,
        zoom: options.zoom,
        ortho: OrthographicProjection::default_3d(),
    });
    (
        projection,
        Transform::from_xyz(cx, cy, 100.0).looking_at(Vec3::new(cx, cy, 0.0), Vec3::Y),
    )
}

/// The view camera's projection: orthographic, showing the face's authored
/// bounds fitted to the viewport under a [`Fit`] policy, then magnified by a
/// per-axis zoom.
///
/// A custom projection rather than an [`OrthographicProjection`] scaling mode
/// because the zoom applies *after* the fit and per axis: the fit picks which
/// bounds axis spans the viewport from the unzoomed bounds, and no
/// [`ScalingMode`] takes a second, anisotropic factor on top of that choice.
/// Bevy hands a custom projection the viewport size through
/// [`CameraProjection::update`] whenever it would recompute a built-in one, so
/// the extent is right on the first frame, on every resize, and after a face
/// swap. The mesh pipeline files it as a nonstandard projection, which only
/// routes its depth↔view-z shader helpers through the general matrix path —
/// the same numbers for an orthographic matrix.
#[derive(Debug, Clone)]
struct FitProjection {
    /// The authored rootBounds size (width, height), in world units.
    bounds: Vec2,
    fit: Fit,
    /// Magnification per axis, applied after the fit.
    zoom: Vec2,
    /// Holds the extent as a `Fixed` scaling mode and does the matrix work.
    ortho: OrthographicProjection,
}

impl CameraProjection for FitProjection {
    fn get_clip_from_view(&self) -> Mat4 {
        self.ortho.get_clip_from_view()
    }

    fn get_clip_from_view_for_sub(&self, sub_view: &SubCameraView) -> Mat4 {
        self.ortho.get_clip_from_view_for_sub(sub_view)
    }

    fn update(&mut self, width: f32, height: f32) {
        let extent = visible_extent(self.bounds, self.fit, self.zoom, width, height);
        self.ortho.scaling_mode = ScalingMode::Fixed {
            width: extent.x,
            height: extent.y,
        };
        self.ortho.update(width, height);
    }

    fn far(&self) -> f32 {
        self.ortho.far()
    }

    fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        self.ortho.get_frustum_corners(z_near, z_far)
    }
}

/// The world-space extent a `width`×`height` viewport shows of a face whose
/// authored bounds are `bounds`: the bounds fitted to the viewport's aspect
/// under `fit`, divided by the per-axis `zoom` (a zoom above 1 magnifies).
///
/// Contain and cover repeat Bevy's `AutoMin` / `AutoMax` arithmetic operation
/// for operation, so the default view renders bit-identically to those modes
/// — the snapshot references were rendered with them.
fn visible_extent(bounds: Vec2, fit: Fit, zoom: Vec2, width: f32, height: f32) -> Vec2 {
    // The bounds' height spanning the viewport, or their width.
    let by_height = Vec2::new(width * bounds.y / height, bounds.y);
    let by_width = Vec2::new(bounds.x, height * bounds.x / width);
    let fitted = match fit {
        Fit::Contain => {
            if width * bounds.y > bounds.x * height {
                by_height
            } else {
                by_width
            }
        }
        Fit::Cover => {
            if width * bounds.y < bounds.x * height {
                by_height
            } else {
                by_width
            }
        }
        Fit::Stretch => bounds,
    };
    fitted / zoom
}

/// Joins each spawned GLB scene with its face's RobotData bindings: node
/// `Name` → entity, for material/morph features the mesh primitive child.
/// Also makes each bound mesh's material unique (GLB materials can be shared)
/// and applies the web renderer's material conventions. Names are looked up
/// under the face's own root, so two faces with the same node names never
/// cross.
#[allow(clippy::too_many_arguments)]
fn index_faces(
    mut faces: Query<(Entity, &mut Face)>,
    options: Res<ViewOptions>,
    names: Query<&Name>,
    children: Query<&Children>,
    meshes: Query<(Entity, &MeshMaterial3d<StandardMaterial>), With<Mesh3d>>,
    morphs: Query<Entity, With<MorphWeights>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for (root, mut face) in &mut faces {
        if face.bindings.ready {
            continue;
        }
        // Wait until every element's node has spawned. Names can collide:
        // the world spawner's own root carries the glTF *scene's* name, which
        // exporters often also give a top-level *node* (Toasty's "Scene").
        // The element is always the innermost bearer, so on a collision keep
        // the deepest entity.
        let mut by_name: HashMap<String, (Entity, usize)> = HashMap::new();
        let mut stack = vec![(root, 0usize)];
        while let Some((entity, depth)) = stack.pop() {
            if let Ok(name) = names.get(entity) {
                let slot = by_name
                    .entry(name.as_str().to_string())
                    .or_insert((entity, depth));
                if depth > slot.1 {
                    *slot = (entity, depth);
                }
            }
            if let Ok(kids) = children.get(entity) {
                stack.extend(kids.iter().map(|kid| (kid, depth + 1)));
            }
        }
        if !face
            .meta
            .elements
            .iter()
            .all(|e| by_name.contains_key(e.node_name.as_str()))
        {
            continue;
        }

        // Per element: the transform target is the named node entity; the
        // material/morph target is its first mesh-bearing descendant.
        let mut mesh_of: HashMap<String, (Entity, Handle<StandardMaterial>)> = HashMap::new();
        let mut morph_of: HashMap<String, Entity> = HashMap::new();
        let mut element_of = HashMap::new();
        for element in &face.meta.elements {
            let node = by_name[element.node_name.as_str()].0;
            // three's MeshBasicMaterial ignores lights: full albedo. `standard`
            // gets the ambient-Lambert factor.
            let factor = if element.material.as_deref() == Some("basic") {
                1.0
            } else {
                options.albedo_factor()
            };
            for descendant in std::iter::once(node).chain(children.iter_descendants(node)) {
                if let Ok((mesh_entity, material)) = meshes.get(descendant) {
                    // Unique material per element, with web conventions applied:
                    // double-sided, unlit with the ambient factor baked into the
                    // albedo (the web's ambient-Lambert model, computed in linear).
                    let mut mat = materials
                        .get(&material.0)
                        .cloned()
                        .unwrap_or_else(StandardMaterial::default);
                    mat.double_sided = true;
                    mat.cull_mode = None;
                    mat.unlit = true;
                    mat.base_color = shade(mat.base_color, factor);
                    let handle = materials.add(mat);
                    commands
                        .entity(mesh_entity)
                        .insert(MeshMaterial3d(handle.clone()));
                    mesh_of.insert(element.node_name.clone(), (mesh_entity, handle));
                    element_of.insert(mesh_entity, element.id.clone());
                    break;
                }
            }
            for descendant in std::iter::once(node).chain(children.iter_descendants(node)) {
                if morphs.get(descendant).is_ok() {
                    morph_of.insert(element.node_name.clone(), descendant);
                    break;
                }
            }
        }

        let mut by_uuid = HashMap::new();
        for (uuid, Binding { node_name, feature }) in &face.meta.animatables {
            let Some(&(node, _)) = by_name.get(node_name.as_str()) else {
                continue;
            };
            let element = face
                .meta
                .elements
                .iter()
                .find(|e| &e.node_name == node_name);
            let factor = if element.and_then(|e| e.material.as_deref()) == Some("basic") {
                1.0
            } else {
                options.albedo_factor()
            };
            let entry = match feature {
                FeatureKind::Translation | FeatureKind::Rotation | FeatureKind::Scale => {
                    (node, feature.clone(), None, factor)
                }
                FeatureKind::Color | FeatureKind::Opacity => {
                    let Some((mesh_entity, _)) = mesh_of.get(node_name) else {
                        continue;
                    };
                    (*mesh_entity, feature.clone(), None, factor)
                }
                FeatureKind::Morph(target) => {
                    let Some(&morph_entity) = morph_of.get(node_name) else {
                        continue;
                    };
                    let Some(index) =
                        element.and_then(|e| e.morph_targets.iter().position(|m| m == target))
                    else {
                        continue;
                    };
                    (morph_entity, feature.clone(), Some(index), factor)
                }
            };
            by_uuid.insert(uuid.clone(), entry);
        }

        log::info!(
            "face {}: scene indexed, {} bindings over {} elements",
            face.id,
            by_uuid.len(),
            face.meta.elements.len(),
        );
        face.bindings = Bindings {
            by_uuid,
            element_of,
            ready: true,
        };
    }
}

/// Applies each device's current pose (the HAL's actuation state) onto its
/// face's scene: transforms, material color/opacity, morph influences.
fn apply_poses(
    faces: Query<&Face>,
    mut transforms: Query<&mut Transform>,
    material_handles: Query<&MeshMaterial3d<StandardMaterial>>,
    mut morph_weights: Query<&mut MorphWeights>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for face in &faces {
        if !face.bindings.ready {
            continue;
        }
        for (path, value) in face.rig.pose() {
            let key = path.to_string();
            let Some((entity, feature, morph_index, factor)) = face.bindings.by_uuid.get(&key)
            else {
                continue;
            };
            match feature {
                FeatureKind::Translation => {
                    if let (Ok(mut transform), Some(v)) =
                        (transforms.get_mut(*entity), as_xyz(&value))
                    {
                        transform.translation = Vec3::from_array(v);
                    }
                }
                FeatureKind::Rotation => {
                    if let (Ok(mut transform), Some(v)) =
                        (transforms.get_mut(*entity), as_xyz(&value))
                    {
                        // three.js euler order ZYX: R = Rz·Ry·Rx, composed
                        // explicitly — EulerRot naming conventions moved between
                        // glam versions, this cannot. Validated pixel-wise against
                        // the web renderer on Toasty, whose tilts are
                        // order-sensitive.
                        transform.rotation = Quat::from_rotation_z(v[2])
                            * Quat::from_rotation_y(v[1])
                            * Quat::from_rotation_x(v[0]);
                    }
                }
                FeatureKind::Scale => {
                    if let Ok(mut transform) = transforms.get_mut(*entity) {
                        if let Some(v) = as_xyz(&value) {
                            transform.scale = Vec3::from_array(v);
                        } else if let Some(s) = as_f32(&value) {
                            transform.scale = Vec3::splat(s);
                        }
                    }
                }
                FeatureKind::Color => {
                    if let (Ok(handle), Some([r, g, b])) =
                        (material_handles.get(*entity), as_rgb(&value))
                    {
                        if let Some(mut mat) = materials.get_mut(&handle.0) {
                            let alpha = mat.base_color.alpha();
                            // Graph color components are linear working-space
                            // floats (three's `Color.setRGB` semantics), not sRGB.
                            let shaded = shade(Color::linear_rgb(r, g, b), *factor);
                            mat.base_color = shaded.with_alpha(alpha);
                        }
                    }
                }
                FeatureKind::Opacity => {
                    if let (Ok(handle), Some(o)) = (material_handles.get(*entity), as_f32(&value)) {
                        if let Some(mut mat) = materials.get_mut(&handle.0) {
                            mat.base_color.set_alpha(o);
                            mat.alpha_mode = if o < 1.0 {
                                AlphaMode::Blend
                            } else {
                                AlphaMode::Opaque
                            };
                        }
                    }
                }
                FeatureKind::Morph(_) => {
                    if let (Ok(mut weights), Some(w), Some(i)) =
                        (morph_weights.get_mut(*entity), as_f32(&value), *morph_index)
                    {
                        if let Some(slot) = weights.weights_mut().get_mut(i) {
                            *slot = w;
                        }
                    }
                }
            }
        }
    }
}

/// A click on a face's mesh, reported as the RobotData ids of the face and
/// the element the mesh belongs to. The event bubbles up the hierarchy; it is
/// recorded once, at its origin.
fn on_click(
    click: On<Pointer<Click>>,
    parents: Query<&ChildOf>,
    faces: Query<&Face>,
    mut picks: ResMut<Picks>,
) {
    let target = click.original_event_target();
    if click.entity != target {
        return;
    }
    let mut current = target;
    let face = loop {
        if let Ok(face) = faces.get(current) {
            break face;
        }
        match parents.get(current) {
            Ok(parent) => current = parent.parent(),
            Err(_) => return,
        }
    };
    if let Some(element_id) = face.bindings.element_of.get(&target) {
        log::info!("face {}: picked element {element_id}", face.id);
        picks.0.push(Picked {
            face_id: face.id.clone(),
            element_id: element_id.clone(),
        });
    }
}

fn as_f32(value: &Value) -> Option<f32> {
    as_float(value).or_else(|| as_bool(value).map(|b| if b { 1.0 } else { 0.0 }))
}

fn as_xyz(value: &Value) -> Option<[f32; 3]> {
    as_vec3(value)
        .or_else(|| as_vector(value).and_then(|v| (v.len() >= 3).then(|| [v[0], v[1], v[2]])))
}

fn as_rgb(value: &Value) -> Option<[f32; 3]> {
    as_color_rgba(value)
        .map(|c| [c[0], c[1], c[2]])
        .or_else(|| as_xyz(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: Vec2 = Vec2::new(4.0, 2.0);
    const NO_ZOOM: Vec2 = Vec2::ONE;

    #[test]
    fn contain_letterboxes_the_viewports_excess_axis() {
        // Wider than the bounds: their height spans, the width shows margins.
        assert_eq!(
            visible_extent(BOUNDS, Fit::Contain, NO_ZOOM, 800.0, 200.0),
            Vec2::new(8.0, 2.0)
        );
        // Taller: their width spans.
        assert_eq!(
            visible_extent(BOUNDS, Fit::Contain, NO_ZOOM, 400.0, 400.0),
            Vec2::new(4.0, 4.0)
        );
    }

    #[test]
    fn cover_crops_the_viewports_excess_axis() {
        assert_eq!(
            visible_extent(BOUNDS, Fit::Cover, NO_ZOOM, 800.0, 200.0),
            Vec2::new(4.0, 1.0)
        );
        assert_eq!(
            visible_extent(BOUNDS, Fit::Cover, NO_ZOOM, 400.0, 400.0),
            Vec2::new(2.0, 2.0)
        );
    }

    #[test]
    fn stretch_shows_exactly_the_bounds_whatever_the_viewport() {
        for (width, height) in [(800.0, 200.0), (400.0, 400.0), (100.0, 900.0)] {
            assert_eq!(
                visible_extent(BOUNDS, Fit::Stretch, NO_ZOOM, width, height),
                BOUNDS
            );
        }
    }

    #[test]
    fn zoom_magnifies_after_the_fit_per_axis() {
        // Contain on the wide viewport shows 8×2; ×2 wide and ×½ tall.
        assert_eq!(
            visible_extent(BOUNDS, Fit::Contain, Vec2::new(2.0, 0.5), 800.0, 200.0),
            Vec2::new(4.0, 4.0)
        );
        assert_eq!(
            visible_extent(BOUNDS, Fit::Stretch, Vec2::splat(2.0), 800.0, 200.0),
            Vec2::new(2.0, 1.0)
        );
    }

    /// Contain and cover are Bevy's `AutoMin` / `AutoMax`: the default view
    /// renders bit-identically to those modes, which the snapshot references
    /// were rendered with.
    #[test]
    fn contain_and_cover_match_bevys_auto_scaling_modes() {
        // Includes the snapshot sizes and the viewport of exactly the bounds' aspect.
        let viewports = [
            (763.0, 486.0),
            (763.0, 760.0),
            (1920.0, 1080.0),
            (1000.0, 500.0),
            (333.0, 777.0),
        ];
        let modes = [
            (
                Fit::Contain,
                ScalingMode::AutoMin {
                    min_width: BOUNDS.x,
                    min_height: BOUNDS.y,
                },
            ),
            (
                Fit::Cover,
                ScalingMode::AutoMax {
                    max_width: BOUNDS.x,
                    max_height: BOUNDS.y,
                },
            ),
        ];
        for (fit, mode) in modes {
            for (width, height) in viewports {
                let mut bevy = OrthographicProjection::default_3d();
                bevy.scaling_mode = mode;
                bevy.update(width, height);
                let mut ours = FitProjection {
                    bounds: BOUNDS,
                    fit,
                    zoom: NO_ZOOM,
                    ortho: OrthographicProjection::default_3d(),
                };
                ours.update(width, height);
                assert_eq!(ours.ortho.area, bevy.area, "{fit:?} at {width}x{height}");
                assert_eq!(
                    ours.get_clip_from_view(),
                    bevy.get_clip_from_view(),
                    "{fit:?} at {width}x{height}"
                );
            }
        }
    }
}
