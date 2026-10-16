//! Vizij's browser module: [`vizij::web`], the view and the Vizij's Arora in
//! one wasm module, as the cdylib wasm-bindgen reads. Linking the `vizij`
//! crate is all it does: its `#[wasm_bindgen]` surface is exported from
//! here. Empty on every other target.
#![cfg(target_arch = "wasm32")]

pub use vizij::web;
