mod utils;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() -> String {
    let greeting = "Hello, animation-player!";
    alert(greeting);
    greeting.to_string()
}
