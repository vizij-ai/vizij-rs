//! Publish rendered frames into the device's store as HAL sensor readings.
//!
//! A system captures what the view renders — the offscreen target when one is
//! set (headless), otherwise the window — via Bevy's screenshot API, so the
//! pixel-calibrated view camera is never touched, at the rate the face's rig
//! holds ([`RigHal::frame_rate`]): the input
//! [`FRAME_RATE`](vizij_arora_hal::FRAME_RATE), which a client writes to turn
//! the frames on, off, or to another rate. The captured frame is encoded
//! (`--frame-format`) and pushed onto the rig HAL's reading feed under the key
//! of its transport ([`host::FrameFormat::key`]); the runtime lands it in the
//! store and fans it to every bridge (the ROS4HRI route — face frames as a
//! topic, no browser — and Semio Studio's).
//!
//! The value is the ROS image message itself — `sensor_msgs/Image` for raw
//! pixels, `sensor_msgs/CompressedImage` for an encoded frame — built by
//! [`vizij_arora_host::frames`], the shape both hosts share. The ROS4HRI
//! exposure profile publishes each key as that message on its
//! `image_transport` topic with no conversion and no schema lookup, which is
//! what keeps a frame off the JSON fallback: a 300 KB PNG would otherwise ride
//! as a `std_msgs/String`. So a device on a ROS 2 bridge without that profile
//! publishes no frame, and offers no rate to set ([`publish_rate`]).
//!
//! [`RigHal::frame_rate`]: vizij_arora_hal::RigHal::frame_rate

use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

use std::time::SystemTime;

use arora_types::data::{Key, StateChange};
use arora_types::value::Value;
use vizij_arora_host::frames as host;

use super::meta::FaceMeta;
use super::{Face, OffscreenTarget};

/// How a published frame's pixels are encoded — and, with it, which ROS image
/// message the frame is and which store key it is written under.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "desktop", derive(clap::ValueEnum))]
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

/// How the view publishes frames (`--frame-format`, `--frame-id`); the rate
/// is the rig's ([`RigHal::frame_rate`](vizij_arora_hal::RigHal::frame_rate)).
#[derive(Resource, Clone)]
pub struct FrameConfig {
    pub format: FrameFormat,
    /// `--frame-id`, when given: the TF frame every face is stamped in.
    pub fixed_frame_id: Option<String>,
    /// The loaded face's own TF frame ([`default_frame_id`]); the view moves
    /// it when the operator loads another face.
    pub face_frame_id: String,
}

impl FrameConfig {
    /// The TF frame frames are stamped with (`header.frame_id`): the one
    /// `--frame-id` fixed, else the loaded face's own.
    pub fn frame_id(&self) -> &str {
        self.fixed_frame_id
            .as_deref()
            .unwrap_or(&self.face_frame_id)
    }
}

/// The rate a run's frames publish at from its start
/// ([`FaceConfig::frame_rate`](crate::face::FaceConfig::frame_rate)): the one
/// given on the command line, else none until a client writes the rig's
/// [`FRAME_RATE`](vizij_arora_hal::FRAME_RATE). `None` under `--ros2
/// --no-ros4hri`, a device that publishes no frame and offers no rate to set:
/// with no ROS4HRI profile to type the frame key, every frame would ride the
/// ROS 2 bridge's JSON scalar plane as a `std_msgs/String` the size of the
/// image. A rate given there is refused.
pub fn publish_rate(given: Option<f64>, ros2: bool, ros4hri: bool) -> anyhow::Result<Option<f64>> {
    match given {
        Some(rate) if rate > 0.0 && ros2 && !ros4hri => anyhow::bail!(
            "--frame-rate {rate} with --ros2 --no-ros4hri: without the ROS4HRI profile a frame \
             would publish as JSON on the scalar plane; drop --no-ros4hri or the rate"
        ),
        _ if ros2 && !ros4hri => Ok(None),
        given => Ok(Some(given.unwrap_or(0.0))),
    }
}

/// The TF frame a face's image is stamped with when `--frame-id` is not
/// given: the face's id from its GLB (the bundle's `metadata.faceId`, e.g.
/// `quori_latest`), which names the face rather than the device it runs on;
/// a GLB without one is `face`.
pub fn default_frame_id(meta: &FaceMeta) -> String {
    meta.bundle
        .face_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .unwrap_or("face")
        .to_string()
}

pub struct FramesPlugin;

impl Plugin for FramesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, request_capture);
    }
}

/// At the rate the shown face's rig holds: request a capture of the offscreen
/// target (headless) or the primary window, with [`publish_frame`] as its
/// observer. Skips while a capture is still in flight, so a slow readback
/// throttles the rate rather than piling up requests. The first frame after
/// the rate turns on is captured at once; at a rate of zero nothing is
/// captured.
fn request_capture(
    mut commands: Commands,
    faces: Query<&Face>,
    offscreen: Option<Res<OffscreenTarget>>,
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut publishing: Local<f64>,
    in_flight: Query<(), With<Screenshot>>,
) {
    let rate = faces
        .iter()
        .min_by_key(|face| face.slot)
        .map_or(0.0, |face| face.rig.frame_rate());
    if rate != *publishing {
        if rate > 0.0 {
            log::info!("publishing the face's frames at {rate} Hz");
        } else {
            log::info!("the face's frames stopped");
        }
        *publishing = rate;
    }
    if rate <= 0.0 {
        *since_last = f32::INFINITY;
        return;
    }
    *since_last += time.delta_secs();
    if *since_last < 1.0 / rate as f32 || !in_flight.is_empty() {
        return;
    }
    *since_last = 0.0;
    let screenshot = match offscreen.as_ref() {
        Some(target) => Screenshot::image(target.0.clone()),
        None => Screenshot::primary_window(),
    };
    commands.spawn(screenshot).observe(publish_frame);
}

