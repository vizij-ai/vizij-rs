//! Publish rendered frames into the device's store as HAL sensor readings.
//!
//! A throttled system captures what the view renders — the offscreen target when
//! one is set (headless), otherwise the window — via Bevy's screenshot API, so
//! the pixel-calibrated view camera is never touched. The captured frame is
//! encoded (`--frame-format`) and pushed onto the rig HAL's reading feed under
//! the key of its transport ([`host::FrameFormat::key`]); the runtime lands it
//! in the store and fans it to every bridge (the ROS4HRI route — face frames
//! as a topic, no browser).
//!
//! The value is the ROS image message itself — `sensor_msgs/Image` for raw
//! pixels, `sensor_msgs/CompressedImage` for an encoded frame — built by
//! [`vizij_arora_host::frames`], the shape both hosts share. The ROS4HRI
//! exposure profile publishes each key as that message on its
//! `image_transport` topic with no conversion and no schema lookup, which is
//! what keeps a frame off the JSON fallback: a 300 KB PNG would otherwise ride
//! as a `std_msgs/String`. Frames are that profile's face image, so by default
//! the view publishes them exactly when the device is exposed as ROS4HRI, and
//! it refuses to publish them on a ROS 2 bridge without that profile.

use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

use std::time::SystemTime;

use arora_types::data::{Key, StateChange};
use arora_types::value::Value;
use vizij_arora_host::frames as host;

use crate::view::{DeviceRes, OffscreenTarget};

/// The rate frames publish at when nothing says otherwise.
pub const DEFAULT_RATE_HZ: f32 = 15.0;

/// How a published frame's pixels are encoded — and, with it, which ROS image
/// message the frame is and which store key it is written under.
#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum FrameFormat {
    /// PNG — compact enough to travel over a bridge (the default); a
    /// `sensor_msgs/CompressedImage`.
    Png,
    /// Raw RGBA8, row-major, no padding — heavy, but decode-free; a
    /// `sensor_msgs/Image`.
    Raw,
}

/// The CLI enum is this crate's; the store key and ROS message a format
/// implies belong to the shared definition, so everything else asks that one.
impl From<FrameFormat> for host::FrameFormat {
    fn from(format: FrameFormat) -> Self {
        match format {
            FrameFormat::Png => host::FrameFormat::Png,
            FrameFormat::Raw => host::FrameFormat::Raw,
        }
    }
}

/// How the view publishes frames (`--frame-format`, `--frame-rate`,
/// `--frame-id`).
#[derive(Resource, Clone)]
pub struct FrameConfig {
    pub format: FrameFormat,
    /// Publish rate in Hz, decoupled from the render/step rate. At or below
    /// zero the view captures nothing and no frame key is written.
    pub rate_hz: f32,
    /// `--frame-id`, when given: the TF frame every face is stamped in.
    pub fixed_frame_id: Option<String>,
    /// The loaded face's own TF frame ([`default_frame_id`]); the view moves
    /// it when the operator loads another face.
    pub face_frame_id: String,
}

impl FrameConfig {
    /// Whether the view will write a frame key at all — what decides whether
    /// the capture system runs.
    pub fn publishes(&self) -> bool {
        self.rate_hz > 0.0
    }

    /// The TF frame frames are stamped with (`header.frame_id`): the one
    /// `--frame-id` fixed, else the loaded face's own.
    pub fn frame_id(&self) -> &str {
        self.fixed_frame_id
            .as_deref()
            .unwrap_or(&self.face_frame_id)
    }
}

/// The rate frames publish at for a run. A rate given on the command line is
/// taken as given; otherwise frames follow the ROS4HRI exposure —
/// [`DEFAULT_RATE_HZ`] when the device is exposed as ROS4HRI, none when it is
/// not. A rate under `--ros2 --no-ros4hri` is refused: with no ROS4HRI profile
/// to type the frame key, every frame would ride the bridge's JSON scalar plane
/// as a `std_msgs/String` the size of the image.
pub fn publish_rate(given: Option<f32>, ros2: bool, ros4hri: bool) -> anyhow::Result<f32> {
    match given {
        Some(rate) if rate > 0.0 && ros2 && !ros4hri => anyhow::bail!(
            "--frame-rate {rate} with --ros2 --no-ros4hri: without the ROS4HRI profile a frame \
             would publish as JSON on the scalar plane; drop --no-ros4hri or the rate"
        ),
        Some(rate) => Ok(rate),
        None if ros2 && ros4hri => Ok(DEFAULT_RATE_HZ),
        None => Ok(0.0),
    }
}

