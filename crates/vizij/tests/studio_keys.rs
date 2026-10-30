//! A Studio-exported face on the wire, as Semio Studio drives it: the
//! device serving Studio's keys (`studio`) takes Studio's update for an
//! entity — every key it sends at once — and lands each target under the
//! animatable id the view indexes, a compound animatable's components joined,
//! with Studio's feedback keys reporting back; an animatable the face's rig
//! drives takes Studio's keys and stays the rig's. A target is taken in
//! either form, `<id>/target.position` (a component's
//! `<id>/target.position.<axis>`) or Studio's `<id>.target_position`
//! (`<id>.<axis>.target_position`), the first when an update carries both.
//! The update goes through the open local bridge, the same inbound path as
//! the Studio bridge's: one that names a key the device never opened, writes
//! a feedback key, or sets a target to another type than it declares is
//! refused whole, with the device's reason.

#![cfg(feature = "studio")]

mod common;

use std::collections::HashMap;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value as Json};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;
use vizij::face::{FaceConfig, ProgramSelect};
use vizij::native::{start, BridgeConfig, Mode, Runtime};
use vizij_api_core::value::{as_float, as_vector, Value};

use common::studio_face_glb;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// A port nobody listens on right now (bound and released; the bridge binds
/// it next).
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind an ephemeral port")
        .local_addr()
        .unwrap()
        .port()
}

async fn request(socket: &mut Socket, message: Json, response_type: &str) -> Json {
    socket
        .send(Message::text(message.to_string()))
        .await
        .expect("send");
    // The live feed interleaves `values_changed` pushes with the answers.
    loop {
        let next = tokio::time::timeout(Duration::from_secs(10), socket.next())
            .await
            .expect("an answer within 10 s")
            .expect("the socket stays open")
            .expect("a frame");
        let Message::Text(text) = next else { continue };
        let json: Json = serde_json::from_str(&text).expect("JSON");
        if json["type"] == response_type {
            return json;
        }
        assert_eq!(json["type"], "values_changed", "unexpected frame {json}");
    }
}

fn number(value: &Json) -> Option<f64> {
    value.get("f64").or_else(|| value.get("f32"))?.as_f64()
}

/// A write of `values`, each a number, as one update.
fn write_values(values: &[(String, f64)]) -> Json {
    let values: serde_json::Map<String, Json> = values
        .iter()
        .map(|(key, value)| (key.clone(), json!({ "f64": value })))
        .collect();
    json!({ "type": "write_values", "values": values })
}

/// Studio's update for one entity, as Studio writes it today: a number's
/// target with the legacy value and the velocity beside it, another
/// number's with the legacy value, and a colour's targets per component.
fn studio_update(x: Uuid, y: Uuid, color: Uuid) -> Json {
    write_values(&[
        (format!("{x}.target_position"), 0.25),
        (format!("{x}.studio_value"), 0.25),
        (format!("{x}.target_velocity"), 0.0),
        (format!("{y}.target_position"), -0.5),
        (format!("{y}.studio_value"), -0.5),
        (format!("{color}.r.target_position"), 1.0),
        (format!("{color}.g.target_position"), 0.0),
        (format!("{color}.b.target_position"), 0.5),
    ])
}

/// The same update with each target in the `k/target.c` form: a
/// component's as `<id>/target.position.<axis>`.
fn slash_update(x: Uuid, y: Uuid, color: Uuid) -> Json {
    write_values(&[
        (format!("{x}/target.position"), 0.25),
        (format!("{x}/target.velocity"), 0.0),
        (format!("{y}/target.position"), -0.5),
        (format!("{color}/target.position.r"), 1.0),
        (format!("{color}/target.position.g"), 0.0),
        (format!("{color}/target.position.b"), 0.5),
    ])
}

/// A Studio-exported face's animatables: two numbers, a yaw, a colour.
struct Face {
    x: Uuid,
    y: Uuid,
    color: Uuid,
}

