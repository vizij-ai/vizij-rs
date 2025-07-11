pub mod animation;
pub mod engine;
pub mod player;
pub mod utils;
pub mod conversions;

use wasm_bindgen::prelude::*;

/// Sets up a panic hook to log panic messages to the browser console.
#[wasm_bindgen(start)]
pub fn on_start() {
    console_error_panic_hook::set_once();
}