/// The TF frame a face's image is stamped with when `--frame-id` is not
/// given: the face's id from its GLB (the bundle's `metadata.faceId`, e.g.
/// `quori_latest`), which names the face rather than the device it runs on;
/// a GLB without one is named by its file stem.
pub fn default_frame_id(face_id: Option<&str>, glb_path: &str) -> String {
    match face_id {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => std::path::Path::new(glb_path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "face".to_string()),
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
            "publishing {} into the store ({width}x{height}, {:?}, frame {:?})",
            host::FrameFormat::from(config.format).key(),
            config.format,
            config.frame_id()
        );
    }
    device
        .rig
        .push_reading(frame_reading(&rgba, width, height, &config));
}

/// The reading a captured frame becomes: the key of the configured format,
/// holding the frame as the message that key is published as, stamped in the
/// configured TF frame.
pub(crate) fn frame_reading(
    rgba: &[u8],
    width: u32,
    height: u32,
    config: &FrameConfig,
) -> StateChange {
    let key = host::FrameFormat::from(config.format).key();
    StateChange::set(
        Key::from(key),
        encode_frame(rgba, width, height, config.format, config.frame_id()),
    )
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

/// The frame as the ROS image message its format names, stamped now in the TF
/// frame `frame_id`.
pub(crate) fn encode_frame(
    rgba: &[u8],
    width: u32,
    height: u32,
    format: FrameFormat,
    frame_id: &str,
) -> Value {
    let stamp = SystemTime::now();
    match format {
        FrameFormat::Raw => host::raw_frame(width, height, rgba.to_vec(), stamp, frame_id),
        FrameFormat::Png => host::compressed_frame(
            host::PNG_FORMAT,
            encode_png(rgba, width, height),
            stamp,
            frame_id,
        ),
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
        let value = encode_frame(&rgba, 2, 1, FrameFormat::Raw, "quori_latest");
        let frame: Image = from_value(value).expect("a raw frame is a sensor_msgs/Image");
        assert_eq!((frame.width, frame.height), (2, 1));
        assert_eq!(frame.encoding, "rgba8");
        assert_eq!(frame.step, 8, "one row of 2 RGBA pixels");
        assert_eq!(frame.data, rgba);
        assert_eq!(frame.header.frame_id, "quori_latest");
    }

    #[test]
    fn a_png_frame_is_a_decodable_sensor_msgs_compressed_image() {
        let rgba = vec![10, 20, 30, 255, 40, 50, 60, 255];
        let value = encode_frame(&rgba, 2, 1, FrameFormat::Png, "quori_latest");
        let frame: CompressedImage =
            from_value(value).expect("a png frame is a sensor_msgs/CompressedImage");
        assert_eq!(frame.header.frame_id, "quori_latest");
        assert_eq!(frame.format, "rgba8; png compressed rgba8");
        assert_eq!(
            &frame.data[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
        let decoded = image::load_from_memory(&frame.data).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.into_raw(), rgba);
    }

    fn config(format: FrameFormat) -> FrameConfig {
        FrameConfig {
            format,
            rate_hz: 1.0,
            fixed_frame_id: None,
            face_frame_id: "quori_latest".into(),
        }
    }

    /// What the view writes is the format's key holding the format's message:
    /// the seam between the capture and the store, where a key/message mix-up
    /// would be silent (the profile would encode the value against the other
    /// message, frame after frame, and publish nothing).
    #[test]
    fn a_reading_lands_on_the_key_of_its_format_holding_that_formats_message() {
        let rgba = vec![10, 20, 30, 255, 40, 50, 60, 255];
        for format in [FrameFormat::Raw, FrameFormat::Png] {
            let change = frame_reading(&rgba, 2, 1, &config(format));
            let key = host::FrameFormat::from(format).key();
            let keys: Vec<&str> = change.set.keys().map(|k| k.path.as_str()).collect();
            assert_eq!(keys, [key], "{format:?}");
            let value = change.set[&Key::from(key)]
                .clone()
                .expect("a frame is a set, not an unset");
            let frame_id = match format {
                FrameFormat::Raw => {
                    from_value::<Image>(value)
                        .expect("an Image")
                        .header
                        .frame_id
                }
                FrameFormat::Png => {
                    from_value::<CompressedImage>(value)
                        .expect("a CompressedImage")
                        .header
                        .frame_id
                }
            };
            assert_eq!(frame_id, "quori_latest");
        }
    }

    /// `--frame-id` fixes the frame; without it the frame is the loaded
    /// face's, and follows a reload.
    #[test]
    fn the_frame_id_is_the_fixed_one_or_the_loaded_faces() {
        let mut following = config(FrameFormat::Png);
        assert_eq!(following.frame_id(), "quori_latest");
        following.face_frame_id = "hugo_latest".into();
        assert_eq!(following.frame_id(), "hugo_latest");
        let mut fixed = FrameConfig {
            fixed_frame_id: Some("head_display".into()),
            ..config(FrameFormat::Png)
        };
        assert_eq!(fixed.frame_id(), "head_display");
        fixed.face_frame_id = "hugo_latest".into();
        assert_eq!(fixed.frame_id(), "head_display");
    }

    /// Frames follow the ROS4HRI exposure unless a rate is given, and a rate
    /// on a ROS 2 bridge without the profile is refused rather than let the
    /// frame ride the JSON scalar plane.
    #[test]
    fn frames_follow_the_ros4hri_exposure_unless_a_rate_is_given() {
        assert_eq!(publish_rate(None, true, true).unwrap(), DEFAULT_RATE_HZ);
        assert_eq!(publish_rate(None, true, false).unwrap(), 0.0);
        assert_eq!(publish_rate(None, false, true).unwrap(), 0.0);
        assert_eq!(publish_rate(None, false, false).unwrap(), 0.0);
        assert_eq!(publish_rate(Some(2.0), false, false).unwrap(), 2.0);
        assert_eq!(publish_rate(Some(2.0), true, true).unwrap(), 2.0);
        assert_eq!(publish_rate(Some(0.0), true, true).unwrap(), 0.0);
        assert_eq!(publish_rate(Some(0.0), true, false).unwrap(), 0.0);
        let refused = publish_rate(Some(2.0), true, false)
            .unwrap_err()
            .to_string();
        assert!(refused.contains("--no-ros4hri"), "{refused}");
    }

    /// The face is named by its GLB; only a GLB without a face id falls back
    /// to the file's name.
    #[test]
    fn the_default_frame_id_is_the_faces_id_from_its_glb() {
        assert_eq!(
            default_frame_id(Some("quori_latest"), "/faces/Quori_Latest_ROS.glb"),
            "quori_latest"
        );
        assert_eq!(
            default_frame_id(None, "/faces/Quori_Latest_ROS.glb"),
            "Quori_Latest_ROS"
        );
        assert_eq!(default_frame_id(Some(""), "hugo.glb"), "hugo");
    }

    /// The key each format writes is the one the ROS4HRI exposure profile
    /// publishes as that format's message — a contract between this crate's
    /// value and the bridge's endpoint, spelled in two crates. A mismatch is
    /// silent at run time either way (an undeclared key rides the JSON
    /// fallback, a declared key never written never publishes), so it is
    /// asserted here, against the profile itself.
    #[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
    #[test]
    fn each_format_writes_the_key_the_ros4hri_profile_publishes_as_its_message() {
        use arora_bridge_ros2::profile::Flow;
        let profile = arora_bridge_ros2::ExposureProfile::ros4hri();
        for format in [FrameFormat::Raw, FrameFormat::Png] {
            let ros = host::FrameFormat::from(format);
            let endpoint = profile
                .endpoints
                .iter()
                .find(|e| {
                    e.flow == Flow::Out
                        && e.routes
                            .iter()
                            .any(|r| r.field.is_empty() && r.key == ros.key())
                })
                .unwrap_or_else(|| panic!("the ros4hri profile publishes {} whole", ros.key()));
            assert_eq!(
                endpoint.ros_type,
                ros.ros_type(),
                "{format:?} writes {} and the profile publishes it on {}",
                ros.key(),
                endpoint.topic
            );
        }
    }
}
