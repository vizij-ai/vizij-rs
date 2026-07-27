//! Vizij: an arora with a head.
//!
//! `cargo run -- --glb <face.glb>` opens the Vizij view (Bevy) rendering the
//! face, driven by an arora device running the face's own graphs (rig +
//! pose-driver from the embedded `VIZIJ_bundle`). `--snapshot out.png` renders
//! offscreen (no window) and writes a PNG instead — the comparison harness
//! against the web renderer.

use anyhow::{anyhow, Result};
use bevy::prelude::*;
use clap::Parser;

mod device;
mod meta;
mod snapshot;
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

    /// Offscreen render size, WIDTHxHEIGHT.
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
    /// behavior), cover fills the window and crops the excess axis.
    #[arg(long, value_enum, default_value_t = view::Fit::Contain)]
    fit: view::Fit,

    /// Compose only these bundle graph kinds (comma-separated), e.g. "rig" or
    /// "rig,pose-driver". Default: rig + pose-driver.
    #[arg(long, default_value = "rig,pose-driver,pose")]
    graphs: String,
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
    let dev = device::start(&cli.glb, wanted, mode)?;
    println!(
        "vizij: {} — {} elements, {} animatables, {} bundle graphs",
        dev.glb_path,
        dev.meta.elements.len(),
        dev.meta.animatables.len(),
        dev.meta.bundle_graphs.len(),
    );

    let [r, g, b] = device::parse_rgb(&cli.background)?;
    let options = view::ViewOptions {
        background: Color::srgb_u8(r, g, b),
        fit: cli.fit,
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

    match &cli.snapshot {
        Some(out) => run_snapshot(&cli, face, device_res, options, out),
        None => run_window(face, device_res, options, events),
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
) -> Result<()> {
    let (width, height) = window_resolution(&face);
    App::new()
        .add_plugins(
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
        .add_plugins(view::ViewPlugin)
        .run();
    // The device's terminal UI runs on the worker thread; returning from here
    // ends the process without unwinding that thread, which would leave the
    // terminal in raw mode on the alternate screen. Undo its setup (arora's
    // `restore_terminal` recipe) on the way out.
    if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        use crossterm::event::DisableMouseCapture;
        use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
    }
    Ok(())
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
