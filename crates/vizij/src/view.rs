//! The head: Bevy systems rendering the face and applying the device's pose.
//!
//! Scene model matches the web renderer (`@vizij/render`): Z-up world, faces
//! in the XY plane layered along Z, orthographic camera fit to the authored
//! `rootBounds`, ambient-only lighting, sRGB output with no tonemapping,
//! double-sided materials, opacity-driven alpha blending.

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;

use bevy::camera::{CameraProjection, Projection, RenderTarget, ScalingMode, SubCameraView};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::gltf::GltfAssetLabel;
use bevy::math::Vec3A;
use bevy::mesh::morph::MorphWeights;
use bevy::prelude::*;
use vizij_api_core::value::{as_bool, as_color_rgba, as_float, as_vec3, as_vector};
use vizij_api_core::Value;

use crate::device::DeviceEvent;
use crate::meta::{Binding, FaceMeta, FeatureKind};

/// The face metadata, as a Bevy resource.
#[derive(Resource)]
pub struct Face {
    pub meta: FaceMeta,
    pub glb_path: String,
}

/// The operator's runtime changes, drained each frame ([`DeviceEvent`]).
/// Absent in snapshot mode.
#[derive(Resource)]
pub struct DeviceEvents(pub Mutex<Receiver<DeviceEvent>>);

/// The device handles the view reads each frame.
#[derive(Resource)]
pub struct DeviceRes {
    pub rig: vizij_arora_hal::RigHal,
}

/// How the camera fits the face's authored rootBounds into the viewport.
#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
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
#[derive(Resource, Clone)]
pub struct OffscreenTarget(pub Handle<Image>);

/// Marker for the view camera.
#[derive(Component)]
pub struct ViewCamera;

/// Index from animatable UUID to the scene entity/feature it drives,
/// built once the GLB scene has spawned.
#[derive(Resource, Default)]
pub struct BindingIndex {
    /// uuid → (target entity, feature, morph index, material shade factor).
    pub by_uuid: HashMap<String, (Entity, FeatureKind, Option<usize>, f32)>,
    pub ready: bool,
}

pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BindingIndex>()
            .add_systems(Startup, (setup_scene, setup_camera))
            .add_systems(
                Update,
                (apply_device_events, index_scene, apply_pose).chain(),
            );
    }
}

fn setup_scene(mut commands: Commands, face: Res<Face>, asset_server: Res<AssetServer>) {
    commands.spawn(WorldAssetRoot(
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(face.glb_path.clone())),
    ));
}

/// Apply the operator's runtime changes: recolor the background, or swap the
/// whole face — despawn the scene, load the new GLB, refit the camera, hand
/// the pose feed over to the new device generation's rig, and let
/// `index_scene` rebuild the joins once the new scene has spawned.
#[allow(clippy::too_many_arguments)]
fn apply_device_events(
    events: Option<Res<DeviceEvents>>,
    mut frame_config: Option<ResMut<crate::frames::FrameConfig>>,
    mut face: ResMut<Face>,
    mut device: ResMut<DeviceRes>,
    mut options: ResMut<ViewOptions>,
    mut index: ResMut<BindingIndex>,
    mut commands: Commands,
    roots: Query<Entity, With<WorldAssetRoot>>,
    mut cameras: Query<(&mut Camera, &mut Projection, &mut Transform), With<ViewCamera>>,
    asset_server: Res<AssetServer>,
) {
    let Some(events) = events else { return };
    let Ok(receiver) = events.0.lock() else {
        return;
    };
    while let Ok(event) = receiver.try_recv() {
        match event {
            DeviceEvent::Background([r, g, b]) => {
                options.background = Color::srgb_u8(r, g, b);
                for (mut camera, _, _) in &mut cameras {
                    camera.clear_color = ClearColorConfig::Custom(options.background);
                }
            }
            DeviceEvent::FaceLoaded {
                glb_path,
                meta,
                rig,
            } => {
                log::info!(
                    "face swapped to {glb_path} ({} elements); reloading the scene",
                    meta.elements.len()
                );
                for root in &roots {
                    commands.entity(root).despawn();
                }
                commands.spawn(WorldAssetRoot(
                    asset_server.load(GltfAssetLabel::Scene(0).from_asset(glb_path.clone())),
                ));
                let (projection, transform) = camera_fit(&meta, &options);
                for (_, mut camera_projection, mut camera_transform) in &mut cameras {
                    *camera_projection = projection.clone();
                    *camera_transform = transform;
                }
                // The published frames are stamped in the loaded face's own
                // frame, so they follow the face.
                if let Some(config) = frame_config.as_mut() {
                    config.face_frame_id =
                        crate::frames::default_frame_id(meta.bundle.face_id.as_deref(), &glb_path);
                }
                *face = Face {
                    meta: *meta,
                    glb_path,
                };
                device.rig = rig;
                *index = BindingIndex::default();
            }
        }
    }
}

