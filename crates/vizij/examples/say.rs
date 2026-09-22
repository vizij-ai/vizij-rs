//! Speak from the command line — the shortest path to hearing the device's TTS.
//!
//! ```bash
//! cargo run -p vizij --example say -- "Hello, world!"
//! cargo run -p vizij --features tts-piper --example say -- "Hello, world!"  # local Piper
//! ```
//!
//! Drives the build's `say` provider exactly as the device does — one call per
//! tick, `Running` until playback ends — printing the viseme stream (the face
//! standard's shapes) as it advances and the utterance when its audio starts
//! and ends. On the device the say skill's run does the same, driving the
//! lips from the one and writing the other as the face's speech state.

use vizij_arora_tts as tts_api;

#[cfg(feature = "tts-piper")]
use vizij::modules::tts_piper as provider;
#[cfg(not(feature = "tts-piper"))]
use vizij_arora_tts as provider;

use std::time::Duration;

use arora_types::call::Call;
use arora_types::value::{StructureField, Value};
use vizij_graph_core::task;

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

    let call = Call {
        module_id: Some(provider::MODULE_ID),
        id: tts_api::SAY_ID,
        args: vec![StructureField {
            id: tts_api::SAY_TEXT_PARAM_ID,
            value: Box::new(Value::String(text)),
        }],
    };

    // The tick loop, standalone: poll the provider until the utterance ends.
    let mut last = String::new();
    let mut speaking = String::new();
    loop {
        let result = provider::say(call.clone()).expect("say reports failure as a status");
        for field in &result.mutated {
            let Value::String(value) = field.value.as_ref() else {
                continue;
            };
            if field.id == tts_api::SAY_VISEME_PARAM_ID && *value != last {
                println!("viseme: {value}");
                last = value.clone();
            }
            if field.id == tts_api::SAY_SPEECH_PARAM_ID && *value != speaking {
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
            provider::shutdown();
            if !ok {
                std::process::exit(1);
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(33));
    }
}
