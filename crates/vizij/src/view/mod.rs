//! The head: Bevy systems rendering the face and applying the device's pose.
//!
//! Scene model matches the web renderer (`@vizij/render`): Z-up world, faces
//! in the XY plane layered along Z, orthographic camera fit to the authored
//! `rootBounds`, ambient-only lighting composed into unlit materials
//! ([`Surface`]: diffuse scaled by `1 − metalness`, plus emissive), sRGB
//! output, no tone mapping unless a face asks for one ([`tone`], composed into
//! the same materials), double-sided materials, opacity-driven alpha
//! blending. The page's [`ViewOptions`] frame every face; a face's own
//! [`FaceView`] overrides them for that face.
//!
//! A face enters as GLB bytes: [`meta`] reads the bindings and the bundle
//! from them, and the scene loads them through the in-memory asset source
//! ([`FaceAssets`]), so the view never touches a file system — the same path
//! on desktop, in the browser and on Android. [`snapshot`] renders offscreen
//! and reads the pixels back; [`frames`] publishes what is rendered into the
//! device's store.
//!
//! What an authoring front end asks of the view beyond the device's pose:
//! a press resolved to the element under it or to a miss ([`pick`]), a glow
//! on the elements it has selected ([`glow`]), rig outputs held against the
//! device ([`hold`]), and a static feature set in place
//! ([`ViewEvent::SetStaticFeature`]).

pub mod frames;
mod glow;
pub mod hold;
pub mod meta;
mod pick;
pub mod snapshot;
pub mod tone;

use std::collections::{HashMap, HashSet};
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
use bevy::ecs::system::SystemParam;
use bevy::gltf::GltfAssetLabel;
use bevy::math::Vec3A;
use bevy::mesh::morph::MorphWeights;
use bevy::prelude::*;
use uuid::Uuid;
use vizij_api_core::value::{as_bool, as_color_rgba, as_float, as_vec3, as_vector};
use vizij_api_core::{TypedPath, Value};

use hold::{HoldTarget, Holds};
use meta::{Binding, FaceMeta, FeatureKind};
pub use tone::ToneMapping;

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

/// A face shown by the view: an entity carrying the metadata, the GLB served
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
    /// animatable id → (target entity, feature, morph index, material shade factor).
    pub by_uuid: HashMap<Uuid, (Entity, FeatureKind, Option<usize>, f32)>,
    /// element id → the entities its features land on: what a pick tests,
    /// what the glow outlines, what a static feature is set on.
    pub elements: HashMap<Uuid, ElementTargets>,
    /// animatable id → the value last applied to the scene, what a held
    /// component keeps ([`hold`]).
    shown: HashMap<Uuid, Decoded>,
    /// The meshes whose materials the view composes ([`Surface`]): what the
    /// face's tone mapping goes over.
    surfaces: Vec<Entity>,
    pub ready: bool,
}

/// The entities one element's features land on.
#[derive(Debug, Clone, Copy)]
pub struct ElementTargets {
    /// Its glTF node: the transform features.
    pub node: Entity,
    /// Its own mesh — the node's, or its first primitive's: the static
    /// material features, the pick, the glow. `None` for a group.
    pub mesh: Option<Entity>,
    /// The descendant carrying its morph weights, if it has morph targets.
    pub morph: Option<Entity>,
}

/// A face being replaced by a reload under the same id: it keeps drawing
/// until its successor is indexed, then goes ([`retire_superseded`]), so a
/// reload never shows an empty rectangle. It is not reported ready and
/// reports no picks.
#[derive(Component)]
pub struct Superseded;

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
    /// Frame and tone-map the face by `view`, in place of the face's previous
    /// view. Kept across the face's reloads, like its placement.
    SetFaceView { face_id: String, view: FaceView },
    /// Outline these elements of the face (RobotData element ids) with the
    /// selection glow; an empty list clears it. Kept across the face's
    /// reloads.
    SetSelection {
        face_id: String,
        elements: Vec<Uuid>,
    },
    /// Hold rig outputs of the face against its device: a held animatable,
    /// or one component of it, keeps the value it shows while the device
    /// writes on ([`hold`]). Kept across the face's reloads.
    Hold {
        face_id: String,
        targets: Vec<HoldTarget>,
    },
    /// Let held outputs follow the device again; `None` releases them all.
    Release {
        face_id: String,
        targets: Option<Vec<HoldTarget>>,
    },
    /// Set one feature of an element in place — a static feature, which no
    /// device output drives — by its RobotData name (`translation`,
    /// `color`, a morph target's name, …). Applied once the face is
    /// indexed; a reload shows whatever its GLB carries.
    SetStaticFeature {
        face_id: String,
        element_id: Uuid,
        feature: String,
        value: Value,
    },
}

/// How one face is framed and tone-mapped, over the page's [`ViewOptions`]:
/// a framing field left `None` takes the page's (and the GLB's authored
/// rootBounds), so several faces on one target can each be framed their own
/// way.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FaceView {
    /// The world rectangle the camera frames instead of the authored
    /// rootBounds: `(center x, center y, width, height)`, as
    /// [`FaceMeta::root_bounds`]. Positive sizes.
    pub bounds: Option<(f32, f32, f32, f32)>,
    /// How the bounds fit the face's rectangle.
    pub fit: Option<Fit>,
    /// Magnification after the fit, per axis. Positive.
    pub zoom: Option<Vec2>,
    /// The curve the face's colors go through; none by default.
    pub tone_mapping: ToneMapping,
}

