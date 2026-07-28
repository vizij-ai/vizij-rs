//! Publish rendered frames into the device's store as HAL sensor readings.
//!
//! A throttled system captures what the view renders — the offscreen target when
//! one is set (headless), otherwise the window — via Bevy's screenshot API, so
//! the pixel-calibrated view camera is never touched. The captured frame is
//! encoded (`--frame-format`) and pushed onto the rig HAL's reading feed under
//! `view/frame`; the runtime lands it in the store and fans it to every bridge
//! (the ROS4HRI route — face frames as a topic, no browser).

use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

use arora_types::data::{Key, StateChange};
use arora_types::keyvalue::{KeyValue, KeyValueField};
use arora_types::value::Value;

use crate::view::{DeviceRes, OffscreenTarget};

/// The store key the rendered frame is published under.
const FRAME_KEY: &str = "view/frame";

/// How a published frame's pixels are encoded.
#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum FrameFormat {
    /// PNG — compact enough to travel over a bridge (the default).
    Png,
    /// Raw RGBA8, row-major, no padding — heavy, but decode-free.
    Raw,
}

/// How the view publishes frames (`--frame-format`, `--frame-rate`).
#[derive(Resource, Clone)]
pub struct FrameConfig {
    pub format: FrameFormat,
    /// Publish rate in Hz, decoupled from the render/step rate.
    pub rate_hz: f32,
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

/// Encode the captured frame and push it onto the rig's reading feed.
fn publish_frame(event: On<ScreenshotCaptured>, device: Res<DeviceRes>, config: Res<FrameConfig>) {
    let Some(reading) = frame_reading(&event.image, config.format) else {
        return;
    };
    device
        .rig
        .push_reading(StateChange::set(Key::from(FRAME_KEY), reading));
}

/// Build the `view/frame` value from a captured image, or `None` if its pixels
/// aren't in a format we can read. RGBA is taken as-is; BGRA is swizzled (window
/// swapchains are commonly BGRA, offscreen targets RGBA).
fn frame_reading(image: &Image, format: FrameFormat) -> Option<Value> {
    let (width, height) = (image.width(), image.height());
    let rgba = to_rgba8(image)?;
    Some(encode_frame(&rgba, width, height, format))
}

/// The image's pixels as row-major RGBA8, or `None` for an unsupported format.
fn to_rgba8(image: &Image) -> Option<Vec<u8>> {
    let data = image.data.as_ref()?;
    match image.texture_descriptor.format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => Some(data.clone()),
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => {
            let mut out = data.clone();
            for px in out.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
            Some(out)
        }
        _ => None,
    }
}

/// The frame as a `view/frame` value: `{ width, height, format, data }`, a
/// string-keyed record so a bridge consumer reads it without a schema.
fn encode_frame(rgba: &[u8], width: u32, height: u32, format: FrameFormat) -> Value {
    let (encoded, format_name) = match format {
        FrameFormat::Raw => (rgba.to_vec(), "rgba8"),
        FrameFormat::Png => (encode_png(rgba, width, height), "png"),
    };
    let mut kv = KeyValue::new();
    for field in [
        KeyValueField::new("width", Value::U32(width)),
        KeyValueField::new("height", Value::U32(height)),
        KeyValueField::new("format", Value::String(format_name.to_string())),
        KeyValueField::new("data", Value::ArrayU8(encoded)),
    ] {
        kv.fields.insert(field.name.clone(), field);
    }
    Value::KeyValue(kv)
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

    fn field<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
        match value {
            Value::KeyValue(kv) => kv.fields.get(name).and_then(|f| f.value.as_deref()),
            _ => None,
        }
    }

    #[test]
    fn raw_frame_carries_the_pixels_verbatim() {
        let rgba = vec![10, 20, 30, 255];
        let value = encode_frame(&rgba, 1, 1, FrameFormat::Raw);
        assert_eq!(field(&value, "width"), Some(&Value::U32(1)));
        assert_eq!(field(&value, "height"), Some(&Value::U32(1)));
        assert_eq!(field(&value, "format"), Some(&Value::String("rgba8".into())));
        assert_eq!(field(&value, "data"), Some(&Value::ArrayU8(rgba)));
    }

    #[test]
    fn png_frame_is_a_decodable_png() {
        let rgba = vec![10, 20, 30, 255, 40, 50, 60, 255];
        let value = encode_frame(&rgba, 2, 1, FrameFormat::Png);
        assert_eq!(field(&value, "format"), Some(&Value::String("png".into())));
        let Some(Value::ArrayU8(png)) = field(&value, "data") else {
            panic!("data is not ArrayU8");
        };
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let decoded = image::load_from_memory(png).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.into_raw(), rgba);
    }
}
