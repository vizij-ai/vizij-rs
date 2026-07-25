//! Headless offscreen rendering: render the face to a texture without a
//! window, read the pixels back, save a PNG. The comparison harness against
//! the web renderer.
//!
//! Recipe from ros-viz-rs `src/snapshot.rs`: `WinitPlugin` disabled (no event
//! loop), `ScheduleRunnerPlugin` + manual `App::update`, camera renders into a
//! `RenderTarget` image with `COPY_SRC`, `gpu_readback` delivers frames.

use std::time::Duration;

use anyhow::{bail, ensure, Context, Result};
use bevy::app::{PluginsState, ScheduleRunnerPlugin};
use bevy::asset::{AssetPlugin, UnapprovedPathMode};
use bevy::image::TextureFormatPixelInfo;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::renderer::RenderDevice;
use bevy::render::RenderPlugin;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use image::RgbaImage;

use crate::view::OffscreenTarget;

/// Matches `TextureFormat::bevy_default()` on desktop; keeps readback bytes in
/// plain RGBA order.
const FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

/// Bounded wait for the async GPU readback (~2 s at 60 fps) so a missing GPU
/// fails fast instead of hanging.
const MAX_READBACK_FRAMES: u32 = 120;

/// Latest raw (row-padded) frame from the GPU readback.
#[derive(Resource, Default)]
struct Frame(Option<Vec<u8>>);

/// Windowless app base: DefaultPlugins without a window or winit, driven by
/// manual updates. Inserts the offscreen render target the view camera uses.
pub struct SnapshotPlugin {
    pub width: u32,
    pub height: u32,
}

impl Plugin for SnapshotPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .set(RenderPlugin {
                    synchronous_pipeline_compilation: true,
                    ..default()
                })
                // The face GLB is given by absolute path, outside assets/.
                .set(AssetPlugin {
                    unapproved_path_mode: UnapprovedPathMode::Allow,
                    ..default()
                })
                .disable::<WinitPlugin>()
                // The app owns logging (env_logger).
                .disable::<bevy::log::LogPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .init_resource::<Frame>();

        // The render-target image the view camera draws into.
        let mut target = Image::new_target_texture(self.width, self.height, FORMAT, None);
        target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let handle = app.world_mut().resource_mut::<Assets<Image>>().add(target);
        app.insert_resource(OffscreenTarget(handle.clone()));
        app.world_mut().spawn(Readback::texture(handle)).observe(
            |event: On<ReadbackComplete>, mut frame: ResMut<Frame>| {
                frame.0 = Some(event.data.clone());
            },
        );
    }
}

/// Finish deferred plugin setup so manual `App::update` calls work. Idempotent.
pub fn ensure_ready(app: &mut App) {
    if app.plugins_state() != PluginsState::Cleaned {
        while app.plugins_state() == PluginsState::Adding {
            bevy::tasks::tick_global_task_pools_on_main_thread();
        }
        app.finish();
        app.cleanup();
    }
}

/// Updates until `ready` reports true (the scene is spawned/bound), then a few
/// settle frames, then captures the next fresh frame.
pub fn capture(
    app: &mut App,
    width: u32,
    height: u32,
    settle_frames: u32,
    mut ready: impl FnMut(&mut App) -> bool,
) -> Result<RgbaImage> {
    ensure_ready(app);

    const MAX_READY_FRAMES: u32 = 600;
    let mut was_ready = false;
    for _ in 0..MAX_READY_FRAMES {
        app.update();
        if ready(app) {
            was_ready = true;
            break;
        }
    }
    if !was_ready {
        bail!("scene did not become ready within {MAX_READY_FRAMES} updates");
    }

    for _ in 0..settle_frames {
        app.update();
    }

    app.world_mut().resource_mut::<Frame>().0 = None;
    for _ in 0..MAX_READBACK_FRAMES {
        app.update();
        if let Some(data) = app.world_mut().resource_mut::<Frame>().0.take() {
            return frame_to_image(&data, width, height);
        }
    }
    bail!("GPU readback did not deliver a frame within {MAX_READBACK_FRAMES} updates");
}

/// Strips wgpu's 256-byte row padding and builds the image.
fn frame_to_image(data: &[u8], width: u32, height: u32) -> Result<RgbaImage> {
    let pixel_size = FORMAT
        .pixel_size()
        .map_err(|e| anyhow::anyhow!("cannot get pixel size of {FORMAT:?}: {e:?}"))?;
    let unpadded_row = width as usize * pixel_size;
    let padded_row = RenderDevice::align_copy_bytes_per_row(unpadded_row);
    ensure!(
        data.len() == padded_row * height as usize,
        "unexpected readback size: got {} bytes for {width}x{height}",
        data.len(),
    );
    let mut pixels = Vec::with_capacity(unpadded_row * height as usize);
    for row in data.chunks_exact(padded_row) {
        pixels.extend_from_slice(&row[..unpadded_row]);
    }
    RgbaImage::from_raw(width, height, pixels).context("readback buffer mismatch")
}

/// Saves a captured frame as PNG.
pub fn save_png(img: &RgbaImage, path: &std::path::Path) -> Result<()> {
    img.save_with_format(path, image::ImageFormat::Png)
        .with_context(|| format!("failed to save PNG to {}", path.display()))
}
