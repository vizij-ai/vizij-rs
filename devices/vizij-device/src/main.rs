//! A Vizij rig as a Semio Studio device.
//!
//! Wires [`vizij_arora_hal::RigHal`] into the arora headless runner: the
//! runner handles the whole Studio side (Firebase auth, token rotation, the
//! Zenoh connection, device registration), this binary only supplies the
//! hardware — the rig.
//!
//! Configuration is environment-only, matching the runner:
//! - `VIZIJ_MODEL_GLB`: path to the rig's GLB, served to Studio as the
//!   device model (optional).
//! - `MODEL_FAMILY`, `HARDWARE_VERSION`, `SOFTWARE_VERSION`: the HAL
//!   description (all optional).
//! - `DEVICE_NAME`, Firebase / `ZENOH_ENDPOINTS` variables: consumed by the
//!   runner itself for registration and connection.

use std::sync::Arc;

use anyhow::{Context, Result};
use arora_hal::HalDescription;
use vizij_arora_hal::RigHal;

fn main() -> Result<()> {
    let rig = RigHal::with_description(HalDescription {
        model_family: std::env::var("MODEL_FAMILY").ok().or_else(|| Some("vizij".to_string())),
        hardware_version: std::env::var("HARDWARE_VERSION").ok(),
        software_version: std::env::var("SOFTWARE_VERSION").ok(),
    });

    if let Ok(path) = std::env::var("VIZIJ_MODEL_GLB") {
        let glb = std::fs::read(&path)
            .with_context(|| format!("could not read the rig model at {path}"))?;
        rig.set_model_glb(glb);
    }

    arora::headless::launch_with_hal(Arc::new(rig))
}