/// Each face's own view ([`ViewEvent::SetFaceView`]), by face id; kept apart
/// from the faces so a view survives the face's reload.
#[derive(Resource, Default)]
struct FaceViews(HashMap<String, FaceView>);

/// Where each face's safe area — the bounds its camera frames — lies on the
/// target, in logical pixels from the target's top-left corner, as of the
/// last frame; by face id. The rectangle can reach past the face's own
/// rectangle when a zoom crops the bounds.
#[derive(Resource, Default)]
pub struct SafeAreas(pub HashMap<String, Rect>);

/// What a face's camera frames, in effect: its own view's fields where set,
/// else the page's options and the GLB's authored rootBounds.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Framing {
    center: Vec2,
    size: Vec2,
    fit: Fit,
    zoom: Vec2,
}

impl Framing {
    fn of(meta: &FaceMeta, options: &ViewOptions, view: Option<&FaceView>) -> Self {
        let (cx, cy, width, height) = view
            .and_then(|view| view.bounds)
            .or(meta.root_bounds)
            .unwrap_or((0.0, 0.0, 5.0, 4.0));
        Self {
            center: Vec2::new(cx, cy),
            size: Vec2::new(width, height),
            fit: view.and_then(|view| view.fit).unwrap_or(options.fit),
            zoom: view.and_then(|view| view.zoom).unwrap_or(options.zoom),
        }
    }
}

/// The front ends' requests, drained each frame ([`ViewEvent`]).
#[derive(Resource)]
pub struct ViewEvents(pub Mutex<Receiver<ViewEvent>>);

/// A pointer press on a face: the element under it, by the ids its face's
/// RobotData declares, or `None` for a press that hit no element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picked {
    pub face_id: String,
    pub element_id: Option<Uuid>,
}

/// The picks (pointer presses on faces) since a consumer last drained them.
#[derive(Resource, Default)]
pub struct Picks(pub Vec<Picked>);

/// Where each face's camera draws, by face id ([`ViewEvent::PlaceFace`]);
/// a face without an entry, or with `None`, draws over the whole target. Kept
/// apart from the faces so a placement survives the face's reload.
#[derive(Resource, Default)]
struct Placements(HashMap<String, Option<[u32; 4]>>);

/// Static feature values waiting for their face to be indexed, by face id
/// ([`ViewEvent::SetStaticFeature`]).
#[derive(Resource, Default)]
struct PendingStatics(HashMap<String, Vec<(Uuid, String, Value)>>);

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

/// What a mesh's material is made of, in the web's terms: the inputs its
/// unlit color is composed from. Kept per mesh so a change to any one of
/// them recomposes the whole, and initialised from the GLB material so a
/// face whose look is authored into the material (a metallic plate, an
/// emissive feature) reads right before its rig writes a thing.
///
/// The composition is what three's ambient-only pipeline makes of a
/// `MeshStandardMaterial` with no environment map: the diffuse term is the
/// base color scaled by `1 − metalness` and the ambient factor (a metal has
/// no diffuse, and with nothing to reflect it is black), the emissive term
/// adds on top, and roughness changes nothing. A `basic` material
/// (`MeshBasicMaterial`) is its base color alone.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Surface {
    /// Linear RGB (three's `color`).
    pub base: [f32; 3],
    pub opacity: f32,
    pub metalness: f32,
    /// Linear RGB (three's `emissive`).
    pub emissive: [f32; 3],
    pub emissive_intensity: f32,
    /// The ambient factor, `ambient/π`; 1 for a `basic` material.
    pub factor: f32,
    /// A `basic` material: full albedo, no metalness, no emissive.
    pub basic: bool,
    /// The face's tone mapping, applied to the composed color.
    pub tone: ToneMapping,
}

impl Surface {
    /// The material as the GLB loader read it.
    fn from_material(
        material: &StandardMaterial,
        factor: f32,
        basic: bool,
        tone: ToneMapping,
    ) -> Self {
        let base = material.base_color.to_linear();
        Self {
            base: [base.red, base.green, base.blue],
            opacity: base.alpha,
            metalness: material.metallic,
            emissive: [
                material.emissive.red,
                material.emissive.green,
                material.emissive.blue,
            ],
            emissive_intensity: 1.0,
            factor,
            basic,
            tone,
        }
    }

    /// The unlit color the material renders with, tone-mapped.
    pub fn color(&self) -> Color {
        let rgb = if self.basic {
            self.base
        } else {
            let diffuse = (1.0 - self.metalness) * self.factor;
            let mut rgb = [0.0; 3];
            for (i, channel) in rgb.iter_mut().enumerate() {
                *channel = self.base[i] * diffuse + self.emissive[i] * self.emissive_intensity;
            }
            rgb
        };
        let rgb = self.tone.apply(rgb);
        Color::LinearRgba(LinearRgba {
            red: rgb[0],
            green: rgb[1],
            blue: rgb[2],
            alpha: self.opacity,
        })
    }

