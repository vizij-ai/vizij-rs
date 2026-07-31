//! The gaze module this device registers: the portable look_at contract
//! ([`vizij_arora_behavior::gaze`]) packaged as an arora host module.

use arora::{HostModule, ModuleBuilder};
use arora_types::call::CallResult;
pub use vizij_arora_behavior::gaze::*;
use vizij_arora_behavior::task;
use vizij_arora_host::skills;

/// The gaze module: the look_at contract, described so bridges discover it.
/// The closure is only reached when the device did not register the fragment
/// (a misconfiguration) — it fails the run rather than pretending to gaze.
pub fn host_module() -> HostModule {
    ModuleBuilder::new(module_id())
        .described_function(
            look_at_id(),
            skills::LOOK_AT_FUNCTION,
            look_at_signature(),
            |_call| {
                log::warn!("look_at invoked as a module call: no task fragment is registered");
                Ok(CallResult {
                    ret: task::failure(),
                    mutated: Vec::new(),
                })
            },
        )
        .build()
}
