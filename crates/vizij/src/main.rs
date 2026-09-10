//! Vizij: an arora with a head.
//!
//! `cargo run -- --glb <face.glb>` opens the Vizij view (Bevy) rendering the
//! face, driven by an arora device running the face's own graphs (rig +
//! pose-driver from the embedded `VIZIJ_bundle`). `--snapshot out.png` renders
//! offscreen (no window) and writes a PNG instead — the comparison harness
//! against the web renderer.

#[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
use anyhow::Context;
use anyhow::{anyhow, Result};
use bevy::prelude::*;
use clap::Parser;

mod animation;
mod device;
mod frames;
mod gaze;
#[cfg(test)]
mod memory_tests;
mod meta;
#[cfg(all(test, feature = "ros2-dds"))]
mod ros2_tests;
mod snapshot;
// The TTS provider modules share one contract (`tts_api`); the `tts-piper`
// feature swaps which provider this build registers.
#[cfg(feature = "tts-piper")]
mod tts_piper;
mod view;

/// Vizij: render a GLB face natively over an arora device.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Path to the face GLB (with embedded RobotData + VIZIJ_bundle).
    #[arg(long)]
    glb: std::path::PathBuf,

    /// Render one frame offscreen to this PNG and exit (no window).
    #[arg(long)]
    snapshot: Option<std::path::PathBuf>,

    /// Run windowless: render offscreen and stream frames (`--frame-rate`) into
    /// the store instead of opening a window. The device still runs with its
    /// bridges — the ROS4HRI route (face frames as a topic, no browser).
    #[arg(long)]
    headless: bool,

    /// Offscreen render size, WIDTHxHEIGHT (`--snapshot` and `--headless`).
    #[arg(long, default_value = "763x486")]
    size: String,

    /// Background clear color, hex RRGGBB. (The web comparison harness passes
    /// 101114, the web page's own background.)
    #[arg(long, default_value = "000000")]
    background: String,

    /// three.js-style ambient intensity (the web renderer uses π/2).
    #[arg(long, default_value_t = std::f32::consts::FRAC_PI_2)]
    ambient: f32,

    /// Render materials unlit (albedo passthrough).
    #[arg(long)]
    unlit: bool,

    /// How the face fits the window: contain letterboxes (the web renderer's
    /// behavior), cover fills the window and crops the excess axis, stretch
    /// fills it on both axes, distorting the face to the window's aspect.
    #[arg(long, value_enum, default_value_t = view::Fit::Contain)]
    fit: view::Fit,

    /// Magnify the fitted face: one factor for both axes (`1.5`) or one per
    /// axis, width x height (`1.5x1.2`). Applies on top of `--fit`, about the
    /// face's center: above 1 enlarges past the fit (cropping the excess),
    /// below 1 shrinks it inside the window.
    #[arg(long, default_value = "1", value_parser = parse_zoom)]
    zoom: Vec2,

    /// Compose only these bundle graph kinds (comma-separated), e.g. "rig" or
    /// "rig,pose-driver". Default: rig + pose-driver + the face's standard
    /// adaptation graphs.
    #[arg(long, default_value = "rig,pose-driver,pose,standard-adaptation")]
    graphs: String,

    /// Publish rendered frames into the store as HAL readings, at this rate in
    /// Hz (decoupled from the step rate); 0 disables. Frames are the face image
    /// a ROS4HRI consumer reads, so by default they publish at 15 Hz when the
    /// device is exposed as ROS4HRI (`--ros2` without `--no-ros4hri`) and not
    /// at all otherwise. A rate given here applies regardless, except under
    /// `--ros2 --no-ros4hri`, which is refused: with no ROS4HRI profile to type
    /// the frame key, a frame would ride the bridge's JSON scalar plane. Works
    /// with a window or headless.
    #[arg(long)]
    frame_rate: Option<f32>,

    /// How published frames are encoded, which decides the key they are
    /// written under: `png` writes `display/face/compressed` (a
    /// `sensor_msgs/CompressedImage`), `raw` writes `display/face` (a
    /// `sensor_msgs/Image`).
    #[arg(long, value_enum, default_value_t = frames::FrameFormat::Png)]
    frame_format: frames::FrameFormat,

    /// The TF frame published frames are stamped with (`header.frame_id`).
    /// Default: the face's id from its GLB (`metadata.faceId`, e.g.
    /// `quori_latest`), else the GLB's file stem.
    #[arg(long)]
    frame_id: Option<String>,

    /// Autoplay this motiongraph program id instead of the bundle's own
    /// `activeMotionGraphId`. Window mode plays the active program by default;
    /// `--snapshot` stays on the neutral face unless a program is named.
    #[arg(long)]
    program: Option<String>,

    /// Don't autoplay any program — hold the rig's authored/neutral pose.
    #[arg(long)]
    no_autoplay: bool,

    /// Don't stage the bundle's `neutralInputs` into the store at boot.
    #[arg(long)]
    no_stage_neutral: bool,

    /// Don't compose the built-in ROS4HRI mapping (on by default: the
    /// `standard/ros4hri/*` keys — expression, gaze, action units, visemes —
    /// drive the face's standard controls, with idle blink and smoothing).
    #[arg(long)]
    no_ros4hri: bool,

    /// Expose the device's keys over ROS 2 topics: `--ros2 [namespace][:domain]`
    /// (namespace empty and domain 0 by default). Composes with the local bridge.
    #[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    ros2: Option<String>,

    /// Attach the Semio Studio bridge, configured from the environment
    /// (`DEVICE_OWNERS`, …). Composes with the local bridge.
    #[cfg(feature = "studio")]
    #[arg(long)]
    studio: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    // In window mode the arora operator flow owns logging (its front end —
    // TUI or headless — installs the log sink); the snapshot harness keeps
    // its own quiet logger.
    if cli.snapshot.is_some() {
        env_logger::init();
    }

    let wanted: Vec<String> = cli
        .graphs
        .split(',')
        .map(|kind| kind.trim().to_string())
        .collect();
    let mode = if cli.snapshot.is_some() {
        device::Mode::Quiet
    } else {
        device::Mode::Operator
    };
    // Which program plays: an explicit `--program` wins; `--no-autoplay` forces
    // none; otherwise the window autoplays the bundle's active program while the
    // snapshot stays on the deterministic neutral face.
    let program = if cli.no_autoplay {
        device::ProgramSelect::None
    } else if let Some(id) = cli.program.clone() {
        device::ProgramSelect::Id(id)
    } else if cli.snapshot.is_some() {
        device::ProgramSelect::None
    } else {
        device::ProgramSelect::Auto
    };
    let config = device::FaceConfig {
        wanted,
        program,
        stage_neutral: !cli.no_stage_neutral,
        ros4hri: !cli.no_ros4hri,
    };
    let bridges = device::BridgeConfig {
        #[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
        ros2: cli.ros2.as_deref().map(parse_ros2).transpose()?,
        #[cfg(feature = "studio")]
        studio: cli.studio,
    };
    // Frames are the ROS4HRI face image, so they follow that exposure unless
    // a rate is given (see `frames::publish_rate`).
    #[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
    let ros2 = cli.ros2.is_some();
    #[cfg(not(any(feature = "ros2-dds", feature = "ros2-zenoh")))]
    let ros2 = false;
    let rate_hz = frames::publish_rate(cli.frame_rate, ros2, !cli.no_ros4hri)?;
    let dev = device::start(&cli.glb, config, bridges, mode)?;
    println!(
        "vizij: {} — {} elements, {} animatables, {} bundle graphs",
        dev.glb_path,
        dev.meta.elements.len(),
        dev.meta.animatables.len(),
        dev.meta.bundle.graphs.len(),
    );
    let frame_config = frames::FrameConfig {
        format: cli.frame_format,
        rate_hz,
        fixed_frame_id: cli.frame_id.clone(),
        face_frame_id: frames::default_frame_id(dev.meta.bundle.face_id.as_deref(), &dev.glb_path),
    };

    let [r, g, b] = device::parse_rgb(&cli.background)?;
    let options = view::ViewOptions {
        background: Color::srgb_u8(r, g, b),
        fit: cli.fit,
        zoom: cli.zoom,
        ambient: cli.ambient,
        unlit: cli.unlit,
    };
    let device::Device {
        rig,
        meta,
        glb_path,
        events,
        ..
    } = dev;
    let face = view::Face { meta, glb_path };
    let device_res = view::DeviceRes { rig };

    match (&cli.snapshot, cli.headless) {
        (Some(out), _) => run_snapshot(&cli, face, device_res, options, out),
        (None, true) => run_headless(&cli.size, face, device_res, options, events, frame_config),
        (None, false) => run_window(face, device_res, options, events, frame_config),
    }
}

