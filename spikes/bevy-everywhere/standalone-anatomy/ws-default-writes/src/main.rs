//! What a WS client gets from the bridge exactly as `arora::local_ws_bridge()`
//! builds it (arora 9.11.1 run.rs:108-133): `AroraWSServer::new(ServerConfig::default())`
//! + `WsBridge::new` + bind + `run_on`, with nothing populating the registry.
//! Only the port differs (9123 instead of the default 9000) to stay clear of a
//! running device.
use arora_bridge::{Bridge, Inbound};
use arora_bridge_ws::bridge::WsBridge;
use arora_bridge_ws::{AroraWSServer, CancellationToken, ServerConfig};
use arora_types::call::CallResult;
use arora_types::value::Value;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() {
    let config = ServerConfig::with_port(9123); // = ServerConfig::default() but the port
    println!(
        "ServerConfig: bind={} validate_paths={} serve_control_panel={}",
        config.bind_address, config.validate_paths, config.serve_control_panel
    );
    let server = Arc::new(AroraWSServer::new(config));
    let mut bridge = WsBridge::new(server.clone()).await;
    let listener = server.bind().await.expect("bind");
    let cancel = CancellationToken::new();
    tokio::spawn({
        let server = server.clone();
        let cancel = cancel.clone();
        async move {
            let _ = server.run_on(listener, cancel).await;
        }
    });
    // A stand-in runtime: acknowledge every command the bridge hands over.
    let mut inbound = bridge.take_inbound();
    tokio::spawn(async move {
        while let Some(event) = inbound.next().await {
            if let Inbound::Command(cmd) = event {
                cmd.reply(Ok(CallResult { ret: Value::Unit, mutated: Vec::new() }));
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut ws, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:9123")
        .await
        .expect("connect");
    for request in [
        r#"{"type":"list_keys"}"#,
        r#"{"type":"list_methods"}"#,
        r#"{"type":"write_values","values":{"face/mouth/open":{"f64":1.0}}}"#,
        r#"{"type":"invoke","method":"reset","request_id":"r1"}"#,
    ] {
        ws.send(Message::Text(request.to_string())).await.unwrap();
        let reply = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("reply within 2s")
            .expect("stream open")
            .expect("message");
        println!("> {request}\n< {reply}");
    }
    ws.close(None).await.ok();

    // The control panel: a plain HTTP GET on the same port.
    let mut tcp = tokio::net::TcpStream::connect("127.0.0.1:9123").await.unwrap();
    tcp.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await.unwrap();
    let mut buf = vec![0u8; 4096];
    let read = tokio::time::timeout(Duration::from_secs(2), tcp.read(&mut buf)).await;
    match read {
        Ok(Ok(0)) => println!("HTTP GET /: connection closed with no bytes (no control panel)"),
        Ok(Ok(n)) => println!("HTTP GET /: {} bytes; first line: {:?}", n, String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or("")),
        Ok(Err(e)) => println!("HTTP GET /: read error {e} (no control panel)"),
        Err(_) => println!("HTTP GET /: no response within 2s (no control panel)"),
    }
    cancel.cancel();
}
