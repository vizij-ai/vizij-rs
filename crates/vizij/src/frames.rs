//! Publish rendered frames into the device's store as HAL sensor readings.
//!
//! A throttled system captures what the view renders — the offscreen target when
//! one is set (headless), otherwise the window — via Bevy's screenshot API, so
//! the pixel-calibrated view camera is never touched. The captured frame is
//! encoded (`--frame-format`) and pushed onto the rig HAL's reading feed under
//! [`FRAME_KEY`]; the runtime lands it in the store and fans it to every bridge
//! (the ROS4HRI route — face frames as a topic, no browser).
//!
//! The value is the ROS image message itself — `sensor_msgs/Image` for raw
//! pixels, `sensor_msgs/CompressedImage` for an encoded frame — built by
//! [`vizij_arora_host::frames`], the shape both hosts share. A ROS 2 bridge
//! declaring the key as that type publishes the frame with no conversion and no
//! schema lookup, which is what keeps a frame off the JSON fallback: a
//! 300 KB PNG would otherwise ride as a `std_msgs/String`.

use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

use std::time::SystemTime;

use arora_types::data::{Key, StateChange};
use arora_types::value::Value;
use vizij_arora_host::frames as host;

use crate::view::{DeviceRes, OffscreenTarget};

pub(crate) use vizij_arora_host::frames::FRAME_KEY;

/// How a published frame's pixels are encoded — and, with it, which ROS image
/// message and topic the frame rides.
#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum FrameFormat {
    /// PNG — compact enough to travel over a bridge (the default); a
    /// `sensor_msgs/CompressedImage`.
    Png,
    /// Raw RGBA8, row-major, no padding — heavy, but decode-free; a
    /// `sensor_msgs/Image`.
    Raw,
}

/// The CLI enum is this crate's; the ROS message name and topic a format
/// implies belong to the shared definition, so a bridge asks that one.
impl From<FrameFormat> for host::FrameFormat {
    fn from(format: FrameFormat) -> Self {
        match format {
            FrameFormat::Png => host::FrameFormat::Png,
            FrameFormat::Raw => host::FrameFormat::Raw,
        }
    }
}

/// How the view publishes frames (`--frame-format`, `--frame-rate`).
#[derive(Resource, Clone)]
pub struct FrameConfig {
    pub format: FrameFormat,
    /// Publish rate in Hz, decoupled from the render/step rate. At or below
    /// zero the view captures nothing and the key is never written.
    pub rate_hz: f32,
}

impl FrameConfig {
    /// Whether the view will write [`FRAME_KEY`] at all — what decides both
    /// whether the capture system runs and whether a bridge declares the key.
    pub fn publishes(&self) -> bool {
        self.rate_hz > 0.0
    }
}

pub struct FramesPlugin;

impl Plugin for FramesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, request_capture);
    }
}

/// Throttled at `rate_hz`: request a capture of the offscreen target (headless)
/// or the primary window, with [`publish_frame`] as its observer. Skips while a
/// capture is still in flight, so a slow readback throttles the rate rather than
/// piling up requests.
fn request_capture(
    mut commands: Commands,
    config: Res<FrameConfig>,
    offscreen: Option<Res<OffscreenTarget>>,
    time: Res<Time>,
    mut since_last: Local<f32>,
    in_flight: Query<(), With<Screenshot>>,
) {
    *since_last += time.delta_secs();
    let period = 1.0 / config.rate_hz.max(0.1);
    if *since_last < period || !in_flight.is_empty() {
        return;
    }
    *since_last = 0.0;
    let screenshot = match offscreen.as_ref() {
        Some(target) => Screenshot::image(target.0.clone()),
        None => Screenshot::primary_window(),
    };
    commands.spawn(screenshot).observe(publish_frame);
}