    /// Write the composed color onto the material, with the alpha mode the
    /// opacity calls for.
    fn apply(&self, material: &mut StandardMaterial) {
        material.base_color = self.color();
        material.alpha_mode = if self.opacity < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        };
    }
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
        // A GLB spawns as a `WorldAsset`: its stored world is copied into the
        // app's through the type registry, one `ReflectComponent` per
        // component, so every component type a face GLB carries has to be
        // registered here — the engine registers none on its own (automatic
        // registration of every reflected type is left out of the build for
        // its size).
        app.register_type::<Name>()
            .register_type::<ChildOf>()
            .register_type::<Children>()
            .register_type::<Transform>()
            .register_type::<GlobalTransform>()
            .register_type::<bevy::transform::components::TransformTreeChanged>()
            .register_type::<Visibility>()
            .register_type::<InheritedVisibility>()
            .register_type::<ViewVisibility>()
            .register_type::<bevy::camera::primitives::Aabb>()
            .register_type::<Mesh3d>()
            .register_type::<MorphWeights>()
            .register_type::<bevy::mesh::morph::MeshMorphWeights>()
            .register_type::<bevy::gltf::GltfSceneName>()
            .register_type::<bevy::gltf::GltfMeshName>()
            .register_type::<bevy::gltf::GltfMaterialName>()
            .register_type::<bevy::gltf::GltfExtras>()
            .register_type::<bevy::gltf::GltfMeshExtras>()
            .register_type::<bevy::gltf::GltfMaterialExtras>();
        // Picking runs its own ray cast over the morphed meshes on a press
        // ([`pick`]); Bevy's picking only delivers the press, through the
        // window backend every press reaches when no other backend reports
        // a hit.
        app.init_resource::<Slots>()
            .init_resource::<Placements>()
            .init_resource::<FaceViews>()
            .init_resource::<SafeAreas>()
            .init_resource::<Picks>()
            .init_resource::<PendingStatics>()
            .init_resource::<Holds>()
            .add_plugins(glow::GlowPlugin)
            .add_observer(pick::on_press)
            .add_systems(
                Update,
                (
                    apply_view_events,
                    place_cameras,
                    frame_cameras,
                    index_faces,
                    retire_superseded,
                    apply_statics,
                    tone_faces,
                    apply_poses,
                    measure_safe_areas,
                )
                    .chain(),
            );
    }
}

/// Apply the front ends' requests: load, reload or unload a face, place it
/// or set its view, recolor the background, and the authoring requests —
/// selection, holds, static features. A load under an id already shown
/// supersedes the face there rather than unloading it: the old scene draws
/// until the new one is indexed ([`retire_superseded`]).
#[allow(clippy::too_many_arguments)]
fn apply_view_events(
    events: Option<Res<ViewEvents>>,
    assets: Res<FaceAssets>,
    mut options: ResMut<ViewOptions>,
    offscreen: Option<Res<OffscreenTarget>>,
    mut frame_config: Option<ResMut<frames::FrameConfig>>,
    mut slots: ResMut<Slots>,
    mut placements: ResMut<Placements>,
    mut views: ResMut<FaceViews>,
    mut selections: ResMut<glow::Selections>,
    mut holds: ResMut<Holds>,
    mut statics: ResMut<PendingStatics>,
    mut commands: Commands,
    faces: Query<(Entity, &Face)>,
    asset_server: Res<AssetServer>,
) {
    let Some(events) = events else { return };
    let Ok(receiver) = events.0.lock() else {
        return;
    };
    // What this drain changed that the query does not show until the
    // drain's commands apply: the faces it took down, and those it spawned.
    let mut gone = HashSet::new();
    let mut spawned: Vec<(String, Entity, Entity, String, usize)> = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        match event {
            ViewEvent::Background([r, g, b]) => {
                options.background = Color::srgb_u8(r, g, b);
            }
            ViewEvent::LoadFace {
                face_id,
                meta,
                glb,
                rig,
            } => {
                for (entity, face) in &faces {
                    if face.id == face_id && !gone.contains(&entity) {
                        commands.entity(entity).insert(Superseded);
                    }
                }
                for (id, entity, ..) in &spawned {
                    if *id == face_id {
                        commands.entity(*entity).insert(Superseded);
                    }
                }
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
                        Framing::of(&meta, &options, views.0.get(&face_id)),
                        slot,
                        offscreen.as_deref(),
                    ))
                    .id();
                let face = commands
                    .spawn((
                        Face {
                            id: face_id.clone(),
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
                spawned.push((face_id, face, camera, asset_path, slot));
            }
            ViewEvent::UnloadFace { face_id } => {
                for (entity, face) in &faces {
                    if face.id == face_id && gone.insert(entity) {
                        despawn_face(entity, face, &assets, &mut slots, &mut commands);
                        log::info!("face {face_id}: unloaded");
                    }
                }
                spawned.retain(|(id, entity, camera, asset_path, slot)| {
                    if *id != face_id {
                        return true;
                    }
                    commands.entity(*camera).despawn();
                    commands.entity(*entity).despawn();
                    assets.remove(asset_path);
                    slots.release(*slot);
                    false
                });
                selections.0.remove(&face_id);
                holds.clear(&face_id);
                statics.0.remove(&face_id);
            }
            ViewEvent::PlaceFace { face_id, rect } => {
                placements.0.insert(face_id, rect);
            }
            ViewEvent::SetFaceView { face_id, view } => {
                views.0.insert(face_id, view);
            }
            ViewEvent::SetSelection { face_id, elements } => {
                if elements.is_empty() {
                    selections.0.remove(&face_id);
                } else {
                    selections.0.insert(face_id, elements);
                }
            }
            ViewEvent::Hold { face_id, targets } => holds.hold(&face_id, targets),
            ViewEvent::Release { face_id, targets } => holds.release(&face_id, targets),
            ViewEvent::SetStaticFeature {
                face_id,
                element_id,
                feature,
                value,
            } => statics
                .0
                .entry(face_id)
                .or_default()
                .push((element_id, feature, value)),
        }
    }
}

