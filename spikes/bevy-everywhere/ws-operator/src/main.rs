//! What the vizij binary can do with arora's public API alone (arora 9.11.1,
//! arora-bridge-ws 4.0.0, arora-bridge 4.1.0 — the versions vizij-rs locks):
//! a device over a `SimpleDataStore`, its open local bridge built by hand from
//! a `ServerConfig` with the control panel on, the registry populated with two
//! input keys, a `reset` method writing the store through a sibling handle,
//! and a `ping` method dispatching a `Call` into the device through its
//! `LocalCaller`. A WebSocket client then checks every message over the wire,
//! and an HTTP GET fetches the panel.
//!
//! Port 9124 (not the 9000 default) so a running device on the machine is not
//! in the way.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use arora::{Arora, Caller, LocalCaller, ModuleBuilder};
use arora_bridge_ws::bridge::WsBridge;
use arora_bridge_ws::{
    AroraWSServer, CancellationToken, InvokeResult, KeyInfo, MethodInfo, ServerConfig, Type,
};
use arora_simple_data_store::SimpleDataStore;
use arora_types::call::{Call, CallResult};
use arora_types::data::{DataStore, Key, StateChange};
use arora_types::value::Value;
use arora_types::Uuid;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message;

const PORT: u16 = 9124;
const MOUTH: &str = "face/mouth/open";
const BROW: &str = "face/brow/raise";
const PING_MODULE: Uuid = Uuid::from_u128(0x7669_7a69_6a00_0000_0000_0000_0000_0001);
const PING_FN: Uuid = Uuid::from_u128(0x7669_7a69_6a00_0000_0000_0000_0000_0002);

fn input(path: &str, default: f64) -> KeyInfo {
    KeyInfo {
        path: path.to_string(),
        kind: Some("input".to_string()),
        value_type: Some(Type::F64),
        min: Some(0.0),
        max: Some(1.0),
        default_value: Some(Value::F64(default)),
        description: None,
    }
}

fn defaults() -> StateChange {
    let mut change = StateChange::new();
    change.set.insert(Key::from(MOUTH), Some(Value::F64(0.0)));
    change.set.insert(Key::from(BROW), Some(Value::F64(0.5)));
    change
}

