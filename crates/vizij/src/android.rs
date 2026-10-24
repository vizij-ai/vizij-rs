//! The Android app: the face as a stand-alone device, full screen in the
//! framework's `android.app.NativeActivity`.
//!
//! The APK the crate's `android/` Gradle project packages carries this
//! crate's cdylib (`libvizij.so`); the activity loads it and calls the
//! `android_main` that [`bevy_main`] emits. That entry starts the device —
//! the [`native`] driver under arora's headless front end, with the open
//! local bridge — on the face the APK ships, and runs the view over it.
//!
//! Logging goes to logcat through [`android_logger`], installed before the
//! device's front end looks for a logger; Bevy's `LogPlugin` stays disabled,
//! as on desktop, so each record reaches logcat once.

use std::io::Read;
use std::sync::Mutex;

use anyhow::{anyhow, Context, Result};
use bevy::prelude::*;

use crate::face::{FaceConfig, ProgramSelect};
use crate::native::{self, BridgeConfig, Mode, Runtime};
use crate::view::{self, FaceAssets, Fit, ViewOptions};

/// The face the APK ships, under its `assets/`.
const SHIPPED_FACE: &std::ffi::CStr = c"face.glb";

#[bevy_main]
fn main() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("vizij")
            .with_max_level(log::LevelFilter::Info),
    );
    if let Err(e) = run() {
        log::error!("vizij: {e:#}");
    }
}

/// Start the device on the shipped face and run the view until the activity
/// is destroyed.
fn run() -> Result<()> {
    let glb = shipped_face()?;
    let config = FaceConfig {
        wanted: ["rig", "pose-driver", "pose", "standard-adaptation"]
            .map(str::to_string)
            .to_vec(),
        program: ProgramSelect::Auto,
        stage_neutral: true,
        ros4hri: true,
        #[cfg(feature = "studio")]
        studio: false,
        // No speech provider: the cloud provider's HTTPS client verifies
        // through `rustls-platform-verifier`, which on Android needs its
        // Kotlin component packaged and initialised before the first request.
        speech: None,
    };
    let Runtime { meta, events, .. } =
        native::start(&glb, config, BridgeConfig::default(), Mode::Operator)?;
    log::info!(
        "vizij: the shipped face — {} elements, {} animatables, {} bundle graphs",
        meta.elements.len(),
        meta.animatables.len(),
        meta.bundle.graphs.len(),
    );

    let mut app = App::new();
    // The in-memory asset source is registered before the default plugins,
    // whose asset plugin's construction requires it.
    FaceAssets::register(&mut app);
    app.insert_resource(view::ViewEvents(Mutex::new(events)))
        .add_plugins(
            DefaultPlugins
                // The face is served from memory ([`FaceAssets`]); no `.meta`
                // file exists to probe for.
                .set(bevy::asset::AssetPlugin {
                    meta_check: bevy::asset::AssetMetaCheck::Never,
                    ..default()
                })
                .disable::<bevy::log::LogPlugin>(),
        )
        .insert_resource(ViewOptions {
            background: Color::BLACK,
            fit: Fit::Contain,
            zoom: Vec2::ONE,
            ambient: std::f32::consts::FRAC_PI_2,
            unlit: false,
        })
        .add_plugins(view::ViewPlugin);
    app.run();
    Ok(())
}

/// The face the APK ships, read whole from its assets.
fn shipped_face() -> Result<Vec<u8>> {
    let android = bevy::android::ANDROID_APP
        .get()
        .ok_or_else(|| anyhow!("no AndroidApp: not started through android_main"))?;
    let mut asset = android
        .asset_manager()
        .open(SHIPPED_FACE)
        .ok_or_else(|| anyhow!("the APK ships no assets/{}", SHIPPED_FACE.to_string_lossy()))?;
    let mut glb = Vec::new();
    asset
        .read_to_end(&mut glb)
        .with_context(|| format!("cannot read assets/{}", SHIPPED_FACE.to_string_lossy()))?;
    Ok(glb)
}
