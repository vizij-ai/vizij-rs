//! Print the module's Arora header, as the wasm executor loads it, in JSON —
//! what `npm/@vizij/animation-module`'s artifact build ships.
//!
//! ```text
//! cargo run -q -p vizij-animation-module --example header
//! ```

use arora_types::module::low::Executor;
use vizij_animation_module::animation;

fn main() {
    let header = animation::header(Executor {
        name: "wasm".to_string(),
        min_version: None,
        max_version: None,
    });
    println!(
        "{}",
        serde_json::to_string(&header).expect("a header serializes to JSON")
    );
}
