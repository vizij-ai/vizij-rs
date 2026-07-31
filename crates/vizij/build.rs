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
    // Linux: emit old-style DT_RPATH (not DT_RUNPATH). RUNPATH is not searched
    // for TRANSITIVE dependencies — libpiper.so's own libonnxruntime.so.1 needs
    // the executable's path to be visible down the load chain (macOS dyld does
    // this by default).
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg=-Wl,--disable-new-dtags");
    }
    println!("cargo:rustc-link-arg=-Wl,-rpath,{install}");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{install}/lib");
}
