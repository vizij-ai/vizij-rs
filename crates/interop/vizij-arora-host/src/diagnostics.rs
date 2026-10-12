//! The device's health as a ROS diagnostics report.
//!
//! A host writes its report under [`KEY`] about once a second, as the
//! `diagnostic_msgs/DiagnosticArray` the ROS4HRI exposure profile publishes
//! on `/diagnostics`; like the face image ([`crate::frames`]), the value is
//! the message's typed form, so the bridge publishes it with no conversion.
//! The report holds one status for the renderer and one per face it shows:
//!
//! - **`vizij: view`** — the rate the host renders at, warning when it
//!   rendered nothing in the last second;
//! - **`vizij: face <id>`** — the face's id, its compliance level with the
//!   face standard ([`crate::profile::coverage`]), and every path its
//!   composition writes from more than one source ([`crate::Contention`]),
//!   a warning: only the last writer's motion shows.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use arora_msgs_ros2::builtin_interfaces::Time;
use arora_msgs_ros2::diagnostic_msgs::{DiagnosticArray, DiagnosticStatus, KeyValue};
use arora_msgs_ros2::std_msgs::Header;
use arora_types::ty::{low, TypeRegistry};
use arora_types::value::Value;
use arora_types::value_serde::bridge::to_value_seeded;
use arora_types::AroraType;

use crate::Contention;

/// The store key the report is written under.
pub const KEY: &str = "diagnostics";

/// The ROS message type the report publishes as.
pub const ROS_TYPE: &str = "diagnostic_msgs/DiagnosticArray";

/// How often a host writes the report, seconds.
pub const PERIOD_SECONDS: f32 = 1.0;

/// What the report says of one face.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceHealth {
    /// The face's id (its GLB's `faceId`), its hardware id in the report.
    pub face_id: String,
    /// Its compliance level with the face standard, `L0`–`L3`.
    pub level: u8,
    /// The paths its composition writes from more than one source.
    pub contended: Vec<Contention>,
}

/// The report: the renderer's `frame_rate` (Hz, over the last period) and
/// each face's health, stamped `stamp`.
pub fn report(frame_rate: f32, faces: &[FaceHealth], stamp: SystemTime) -> Value {
    let mut status = vec![view_status(frame_rate)];
    status.extend(faces.iter().map(face_status));
    let since_epoch = stamp.duration_since(UNIX_EPOCH).unwrap_or_default();
    let message = DiagnosticArray {
        header: Header {
            stamp: Time {
                sec: since_epoch.as_secs() as i32,
                nanosec: since_epoch.subsec_nanos(),
            },
            frame_id: String::new(),
        },
        status,
    };
    static TYPE: OnceLock<(low::Type, TypeRegistry)> = OnceLock::new();
    let (ty, registry) = TYPE.get_or_init(DiagnosticArray::arora_type_with_registry);
    to_value_seeded(&message, ty, registry).expect("a DiagnosticArray is its own type's value")
}

fn view_status(frame_rate: f32) -> DiagnosticStatus {
    let (level, message) = if frame_rate > 0.0 {
        (
            DiagnosticStatus::OK,
            format!("rendering at {frame_rate:.1} Hz"),
        )
    } else {
        (
            DiagnosticStatus::WARN,
            "no frame rendered in the last second".to_string(),
        )
    };
    DiagnosticStatus {
        level,
        name: "vizij: view".into(),
        message,
        hardware_id: String::new(),
        values: vec![value("frame_rate", format!("{frame_rate:.1}"))],
    }
}

fn face_status(face: &FaceHealth) -> DiagnosticStatus {
    let (level, message) = match face.contended.len() {
        0 => (DiagnosticStatus::OK, format!("compliance L{}", face.level)),
        1 => (
            DiagnosticStatus::WARN,
            "1 path written by more than one source".to_string(),
        ),
        n => (
            DiagnosticStatus::WARN,
            format!("{n} paths written by more than one source"),
        ),
    };
    let mut values = vec![
        value("face_id", face.face_id.clone()),
        value("compliance_level", format!("L{}", face.level)),
    ];
    values.extend(face.contended.iter().map(|contention| {
        value(
            &format!("contended: {}", contention.path),
            contention.sources.join(", "),
        )
    }));
    DiagnosticStatus {
        level,
        name: format!("vizij: face {}", face.face_id),
        message,
        hardware_id: face.face_id.clone(),
        values,
    }
}

fn value(key: &str, value: String) -> KeyValue {
    KeyValue {
        key: key.to_string(),
        value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// The report reads back as the message: a status for the view, one per
    /// face, warning on a contended path and on a renderer that stopped.
    #[test]
    fn the_report_says_the_rate_the_face_and_its_contention() {
        let faces = [
            FaceHealth {
                face_id: "quori_latest".into(),
                level: 2,
                contended: Vec::new(),
            },
            FaceHealth {
                face_id: "toasty".into(),
                level: 0,
                contended: vec![Contention {
                    path: "rig/toasty/standard/vizij/blink".into(),
                    sources: vec!["rig".into(), "ros4hri".into()],
                }],
            },
        ];
        let stamp = UNIX_EPOCH + Duration::new(1_700_000_000, 5);
        let report = report(59.84, &faces, stamp);
        let message = DiagnosticArray::try_from(report).expect("a DiagnosticArray value");

        assert_eq!(message.header.stamp.sec, 1_700_000_000);
        assert_eq!(message.header.stamp.nanosec, 5);
        let statuses: Vec<(u8, &str, &str)> = message
            .status
            .iter()
            .map(|s| (s.level, s.name.as_str(), s.message.as_str()))
            .collect();
        assert_eq!(
            statuses,
            [
                (DiagnosticStatus::OK, "vizij: view", "rendering at 59.8 Hz"),
                (
                    DiagnosticStatus::OK,
                    "vizij: face quori_latest",
                    "compliance L2"
                ),
                (
                    DiagnosticStatus::WARN,
                    "vizij: face toasty",
                    "1 path written by more than one source"
                ),
            ]
        );
        let values: Vec<(&str, &str)> = message.status[2]
            .values
            .iter()
            .map(|kv| (kv.key.as_str(), kv.value.as_str()))
            .collect();
        assert_eq!(
            values,
            [
                ("face_id", "toasty"),
                ("compliance_level", "L0"),
                ("contended: rig/toasty/standard/vizij/blink", "rig, ros4hri"),
            ]
        );
        assert_eq!(message.status[2].hardware_id, "toasty");

        let stopped = DiagnosticArray::try_from(super::report(0.0, &[], stamp)).unwrap();
        assert_eq!(stopped.status[0].level, DiagnosticStatus::WARN);
    }
}
