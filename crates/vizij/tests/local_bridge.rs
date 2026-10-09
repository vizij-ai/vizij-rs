//! The desktop device on the wire: a client on the open local bridge lists
//! the device's keys and which of them are the face's inputs, writes one and
//! reads it back, is refused a key that is no input, subscribes to the key it
//! watches, calls the face's skills by name, starts and stops a run, invokes
//! `reset`, and gets the control panel on the same port. What a script, a
//! phone on the LAN or an editor sees of `vizij --port`.
//!
//! Needs the Quori GLB: `VIZIJ_FIXTURES` pointing at the directory holding
//! it (the snapshot-regression convention); skipped otherwise.

use std::path::PathBuf;
use std::time::Duration;

use arora_types::data::{DataStore, Key};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value as Json};
use tokio_tungstenite::tungstenite::Message;
use vizij::face::{FaceConfig, ProgramSelect};
use vizij::native::{start, BridgeConfig, Mode};

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
        #[cfg(feature = "studio")]
        studio: false,
        speech: None,
    };
    // The other bridges are feature-gated fields; only the local one is set.
    #[allow(clippy::needless_update)]
    let bridges = BridgeConfig {
        local: arora::bridge_ws::ServerConfig::with_port(port).serve_control_panel(true),
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

    // The client is pushed the key it watches and nothing else — the device's
    // own keys (the clock among them) change every step.
    let smile = "standard/ros4hri/au/12";
    let subscribed = request(
        &mut socket,
        json!({"type": "subscribe", "keys": [smile]}),
        "subscribe_resp",
    )
    .await;
    assert_eq!(subscribed["keys"], json!([smile]), "{subscribed}");

    // The keys come with what the store says of them: the face's free inputs,
    // the ROS4HRI ones among them, open to the client, typed, ranged and
    // resting where the face does; the device's own keys closed.
    let keys = request(&mut socket, json!({"type": "list_keys"}), "list_keys_resp").await;
    let keys = keys["keys"].as_array().expect("keys");
    let meta_of = |path: &str| {
        keys.iter()
            .find(|key| key["path"] == path)
            .map(|key| key["__meta"].clone())
            .unwrap_or_else(|| panic!("no {path} among the keys"))
    };
    let input = meta_of(smile);
    assert_eq!(input["editable"], true, "{input}");
    // Typed as the `hri_msgs` field a ROS 2 bridge writes it from.
    assert_eq!(input["ty"], "f32", "{input}");
    assert_eq!(
        (input["min"].as_f64(), input["max"].as_f64()),
        (Some(0.0), Some(1.0))
    );
    assert_eq!(number(&input["default"]), Some(0.0), "{input}");
    let clock = meta_of("arora/time");
    assert_eq!(clock["editable"], false, "{clock}");

    // A key that is no input is the device's alone: the write is refused, by
    // name, rather than acknowledged and dropped.
    let refused = request(
        &mut socket,
        json!({"type": "write_values", "values": {"arora/time": {"f64": 0.0}}}),
        "write_values_resp",
    )
    .await;
    assert_eq!(refused["success"], false, "{refused}");
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|message| message.contains("arora/time")),
        "{refused}"
    );

    // A write lands in the device's store and reads back.
    let written = request(
        &mut socket,
        json!({"type": "write_values", "values": {smile: {"f32": 0.25}}}),
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

    // The methods: the device's own, described by the modules that export
    // them — the skills (`say` only with a speech provider, and the device has
    // none here) and the rest module's `reset`.
    let methods = request(
        &mut socket,
        json!({"type": "list_methods"}),
        "list_methods_resp",
    )
    .await;
    let listed = methods["methods"].as_array().expect("methods");
    let names: Vec<&str> = listed
        .iter()
        .map(|method| method["path"].as_str().unwrap())
        .collect();
    for expected in ["reset", "look_at", "play_viseme"] {
        assert!(names.contains(&expected), "no {expected} among {names:?}");
    }
    assert!(!names.contains(&"say"), "say without a speech provider");
    let play_viseme = listed
        .iter()
        .find(|method| method["path"] == "play_viseme")
        .expect("play_viseme is listed");
    assert_eq!(
        play_viseme["task"], true,
        "a viseme is a run: {play_viseme}"
    );
    let parameters: Vec<&str> = play_viseme["params"]
        .as_array()
        .expect("params")
        .iter()
        .map(|param| param["name"].as_str().unwrap())
        .collect();
    assert_eq!(parameters, ["shape", "weight"], "{play_viseme}");

    // A skill is called by name and answers with its run; the run stops on
    // `halt`, by the id it carried.
    let started = request(
        &mut socket,
        json!({
            "type": "invoke",
            "method": "play_viseme",
            "args": {"shape": {"str": "aa"}, "weight": {"f32": 0.8}},
            "request_id": "v1"
        }),
        "invoke_resp",
    )
    .await;
    assert_eq!(started["success"], true, "{started}");
    let run = started["value"]["keyvalue"]["fields"]["run"]["value"]["str"]
        .as_str()
        .unwrap_or_else(|| panic!("the answer names the run: {started}"))
        .to_string();
    let halted = request(
        &mut socket,
        json!({"type": "halt", "run": run, "request_id": "v2"}),
        "halt_resp",
    )
    .await;
    assert_eq!(halted["success"], true, "{halted}");
    assert_eq!(halted["request_id"], "v2");
    // `reset` puts every key back where the store says it rests.
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