/// Encode the captured frame and push it onto the rig's reading feed of the
/// face the target shows — the one in the lowest slot, which is the only
/// one where a frame is published from (the desktop and headless runs show
/// one face). RGBA is taken as-is; BGRA is swizzled (window swapchains are
/// commonly BGRA, offscreen targets RGBA); an unreadable format is skipped,
/// and so is a frame captured before the rate turned to zero.
fn publish_frame(
    event: On<ScreenshotCaptured>,
    faces: Query<&Face>,
    config: Res<FrameConfig>,
    mut announced: Local<bool>,
) {
    let Some(face) = faces.iter().min_by_key(|face| face.slot) else {
        return;
    };
    if face.rig.frame_rate() <= 0.0 {
        return;
    }
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
    face.rig
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

    /// Frames start off unless a rate is given, and a device on a ROS 2 bridge
    /// without the profile publishes none: a rate given there is refused
    /// rather than let the frame ride the JSON scalar plane.
    #[test]
    fn frames_start_off_unless_a_rate_is_given() {
        assert_eq!(publish_rate(None, true, true).unwrap(), Some(0.0));
        assert_eq!(publish_rate(None, false, true).unwrap(), Some(0.0));
        assert_eq!(publish_rate(None, false, false).unwrap(), Some(0.0));
        assert_eq!(publish_rate(Some(2.0), false, false).unwrap(), Some(2.0));
        assert_eq!(publish_rate(Some(2.0), true, true).unwrap(), Some(2.0));
        assert_eq!(publish_rate(Some(0.0), true, true).unwrap(), Some(0.0));
        assert_eq!(publish_rate(None, true, false).unwrap(), None);
        assert_eq!(publish_rate(Some(0.0), true, false).unwrap(), None);
        let refused = publish_rate(Some(2.0), true, false)
            .unwrap_err()
            .to_string();
        assert!(refused.contains("--no-ros4hri"), "{refused}");
    }

    /// The captures the view requested.
    #[derive(Resource, Default)]
    struct Captured(u32);

    /// Complete every capture requested this frame — despawn its
    /// `Screenshot` — and count it.
    fn complete(
        mut commands: Commands,
        shots: Query<Entity, With<Screenshot>>,
        mut captured: ResMut<Captured>,
    ) {
        for shot in &shots {
            commands.entity(shot).despawn();
            captured.0 += 1;
        }
    }

    /// The captures requested over `seconds` of 60 Hz frames.
    fn captures(app: &mut App, seconds: u32) -> u32 {
        app.world_mut().resource_mut::<Captured>().0 = 0;
        for _ in 0..seconds * 60 {
            app.update();
        }
        app.world().resource::<Captured>().0
    }

    /// The view publishes at the rate the shown face's rig holds: none from a
    /// start at zero — the default — then, once a client writes the rate,
    /// frames at about that rate, and none again once it writes zero.
    #[test]
    fn frames_publish_at_the_rate_the_rig_holds() {
        use std::time::Duration;

        use arora_hal::Hal;
        use bevy::time::{TimePlugin, TimeUpdateStrategy};
        use vizij_arora_hal::{RigHal, FRAME_RATE};

        let rig = RigHal::new();
        rig.offer_frame_rate(publish_rate(None, false, true).unwrap().unwrap());
        let mut app = App::new();
        app.add_plugins((TimePlugin, FramesPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / 60.0,
            )))
            .insert_resource(config(FrameFormat::Png))
            .init_resource::<Captured>()
            .add_systems(Last, complete);
        app.world_mut().spawn(Face {
            id: "face".into(),
            meta: FaceMeta::default(),
            asset_path: String::new(),
            rig: rig.clone(),
            slot: 0,
            bindings: Default::default(),
            camera: Entity::PLACEHOLDER,
            ready: true,
        });
        let write = |rate: f64| rig.try_send(&StateChange::set(FRAME_RATE, Value::F64(rate)));

        assert_eq!(captures(&mut app, 2), 0, "no frame at startup");
        write(10.0);
        let at_ten = captures(&mut app, 2);
        assert!(
            (16..=21).contains(&at_ten),
            "{at_ten} frames in 2 s at 10 Hz"
        );
        write(4.0);
        let at_four = captures(&mut app, 2);
        assert!(
            (6..=9).contains(&at_four),
            "{at_four} frames in 2 s at 4 Hz"
        );
        write(0.0);
        assert_eq!(captures(&mut app, 2), 0, "no frame once the rate is 0");
    }

    /// The face is named by its GLB; only a GLB without a face id falls back
    /// to `face`.
    #[test]
    fn the_default_frame_id_is_the_faces_id_from_its_glb() {
        let named = |face_id: Option<&str>| {
            let mut meta = FaceMeta::default();
            meta.bundle.face_id = face_id.map(str::to_string);
            default_frame_id(&meta)
        };
        assert_eq!(named(Some("quori_latest")), "quori_latest");
        assert_eq!(named(None), "face");
        assert_eq!(named(Some("")), "face");
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