/// The window size the app opens at: the face's authored aspect at a 720px
/// height, so it starts letterbox-free (resizes and full screen then follow
/// the `--fit` policy).
fn window_resolution(face: &view::Face) -> (u32, u32) {
    let (_, _, bw, bh) = face.meta.root_bounds.unwrap_or((0.0, 0.0, 5.0, 4.0));
    let height = 720.0_f32;
    let width = (height * bw / bh).clamp(320.0, 1600.0);
    (width.round() as u32, height as u32)
}

fn run_window(
    face: view::Face,
    device_res: view::DeviceRes,
    options: view::ViewOptions,
    events: std::sync::mpsc::Receiver<device::DeviceEvent>,
    frame_config: frames::FrameConfig,
) -> Result<()> {
    let (width, height) = window_resolution(&face);
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Vizij".to_string(),
                    resolution: (width, height).into(),
                    ..default()
                }),
                ..default()
            })
            .set(bevy::asset::AssetPlugin {
                unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                ..default()
            })
            .disable::<bevy::log::LogPlugin>(),
    )
    .insert_resource(face)
    .insert_resource(device_res)
    .insert_resource(options)
    .insert_resource(view::DeviceEvents(std::sync::Mutex::new(events)))
    .add_plugins(view::ViewPlugin);
    // Frame publishing works with a window too (not only headless): capture the
    // window and push the frame onto the device's reading feed.
    if frame_config.publishes() {
        app.insert_resource(frame_config)
            .add_plugins(frames::FramesPlugin);
    }
    app.run();
    restore_terminal();
    Ok(())
}