/// Confine each face's camera to its placement, when it has one, and let
/// the lowest camera clear the target with the background — whichever face
/// that is as faces come and go — while the others draw over it.
fn place_cameras(
    placements: Res<Placements>,
    options: Res<ViewOptions>,
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
    let lowest = cameras.iter().map(|camera| camera.order).min();
    for mut camera in &mut cameras {
        let clears = Some(camera.order) == lowest;
        let current = match camera.clear_color {
            ClearColorConfig::Custom(color) => Some(color),
            _ => None,
        };
        let wanted = clears.then_some(options.background);
        if current != wanted {
            camera.clear_color = match wanted {
                Some(color) => ClearColorConfig::Custom(color),
                None => ClearColorConfig::None,
            };
        }
    }
}

/// Keep each face's camera on its framing: the projection and the position
/// follow the face's view as it changes.
fn frame_cameras(
    views: Res<FaceViews>,
    options: Res<ViewOptions>,
    faces: Query<&Face>,
    mut cameras: Query<(&mut Projection, &mut Transform), With<FaceCamera>>,
) {
    for face in &faces {
        let Ok((mut projection, mut transform)) = cameras.get_mut(face.camera) else {
            continue;
        };
        let framing = Framing::of(&face.meta, &options, views.0.get(&face.id));
        let current = match &*projection {
            Projection::Custom(custom) => custom
                .get::<FitProjection>()
                .map(|fit| (fit.bounds, fit.fit, fit.zoom)),
            _ => None,
        };
        if current != Some((framing.size, framing.fit, framing.zoom)) {
            *projection = fit_projection(&framing);
        }
        let position = slot_origin(face.slot) + framing.center.extend(CAMERA_HEIGHT);
        if transform.translation != position {
            transform.translation = position;
        }
    }
}

/// Keep each indexed face's materials on its tone mapping as its view
/// changes.
fn tone_faces(
    views: Res<FaceViews>,
    faces: Query<&Face>,
    mut surfaces: Query<(&MeshMaterial3d<StandardMaterial>, &mut Surface)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for face in &faces {
        let tone = views
            .0
            .get(&face.id)
            .map(|view| view.tone_mapping)
            .unwrap_or_default();
        for mesh in &face.bindings.surfaces {
            let Ok((handle, mut surface)) = surfaces.get_mut(*mesh) else {
                continue;
            };
            if surface.tone != tone {
                surface.tone = tone;
                if let Some(mut material) = materials.get_mut(&handle.0) {
                    surface.apply(&mut material);
                }
            }
        }
    }
}

/// Measure where each face's safe area lies ([`SafeAreas`]).
fn measure_safe_areas(
    views: Res<FaceViews>,
    options: Res<ViewOptions>,
    faces: Query<&Face, Without<Superseded>>,
    cameras: Query<&Camera, With<FaceCamera>>,
    mut areas: ResMut<SafeAreas>,
) {
    areas.0.clear();
    for face in &faces {
        let Some(viewport) = cameras
            .get(face.camera)
            .ok()
            .and_then(Camera::logical_viewport_rect)
        else {
            continue;
        };
        let framing = Framing::of(&face.meta, &options, views.0.get(&face.id));
        areas
            .0
            .insert(face.id.clone(), safe_area(&framing, viewport));
    }
}

/// The part of `viewport` the framed bounds cover: centered, as the camera
/// is on them, and scaled as the projection scales them.
fn safe_area(framing: &Framing, viewport: Rect) -> Rect {
    let size = viewport.size();
    let extent = visible_extent(framing.size, framing.fit, framing.zoom, size.x, size.y);
    Rect::from_center_size(viewport.center(), framing.size / extent * size)
}

/// Take a face down: its scene, its camera, its GLB, its slot.
fn despawn_face(
    entity: Entity,
    face: &Face,
    assets: &FaceAssets,
    slots: &mut Slots,
    commands: &mut Commands,
) {
    commands.entity(face.camera).despawn();
    commands.entity(entity).despawn();
    assets.remove(&face.asset_path);
    slots.release(face.slot);
}

/// Once a reloaded face is indexed, the faces it supersedes go: the swap
/// lands in one frame, the new scene posed before it draws.
fn retire_superseded(
    faces: Query<(Entity, &Face, Has<Superseded>)>,
    assets: Res<FaceAssets>,
    mut slots: ResMut<Slots>,
    mut commands: Commands,
) {
    for (_, face, superseded) in &faces {
        if superseded || !face.bindings.ready {
            continue;
        }
        for (old, old_face, old_superseded) in &faces {
            if old_superseded && old_face.id == face.id {
                despawn_face(old, old_face, &assets, &mut slots, &mut commands);
                log::info!("face {}: reloaded, slot {} retired", face.id, old_face.slot);
            }
        }
    }
}

