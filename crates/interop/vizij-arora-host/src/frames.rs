//! The rendered face as a ROS image.
//!
//! Each host captures its own pixels (Bevy's screenshot readback natively, the
//! WebGL canvas in the standalone) and writes the frame under the key of the
//! transport it encodes ([`FrameFormat::key`]), in the shape ROS expects —
//! `display/face` holds a `sensor_msgs/Image` of raw pixels,
//! `display/face/compressed` a `sensor_msgs/CompressedImage` of an encoded
//! frame. The ROS4HRI exposure profile publishes each key as that message on
//! its `image_transport` topic, so a bridge needs no conversion of its own; a
//! face writes one of the two keys, and a key never written never publishes.
//! The value is the message's typed form (the registry's field ids, not
//! names): the same bytes ROS tools decode with the bundled `sensor_msgs`
//! definition, and the shape `arora-msgs-ros2`'s CDR codec encodes without a
//! schema lookup on the way out.

use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arora_msgs_ros2::builtin_interfaces::Time;
use arora_msgs_ros2::sensor_msgs::{CompressedImage, Image};
use arora_msgs_ros2::std_msgs::Header;
use arora_types::ty::{low, TypeRegistry};
use arora_types::value::Value;
use arora_types::value_serde::bridge::to_value_seeded;
use arora_types::AroraType;
use serde::Serialize;

/// The `format` a PNG frame declares, in the shape `CompressedImage` specifies:
/// `ORIG_PIXFMT; CODEC compressed [COMPRESSED_PIXFMT]`. The pattern is what
/// matters — a `format` that does not match it, `"png"` included, is defined to
/// be read as a **bgr8 JPEG**, so the codec name alone silently mis-describes
/// the buffer. The message lists only `[bgr8, rgb8, bgr16, rgb16]` as png
/// compressed pixel formats and our frames keep their alpha, so the compressed
/// format states what the buffer is rather than the nearest listed value; a PNG
/// carries its own colour type, so a decoder reads the truth either way.
pub const PNG_FORMAT: &str = "rgba8; png compressed rgba8";

/// How a published frame's pixels are encoded — and, with it, which ROS image
/// message the frame is and which store key it is written under.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameFormat {
    /// PNG — compact enough to travel over a bridge; a `CompressedImage`.
    Png,
    /// Raw RGBA8, row-major, no padding — heavy, but decode-free; an `Image`.
    Raw,
}

impl FrameFormat {
    /// The ROS message type the frame publishes as.
    pub fn ros_type(self) -> &'static str {
        match self {
            FrameFormat::Png => "sensor_msgs/CompressedImage",
            FrameFormat::Raw => "sensor_msgs/Image",
        }
    }

    /// The store key a frame in this format is written under. One key per
    /// transport, mirroring `image_transport`'s topic pair, because the
    /// exposure profile that publishes it is static and a face encodes one
    /// way: the key names the transport, so the message it holds is fixed.
    pub fn key(self) -> &'static str {
        match self {
            FrameFormat::Png => "display/face/compressed",
            FrameFormat::Raw => "display/face",
        }
    }
}

/// A raw RGBA8 frame as a `sensor_msgs/Image` value (`encoding: "rgba8"`,
/// `step: width * 4`), stamped `stamp` in the TF frame `frame_id`.
pub fn raw_frame(
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    stamp: SystemTime,
    frame_id: &str,
) -> Value {
    static TYPE: OnceLock<(low::Type, TypeRegistry)> = OnceLock::new();
    let (ty, registry) = TYPE.get_or_init(Image::arora_type_with_registry);
    seed_around_pixels(
        &Image {
            header: header(stamp, frame_id),
            height,
            width,
            encoding: "rgba8".to_string(),
            is_bigendian: 0,
            step: width * 4,
            data: Vec::new(),
        },
        rgba,
        ty,
        registry,
    )
}

/// An encoded frame (`format` in the shape [`PNG_FORMAT`] has) as a
/// `sensor_msgs/CompressedImage` value, stamped `stamp` in the TF frame
/// `frame_id`.
pub fn compressed_frame(format: &str, data: Vec<u8>, stamp: SystemTime, frame_id: &str) -> Value {
    static TYPE: OnceLock<(low::Type, TypeRegistry)> = OnceLock::new();
    let (ty, registry) = TYPE.get_or_init(CompressedImage::arora_type_with_registry);
    seed_around_pixels(
        &CompressedImage {
            header: header(stamp, frame_id),
            format: format.to_string(),
            data: Vec::new(),
        },
        data,
        ty,
        registry,
    )
}