#[tokio::main]
async fn main() {
    let store = SimpleDataStore::new();
    let config = ServerConfig::with_port(PORT).serve_control_panel(true);
    println!(
        "ServerConfig: bind={} port={} validate_paths={} serve_control_panel={}",
        config.bind_address, config.port, config.validate_paths, config.serve_control_panel
    );
    let server = Arc::new(AroraWSServer::new(config));

    // The registry mirror: the device's input keys (what `write_values` may
    // target under `validate_paths`) and the app-level `reset`.
    server
        .registry()
        .set_keys(vec![input(MOUTH, 0.0), input(BROW, 0.5)])
        .await;
    let reset_store = store.clone();
    server
        .registry()
        .register_method_fn(
            MethodInfo {
                path: "reset".to_string(),
                params: vec![],
                return_type: None,
                description: Some("Restore every input to its default".to_string()),
            },
            move |_args| match reset_store.write(defaults()) {
                Ok(()) => InvokeResult::ok(),
                Err(e) => InvokeResult::err(format!("{e:?}")),
            },
        )
        .await;

    // The bridge over the server, bound and served like `arora::local_ws_bridge`.
    let bridge = WsBridge::new(server.clone()).await;
    let listener = server.bind().await.expect("bind");
    let cancel = CancellationToken::new();
    tokio::spawn({
        let server = server.clone();
        let cancel = cancel.clone();
        async move {
            if let Err(e) = server.run_on(listener, cancel).await {
                eprintln!("server: {e}");
            }
        }
    });

    // The device on its own thread (`Arora` is single-owner, not `Send`): a
    // host module with one function, the store, the bridge; its `LocalCaller`
    // comes back so a registry method can dispatch into the run.
    let (caller_tx, caller_rx) = std::sync::mpsc::channel::<LocalCaller>();
    let device_store = store.clone();
    let stop = CancellationToken::new();
    let device = std::thread::spawn({
        let stop = stop.clone();
        move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                let ping = ModuleBuilder::new(PING_MODULE)
                    .function(PING_FN, |call: Call| {
                        Ok(CallResult {
                            ret: Value::String(format!("pong ({} args)", call.args.len())),
                            mutated: Vec::new(),
                        })
                    })
                    .build();
                let mut arora = Arora::builder()
                    .with_data_store(Box::new(device_store))
                    .with_bridge(Box::new(bridge))
                    .with_host_module(ping)
                    .build()
                    .expect("build the device");
                caller_tx.send(arora.caller()).unwrap();
                tokio::select! {
                    result = arora.run(Arora::DEFAULT_STEP_PERIOD) => println!("device ended: {result:?}"),
                    _ = stop.cancelled() => {}
                }
            });
        }
    });
    let caller = caller_rx.recv().expect("the device's caller");

    // `ping`: a registry method whose handler dispatches a Call into the
    // device. The registry handler is synchronous, so the call is enqueued
    // (it is, before `call` returns) and its reply is awaited on a task; the
    // `invoke_resp` only says the call was accepted.
    let handle = tokio::runtime::Handle::current();
    server
        .registry()
        .register_method_fn(
            MethodInfo {
                path: "ping".to_string(),
                params: vec![],
                return_type: Some(Type::String),
                description: Some("Call the device's ping function".to_string()),
            },
            move |_args| {
                // `CallFuture` borrows the caller, so the task owns a clone.
                let caller = caller.clone();
                handle.spawn(async move {
                    let reply = caller
                        .call(Call {
                            module_id: Some(PING_MODULE),
                            id: PING_FN,
                            args: Vec::new(),
                        })
                        .await;
                    println!("  ping reply from the device: {reply:?}");
                });
                InvokeResult::ok_with_value(Value::String("accepted".to_string()))
            },
        )
        .await;

    tokio::time::sleep(Duration::from_millis(300)).await;

    // The wire.
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{PORT}"))
        .await
        .expect("connect");

    // Read messages until one of `ty` arrives (pushes may interleave), printing all.
    async fn expect(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        ty: &str,
    ) -> serde_json::Value {
        loop {
            let reply = tokio::time::timeout(Duration::from_secs(3), ws.next())
                .await
                .unwrap_or_else(|_| panic!("no {ty} within 3s"))
                .expect("stream open")
                .expect("message");
            let text = reply.into_text().unwrap();
            println!("< {text}");
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            if json["type"] == ty {
                return json;
            }
        }
    }
    async fn send(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        request: &str,
    ) {
        println!("> {request}");
        ws.send(Message::Text(request.to_string())).await.unwrap();
    }

    send(&mut ws, r#"{"type":"list_keys"}"#).await;
    let keys = expect(&mut ws, "list_keys_resp").await;
    assert_eq!(keys["keys"].as_array().unwrap().len(), 2, "two keys advertised");

    send(&mut ws, r#"{"type":"list_methods"}"#).await;
    let methods = expect(&mut ws, "list_methods_resp").await;
    let names: Vec<&str> = methods["methods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"reset") && names.contains(&"ping"), "{names:?}");

    send(&mut ws, r#"{"type":"write_values","values":{"face/mouth/open":{"f64":0.7}}}"#).await;
    let resp = expect(&mut ws, "write_values_resp").await;
    assert_eq!(resp["success"], true, "a registered input key is writable");
    // The device applies the Update at its next step and flushes it back out.
    let pushed = expect(&mut ws, "values_changed").await;
    assert_eq!(pushed["values"]["face/mouth/open"]["f64"], 0.7);
    assert_eq!(
        store.read(&[Key::from(MOUTH)]),
        vec![Some(Value::F64(0.7))],
        "the sibling store handle sees the write"
    );

    send(&mut ws, r#"{"type":"read_values","keys":["face/mouth/open"]}"#).await;
    let read = expect(&mut ws, "read_values_resp").await;
    assert_eq!(read["values"]["face/mouth/open"]["f64"], 0.7);

    send(&mut ws, r#"{"type":"write_values","values":{"not/registered":{"f64":1.0}}}"#).await;
    let resp = expect(&mut ws, "write_values_resp").await;
    assert_eq!(resp["success"], false, "validate_paths still rejects unknown keys");

    send(&mut ws, r#"{"type":"invoke","method":"reset","request_id":"r1"}"#).await;
    let resp = expect(&mut ws, "invoke_resp").await;
    assert_eq!(resp["success"], true);
    let pushed = expect(&mut ws, "values_changed").await;
    assert_eq!(pushed["values"]["face/mouth/open"]["f64"], 0.0, "reset fanned out");

    send(&mut ws, r#"{"type":"invoke","method":"ping","request_id":"r2"}"#).await;
    let resp = expect(&mut ws, "invoke_resp").await;
    assert_eq!(resp["success"], true);
    tokio::time::sleep(Duration::from_millis(100)).await;

    // The idle feed: what a connected client receives while nothing writes.
    let mut pushes = 0usize;
    let mut clock_only = 0usize;
    let window = tokio::time::Instant::now() + Duration::from_millis(500);
    while let Ok(Some(Ok(msg))) = tokio::time::timeout_at(window, ws.next()).await {
        let json: serde_json::Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        if json["type"] == "values_changed" {
            pushes += 1;
            let keys = json["values"].as_object().unwrap();
            if keys.keys().all(|k| k.starts_with("arora/")) {
                clock_only += 1;
            }
        }
    }
    println!(
        "idle feed over 500 ms: {pushes} values_changed pushes, {clock_only} carrying only arora/* keys"
    );
    ws.close(None).await.ok();

    // The control panel: a plain HTTP GET on the same port.
    let mut tcp = tokio::net::TcpStream::connect(("127.0.0.1", PORT)).await.unwrap();
    tcp.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await.unwrap();
    let mut body = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), tcp.read_to_end(&mut body))
        .await
        .expect("panel within 3s")
        .unwrap();
    let text = String::from_utf8_lossy(&body);
    println!(
        "HTTP GET /: {} bytes; status {:?}; title present: {}",
        body.len(),
        text.lines().next().unwrap_or(""),
        text.contains("<title>Arora Control</title>")
    );
    assert!(text.starts_with("HTTP/1.1 200 OK"));

    stop.cancel();
    cancel.cancel();
    device.join().unwrap();
    let _ = HashMap::<String, Value>::new();
    println!("OK");
}
