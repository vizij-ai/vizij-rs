//! Speak from the command line — the shortest path to hearing the device's TTS.
//!
//! ```bash
//! cargo run -p vizij --example say -- "Hello, world!"
//! cargo run -p vizij --features tts-piper --example say -- "Hello, world!"  # local Piper
//! ```
//!
//! Drives the build's `say` provider exactly as the device does — registered
//! as a host module on an arora and called once per tick, `Running` until
//! playback ends — printing the viseme stream (the face standard's shapes)
//! as it advances and the utterance when its audio starts and ends. On the
//! device the say skill's run does the same, driving the lips from the one
//! and writing the other as the face's speech state.

use std::time::Duration;

use arora_types::call::Call;
use arora_types::value::{StructureField, Value};
use vizij_arora_store::BlackboardStore;
use vizij_arora_tts as tts_api;
use vizij_arora_tts::say::ids::say;
use vizij_graph_core::task;

/// This build's provider: the cloud one at `API_URL` (or its default), the
/// local Piper one under `tts-piper`.
#[cfg(not(feature = "tts-piper"))]
fn provider() -> (uuid::Uuid, arora::HostModule) {
    let api_base =
        std::env::var("API_URL").unwrap_or_else(|_| tts_api::DEFAULT_API_BASE.to_string());
    (
        tts_api::MODULE_ID,
        tts_api::host_module(tts_api::Config { api_base }),
    )
}

#[cfg(feature = "tts-piper")]
fn provider() -> (uuid::Uuid, arora::HostModule) {
    use vizij::modules::tts_piper;
    (tts_piper::MODULE_ID, tts_piper::host_module())
}

fn main() {
    // Surface the providers' log::error!/warn! diagnostics on the console.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let text: String = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let text = if text.is_empty() {
        "Hello, world!".to_string()
    } else {
        text
    };
    println!("saying: {text}");

    let (module_id, module) = provider();
    let mut arora = arora::Arora::builder()
        .with_data_store(Box::new(BlackboardStore::new()))
        .with_host_module(module)
        .build()
        .expect("build arora");
    let call = Call {
        module_id: Some(module_id),
        id: say::FUNCTION,
        args: vec![
            StructureField {
                id: say::TEXT,
                value: Box::new(Value::String(text)),
            },
            StructureField {
                id: say::VISEME,
                value: Box::new(Value::String(tts_api::SILENCE_VISEME.to_string())),
            },
            StructureField {
                id: say::SPEECH,
                value: Box::new(Value::String(String::new())),
            },
        ],
    };

    // The tick loop, standalone: call the provider through the device until
    // the utterance ends.
    let mut last = String::new();
    let mut speaking = String::new();
    loop {
        let result = arora
            .call(call.clone())
            .expect("say dispatches through the device");
        for field in &result.mutated {
            let Value::String(value) = field.value.as_ref() else {
                continue;
            };
            if field.id == say::VISEME && *value != last {
                println!("viseme: {value}");
                last = value.clone();
            }
            if field.id == say::SPEECH && *value != speaking {
                if value.is_empty() {
                    println!("speech: ended");
                } else {
                    println!("speech: {value}");
                }
                speaking = value.clone();
            }
        }
        if result.ret != task::running() {
            let ok = result.ret == task::success();
            println!("{}", if ok { "done" } else { "failed" });
            // Tear the voice down deliberately: leaving libpiper to C++
            // static-destruction order aborts at exit.
            #[cfg(feature = "tts-piper")]
            vizij::modules::tts_piper::shutdown();
            if !ok {
                std::process::exit(1);
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(33));
    }
}
