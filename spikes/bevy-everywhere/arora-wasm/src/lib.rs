//! Compile spike: which arora/vizij crates build for wasm32-unknown-unknown.
//! The body only names the composition entry points so the linker sees them.
#![allow(unused_imports)]
pub use arora::{Arora, AroraBuilder, HostModule, ModuleBuilder};
#[cfg(feature = "step1")]
pub use vizij_api_core::value::Value as VizijValue;
#[cfg(feature = "step2")]
pub use vizij_arora_hal::RigHal;
#[cfg(feature = "step2")]
pub use vizij_arora_store::BlackboardStore;
#[cfg(feature = "step3")]
pub use vizij_arora_host::ProgramSelect;
#[cfg(feature = "step4")]
pub use vizij_arora_behavior::{parse_spec, ProcessingGraph};
#[cfg(feature = "step5")]
pub use vizij_animation_module::ids as animation_ids;
#[cfg(feature = "step6")]
pub use vizij_arora_tts::host_module as tts_host_module;
#[cfg(all(feature = "step7", target_arch = "wasm32"))]
pub use arora_web::AroraWeb;

/// The device composition device.rs performs, minus the platform-bound parts.
#[cfg(feature = "step5")]
pub fn compose(spec: &str) -> Option<AroraBuilder> {
    let spec = parse_spec(spec).ok()?;
    let graph = ProcessingGraph::from_spec(spec).ok()?;
    Some(
        Arora::builder()
            .with_hal(Box::new(RigHal::new()))
            .with_data_store(Box::new(BlackboardStore::new()))
            .with_behavior_interpreter(Box::new(graph))
            // The host-module registration device.rs performs (animation.rs
            // builds its module the same way: ModuleBuilder + closures).
            .with_host_module(animation_host_module()),
    )
}

/// A host module over the animation module's native functions, the shape
/// vizij-rs/crates/vizij/src/animation.rs builds.
#[cfg(feature = "step5")]
pub fn animation_host_module() -> HostModule {
    use arora_types::call::{CallResult, CallError};
    use arora_types::value::Value;
    ModuleBuilder::new(animation_ids::MODULE)
        .function(animation_ids::PLAYER_STATES, |_call| -> Result<CallResult, CallError> {
            let states = vizij_animation_module::player_states();
            Ok(CallResult { ret: Value::U32(states.len() as u32), mutated: Vec::new() })
        })
        .build()
}

/// Stepping from an embedder's clock: what a Bevy system would do each frame.
pub fn step_once(arora: &mut Arora, dt_ms: f64) -> Result<(), arora::RuntimeError> {
    arora.step(std::time::Duration::from_secs_f64(dt_ms / 1_000.0))
}
