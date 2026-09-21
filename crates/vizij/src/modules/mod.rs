//! The host modules: functions any Arora loads, Vizij's or a robot's — the
//! animation engine, the gaze and viseme skills' hosts, a local speech
//! provider. Tied to no hardware; a device is what composes them.

pub mod animation;
pub mod gaze;
#[cfg(all(not(target_arch = "wasm32"), feature = "tts-piper"))]
pub mod tts_piper;
pub mod viseme;