/// The camera placement for a face: a [`FitProjection`] over the authored
/// rootBounds, centered on them.
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

fn setup_camera(
    mut commands: Commands,
    face: Res<Face>,
    options: Res<ViewOptions>,
    offscreen: Option<Res<OffscreenTarget>>,
) {
    // Lighting is baked into the materials (see `ViewOptions::albedo_factor`);
    // no scene light is spawned.

    let (projection, transform) = camera_fit(&face.meta, &options);

    let mut camera = commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(options.background),
            ..default()
        },
        projection,
        transform,
        Tonemapping::None,
        DebandDither::Disabled,
        Msaa::Sample4,
        ViewCamera,
    ));
    // The render target is its own component since Bevy 0.17.
    if let Some(target) = &offscreen {
        camera.insert(RenderTarget::Image(target.0.clone().into()));
    }
}

/// Joins the spawned GLB scene with the RobotData bindings: node `Name` →
/// entity; for material/morph features, the mesh primitive child. Also makes
/// each bound mesh's material unique (GLB materials can be shared) and applies
/// the web renderer's material conventions.
#[allow(clippy::too_many_arguments)]
fn index_scene(
    mut index: ResMut<BindingIndex>,
    face: Res<Face>,
    options: Res<ViewOptions>,
    names: Query<(Entity, &Name)>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    meshes: Query<(Entity, &MeshMaterial3d<StandardMaterial>), With<Mesh3d>>,
    morphs: Query<Entity, With<MorphWeights>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    if index.ready {
        return;
    }
    // Wait until every element's node has spawned. Names can collide: the
    // world spawner's own root carries the glTF *scene's* name, which
    // exporters often also give a top-level *node* (Toasty's "Scene"). The
    // element is always the innermost bearer, so on a collision keep the
    // deepest entity.
    let depth = |entity: Entity| {
        let mut depth = 0usize;
        let mut current = entity;
        while let Ok(parent) = parents.get(current) {
            depth += 1;
            current = parent.parent();
        }
        depth
    };
    let mut by_name: HashMap<&str, Entity> = HashMap::new();
    for (entity, name) in &names {
        by_name
            .entry(name.as_str())
            .and_modify(|kept| {
                if depth(entity) > depth(*kept) {
                    *kept = entity;
                }
            })
            .or_insert(entity);
    }
    if !face
        .meta
        .elements
        .iter()
        .all(|e| by_name.contains_key(e.node_name.as_str()))
    {
        return;
    }

    // Per element: the transform target is the named node entity; the
    // material/morph target is its first mesh-bearing descendant.
    let mut mesh_of: HashMap<String, (Entity, Handle<StandardMaterial>)> = HashMap::new();
    let mut morph_of: HashMap<String, Entity> = HashMap::new();
    for element in &face.meta.elements {
        let node = by_name[element.node_name.as_str()];
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
        let Some(&node) = by_name.get(node_name.as_str()) else {
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
        "scene indexed: {} bindings over {} elements",
        by_uuid.len(),
        face.meta.elements.len(),
    );
    index.by_uuid = by_uuid;
    index.ready = true;
}

/// Applies the device's current pose (the HAL's actuation state) onto the
/// scene: transforms, material color/opacity, morph influences.
fn apply_pose(
    index: Res<BindingIndex>,
    device: Res<DeviceRes>,
    mut transforms: Query<&mut Transform>,
    material_handles: Query<&MeshMaterial3d<StandardMaterial>>,
    mut morph_weights: Query<&mut MorphWeights>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !index.ready {
        return;
    }
    for (path, value) in device.rig.pose() {
        let key = path.to_string();
        let Some((entity, feature, morph_index, factor)) = index.by_uuid.get(&key) else {
            continue;
        };
        match feature {
            FeatureKind::Translation => {
                if let (Ok(mut transform), Some(v)) = (transforms.get_mut(*entity), as_xyz(&value))
                {
                    transform.translation = Vec3::from_array(v);
                }
            }
            FeatureKind::Rotation => {
                if let (Ok(mut transform), Some(v)) = (transforms.get_mut(*entity), as_xyz(&value))
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
