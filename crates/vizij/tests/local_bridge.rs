//! The desktop device on the wire: a client on the open local bridge lists
//! the face's inputs, writes one and reads it back, invokes `reset`, and gets
//! the control panel on the same port. What a script, a phone on the LAN or
//! an editor sees of `vizij --port`.
//!
//! Needs the Quori GLB: `VIZIJ_FIXTURES` pointing at the directory holding
//! it (the snapshot-regression convention); skipped otherwise.

use std::path::PathBuf;
use std::time::Duration;

use arora_types::data::{DataStore, Key};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value as Json};
use tokio_tungstenite::tungstenite::Message;
use vizij::device::bridge::LocalBridgeConfig;
use vizij::device::native::{start, BridgeConfig, Mode};
use vizij::device::{FaceConfig, ProgramSelect};

/// A port nobody listens on right now (bound and released; the bridge binds
/// it next).
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind an ephemeral port")
        .local_addr()
        .unwrap()
        .port()
}

async fn request(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    message: Json,
    response_type: &str,
) -> Json {
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

/// The number a wire value carries, whichever float width it was written as.
fn number(value: &Json) -> Option<f64> {
    value.get("f64").or_else(|| value.get("f32"))?.as_f64()
}

#[tokio::test]
async fn a_client_on_the_local_bridge_drives_the_face() {
    let Some(fixtures) = std::env::var_os("VIZIJ_FIXTURES").map(PathBuf::from) else {
        eprintln!("VIZIJ_FIXTURES unset — skipping the local bridge test");
        return;
    };
    let glb = std::fs::read(fixtures.join("Quori_Current_Extended.glb")).expect("read Quori");
    let port = free_port();
    let config = FaceConfig {
        wanted: ["rig", "pose-driver", "pose", "standard-adaptation"]
            .map(String::from)
            .to_vec(),
        program: ProgramSelect::None,
        stage_neutral: true,
        ros4hri: true,
        speech: None,
    };
    // The other bridges are feature-gated fields; only the local one is set.
    #[allow(clippy::needless_update)]
    let bridges = BridgeConfig {
        local: LocalBridgeConfig {
            port,
            ..LocalBridgeConfig::default()
        },
        ..BridgeConfig::default()
    };
    let device = start(&glb, config, bridges, Mode::Operator).expect("start");

    // The bridge comes up with the device's first generation.
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

    // The registry advertises the face's free inputs, the ROS4HRI ones among
    // them, each typed.
    let keys = request(&mut socket, json!({"type": "list_keys"}), "list_keys_resp").await;
    let paths: Vec<&str> = keys["keys"]
        .as_array()
        .expect("keys")
        .iter()
        .map(|key| key["path"].as_str().unwrap())
        .collect();
    let smile = "standard/ros4hri/au/12";
    assert!(paths.contains(&smile), "no {smile} among {paths:?}");
    assert!(
        paths.iter().all(|path| !path.starts_with("arora/")),
        "built-in keys are not advertised: {paths:?}"
    );

    // A write lands in the device's store and reads back.
    let written = request(
        &mut socket,
        json!({"type": "write_values", "values": {smile: {"f64": 0.25}}}),
        "write_values_resp",
    )
    .await;
    assert_eq!(written["success"], true, "{written}");
    let mut read_back = None;
    for _ in 0..50 {
        let read = request(
            &mut socket,
            json!({"type": "read_values", "keys": [smile]}),
            "read_values_resp",
        )
        .await;
        if number(&read["values"][smile]) == Some(0.25) {
            read_back = Some(read);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(read_back.is_some(), "the write never read back");
    let held = device
        .store
        .read(&[Key::from(smile)])
        .into_iter()
        .next()
        .flatten();
    assert_eq!(
        held.as_ref().and_then(vizij_api_core::value::as_float),
        Some(0.25),
        "the store holds the written value: {held:?}"
    );

    // The methods: the skills of the face (`say` only with a speech
    // provider, and the device has none here), `stop` and `reset`.
    let methods = request(
        &mut socket,
        json!({"type": "list_methods"}),
        "list_methods_resp",
    )
    .await;
    let names: Vec<&str> = methods["methods"]
        .as_array()
        .expect("methods")
        .iter()
        .map(|method| method["path"].as_str().unwrap())
        .collect();
    for expected in ["reset", "look_at", "play_viseme", "stop"] {
        assert!(names.contains(&expected), "no {expected} among {names:?}");
    }
    assert!(!names.contains(&"say"), "say without a speech provider");
    // `reset` puts the neutral pose back.
    let reset = request(
        &mut socket,
        json!({"type": "invoke", "method": "reset", "request_id": "r1"}),
        "invoke_resp",
    )
    .await;
    assert_eq!(reset["success"], true, "{reset}");
    assert_eq!(reset["request_id"], "r1");
    let mut neutral = None;
    for _ in 0..50 {
        let read = request(
            &mut socket,
            json!({"type": "read_values", "keys": [smile]}),
            "read_values_resp",
        )
        .await;
        if number(&read["values"][smile]) == Some(0.0) {
            neutral = Some(read);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(neutral.is_some(), "reset left the smile where it was");

    // The control panel is served on the same port.
    let page = reqwest::get(format!("http://127.0.0.1:{port}/"))
        .await
        .expect("GET /");
    assert_eq!(page.status(), 200);
    let html = page.text().await.expect("body");
    assert!(html.contains("<html"), "not a page: {html:.80}");
}
