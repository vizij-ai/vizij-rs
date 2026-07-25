//! Vizij: an arora with a head.
//!
//! `cargo run -- --glb <face.glb>` opens the Vizij view (Bevy) rendering the
//! face, driven by an arora device running the face's own graphs (rig +
//! pose-driver from the embedded `VIZIJ_bundle`). `--snapshot out.png` renders
//! offscreen (no window) and writes a PNG instead — the comparison harness
//! against the web renderer.

use anyhow::{anyhow, Context, Result};
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

    /// Background clear color, hex RRGGBB.
    #[arg(long, default_value = "101114")]
    background: String,

    /// three.js-style ambient intensity (the web renderer uses π/2).
    #[arg(long, default_value_t = std::f32::consts::FRAC_PI_2)]
    ambient: f32,

    /// Render materials unlit (albedo passthrough).
    #[arg(long)]
    unlit: bool,

    /// Compose only these bundle graph kinds (comma-separated), e.g. "rig" or
    /// "rig,pose-driver". Default: rig + pose-driver.
    #[arg(long, default_value = "rig,pose-driver,pose")]
    graphs: String,
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    let face_meta = meta::FaceMeta::from_glb_file(&cli.glb)?;
    log::info!(
        "loaded {}: {} elements, {} animatables, {} bundle graphs, rootBounds {:?}",
        cli.glb.display(),
        face_meta.elements.len(),
        face_meta.animatables.len(),
        face_meta.bundle_graphs.len(),
        face_meta.root_bounds,
    );

    // Compose the selected bundle graphs into the device's one behavior graph.
    let wanted: Vec<&str> = cli.graphs.split(',').map(str::trim).collect();
    let graphs: Vec<(String, serde_json::Value)> = face_meta
        .bundle_graphs
        .iter()
        .filter(|(kind, _)| wanted.contains(&kind.as_str()))
        .cloned()
        .collect();
    if graphs.is_empty() {
        log::warn!(
            "no bundle graphs matched {:?}; the face will hold its authored pose",
            wanted
        );
    }
    let composed = device::compose(&graphs)?;
    let dev = device::start(&composed)?;

    let glb_path = cli
        .glb
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", cli.glb.display()))?
        .to_string_lossy()
        .into_owned();

    let options = view::ViewOptions {
        background: parse_hex_color(&cli.background)?,
        ambient: cli.ambient,
        unlit: cli.unlit,
    };
    let face = view::Face {
        meta: face_meta,
        glb_path,
    };
    let device_res = view::DeviceRes {
        rig: dev.rig.clone(),
    };

    match &cli.snapshot {
        Some(out) => run_snapshot(&cli, face, device_res, options, out),
        None => run_window(face, device_res, options),
    }
}

fn run_window(
    face: view::Face,
    device_res: view::DeviceRes,
    options: view::ViewOptions,
) -> Result<()> {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Vizij".to_string(),
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
        .add_plugins(view::ViewPlugin)
        .run();
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

fn parse_hex_color(hex: &str) -> Result<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err(anyhow!("--background must be RRGGBB"));
    }
    let r = u8::from_str_radix(&hex[0..2], 16)?;
    let g = u8::from_str_radix(&hex[2..4], 16)?;
    let b = u8::from_str_radix(&hex[4..6], 16)?;
    Ok(Color::srgb_u8(r, g, b))
}
