//! Under `tts-piper`, bake the Piper install's rpath into this binary: the
//! dylibs (libpiper, onnxruntime) live in vizij-piper's cache install, and a
//! dependency's `rustc-link-arg` does not reach the consumer's link — so the
//! path arrives over the DEP_PIPER_* metadata channel and the rpath is added
//! here.

use std::env;

fn main() {
    if env::var("CARGO_FEATURE_TTS_PIPER").is_err() {
        return;
    }
    let install =
        env::var("DEP_PIPER_INSTALL_DIR").expect("vizij-piper exports DEP_PIPER_INSTALL_DIR");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{install}");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{install}/lib");
}