fn header(stamp: SystemTime, frame_id: &str) -> Header {
    let since_epoch = stamp.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    Header {
        stamp: Time {
            sec: since_epoch.as_secs() as i32,
            nanosec: since_epoch.subsec_nanos(),
        },
        frame_id: frame_id.to_string(),
    }
}

/// Seed `message` — whose payload field must be left **empty** — and drop
/// `pixels` into that field afterwards.
///
/// Serde sees a `Vec<u8>` as a sequence, and the seeding bridge materialises one
/// `Value` per element before packing them back into an `ArrayU8`. A `Value` is
/// 72 bytes, so a full-size frame costs ~110 MB of transient allocation and
/// ~430 ms — per frame, on the thread that just rendered it. The pixels are the
/// one field whose shape is known here without asking serde, so they bypass it
/// and the message around them stays a handful of scalars.
fn seed_around_pixels<T: Serialize>(
    message: &T,
    pixels: Vec<u8>,
    ty: &low::Type,
    registry: &TypeRegistry,
) -> Value {
    // The message is a plain struct of the registry's own types: seeding it
    // cannot fail short of a codegen/derive mismatch, which the tests catch.
    let mut value =
        to_value_seeded(message, ty, registry).expect("a ROS image message seeds as its own type");
    let Value::Structure(structure) = &mut value else {
        panic!("a ROS image message seeds as a structure");
    };
    let payload = structure
        .fields
        .iter_mut()
        .find(|field| matches!(*field.value, Value::ArrayU8(_)))
        .expect("a ROS image message has one uint8[] payload field");
    *payload.value = Value::ArrayU8(pixels);
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_msgs_ros2::{decode, encode, registry};
    use arora_types::value_serde::bridge::from_value;

    /// The frame values round-trip through the bridge's registry and CDR codec
    /// under the names a `Flow::Out` endpoint declares.
    fn round_trips(format: FrameFormat, value: &Value) {
        let registry = registry();
        let ty = registry
            .get_by_name(format.ros_type())
            .unwrap_or_else(|| panic!("{} is a bundled message", format.ros_type()));
        let bytes = encode(ty, registry.types(), value).expect("encode");
        let decoded = decode(ty, registry.types(), &bytes).expect("decode");
        assert_eq!(&decoded, value);
    }

    #[test]
    fn a_raw_frame_is_a_sensor_msgs_image() {
        let value = raw_frame(
            2,
            1,
            vec![1, 2, 3, 255, 4, 5, 6, 255],
            UNIX_EPOCH,
            "quori_latest",
        );
        round_trips(FrameFormat::Raw, &value);
        let Value::Structure(image) = &value else {
            panic!("an Image is a structure");
        };
        assert_eq!(image.fields.len(), 7, "header + 6 Image fields");
        let image: Image = from_value(value).expect("reads back as an Image");
        assert_eq!(image.header.frame_id, "quori_latest");
    }

    #[test]
    fn a_png_frame_is_a_sensor_msgs_compressed_image() {
        let stamp = UNIX_EPOCH + Duration::new(1_700_000_000, 42);
        let value = compressed_frame(
            PNG_FORMAT,
            vec![0x89, b'P', b'N', b'G'],
            stamp,
            "hugo_latest",
        );
        round_trips(FrameFormat::Png, &value);
        let Value::Structure(image) = &value else {
            panic!("a CompressedImage is a structure");
        };
        assert_eq!(image.fields.len(), 3, "header, format, data");
        let image: CompressedImage = from_value(value).expect("reads back as a CompressedImage");
        assert_eq!(image.header.frame_id, "hugo_latest");
        assert_eq!(
            (image.header.stamp.sec, image.header.stamp.nanosec),
            (1_700_000_000, 42)
        );
    }

    /// The two keys are what the ROS4HRI exposure profile publishes; a
    /// transport writing the other's key would publish as the wrong message.
    #[test]
    fn each_format_has_its_own_key_named_for_its_transport() {
        assert_eq!(FrameFormat::Raw.key(), "display/face");
        assert_eq!(FrameFormat::Png.key(), "display/face/compressed");
        assert_eq!(FrameFormat::Raw.ros_type(), "sensor_msgs/Image");
        assert_eq!(FrameFormat::Png.ros_type(), "sensor_msgs/CompressedImage");
    }
}
