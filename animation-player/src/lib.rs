pub mod animation_data;
mod animation_player;
pub mod value;

// Re-export common functionality that should be available in both environments
#[cfg(not(target_arch = "wasm32"))]
pub use animation_player::{get_animation, load_animation, unload_animation};

// Conditionally compile wasm-specific code
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::{get_animation, greet, load_animation, unload_animation};
