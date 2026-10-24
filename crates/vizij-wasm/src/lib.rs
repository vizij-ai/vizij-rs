//! Vizij's browser module: [`vizij::web`], the view and the Vizij's Arora in
//! one wasm module, as the cdylib wasm-bindgen reads. Linking the `vizij`
//! crate is all it does: its `#[wasm_bindgen]` surface is exported from
//! here, and [`run_constructors`] is its start function. Empty on every other
//! target.
#![cfg(target_arch = "wasm32")]

pub use vizij::web;

use wasm_bindgen::prelude::*;

unsafe extern "C" {
    /// The linker's function that runs the module's constructors.
    fn __wasm_call_ctors();
}

/// Run the module's constructors, once, when the module is instantiated.
///
/// Bevy's reflection registers every reflected type through an `inventory`
/// constructor, hundreds of them. A module whose code never calls
/// `__wasm_call_ctors` is linked command-style: wasm-ld makes every export
/// run all the constructors before its body. JavaScript calls into the
/// module for every value it builds or drops — an `invoke` answer crosses for
/// each object, string and number it holds — so every crossing would pay for
/// them. Calling them here links the module as a library, whose exports run
/// their bodies only. `inventory`'s constructors are idempotent, so a later
/// call, such as bevy_reflect's when auto-registration is on, changes nothing.
#[wasm_bindgen(start)]
fn run_constructors() {
    // SAFETY: the linker defines `__wasm_call_ctors`. The start function runs
    // before any other export, and the constructors only link static nodes
    // into `inventory`'s registries.
    unsafe { __wasm_call_ctors() }
}
