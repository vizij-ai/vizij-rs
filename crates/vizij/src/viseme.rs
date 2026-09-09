//! The viseme module this device registers: the portable play_viseme
//! contract ([`vizij_arora_behavior::viseme`]) packaged as an arora host
//! module.

use arora::{HostModule, ModuleBuilder};
use arora_types::call::CallResult;
use vizij_arora_behavior::task;
pub use vizij_arora_behavior::viseme::*;
use vizij_arora_host::skills;

/// The viseme module: the play_viseme contract, described so bridges
/// discover it. The closure is only reached when the device did not register
/// the fragment (a misconfiguration) — it fails the run rather than
/// pretending to move the lips.
pub fn host_module() -> HostModule {
    ModuleBuilder::new(MODULE_ID)
        .described_function(
            PLAY_VISEME_ID,
            skills::PLAY_VISEME_FUNCTION,
            play_viseme_signature(),
            |_call| {
                log::warn!("play_viseme invoked as a module call: no task fragment is registered");
                Ok(CallResult {
                    ret: task::failure(),
                    mutated: Vec::new(),
                })
            },
        )
        .build()
}
