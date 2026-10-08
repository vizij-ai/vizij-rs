//! A Studio-exported face on the wire, as Semio Studio drives it: the
//! device serving Studio's keys (`studio`) takes Studio's update for an
//! entity — every key it sends at once — and lands each target under the
//! animatable id the view indexes, a compound animatable's components joined,
//! with Studio's feedback keys reporting back; an animatable the face's rig
//! drives takes Studio's keys and stays the rig's. The update goes through the
//! open local bridge, the same inbound path as the Studio bridge's: one
//! that names a key the device never opened is refused whole.

#![cfg(feature = "studio")]

mod common;

use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value as Json};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;
use vizij::face::{FaceConfig, ProgramSelect};
use vizij::native::{start, BridgeConfig, Mode};
use vizij_api_core::value::{as_float, as_vector};

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
        .send(Message::Text(message.to_string()))
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

/// Studio's update for one entity: a number's target with the two keys it
/// sends beside it, another number's, and a colour's targets per component.
fn studio_update(x: Uuid, y: Uuid, color: Uuid) -> Json {
    let mut values = serde_json::Map::new();
    for (key, value) in [
        (format!("{x}.target_position"), 0.25),
        (format!("{x}.studio_value"), 0.25),
        (format!("{x}.target_velocity"), 0.0),
        (format!("{y}.target_position"), -0.5),
        (format!("{y}.studio_value"), -0.5),
        (format!("{color}.r.target_position"), 1.0),
        (format!("{color}.g.target_position"), 0.0),
        (format!("{color}.b.target_position"), 0.5),
    ] {
        values.insert(key, json!({ "f64": value }));
    }
    json!({ "type": "write_values", "values": values })
}

#[tokio::test]
async fn studio_s_update_drives_a_studio_exported_face() {
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
    let mut socket = socket.expect("the local bridge listens within 10 s");

    // Studio's keys are open, numbers in the units of the feature they
    // drive: no `[0, 1]` range on a translation.
    let keys = request(&mut socket, json!({"type": "list_keys"}), "list_keys_resp").await;
    let keys = keys["keys"].as_array().expect("keys");
    let target = keys
        .iter()
        .find(|key| key["path"] == format!("{x}.target_position"))
        .map(|key| key["__meta"].clone())
        .expect("the target is listed");
    assert_eq!(target["editable"], true, "{target}");
    assert_eq!(target["ty"], "f64", "{target}");
    assert!(
        target.get("min").is_none() && target.get("max").is_none(),
        "{target}"
    );

    let written = request(&mut socket, studio_update(x, y, color), "write_values_resp").await;
    assert_eq!(written["success"], true, "{written}");

    // The view's keys: the bare animatable ids, read off the rig the view
    // draws from.
    let mut pose = None;
    for _ in 0..100 {
        let current: std::collections::HashMap<String, _> = device
            .rig
            .pose()
            .into_iter()
            .map(|(path, value)| (path.to_string(), value))
            .collect();
        let landed = current.get(&x.to_string()).and_then(as_float) == Some(0.25)
            && current.get(&y.to_string()).and_then(as_float) == Some(0.75)
            && current.get(&color.to_string()).and_then(as_vector) == Some(&[1.0, 0.0, 0.5][..]);
        if landed {
            pose = Some(current);
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(pose.is_some(), "Studio's targets never reached the rig");
    // The rig keeps `y` past the update, and no feedback claims Studio moved it.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let held = device
        .rig
        .pose()
        .into_iter()
        .find(|(path, _)| path.to_string() == y.to_string())
        .and_then(|(_, value)| as_float(&value));
    assert_eq!(held, Some(0.75));

    // Studio's feedback reads back on the wire.
    let feedback = [
        format!("{x}.position"),
        format!("{color}.b.position"),
        format!("{y}.position"),
    ];
    let read = request(
        &mut socket,
        json!({"type": "read_values", "keys": feedback}),
        "read_values_resp",
    )
    .await;
    let number = |value: &Json| value.get("f64").or_else(|| value.get("f32"))?.as_f64();
    assert_eq!(number(&read["values"][&feedback[0]]), Some(0.25), "{read}");
    assert_eq!(number(&read["values"][&feedback[1]]), Some(0.5), "{read}");
    assert_eq!(number(&read["values"][&feedback[2]]), None, "{read}");

    // An update naming a key the device never opened is refused whole, by
    // name: a target for an animatable the face does not have.
    let mut stray = studio_update(x, y, color);
    stray["values"][format!("{}.target_position", Uuid::new_v4())] = json!({ "f64": 1.0 });
    let refused = request(&mut socket, stray, "write_values_resp").await;
    assert_eq!(refused["success"], false, "{refused}");
}
