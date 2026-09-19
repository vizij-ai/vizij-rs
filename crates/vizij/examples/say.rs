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
//! as it advances. On the device the say skill's run does the same and
//! drives the lips from it.

use std::time::Duration;

use arora_types::call::Call;
use arora_types::value::{StructureField, Value};
use vizij_arora_store::BlackboardStore;
use vizij_arora_tts as tts_api;
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
    use vizij::device::tts_piper;
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
        id: tts_api::SAY_ID,
        args: vec![StructureField {
            id: tts_api::SAY_TEXT_PARAM_ID,
            value: Box::new(Value::String(text)),
        }],
    };

    // The tick loop, standalone: call the provider through the device until
    // the utterance ends.
    let mut last = String::new();
    loop {
        let result = arora
            .call(call.clone())
            .expect("say dispatches through the device");
        for field in &result.mutated {
            if field.id == tts_api::SAY_VISEME_PARAM_ID {
                if let Value::String(viseme) = field.value.as_ref() {
                    if *viseme != last {
                        println!("viseme: {viseme}");
                        last = viseme.clone();
                    }
                }
            }
        }
        if result.ret != task::running() {
            let ok = result.ret == task::success();
            println!("{}", if ok { "done" } else { "failed" });
            // Tear the voice down deliberately: leaving libpiper to C++
            // static-destruction order aborts at exit.
            #[cfg(feature = "tts-piper")]
            vizij::device::tts_piper::shutdown();
            if !ok {
                std::process::exit(1);
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(33));
    }
}
