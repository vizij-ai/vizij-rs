//! The Android app: the face as a stand-alone device, full screen in the
//! APK's one activity.
//!
//! The APK the crate's `android/` Gradle project packages carries this
//! crate's cdylib (`libvizij.so`). Its activity, `ai.vizij.app.VizijActivity`
//! (Kotlin, `android/app/src/main/kotlin`), is a `NativeActivity` that loads
//! it and calls the `android_main` that [`bevy_main`] emits, and does what
//! native code cannot through a NativeActivity: it takes a face (a GLB file)
//! from the system's document picker or from a VIEW intent into the app's
//! files directory as `model.glb`, and holds the screen on with the system
//! bars hidden.
//!
//! The entry point starts the device — the [`native`] driver under arora's
//! headless front end, with the open local bridge, and with the `studio`
//! feature the Semio Studio bridge — on `model.glb` when it
//! holds a face, so the face last opened comes back on every launch, and on
//! the face the APK ships otherwise; then it runs the view over it. A face
//! received while the app runs reaches the device through
//! [`RuntimeHandle::reload`]. A long press on the face opens the picker.
//!
//! With the `settings` feature, a tap on the face shows the settings pane's
//! Settings button ([`crate::settings`]): the pane keeps its settings in the
//! app's files directory, opens the picker on its Open a face…, and is the
//! device's operator. The back gesture closes the pane, and with the pane
//! closed moves the app to the back (`VizijActivity.leaveApp`); the pane keeps
//! clear of the system bars and the display's cutout, which the activity
//! reports (`insetsChanged`).
//!
//! Studio keeps its credentials in the app's files directory (the device
//! directory) and the device registers under the identity kept there,
//! `studio-identity.json` (name, owners, model family). A launch connects
//! only with an identity kept, so that an install nobody associated — a CI
//! emulator's — registers nothing; the pane's identity form keeps one and
//! connects. The pane is not asked the registration's questions: arora 15
//! asks nobody in a given device directory, and the one way to give it the
//! app's — `DEVICE_DIR` in the environment — is not sound here, since the
//! process' JVM threads may read the environment while `set_var` writes it.
//!
//! Logging goes to logcat through [`android_logger`], installed before the
//! device's front end looks for a logger; Bevy's `LogPlugin` stays disabled,
//! as on desktop, so each record reaches logcat once.

use std::ffi::c_void;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Context, Result};
use bevy::android::android_activity::AndroidApp;
use bevy::prelude::*;
use bevy::render::error_handler::{ErrorType, RenderError, RenderErrorHandler, RenderErrorPolicy};
use bevy::render::settings::RenderCreation;

use crate::face::{FaceConfig, ProgramSelect};
use crate::native::{self, BridgeConfig, Mode, Runtime, RuntimeHandle};
use crate::view::{self, FaceAssets, Fit, ViewOptions};

/// The face the APK ships, under its `assets/`.
const SHIPPED_FACE: &std::ffi::CStr = c"face.glb";

/// The face last opened, in the app's files directory, where the activity
/// copies a face it receives.
const OPENED_FACE: &str = "model.glb";

/// The identity the device registers with Studio under, in the app's files
/// directory.
#[cfg(feature = "studio")]
const STUDIO_IDENTITY: &str = "studio-identity.json";

/// The settings pane's settings, in the app's files directory.
#[cfg(feature = "settings")]
const SETTINGS: &str = "settings.json";

/// How long a still touch is held to open the picker.
const LONG_PRESS_SECS: f32 = 1.0;

/// How far, in logical pixels, a long press may drift.
const LONG_PRESS_SLOP: f32 = 24.0;

/// The running device, for the faces the activity receives while it runs.
static DEVICE: OnceLock<RuntimeHandle> = OnceLock::new();

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