/// Render windowless and stream frames into the store — same view and device as
/// the window, but a `ScheduleRunnerPlugin` loop drawing into an offscreen image
/// the frame publisher captures, no winit. The device runs on its worker thread
/// as usual (bridges attached), so frames fan out over every bridge.
fn run_headless(
    size: &str,
    face: view::Face,
    device_res: view::DeviceRes,
    options: view::ViewOptions,
    events: std::sync::mpsc::Receiver<device::DeviceEvent>,
    frame_config: frames::FrameConfig,
) -> Result<()> {
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::render::render_resource::TextureUsages;

    let (width, height) = parse_size(size)?;
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                ..default()
            })
            .set(bevy::asset::AssetPlugin {
                unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                ..default()
            })
            .disable::<bevy::winit::WinitPlugin>()
            .disable::<bevy::log::LogPlugin>(),
    )
    .add_plugins(ScheduleRunnerPlugin::run_loop(
        std::time::Duration::from_secs_f64(1.0 / 60.0),
    ))
    .insert_resource(face)
    .insert_resource(device_res)
    .insert_resource(options)
    .insert_resource(view::DeviceEvents(std::sync::Mutex::new(events)))
    .add_plugins(view::ViewPlugin);

    // The offscreen image the view camera renders into (COPY_SRC so the frame
    // publisher's capture can read it back); its presence makes the camera
    // target it instead of a window.
    let mut target = Image::new_target_texture(width, height, snapshot::FORMAT, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let handle = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    app.insert_resource(view::OffscreenTarget(handle));

    if frame_config.publishes() {
        app.insert_resource(frame_config)
            .add_plugins(frames::FramesPlugin);
    } else {
        log::warn!(
            "--headless renders but publishes no frames: expose the device as ROS4HRI (--ros2) or \
             give a --frame-rate"
        );
    }
    app.run();
    restore_terminal();
    Ok(())
}

