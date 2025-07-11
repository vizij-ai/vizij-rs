//! Utility and test functions for WebAssembly.
use wasm_bindgen::prelude::*;

/// Logs a message to the browser console.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

/// A convenience function for logging from Rust to the browser console.
///
/// # Example
///
/// ```javascript
/// import { console_log } from "./pkg/animation_player.js";
/// console_log("Hello from Rust!");
/// ```
///
/// @param {string} message - The message to log.
#[wasm_bindgen]
pub fn console_log(message: &str) {
    log(message);
}

/// A simple test function that returns a greeting.
///
/// # Example
///
/// ```javascript
/// import { greet } from "./pkg/animation_player.js";
/// const greeting = greet("World");
/// console.log(greeting); // "Hello, World! Animation Player WASM is ready."
/// ```
///
/// @param {string} name - The name to include in the greeting.
/// @returns {string} The greeting message.
#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Animation Player WASM is ready.", name)
}
