//! Speak from the command line — the shortest path to hearing the device's TTS.
//!
//! ```bash
//! cargo run -p vizij --example say -- "Hello, world!"
//! cargo run -p vizij --features tts-piper --example say -- "Hello, world!"  # local Piper
//! ```
//!
//! Drives the build's `say` provider exactly as the device does — one call per
//! tick, `Running` until playback ends — printing the viseme/phoneme stream as
//! it advances.

// The provider modules are compiled in whole via #[path]; the example only
// exercises their call surface.
#![allow(dead_code)]

#[path = "../src/tts_api.rs"]
mod tts_api;

#[cfg(not(feature = "tts-piper"))]
#[path = "../src/tts.rs"]
mod provider;
#[cfg(feature = "tts-piper")]
#[path = "../src/tts_piper.rs"]
mod provider;

use std::time::Duration;

use arora_types::call::Call;
use arora_types::value::{StructureField, Value};
use vizij_graph_core::task;

fn main() {
    let text: String = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let text = if text.is_empty() {
        "Hello, world!".to_string()
    } else {
        text
    };
    println!("saying: {text}");

    let call = Call {
        module_id: Some(provider::module_id()),
        id: tts_api::say_id(),
        args: vec![StructureField {
            id: tts_api::text_param_id(),
            value: Box::new(Value::String(text)),
        }],
    };

    // The tick loop, standalone: poll the provider until the utterance ends.
    let mut last = String::new();
    loop {
        let result = provider::say(call.clone()).expect("say reports failure as a status");
        for field in &result.mutated {
            if field.id == tts_api::viseme_param_id() {
                if let Value::String(viseme) = field.value.as_ref() {
                    if *viseme != last {
                        println!("viseme: {viseme}");
                        last = viseme.clone();
                    }
                }
            }
        }
        if result.ret != task::running() {
            if result.ret == task::success() {
                println!("done");
            } else {
                println!("failed");
                std::process::exit(1);
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(33));
    }
}