/// Start the device on the face to show and run the view until the
/// activity is destroyed.
fn run() -> Result<()> {
    let android = android_app()?;
    #[cfg(feature = "settings")]
    let pane = crate::settings::PaneOperator::default();
    #[cfg(feature = "settings")]
    let mode = Mode::OperatedBy(std::sync::Arc::new(pane.clone()));
    #[cfg(not(feature = "settings"))]
    let mode = Mode::Operator;
    let Runtime {
        meta,
        events,
        handle,
        ..
    } = start_device(android, mode)?;
    log::info!(
        "vizij: {} elements, {} animatables, {} bundle graphs",
        meta.elements.len(),
        meta.animatables.len(),
        meta.bundle.graphs.len(),
    );
    let _ = DEVICE.set(handle.clone());

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
        .insert_resource(RenderErrorHandler(on_render_error))
        .insert_resource(ViewOptions {
            background: Color::BLACK,
            fit: Fit::Contain,
            zoom: Vec2::ONE,
            ambient: std::f32::consts::FRAC_PI_2,
            unlit: false,
        })
        .add_plugins(view::ViewPlugin)
        .add_systems(Update, long_press);
    #[cfg(feature = "settings")]
    app.add_plugins(crate::settings::SettingsPlugin {
        handle,
        operator: pane,
        file: files_dir(android).join(SETTINGS),
    })
    .add_systems(Update, (open_picker_on_ask, leave_on_back, follow_insets));
    #[cfg(not(feature = "settings"))]
    let _ = handle;
    app.run();
    Ok(())
}

/// The device on the face last opened, or on the shipped face when none was
/// or the one opened does not compose.
fn start_device(android: &AndroidApp, mode: Mode) -> Result<Runtime> {
    if let Some(path) = opened_face_path(android).filter(|path| path.exists()) {
        match std::fs::read(&path)
            .with_context(|| format!("cannot read {}", path.display()))
            .and_then(|glb| native::start(&glb, face_config(), bridges(android), mode.clone()))
        {
            Ok(runtime) => {
                log::info!("vizij: the face last opened ({})", path.display());
                return Ok(runtime);
            }
            Err(e) => log::error!(
                "vizij: the face last opened does not load, showing the shipped one: {e:#}"
            ),
        }
    }
    let glb = shipped_face(android)?;
    log::info!("vizij: the shipped face");
    native::start(&glb, face_config(), bridges(android), mode)
}

/// The open local bridge, and with the `studio` feature the Studio bridge in
/// the app's files directory.
#[cfg_attr(not(feature = "studio"), allow(unused_variables))]
fn bridges(android: &AndroidApp) -> BridgeConfig {
    BridgeConfig {
        #[cfg(feature = "studio")]
        studio: android
            .internal_data_path()
            .map(|files| native::StudioConfig {
                identity: files.join(STUDIO_IDENTITY),
                device_dir: Some(files),
                connect: native::Connect::IfKept,
            }),
        ..BridgeConfig::default()
    }
}

fn face_config() -> FaceConfig {
    FaceConfig {
        wanted: ["rig", "pose-driver", "pose", "standard-adaptation"]
            .map(str::to_string)
            .to_vec(),
        program: ProgramSelect::Auto,
        stage_neutral: true,
        ros4hri: true,
        #[cfg(feature = "studio")]
        studio: true,
        // The cloud provider at its default deployment; its audio plays
        // through cpal's oboe backend.
        speech: Some(std::sync::Arc::new(|| {
            vizij_arora_tts::host_module(vizij_arora_tts::Config::cloud())
        })),
    }
}

fn android_app() -> Result<&'static AndroidApp> {
    bevy::android::ANDROID_APP
        .get()
        .ok_or_else(|| anyhow!("no AndroidApp: not started through android_main"))
}

/// `model.glb` in the app's files directory.
fn opened_face_path(android: &AndroidApp) -> Option<PathBuf> {
    android
        .internal_data_path()
        .map(|files| files.join(OPENED_FACE))
}

