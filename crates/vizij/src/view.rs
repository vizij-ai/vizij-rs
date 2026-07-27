//! The head: Bevy systems rendering the face and applying the device's pose.
//!
//! Scene model matches the web renderer (`@vizij/render`): Z-up world, faces
//! in the XY plane layered along Z, orthographic camera fit to the authored
//! `rootBounds`, ambient-only lighting, sRGB output with no tonemapping,
//! double-sided materials, opacity-driven alpha blending.

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;

use bevy::camera::{Projection, RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::gltf::GltfAssetLabel;
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
}

/// View options (CLI knobs while calibrating against the web renderer).
#[derive(Resource, Clone)]
pub struct ViewOptions {
    /// Background clear color.
    pub background: Color,
    /// How the face fits the window.
    pub fit: Fit,
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
                let (scaling, transform) = camera_fit(&meta, options.fit);
                for (_, mut projection, mut camera_transform) in &mut cameras {
                    if let Projection::Orthographic(orthographic) = &mut *projection {
                        orthographic.scaling_mode = scaling;
                    }
                    *camera_transform = transform;
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

/// The camera placement for a face: orthographic fit to the authored
/// rootBounds, centered on them. Contain keeps at least the bounds visible
/// (the web computes zoom = min(w/bw, h/bh)); cover keeps at most.
fn camera_fit(meta: &FaceMeta, fit: Fit) -> (ScalingMode, Transform) {
    let (cx, cy, bw, bh) = meta.root_bounds.unwrap_or((0.0, 0.0, 5.0, 4.0));
    let scaling = match fit {
        Fit::Contain => ScalingMode::AutoMin {
            min_width: bw,
            min_height: bh,
        },
        Fit::Cover => ScalingMode::AutoMax {
            max_width: bw,
            max_height: bh,
        },
    };
    (
        scaling,
        Transform::from_xyz(cx, cy, 100.0).looking_at(Vec3::new(cx, cy, 0.0), Vec3::Y),
    )
}

fn setup_camera(
    mut commands: Commands,
    face: Res<Face>,
    options: Res<ViewOptions>,
    offscreen: Option<Res<OffscreenTarget>>,
) {
    // Lighting is baked into the materials (see `ViewOptions::albedo_factor`);
    // no scene light is spawned.

    let (scaling, transform) = camera_fit(&face.meta, options.fit);
    let mut projection = OrthographicProjection::default_3d();
    projection.scaling_mode = scaling;

    let mut camera = commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(options.background),
            ..default()
        },
        Projection::Orthographic(projection),
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
