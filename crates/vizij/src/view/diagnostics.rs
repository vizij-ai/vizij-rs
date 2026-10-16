//! The device's health, written into its store as a HAL reading about once
//! a second.
//!
//! A system counts the frames the view renders, and every
//! [`PERIOD_SECONDS`](host::PERIOD_SECONDS) pushes, onto each shown face's
//! rig, the report [`vizij_arora_host::diagnostics`] builds: the rate the
//! view rendered at over the period, the face's id, its compliance level
//! with the face standard (graded once per face, from its bundle's graphs),
//! and the paths its composition writes from more than one source, as its
//! device reported them ([`ViewEvent::Composed`](super::ViewEvent::Composed)).
//! The runtime lands the reading under [`host::KEY`], where the ROS4HRI
//! exposure profile publishes it on `/diagnostics` — so, like the face
//! image, the view writes it when the device is exposed as ROS4HRI.

use std::collections::HashMap;
use std::time::SystemTime;

use arora_types::data::{Key, StateChange};
use bevy::prelude::*;
use vizij_arora_host::diagnostics::{self as host, FaceHealth};
use vizij_arora_host::profile;

use super::{Compositions, Face, Superseded};

pub struct DiagnosticsPlugin;

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Meter>().add_systems(Update, report);
    }
}

/// The frames rendered since the last report, and the seconds they took.
#[derive(Resource, Default)]
struct Meter {
    frames: u32,
    seconds: f32,
}

/// Count this frame; once a period has passed, report on every shown face
/// and start the next period.
fn report(
    time: Res<Time>,
    mut meter: ResMut<Meter>,
    faces: Query<(Entity, &Face), Without<Superseded>>,
    compositions: Res<Compositions>,
    mut levels: Local<HashMap<Entity, u8>>,
) {
    meter.frames += 1;
    meter.seconds += time.delta_secs();
    if meter.seconds < host::PERIOD_SECONDS {
        return;
    }
    let frame_rate = meter.frames as f32 / meter.seconds;
    *meter = Meter::default();

    levels.retain(|entity, _| faces.contains(*entity));
    let stamp = SystemTime::now();
    for (entity, face) in &faces {
        let bundle = &face.meta.bundle;
        let level = *levels.entry(entity).or_insert_with(|| {
            profile::coverage(bundle.face_id.clone(), &bundle.input_paths()).level
        });
        let health = FaceHealth {
            face_id: bundle.face_id.clone().unwrap_or_else(|| face.id.clone()),
            level,
            contended: compositions.0.get(&face.id).cloned().unwrap_or_default(),
        };
        let report = host::report(frame_rate, &[health], stamp);
        face.rig
            .push_reading(StateChange::set(Key::from(host::KEY), report));
    }
}

#[cfg(test)]
mod tests {
    /// The key the report is written under is the one the ROS4HRI exposure
    /// profile publishes whole, as the report's message: a contract spelled
    /// in two crates, silent at run time when broken (an undeclared key
    /// rides the JSON fallback), so asserted against the profile itself.
    #[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
    #[test]
    fn the_report_is_the_message_the_ros4hri_profile_publishes_from_its_key() {
        use arora_bridge_ros2::profile::Flow;
        let profile = arora_bridge_ros2::ExposureProfile::ros4hri();
        let endpoint = profile
            .endpoints
            .iter()
            .find(|e| {
                e.flow == Flow::Out
                    && e.routes
                        .iter()
                        .any(|r| r.field.is_empty() && r.key == host::KEY)
            })
            .expect("the ros4hri profile publishes the report whole");
        assert_eq!(endpoint.ros_type, host::ROS_TYPE);
        assert_eq!(endpoint.topic, "/diagnostics");
    }

    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;

    use arora_hal::Hal;
    use arora_msgs_ros2::diagnostic_msgs::{DiagnosticArray, DiagnosticStatus};
    use futures::{FutureExt, StreamExt};
    use vizij_arora_hal::RigHal;

    use super::*;
    use crate::view::meta::FaceMeta;
    use crate::view::Bindings;

    /// A face shown by the view reports about once a second: the rate the
    /// view renders at, the face's id and compliance level, and the paths
    /// its device composed contended, under the `diagnostics` key.
    #[test]
    fn a_shown_face_reports_its_health_once_a_second() {
        let meta = FaceMeta {
            bundle: vizij_arora_host::Bundle::from_bundle_json(&serde_json::json!({
                "metadata": { "faceId": "diag_face" },
                "graphs": [],
            })),
            ..FaceMeta::default()
        };
        let rig = RigHal::new();
        let mut readings = rig.updates();

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Compositions>()
            .add_plugins(DiagnosticsPlugin);
        app.world_mut().resource_mut::<Compositions>().0.insert(
            "face".into(),
            vec![vizij_arora_host::Contention {
                path: "rig/diag_face/standard/vizij/blink".into(),
                sources: vec!["rig".into(), "ros4hri".into()],
            }],
        );
        app.world_mut().spawn(Face {
            id: "face".into(),
            meta,
            asset_path: String::new(),
            rig: rig.clone(),
            slot: 0,
            bindings: Bindings::default(),
            camera: Entity::PLACEHOLDER,
            ready: false,
        });

        // 2.5 s of frames at 100 Hz on the app's clock, stepped by hand so
        // the rate is the test's and not the host's: two reports.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            10,
        )));
        for _ in 0..=250 {
            app.update();
        }
        let mut reports = Vec::new();
        while let Some(Some(change)) = readings.next().now_or_never() {
            reports.push(change);
        }
        assert_eq!(reports.len(), 2, "one report a second");

        let value = reports[1]
            .set
            .get(&Key::from(host::KEY))
            .cloned()
            .flatten()
            .expect("the report is under the diagnostics key");
        let report = DiagnosticArray::try_from(value).expect("a DiagnosticArray");
        let [view, face] = report.status.as_slice() else {
            panic!("a view status and a face status, got {:?}", report.status);
        };
        assert_eq!(view.name, "vizij: view");
        let rate: f32 = view.values[0].value.parse().expect("a rate");
        assert!((99.0..=101.0).contains(&rate), "rendering at {rate} Hz");
        assert_eq!(face.name, "vizij: face diag_face");
        assert_eq!(face.level, DiagnosticStatus::WARN, "a contended path warns");
        let values: Vec<(&str, &str)> = face
            .values
            .iter()
            .map(|kv| (kv.key.as_str(), kv.value.as_str()))
            .collect();
        assert_eq!(
            values,
            [
                ("face_id", "diag_face"),
                ("compliance_level", "L0"),
                (
                    "contended: rig/diag_face/standard/vizij/blink",
                    "rig, ros4hri"
                ),
            ]
        );
    }
}