/// The face the APK ships, read whole from its assets.
fn shipped_face(android: &AndroidApp) -> Result<Vec<u8>> {
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

/// `VizijActivity.modelOpened()`: the activity copied a face it received to
/// `model.glb` while the app runs; the device moves onto it. A face that
/// does not compose is an error in the log and the device keeps its face.
#[unsafe(no_mangle)]
extern "system" fn Java_ai_vizij_app_VizijActivity_modelOpened(
    _env: *mut c_void,
    _this: *mut c_void,
) {
    let Some(device) = DEVICE.get() else {
        // Before the device runs, the launch reads `model.glb` itself.
        return;
    };
    let Some(path) = android_app().ok().and_then(opened_face_path) else {
        return;
    };
    match std::fs::read(&path) {
        Ok(glb) => {
            log::info!("vizij: opening the face received ({})", path.display());
            device.reload(glb);
        }
        Err(e) => log::error!("vizij: cannot read {}: {e}", path.display()),
    }
}

/// A touch held still for [`LONG_PRESS_SECS`] opens the system's document
/// picker; the face picked arrives through `modelOpened`.
fn long_press(touches: Res<Touches>, time: Res<Time>, mut held: Local<Option<(u64, f32, bool)>>) {
    let mut active = touches.iter();
    let (Some(touch), None) = (active.next(), active.next()) else {
        *held = None;
        return;
    };
    let now = time.elapsed_secs();
    let (id, since, opened) = match *held {
        Some(press) if press.0 == touch.id() => press,
        _ => (touch.id(), now, false),
    };
    if touch.distance().length() > LONG_PRESS_SLOP {
        *held = None;
        return;
    }
    let open = !opened && now - since >= LONG_PRESS_SECS;
    if open {
        if let Err(e) = open_picker() {
            log::error!("vizij: cannot open the picker: {e:#}");
        }
    }
    *held = Some((id, since, opened || open));
}

/// Call `VizijActivity.openPicker()` on the activity.
fn open_picker() -> Result<()> {
    call_activity("openPicker")
}

/// Call the activity's `method`, which takes nothing and returns nothing.
fn call_activity(method: &str) -> Result<()> {
    let android = android_app()?;
    // SAFETY: both pointers are the activity's, valid while the app runs:
    // the process' JavaVM and a global reference to the activity object.
    let vm = unsafe { jni::JavaVM::from_raw(android.vm_as_ptr().cast()) }?;
    let mut env = vm.attach_current_thread()?;
    let activity = unsafe { jni::objects::JObject::from_raw(android.activity_as_ptr().cast()) };
    env.call_method(&activity, method, "()V", &[])?;
    Ok(())
}

/// The app's files directory: the face last opened, the Studio identity and
/// credentials, the settings.
#[cfg(feature = "settings")]
fn files_dir(android: &AndroidApp) -> PathBuf {
    android
        .internal_data_path()
        .unwrap_or_else(std::env::temp_dir)
}

/// The pane's Open a face… opens the picker.
#[cfg(feature = "settings")]
fn open_picker_on_ask(mut asked: MessageReader<crate::settings::OpenFace>) {
    if asked.read().count() > 0 {
        if let Err(e) = open_picker() {
            log::error!("vizij: cannot open the picker: {e:#}");
        }
    }
}

/// A back gesture the pane did not take moves the app to the back, as the
/// standalone's did.
#[cfg(feature = "settings")]
fn leave_on_back(mut back: MessageReader<crate::settings::LeaveApp>) {
    if back.read().count() > 0 {
        if let Err(e) = call_activity("leaveApp") {
            log::error!("vizij: cannot leave the app: {e:#}");
        }
    }
}

/// The insets the activity last reported, in physical pixels (left, top,
/// right, bottom), until the pane reads them.
static INSETS: Mutex<Option<[i32; 4]>> = Mutex::new(None);

/// `VizijActivity.insetsChanged(left, top, right, bottom)`: the system bars'
/// and the display cutout's insets, in physical pixels, as the window's
/// insets change. Exported whatever the features, as the activity calls it.
#[unsafe(no_mangle)]
extern "system" fn Java_ai_vizij_app_VizijActivity_insetsChanged(
    _env: *mut c_void,
    _this: *mut c_void,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) {
    if let Ok(mut insets) = INSETS.lock() {
        *insets = Some([left, top, right, bottom]);
    }
}

/// The pane keeps clear of the insets the activity reported, in logical
/// pixels.
#[cfg(feature = "settings")]
fn follow_insets(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut safe: ResMut<crate::settings::SafeArea>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Some([left, top, right, bottom]) = INSETS.lock().ok().and_then(|mut insets| insets.take())
    else {
        return;
    };
    let scale = window.scale_factor();
    *safe = crate::settings::SafeArea {
        left: left as f32 / scale,
        top: top as f32 / scale,
        right: right as f32 / scale,
        bottom: bottom as f32 / scale,
    };
}

/// A lost device recovers: the renderer is created anew and the face drawn
/// again, where Bevy's default ends the app. wgpu reports a frame slower than
/// its acquire timeout as a device loss, and a tablet's driver may reset.
/// Any other rendering error ends the app, as by default: it would recur.
fn on_render_error(
    error: &RenderError,
    main_world: &mut World,
    _: &mut World,
) -> RenderErrorPolicy {
    if error.ty == ErrorType::DeviceLost {
        log::warn!("vizij: the GPU device was lost; recreating the renderer");
        return RenderErrorPolicy::Recover(RenderCreation::Automatic(Box::default()));
    }
    log::error!("vizij: quitting on a {:?} rendering error", error.ty);
    main_world.write_message(AppExit::error());
    RenderErrorPolicy::StopRendering
}
