//! The app's half of the view: the operator's runtime changes, and the device
//! standing behind `vizij-render`'s pose feed.
//!
//! The renderer itself — the scene join, the camera fit, the material
//! conventions, the value application — is `vizij-render`. What is here
//! is what only this app has: a live Arora device, and an operator who can
//! recolor the background or swap the whole face while it runs.

use std::sync::mpsc::Receiver;
use std::sync::Mutex;

use bevy::camera::Projection;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

use crate::device::DeviceEvent;

pub use vizij_render::view::{
    camera_fit, BindingIndex, Face, Fit, OffscreenTarget, PoseFeed, ViewCamera, ViewOptions,
    ViewSystems,
};

/// The operator's runtime changes, drained each frame ([`DeviceEvent`]).
/// Absent in snapshot mode.
#[derive(Resource)]
pub struct DeviceEvents(pub Mutex<Receiver<DeviceEvent>>);

/// The device handle the app reads and writes outside the render path — the
/// frame publisher pushes readings through it.
#[derive(Resource)]
pub struct DeviceRes {
    pub rig: vizij_arora_hal::RigHal,
}

/// A pose feed reading the rig's actuation state.
pub fn pose_feed(rig: &vizij_arora_hal::RigHal) -> PoseFeed {
    let rig = rig.clone();
    PoseFeed::new(move || rig.pose())
}

/// The render plugin plus this app's operator handling, ordered so a face swap
/// is visible to the scene join in the same frame.
pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(vizij_render::ViewPlugin)
            .add_systems(Update, apply_device_events.before(ViewSystems));
    }
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
                let assets = *meta;
                let (projection, transform) = camera_fit(&assets.render, &options);
                for (_, mut camera_projection, mut camera_transform) in &mut cameras {
                    *camera_projection = projection.clone();
                    *camera_transform = transform;
                }
                // The published frames are stamped in the loaded face's own
                // frame, so they follow the face.
                if let Some(config) = frame_config.as_mut() {
                    config.face_frame_id = crate::frames::default_frame_id(
                        assets.bundle.face_id.as_deref(),
                        &glb_path,
                    );
                }
                *face = Face {
                    meta: assets.render,
                    glb_path,
                };
                commands.insert_resource(pose_feed(&rig));
                device.rig = rig;
                *index = BindingIndex::default();
            }
        }
    }
}