/// A device serving a Studio-exported face's keys, whose rig drives `y`,
/// and a client on its open local bridge.
async fn serve() -> (Runtime, Socket, Face) {
    let (x, y, yaw, color) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    // The face's rig drives `y`: Studio's keys for it are accepted and the
    // rig's value holds.
    let rig = json!({ "graphs": [{ "id": "rig", "kind": "rig", "spec": {
        "nodes": [
            { "id": "c", "type": "constant", "params": { "value": 0.75 } },
            { "id": "o", "type": "output", "params": { "path": y.to_string() } },
        ],
        "edges": [{ "from": { "node_id": "c" }, "to": { "node_id": "o", "input": "in" } }],
    } }] });
    let glb = studio_face_glb(x, y, yaw, color, Some(rig));
    let port = free_port();
    let config = FaceConfig {
        wanted: ["rig", "pose-driver", "pose", "standard-adaptation"]
            .map(String::from)
            .to_vec(),
        program: ProgramSelect::None,
        stage_neutral: true,
        ros4hri: false,
        studio: true,
        speech: None,
    };
    // The other bridges are feature-gated fields; only the local one is set.
    #[allow(clippy::needless_update)]
    let bridges = BridgeConfig {
        local: arora::bridge_ws::ServerConfig::with_port(port),
        ..BridgeConfig::default()
    };
    let device = start(&glb, config, bridges, Mode::Operator).expect("start");

    let url = format!("ws://127.0.0.1:{port}");
    let mut socket = None;
    for _ in 0..100 {
        if let Ok((connected, _)) = tokio_tungstenite::connect_async(&url).await {
            socket = Some(connected);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let socket = socket.expect("the local bridge listens within 10 s");
    (device, socket, Face { x, y, color })
}

/// The view's keys, read off the rig the view draws from once `landed`
/// holds of them, within 2 s.
async fn pose_once(
    device: &Runtime,
    landed: impl Fn(&HashMap<String, Value>) -> bool,
) -> Option<HashMap<String, Value>> {
    for _ in 0..100 {
        let current: HashMap<String, Value> = device
            .rig
            .pose()
            .into_iter()
            .map(|(path, value)| (path.to_string(), value))
            .collect();
        if landed(&current) {
            return Some(current);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    None
}

/// Whether `pose` holds the targets of [`studio_update`] and
/// [`slash_update`], `y` at the rig's value.
fn update_landed(face: &Face) -> impl Fn(&HashMap<String, Value>) -> bool + '_ {
    |pose| {
        pose.get(&face.x.to_string()).and_then(as_float) == Some(0.25)
            && pose.get(&face.y.to_string()).and_then(as_float) == Some(0.75)
            && pose.get(&face.color.to_string()).and_then(as_vector) == Some(&[1.0, 0.0, 0.5][..])
    }
}

/// Every form of Studio's keys is open, a number in the units of the
/// feature it drives: no `[0, 1]` range on a translation.
#[tokio::test]
async fn the_device_lists_every_form_of_studio_s_keys_as_inputs() {
    let (_device, mut socket, Face { x, y, color }) = serve().await;
    let keys = request(&mut socket, json!({"type": "list_keys"}), "list_keys_resp").await;
    let keys = keys["keys"].as_array().expect("keys");
    let mut forms = Vec::new();
    for key in [x.to_string(), y.to_string()] {
        forms.extend([
            format!("{key}/target.position"),
            format!("{key}.target_position"),
            format!("{key}.studio_value"),
            format!("{key}/target.velocity"),
            format!("{key}.target_velocity"),
        ]);
    }
    forms.extend([
        format!("{color}/target.position.r"),
        format!("{color}.r.target_position"),
        format!("{color}.r.studio_value"),
        format!("{color}/target.velocity.r"),
        format!("{color}.r.target_velocity"),
    ]);
    for form in forms {
        let meta = keys
            .iter()
            .find(|key| key["path"] == form.as_str())
            .map(|key| key["__meta"].clone())
            .unwrap_or_else(|| panic!("{form} is listed"));
        assert_eq!(meta["editable"], true, "{form}: {meta}");
        assert_eq!(meta["ty"], "f64", "{form}: {meta}");
        assert!(
            meta.get("min").is_none() && meta.get("max").is_none(),
            "{form}: {meta}"
        );
        assert_eq!(number(&meta["default"]), Some(0.0), "{form}: {meta}");
    }
}

#[tokio::test]
async fn studio_s_update_drives_a_studio_exported_face() {
    let (device, mut socket, face) = serve().await;
    let Face { x, y, color } = face;

    // Until Studio writes, the face holds its RobotData defaults: the
    // colour rests at the components' defaults, joined.
    let rested = pose_once(&device, |pose| pose.contains_key(&color.to_string()))
        .await
        .and_then(|pose| as_vector(&pose[&color.to_string()]).map(<[f32]>::to_vec));
    assert_eq!(rested, Some(vec![0.0, 0.0, 0.0]));

    let written = request(&mut socket, studio_update(x, y, color), "write_values_resp").await;
    assert_eq!(written["success"], true, "{written}");
    assert!(
        pose_once(&device, update_landed(&face)).await.is_some(),
        "Studio's targets never reached the rig"
    );
    // The rig keeps `y` past the update, and no feedback claims Studio moved it.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let held = device
        .rig
        .pose()
        .into_iter()
        .find(|(path, _)| path.to_string() == y.to_string())
        .and_then(|(_, value)| as_float(&value));
    assert_eq!(held, Some(0.75));

    // Studio's feedback reads back on the wire, a component's under both
    // spellings of its axis, and the target reads the same in both forms.
    let read_keys = [
        format!("{x}.position"),
        format!("{color}.b.position"),
        format!("{y}.position"),
        format!("{x}/target.position"),
        format!("{color}.position.b"),
    ];
    let read = request(
        &mut socket,
        json!({"type": "read_values", "keys": read_keys}),
        "read_values_resp",
    )
    .await;
    assert_eq!(number(&read["values"][&read_keys[0]]), Some(0.25), "{read}");
    assert_eq!(number(&read["values"][&read_keys[1]]), Some(0.5), "{read}");
    assert_eq!(number(&read["values"][&read_keys[2]]), None, "{read}");
    assert_eq!(number(&read["values"][&read_keys[3]]), Some(0.25), "{read}");
    assert_eq!(number(&read["values"][&read_keys[4]]), Some(0.5), "{read}");

    // An update naming a key the device never opened is refused whole, by
    // name: a target for an animatable the face does not have.
    let mut stray = studio_update(x, y, color);
    stray["values"][format!("{}.target_position", Uuid::new_v4())] = json!({ "f64": 1.0 });
    let refused = request(&mut socket, stray, "write_values_resp").await;
    assert_eq!(refused["success"], false, "{refused}");
}

/// The same update with each target as a field of `<id>/target` drives the
/// face the same way.
#[tokio::test]
async fn an_update_in_the_slash_form_drives_the_face() {
    let (device, mut socket, face) = serve().await;
    let Face { x, y, color } = face;
    let written = request(&mut socket, slash_update(x, y, color), "write_values_resp").await;
    assert_eq!(written["success"], true, "{written}");
    assert!(
        pose_once(&device, update_landed(&face)).await.is_some(),
        "the targets never reached the rig"
    );
    let read_keys = [
        format!("{x}.position"),
        format!("{x}.target_position"),
        format!("{color}.r.position"),
        format!("{color}.position.r"),
        format!("{color}.r.target_position"),
    ];
    let read = request(
        &mut socket,
        json!({"type": "read_values", "keys": read_keys}),
        "read_values_resp",
    )
    .await;
    assert_eq!(number(&read["values"][&read_keys[0]]), Some(0.25), "{read}");
    assert_eq!(number(&read["values"][&read_keys[1]]), Some(0.25), "{read}");
    // A component's feedback under both spellings of its axis, and its
    // target in Studio's per-axis form.
    for key in &read_keys[2..] {
        assert_eq!(number(&read["values"][key]), Some(1.0), "{key}: {read}");
    }
}

/// Studio's feedback keys are the device's outputs: listed, under both
/// spellings of a component's axis, and closed to a client. An update writing
/// one, or a target of another type than the `f64` it declares, in any of its
/// forms, is refused whole, the device's reason reaching the client: the
/// valid target beside it moves nothing either.
#[tokio::test]
async fn a_write_to_feedback_or_of_another_type_is_refused_with_the_reason() {
    let (device, mut socket, Face { x, color, .. }) = serve().await;
    let feedbacks = [
        format!("{x}.position"),
        format!("{color}.position.r"),
        format!("{color}.r.position"),
    ];
    // The feedback is listed once the face has stepped and written it.
    let mut listed = Vec::new();
    for _ in 0..50 {
        let keys = request(&mut socket, json!({"type": "list_keys"}), "list_keys_resp").await;
        listed = keys["keys"].as_array().expect("keys").clone();
        if feedbacks
            .iter()
            .all(|key| listed.iter().any(|listed| listed["path"] == key.as_str()))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    for key in &feedbacks {
        let meta = listed
            .iter()
            .find(|listed| listed["path"] == key.as_str())
            .map(|listed| listed["__meta"].clone())
            .unwrap_or_else(|| panic!("{key} is listed"));
        assert_eq!(meta["editable"], false, "{key} is closed: {meta}");
    }

    // Each refused update carries a valid target beside the refused key.
    let valid = format!("{x}/target.position");
    for key in &feedbacks {
        let update = write_values(&[(valid.clone(), 0.5), (key.clone(), 0.5)]);
        let refused = request(&mut socket, update, "write_values_resp").await;
        assert_eq!(refused["success"], false, "{key}: {refused}");
        let reason = refused["message"].as_str().unwrap_or_default();
        assert!(
            reason.contains("not an input") && reason.contains(key.as_str()),
            "{key}: {refused}"
        );
    }
    for key in [
        format!("{x}/target.position"),
        format!("{x}.target_position"),
        format!("{color}/target.position.r"),
        format!("{color}.r.target_position"),
    ] {
        let update = json!({ "type": "write_values", "values": {
            format!("{color}/target.position.g"): { "f64": 0.5 },
            &key: { "bool": true },
        } });
        let refused = request(&mut socket, update, "write_values_resp").await;
        assert_eq!(refused["success"], false, "{key}: {refused}");
        let reason = refused["message"].as_str().unwrap_or_default();
        assert!(
            reason.contains("declared type") && reason.contains(key.as_str()),
            "{key}: {refused}"
        );
    }

    // A later valid update lands; the refused ones moved nothing.
    let sentinel = write_values(&[(format!("{color}/target.position.b"), 0.25)]);
    let written = request(&mut socket, sentinel, "write_values_resp").await;
    assert_eq!(written["success"], true, "{written}");
    let pose = pose_once(&device, |pose| {
        pose.get(&color.to_string()).and_then(as_vector) == Some(&[0.0, 0.0, 0.25][..])
    })
    .await;
    let pose = pose.expect("the colour's blue lands, its red and green at rest");
    assert_eq!(pose.get(&x.to_string()).and_then(as_float), Some(0.0));
    let read = request(
        &mut socket,
        json!({"type": "read_values", "keys": feedbacks}),
        "read_values_resp",
    )
    .await;
    for key in &feedbacks {
        assert_eq!(number(&read["values"][key]), Some(0.0), "{key}: {read}");
    }
}

/// An update carrying a target in both forms lands the
/// `<id>/target.position` form's value; a later update in either form
/// moves it again.
#[tokio::test]
async fn an_update_carrying_both_forms_takes_the_slash_form() {
    let (device, mut socket, Face { x, .. }) = serve().await;
    let at = |value: f32| {
        move |pose: &HashMap<String, Value>| {
            pose.get(&x.to_string()).and_then(as_float) == Some(value)
        }
    };
    for (slash, underscored, landed) in [
        (Some(0.125), Some(0.875), 0.125),
        (None, Some(0.375), 0.375),
        (Some(0.625), None, 0.625),
    ] {
        let mut values = Vec::new();
        values.extend(slash.map(|value| (format!("{x}/target.position"), value)));
        values.extend(underscored.map(|value| (format!("{x}.target_position"), value)));
        let written = request(&mut socket, write_values(&values), "write_values_resp").await;
        assert_eq!(written["success"], true, "{written}");
        assert!(
            pose_once(&device, at(landed as f32)).await.is_some(),
            "{values:?} lands {landed}"
        );
        let feedback = format!("{x}.position");
        let read = request(
            &mut socket,
            json!({"type": "read_values", "keys": [&feedback]}),
            "read_values_resp",
        )
        .await;
        assert_eq!(number(&read["values"][&feedback]), Some(landed), "{read}");
    }
}
