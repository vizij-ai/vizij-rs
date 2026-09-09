//! The rendered face as a ROS image.
//!
//! Each host captures its own pixels (Bevy's screenshot readback natively, the
//! WebGL canvas in the standalone) and writes the frame under [`FRAME_KEY`]
//! in the shape ROS expects — `sensor_msgs/Image` for raw pixels,
//! `sensor_msgs/CompressedImage` for an encoded frame — so a ROS 2 bridge
//! publishing that key as its declared type needs no conversion of its own.
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

/// The store key the rendered frame is published under.
pub const FRAME_KEY: &str = "view/frame";

/// The `header.frame_id` every frame carries: the face is its own frame.
pub const FRAME_ID: &str = "robot_face";

/// How a published frame's pixels are encoded — and, with it, which ROS image
/// message and topic the frame rides.
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

    /// The ROS4HRI topic for the face image (`image_transport` naming: the raw
    /// image, and its compressed sibling under it).
    pub fn topic(self) -> &'static str {
        match self {
            FrameFormat::Png => "/robot_face/image_raw/compressed",
            FrameFormat::Raw => "/robot_face/image_raw",
        }
    }
}

/// A raw RGBA8 frame as a `sensor_msgs/Image` value (`encoding: "rgba8"`,
/// `step: width * 4`).
pub fn raw_frame(width: u32, height: u32, rgba: Vec<u8>, stamp: SystemTime) -> Value {
    static TYPE: OnceLock<(low::Type, TypeRegistry)> = OnceLock::new();
    let (ty, registry) = TYPE.get_or_init(Image::arora_type_with_registry);
    seed(
        &Image {
            header: header(stamp),
            height,
            width,
            encoding: "rgba8".to_string(),
            is_bigendian: 0,
            step: width * 4,
            data: rgba,
        },
        ty,
        registry,
    )
}

/// An encoded frame (`format` names the codec, e.g. `"png"`) as a
/// `sensor_msgs/CompressedImage` value.
pub fn compressed_frame(format: &str, data: Vec<u8>, stamp: SystemTime) -> Value {
    static TYPE: OnceLock<(low::Type, TypeRegistry)> = OnceLock::new();
    let (ty, registry) = TYPE.get_or_init(CompressedImage::arora_type_with_registry);
    seed(
        &CompressedImage {
            header: header(stamp),
            format: format.to_string(),
            data,
        },
        ty,
        registry,
    )
}

fn header(stamp: SystemTime) -> Header {
    let since_epoch = stamp.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    Header {
        stamp: Time {
            sec: since_epoch.as_secs() as i32,
            nanosec: since_epoch.subsec_nanos(),
        },
        frame_id: FRAME_ID.to_string(),
    }
}

fn seed<T: Serialize>(message: &T, ty: &low::Type, registry: &TypeRegistry) -> Value {
    // The message is a plain struct of the registry's own types: seeding it
    // cannot fail short of a codegen/derive mismatch, which the tests catch.
    to_value_seeded(message, ty, registry).expect("a ROS image message seeds as its own type")
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_msgs_ros2::{decode, encode, registry};

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
        let value = raw_frame(2, 1, vec![1, 2, 3, 255, 4, 5, 6, 255], UNIX_EPOCH);
        round_trips(FrameFormat::Raw, &value);
        let Value::Structure(image) = &value else {
            panic!("an Image is a structure");
        };
        assert_eq!(image.fields.len(), 7, "header + 6 Image fields");
    }

    #[test]
    fn a_png_frame_is_a_sensor_msgs_compressed_image() {
        let stamp = UNIX_EPOCH + Duration::new(1_700_000_000, 42);
        let value = compressed_frame("png", vec![0x89, b'P', b'N', b'G'], stamp);
        round_trips(FrameFormat::Png, &value);
        let Value::Structure(image) = &value else {
            panic!("a CompressedImage is a structure");
        };
        assert_eq!(image.fields.len(), 3, "header, format, data");
    }
}