/// The device's terminal UI runs on the worker thread; returning from a Bevy run
/// ends the process without unwinding that thread, which would leave the terminal
/// in raw mode on the alternate screen. Undo its setup (arora's `restore_terminal`
/// recipe) on the way out.
fn restore_terminal() {
    if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        use crossterm::event::DisableMouseCapture;
        use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
    }
}

fn run_snapshot(
    cli: &Cli,
    face: view::Face,
    device_res: view::DeviceRes,
    options: view::ViewOptions,
    out: &std::path::Path,
) -> Result<()> {
    let (width, height) = parse_size(&cli.size)?;
    let mut app = App::new();
    app.add_plugins(snapshot::SnapshotPlugin { width, height })
        .insert_resource(face)
        .insert_resource(device_res)
        .insert_resource(options)
        .add_plugins(view::ViewPlugin);

    // Ready once the scene is indexed (bindings joined); settle ~1 s of frames
    // so the device's pose has flowed through the HAL onto the scene.
    let img = snapshot::capture(&mut app, width, height, 60, |app| {
        app.world()
            .get_resource::<view::BindingIndex>()
            .map(|i| i.ready)
            .unwrap_or(false)
    })?;
    snapshot::save_png(&img, out)?;
    println!("snapshot written to {}", out.display());
    Ok(())
}

fn parse_size(size: &str) -> Result<(u32, u32)> {
    let (w, h) = size
        .split_once('x')
        .ok_or_else(|| anyhow!("--size must be WIDTHxHEIGHT"))?;
    Ok((w.parse()?, h.parse()?))
}

/// `--zoom` value: one positive factor for both axes, or `<x>x<y>`, one per
/// axis (width, then height).
fn parse_zoom(spec: &str) -> Result<Vec2> {
    let factor = |s: &str| -> Result<f32> {
        let f: f32 = s
            .trim()
            .parse()
            .map_err(|_| anyhow!("--zoom factors must be numbers, got {s:?}"))?;
        if f.is_finite() && f > 0.0 {
            Ok(f)
        } else {
            Err(anyhow!("--zoom factors must be positive, got {s}"))
        }
    };
    Ok(match spec.split_once('x') {
        Some((x, y)) => Vec2::new(factor(x)?, factor(y)?),
        None => Vec2::splat(factor(spec)?),
    })
}

/// `--ros2` value `[namespace][:domain]` → (namespace, domain), each optional
/// (empty namespace, domain 0 by default).
#[cfg(any(feature = "ros2-dds", feature = "ros2-zenoh"))]
fn parse_ros2(spec: &str) -> Result<(String, u16)> {
    let (namespace, domain) = spec.split_once(':').unwrap_or((spec, ""));
    let domain = if domain.is_empty() {
        0
    } else {
        domain.parse().context("--ros2 domain must be a number")?
    };
    Ok((namespace.to_string(), domain))
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn a_single_zoom_factor_applies_to_both_axes() {
        assert_eq!(parse_zoom("1.5").unwrap(), Vec2::splat(1.5));
    }

    #[test]
    fn a_zoom_pair_is_width_then_height() {
        assert_eq!(parse_zoom("2x0.5").unwrap(), Vec2::new(2.0, 0.5));
    }

    #[test]
    fn zoom_rejects_non_positive_and_malformed_factors() {
        for bad in ["0", "-1", "nan", "inf", "2x", "x2", "2x0", "big", "1x2x3"] {
            assert!(parse_zoom(bad).is_err(), "{bad:?} should be rejected");
        }
    }
}