/// Encode the captured frame and push it onto the rig's reading feed. RGBA is
/// taken as-is; BGRA is swizzled (window swapchains are commonly BGRA, offscreen
/// targets RGBA); an unreadable format is skipped.
fn publish_frame(
    event: On<ScreenshotCaptured>,
    device: Res<DeviceRes>,
    config: Res<FrameConfig>,
    mut announced: Local<bool>,
) {
    let image = &event.image;
    let (width, height) = (image.width(), image.height());
    let Some(rgba) = to_rgba8(image) else {
        return;
    };
    if !std::mem::replace(&mut *announced, true) {
        log::info!(
            "publishing {FRAME_KEY} into the store ({width}x{height}, {:?})",
            config.format
        );
    }
    let reading = encode_frame(&rgba, width, height, config.format);
    device
        .rig
        .push_reading(StateChange::set(Key::from(FRAME_KEY), reading));
}

/// The image's pixels as row-major RGBA8, or `None` for an unsupported format.
fn to_rgba8(image: &Image) -> Option<Vec<u8>> {
    let data = image.data.as_ref()?;
    match image.texture_descriptor.format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => Some(data.clone()),
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => {
            let mut out = data.clone();
            for px in out.as_chunks_mut::<4>().0 {
                px.swap(0, 2);
            }
            Some(out)
        }
        _ => None,
    }
}

/// The frame as the ROS image message its format names, stamped now.
pub(crate) fn encode_frame(rgba: &[u8], width: u32, height: u32, format: FrameFormat) -> Value {
    let stamp = SystemTime::now();
    match format {
        FrameFormat::Raw => host::raw_frame(width, height, rgba.to_vec(), stamp),
        FrameFormat::Png => host::compressed_frame("png", encode_png(rgba, width, height), stamp),
    }
}

/// PNG-encode RGBA8 pixels; on failure (never expected for valid dimensions),
/// falls back to the raw bytes so a frame still ships.
fn encode_png(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut out);
    match encoder.write_image(rgba, width, height, image::ExtendedColorType::Rgba8) {
        Ok(()) => out,
        Err(_) => {
            log::warn!("frame PNG encode failed; shipping raw");
            rgba.to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_msgs_ros2::sensor_msgs::{CompressedImage, Image};
    use arora_types::value_serde::bridge::from_value;

    /// What the view contributes to a frame is the pixels: the swizzle that
    /// makes a BGRA swapchain readback RGBA, and the PNG encoding. Both tests
    /// read the message back as its Rust struct, so a field landing in the
    /// wrong place fails here rather than at a ROS subscriber.
    #[test]
    fn a_raw_frame_carries_the_pixels_as_a_sensor_msgs_image() {
        let rgba = vec![10, 20, 30, 255, 40, 50, 60, 255];
        let value = encode_frame(&rgba, 2, 1, FrameFormat::Raw);
        let frame: Image = from_value(value).expect("a raw frame is a sensor_msgs/Image");
        assert_eq!((frame.width, frame.height), (2, 1));
        assert_eq!(frame.encoding, "rgba8");
        assert_eq!(frame.step, 8, "one row of 2 RGBA pixels");
        assert_eq!(frame.data, rgba);
    }

    #[test]
    fn a_png_frame_is_a_decodable_sensor_msgs_compressed_image() {
        let rgba = vec![10, 20, 30, 255, 40, 50, 60, 255];
        let value = encode_frame(&rgba, 2, 1, FrameFormat::Png);
        let frame: CompressedImage =
            from_value(value).expect("a png frame is a sensor_msgs/CompressedImage");
        assert_eq!(frame.format, "png");
        assert_eq!(
            &frame.data[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
        let decoded = image::load_from_memory(&frame.data).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.into_raw(), rgba);
    }

    /// The declared ROS type has to be the one the value actually is, or the
    /// bridge encodes a frame against the wrong message.
    #[test]
    fn each_format_declares_the_message_it_encodes() {
        use host::FrameFormat as Ros;
        assert_eq!(Ros::from(FrameFormat::Raw).ros_type(), "sensor_msgs/Image");
        assert_eq!(
            Ros::from(FrameFormat::Png).ros_type(),
            "sensor_msgs/CompressedImage"
        );
        assert_ne!(
            Ros::from(FrameFormat::Raw).topic(),
            Ros::from(FrameFormat::Png).topic()
        );
    }
}