/// A face's camera: the fit over its framing, over its slot. Which camera
/// clears the target is [`place_cameras`]'.
fn camera_for(framing: Framing, slot: usize, offscreen: Option<&OffscreenTarget>) -> impl Bundle {
    // Lighting is baked into the materials (see `ViewOptions::albedo_factor`);
    // no scene light is spawned.
    let projection = fit_projection(&framing);
    let transform =
        Transform::from_translation(slot_origin(slot) + framing.center.extend(CAMERA_HEIGHT))
            .looking_to(Vec3::NEG_Z, Vec3::Y);
    (
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::None,
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

/// How far in front of the faces' plane a camera stands, looking down −Z.
const CAMERA_HEIGHT: f32 = 100.0;

fn fit_projection(framing: &Framing) -> Projection {
    Projection::custom(FitProjection {
        bounds: framing.size,
        fit: framing.fit,
        zoom: framing.zoom,
        ortho: OrthographicProjection::default_3d(),
    })
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
    views: Res<FaceViews>,
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
        let mut elements = HashMap::new();
        let tone = views
            .0
            .get(&face.id)
            .map(|view| view.tone_mapping)
            .unwrap_or_default();
        for element in &face.meta.elements {
            let node = by_name[element.node_name.as_str()].0;
            // three's MeshBasicMaterial ignores lights: full albedo. `standard`
            // gets the ambient-Lambert factor.
            let basic = element.material.as_deref() == Some("basic");
            let factor = if basic { 1.0 } else { options.albedo_factor() };
            for descendant in std::iter::once(node).chain(children.iter_descendants(node)) {
                if let Ok((mesh_entity, material)) = meshes.get(descendant) {
                    // Unique material per element, with web conventions applied:
                    // double-sided, unlit with the web's ambient-only model
                    // composed into the albedo ([`Surface`]) from what the
                    // loader read — base color, metalness, emissive.
                    let mut mat = materials
                        .get(&material.0)
                        .cloned()
                        .unwrap_or_else(StandardMaterial::default);
                    mat.double_sided = true;
                    mat.cull_mode = None;
                    mat.unlit = true;
                    let surface = Surface::from_material(&mat, factor, basic, tone);
                    surface.apply(&mut mat);
                    let handle = materials.add(mat);
                    commands
                        .entity(mesh_entity)
                        .insert((MeshMaterial3d(handle.clone()), surface));
                    mesh_of.insert(element.node_name.clone(), (mesh_entity, handle));
                    break;
                }
            }
            for descendant in std::iter::once(node).chain(children.iter_descendants(node)) {
                if morphs.get(descendant).is_ok() {
                    morph_of.insert(element.node_name.clone(), descendant);
                    break;
                }
            }
            // The element's own mesh is the node's or one of its primitives
            // (the node's children); a deeper mesh is another element's.
            let own_mesh = std::iter::once(node)
                .chain(children.get(node).into_iter().flat_map(|kids| kids.iter()))
                .find(|entity| meshes.contains(*entity));
            elements.insert(
                element.id,
                ElementTargets {
                    node,
                    mesh: own_mesh,
                    morph: morph_of.get(&element.node_name).copied(),
                },
            );
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
                FeatureKind::Color
                | FeatureKind::Opacity
                | FeatureKind::Metalness
                | FeatureKind::Roughness
                | FeatureKind::Emissive
                | FeatureKind::EmissiveIntensity => {
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
            by_uuid.insert(*uuid, entry);
        }

        log::info!(
            "face {}: scene indexed, {} bindings over {} elements",
            face.id,
            by_uuid.len(),
            face.meta.elements.len(),
        );
        face.bindings = Bindings {
            by_uuid,
            elements,
            shown: HashMap::new(),
            surfaces: mesh_of.values().map(|(mesh, _)| *mesh).collect(),
            ready: true,
        };
    }
}

/// Set the static features waiting for their face, once it is indexed.
fn apply_statics(
    mut pending: ResMut<PendingStatics>,
    faces: Query<&Face, Without<Superseded>>,
    mut scene: SceneFeatures,
) {
    if pending.0.is_empty() {
        return;
    }
    for face in &faces {
        if !face.bindings.ready {
            continue;
        }
        let Some(edits) = pending.0.remove(&face.id) else {
            continue;
        };
        for (element_id, feature, value) in edits {
            let target = face
                .meta
                .elements
                .iter()
                .find(|element| element.id == element_id)
                .zip(face.bindings.elements.get(&element_id));
            let Some((element, targets)) = target else {
                log::warn!("face {}: no element {element_id}", face.id);
                continue;
            };
            let Some(kind) = FeatureKind::from_name(&feature, &element.morph_targets) else {
                log::warn!("face {}: {element_id} has no feature {feature:?}", face.id);
                continue;
            };
            let (entity, morph_index) = match &kind {
                FeatureKind::Translation | FeatureKind::Rotation | FeatureKind::Scale => {
                    (Some(targets.node), None)
                }
                FeatureKind::Morph(target) => (
                    targets.morph,
                    element.morph_targets.iter().position(|m| m == target),
                ),
                _ => (targets.mesh, None),
            };
            let (Some(entity), Some(decoded)) = (entity, decode(&kind, &value)) else {
                log::warn!(
                    "face {}: {feature} of {element_id} does not take {value:?}",
                    face.id
                );
                continue;
            };
            scene.apply(entity, &kind, morph_index, decoded);
        }
    }
}

/// Applies each device's current pose (the HAL's actuation state) onto its
/// face's scene: transforms, material surfaces, morph influences — except
/// what is held against the device ([`hold`]).
fn apply_poses(mut faces: Query<&mut Face>, holds: Res<Holds>, mut scene: SceneFeatures) {
    for mut face in &mut faces {
        if !face.bindings.ready {
            continue;
        }
        let face = &mut *face;
        let held = holds.of(&face.id);
        let bindings = &mut face.bindings;
        for (path, value) in face.rig.pose() {
            let Some(id) = animatable_of(&path) else {
                continue;
            };
            let Some((entity, feature, morph_index, _factor)) = bindings.by_uuid.get(&id) else {
                continue;
            };
            let Some(mut decoded) = decode(feature, &value) else {
                continue;
            };
            if let Some(held) = held {
                let shown = || {
                    bindings
                        .shown
                        .get(&id)
                        .copied()
                        .or_else(|| scene.read(*entity, feature, *morph_index))
                };
                match hold::resolve(held, id, decoded, shown) {
                    Some(kept) => decoded = kept,
                    None => continue,
                }
            }
            scene.apply(*entity, feature, *morph_index, decoded);
            bindings.shown.insert(id, decoded);
        }
    }
}

/// A feature value in the form the scene takes it: a triple (a vector, an
/// euler, a linear RGB colour) or a scalar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Decoded {
    Triple([f32; 3]),
    Scalar(f32),
}

/// `value` read as what `feature` takes; `None` when it is no such value.
fn decode(feature: &FeatureKind, value: &Value) -> Option<Decoded> {
    match feature {
        FeatureKind::Translation | FeatureKind::Rotation => as_xyz(value).map(Decoded::Triple),
        FeatureKind::Scale => as_xyz(value)
            .map(Decoded::Triple)
            .or_else(|| as_f32(value).map(Decoded::Scalar)),
        // Graph color components are linear working-space floats (three's
        // `Color.setRGB` semantics), not sRGB.
        FeatureKind::Color | FeatureKind::Emissive => as_rgb(value).map(Decoded::Triple),
        FeatureKind::Opacity
        | FeatureKind::Metalness
        | FeatureKind::Roughness
        | FeatureKind::EmissiveIntensity
        | FeatureKind::Morph(_) => as_f32(value).map(Decoded::Scalar),
    }
}

/// The scene state the features land on: node transforms, mesh surfaces
/// (and the materials they compose), morph weights.
#[derive(SystemParam)]
struct SceneFeatures<'w, 's> {
    transforms: Query<'w, 's, &'static mut Transform>,
    surfaces: Query<
        'w,
        's,
        (
            &'static MeshMaterial3d<StandardMaterial>,
            &'static mut Surface,
        ),
    >,
    morph_weights: Query<'w, 's, &'static mut MorphWeights>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

impl SceneFeatures<'_, '_> {
    /// Set `feature` of `entity` to `value`; a value of the wrong shape for
    /// the feature changes nothing.
    fn apply(
        &mut self,
        entity: Entity,
        feature: &FeatureKind,
        morph_index: Option<usize>,
        value: Decoded,
    ) {
        use Decoded::{Scalar, Triple};
        match (feature, value) {
            (FeatureKind::Translation, Triple(v)) => {
                if let Ok(mut transform) = self.transforms.get_mut(entity) {
                    transform.translation = Vec3::from_array(v);
                }
            }
            (FeatureKind::Rotation, Triple(v)) => {
                if let Ok(mut transform) = self.transforms.get_mut(entity) {
                    transform.rotation = euler_zyx(v);
                }
            }
            (FeatureKind::Scale, Triple(v)) => {
                if let Ok(mut transform) = self.transforms.get_mut(entity) {
                    transform.scale = Vec3::from_array(v);
                }
            }
            (FeatureKind::Scale, Scalar(s)) => {
                if let Ok(mut transform) = self.transforms.get_mut(entity) {
                    transform.scale = Vec3::splat(s);
                }
            }
            // A material feature lands on the mesh's surface, and the surface
            // recomposes the material's color.
            (FeatureKind::Color, Triple(rgb)) => self.edit_surface(entity, |s| s.base = rgb),
            (FeatureKind::Opacity, Scalar(o)) => self.edit_surface(entity, |s| s.opacity = o),
            (FeatureKind::Metalness, Scalar(m)) => self.edit_surface(entity, |s| s.metalness = m),
            (FeatureKind::Emissive, Triple(rgb)) => self.edit_surface(entity, |s| s.emissive = rgb),
            (FeatureKind::EmissiveIntensity, Scalar(i)) => {
                self.edit_surface(entity, |s| s.emissive_intensity = i)
            }
            // No direct light and no environment map: roughness has nothing
            // to shape.
            (FeatureKind::Roughness, _) => {}
            (FeatureKind::Morph(_), Scalar(w)) => {
                if let (Ok(mut weights), Some(i)) =
                    (self.morph_weights.get_mut(entity), morph_index)
                {
                    if let Some(slot) = weights.weights_mut().get_mut(i) {
                        *slot = w;
                    }
                }
            }
            _ => {}
        }
    }

    /// Edit `entity`'s surface and recompose its material when the edit
    /// changed it.
    fn edit_surface(&mut self, entity: Entity, edit: impl FnOnce(&mut Surface)) {
        let Ok((handle, mut surface)) = self.surfaces.get_mut(entity) else {
            return;
        };
        let mut edited = surface.clone();
        edit(&mut edited);
        if edited != *surface {
            if let Some(mut material) = self.materials.get_mut(&handle.0) {
                edited.apply(&mut material);
            }
            *surface = edited;
        }
    }

    /// What `feature` of `entity` currently shows.
    fn read(
        &self,
        entity: Entity,
        feature: &FeatureKind,
        morph_index: Option<usize>,
    ) -> Option<Decoded> {
        let transform = || self.transforms.get(entity).ok();
        let surface = || self.surfaces.get(entity).ok().map(|(_, surface)| surface);
        Some(match feature {
            FeatureKind::Translation => Decoded::Triple(transform()?.translation.to_array()),
            FeatureKind::Rotation => {
                let (z, y, x) = transform()?.rotation.to_euler(EulerRot::ZYX);
                Decoded::Triple([x, y, z])
            }
            FeatureKind::Scale => Decoded::Triple(transform()?.scale.to_array()),
            FeatureKind::Color => Decoded::Triple(surface()?.base),
            FeatureKind::Emissive => Decoded::Triple(surface()?.emissive),
            FeatureKind::Opacity => Decoded::Scalar(surface()?.opacity),
            FeatureKind::Metalness => Decoded::Scalar(surface()?.metalness),
            FeatureKind::EmissiveIntensity => Decoded::Scalar(surface()?.emissive_intensity),
            FeatureKind::Roughness => return None,
            FeatureKind::Morph(_) => Decoded::Scalar(
                *self
                    .morph_weights
                    .get(entity)
                    .ok()?
                    .weights()
                    .get(morph_index?)?,
            ),
        })
    }
}

/// An euler triple as a rotation, in three.js's default order: R = Rz·Ry·Rx,
/// composed explicitly — EulerRot naming conventions moved between glam
/// versions, this cannot. Validated pixel-wise against the web renderer on
/// Toasty, whose tilts are order-sensitive.
fn euler_zyx([x, y, z]: [f32; 3]) -> Quat {
    Quat::from_rotation_z(z) * Quat::from_rotation_y(y) * Quat::from_rotation_x(x)
}

/// The animatable a rig key names: the rig keys its pose by bare animatable
/// id, so a namespaced or field-selected path is no animatable.
fn animatable_of(path: &TypedPath) -> Option<Uuid> {
    if !path.namespaces.is_empty() || !path.fields.is_empty() {
        return None;
    }
    Uuid::parse_str(&path.target).ok()
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

    fn rgb(color: Color) -> [f32; 3] {
        let lin = color.to_linear();
        [lin.red, lin.green, lin.blue]
    }

    fn standard(base: [f32; 3], metalness: f32, emissive: [f32; 3]) -> Surface {
        Surface {
            base,
            opacity: 1.0,
            metalness,
            emissive,
            emissive_intensity: 1.0,
            factor: 0.5,
            basic: false,
            tone: ToneMapping::None,
        }
    }

    /// The web's ambient-only result for a standard material: a metal has
    /// no diffuse and nothing to reflect, so a white metallic plate is black;
    /// an emissive feature shows its emissive color on a black base; and a
    /// plain colored material is its base scaled by the ambient factor — the
    /// faces authored on `color` alone render as before.
    #[test]
    fn a_surface_composes_the_webs_ambient_only_color() {
        let black = [0.0, 0.0, 0.0];
        assert_eq!(rgb(standard([1.0, 1.0, 1.0], 1.0, black).color()), black);
        let olive = [0.308, 0.332, 0.0];
        assert_eq!(rgb(standard(black, 1.0, olive).color()), olive);
        let mut dim = standard(black, 1.0, olive);
        dim.emissive_intensity = 0.5;
        assert_eq!(rgb(dim.color()), [0.154, 0.166, 0.0]);
        assert_eq!(
            rgb(standard([0.2, 0.4, 0.8], 0.0, black).color()),
            [0.1, 0.2, 0.4]
        );
        // Half metallic: half the diffuse, plus the emissive.
        assert_eq!(
            rgb(standard([1.0, 1.0, 1.0], 0.5, [0.0, 0.0, 0.25]).color()),
            [0.25, 0.25, 0.5]
        );
    }

    /// A `basic` material is its base color, whatever the other inputs say.
    #[test]
    fn a_basic_surface_is_its_base_color() {
        let mut surface = standard([0.2, 0.4, 0.8], 1.0, [1.0, 1.0, 1.0]);
        surface.basic = true;
        surface.factor = 1.0;
        assert_eq!(rgb(surface.color()), [0.2, 0.4, 0.8]);
        surface.opacity = 0.5;
        assert_eq!(surface.color().alpha(), 0.5);
    }

    /// The surface reads the GLB material as the loader delivered it, so a
    /// look authored into the material holds before the rig writes anything.
    #[test]
    fn a_surface_starts_from_the_loaded_material() {
        let material = StandardMaterial {
            base_color: Color::linear_rgb(1.0, 1.0, 1.0),
            metallic: 1.0,
            emissive: LinearRgba::rgb(0.3, 0.3, 0.0),
            ..StandardMaterial::default()
        };
        let surface = Surface::from_material(&material, 0.5, false, ToneMapping::None);
        assert_eq!(surface.metalness, 1.0);
        assert_eq!(surface.emissive, [0.3, 0.3, 0.0]);
        assert_eq!(rgb(surface.color()), [0.3, 0.3, 0.0]);
    }

    /// A face's tone mapping goes over the composed color, not the inputs.
    #[test]
    fn a_surface_is_tone_mapped_after_composition() {
        let mut surface = standard([0.2, 0.4, 0.8], 0.0, [0.0, 0.0, 0.0]);
        surface.tone = ToneMapping::Aces;
        assert_eq!(
            rgb(surface.color()),
            ToneMapping::Aces.apply([0.1, 0.2, 0.4])
        );
    }

    fn framing(fit: Fit, zoom: Vec2) -> Framing {
        Framing {
            center: Vec2::new(1.0, -1.0),
            size: BOUNDS,
            fit,
            zoom,
        }
    }

    /// The safe area is the framed bounds where the camera draws them:
    /// centered on the face's rectangle, the size the fit gives them, and
    /// past the rectangle when a zoom crops them.
    #[test]
    fn the_safe_area_is_the_bounds_as_drawn() {
        let viewport = Rect::new(10.0, 20.0, 810.0, 220.0);
        // Contain on 800×200 shows 8×2: the 4×2 bounds cover half the width.
        assert_eq!(
            safe_area(&framing(Fit::Contain, NO_ZOOM), viewport),
            Rect::new(210.0, 20.0, 610.0, 220.0)
        );
        // Cover shows 4×1: the bounds span the width and twice the height.
        assert_eq!(
            safe_area(&framing(Fit::Cover, NO_ZOOM), viewport),
            Rect::new(10.0, -80.0, 810.0, 320.0)
        );
        assert_eq!(
            safe_area(&framing(Fit::Contain, Vec2::splat(2.0)), viewport),
            Rect::new(10.0, -80.0, 810.0, 320.0)
        );
        assert_eq!(
            safe_area(&framing(Fit::Stretch, NO_ZOOM), viewport),
            viewport
        );
    }

    /// A face's view overrides the page's framing field by field, and its
    /// bounds the authored ones.
    #[test]
    fn a_face_view_overrides_the_page_field_by_field() {
        let options = ViewOptions {
            background: Color::NONE,
            fit: Fit::Cover,
            zoom: Vec2::splat(3.0),
            ambient: 1.0,
            unlit: false,
        };
        let meta = FaceMeta {
            root_bounds: Some((1.0, 2.0, 3.0, 4.0)),
            ..FaceMeta::default()
        };
        let page = Framing::of(&meta, &options, None);
        assert_eq!(page.center, Vec2::new(1.0, 2.0));
        assert_eq!(page.size, Vec2::new(3.0, 4.0));
        assert_eq!((page.fit, page.zoom), (Fit::Cover, Vec2::splat(3.0)));
        let view = FaceView {
            bounds: Some((0.0, 0.0, 10.0, 8.0)),
            zoom: Some(Vec2::ONE),
            ..FaceView::default()
        };
        let own = Framing::of(&meta, &options, Some(&view));
        assert_eq!(own.center, Vec2::ZERO);
        assert_eq!(own.size, Vec2::new(10.0, 8.0));
        assert_eq!((own.fit, own.zoom), (Fit::Cover, Vec2::ONE));
    }

    /// What a held rotation component keeps is read back off the scene's
    /// quaternion in the order the rig writes it: the triple round-trips.
    #[test]
    fn a_rotation_reads_back_the_euler_it_was_set_from() {
        for v in [[0.3, -0.2, 1.1], [0.0, 0.0, 0.0], [-1.2, 0.4, -2.5]] {
            let (z, y, x) = euler_zyx(v).to_euler(EulerRot::ZYX);
            for (got, want) in [x, y, z].iter().zip(v) {
                assert!(
                    (got - want).abs() < 1e-5,
                    "{v:?} read back as {:?}",
                    [x, y, z]
                );
            }
        }
    }

    /// A feature takes the value shapes it can show, and nothing else.
    #[test]
    fn a_value_decodes_to_what_its_feature_takes() {
        let vec3 = vizij_api_core::value::vec3([1.0, 2.0, 3.0]);
        let float = vizij_api_core::value::float(0.5);
        assert_eq!(
            decode(&FeatureKind::Translation, &vec3),
            Some(Decoded::Triple([1.0, 2.0, 3.0]))
        );
        assert_eq!(decode(&FeatureKind::Translation, &float), None);
        assert_eq!(
            decode(&FeatureKind::Scale, &float),
            Some(Decoded::Scalar(0.5))
        );
        assert_eq!(
            decode(&FeatureKind::Morph("open".into()), &float),
            Some(Decoded::Scalar(0.5))
        );
        assert_eq!(
            decode(&FeatureKind::Color, &vec3),
            Some(Decoded::Triple([1.0, 2.0, 3.0]))
        );
    }

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
