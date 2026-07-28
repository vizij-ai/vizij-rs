//! Loading the animation module into the device.
//!
//! The module is baked into the binary: `vizij-animation-module` is an artifact
//! dependency built for `wasm32-wasip1` — its `.wasm` reached through
//! `CARGO_CDYLIB_FILE_*` — paired with the header it ships as `MODULE_YAML`.
//! [`load`] parses that header and returns it with the bytes for
//! [`AroraBuilder::with_module`](arora::AroraBuilder::with_module);
//! [`function_modules`] builds the `function -> module` map that routes a bare
//! function handle — a graph `ExternalFunction` node (the animation source's
//! `step`/`player_states`), or an in-process call — to the engine's by-module
//! dispatch. Once loaded, its transport functions are callable over any bridge.

use std::collections::HashMap;

use anyhow::{Context, Result};
use arora_types::module::low::Header;
use uuid::Uuid;

/// The animation module's `.wasm`, built for `wasm32-wasip1` by the artifact
/// dependency and baked into this binary. Cargo hands the built file's path to
/// this crate's compilation as `CARGO_CDYLIB_FILE_<DEP>_<crate>` — here
/// `CARGO_CDYLIB_FILE_VIZIJ_ANIMATION_MODULE_vizij_animation_module`.
const WASM: &[u8] = include_bytes!(env!(
    "CARGO_CDYLIB_FILE_VIZIJ_ANIMATION_MODULE_vizij_animation_module"
));

/// The module's header (parsed from the `MODULE_YAML` it ships) and its `.wasm`
/// bytes — the pair [`AroraBuilder::with_module`](arora::AroraBuilder::with_module)
/// loads.
pub fn load() -> Result<(Header, Vec<u8>)> {
    let header: Header = serde_yaml::from_str(vizij_animation_module::MODULE_YAML)
        .context("parse the animation module header")?;
    Ok((header, WASM.to_vec()))
}

/// `function id -> module id` over the module's exports — how a bare function
/// handle (a graph `ExternalFunction` node, an in-process call) routes to the
/// engine's by-module dispatch.
pub fn function_modules(header: &Header) -> HashMap<Uuid, Uuid> {
    header
        .exports
        .iter()
        .map(|export| (*export.id(), header.id))
        .collect()
}
