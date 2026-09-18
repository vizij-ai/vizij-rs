//! A command line over the scene.
//!
//! One host among several. The scene itself is the library beside this; what
//! lives here is flag parsing, the window, and the parts of a run that only a
//! desktop process can do — resolving assets from a directory and writing a
//! PNG to disk. A wasm host replaces exactly this file.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::PresentMode;
use scene::{RobotSpec, SceneOptions, ScenePlugin};

#[derive(clap::Parser)]
struct Cli {
    /// The robot to load.
    #[arg(long, default_value = "Quori_2_RevB.glb")]
    glb: String,

    /// Where assets are resolved from. A relative path is rooted at the crate
    /// directory, not the workspace, so absolute is usually what you want.
    #[arg(long, default_value = "fixtures")]
    assets: PathBuf,

    /// Stop after this many seconds and report. Zero runs until closed.
    #[arg(long, default_value_t = 0.0)]
    seconds: f32,

    /// Hold the camera still instead of orbiting, to separate the cost of a
    /// moving view from the cost of the scene.
    #[arg(long, default_value_t = false)]
    still: bool,

    /// Where the camera starts, as a yaw in radians about the model. Defaults
    /// to looking the robot in the face.
    #[arg(long, default_value_t = 0.0)]
    angle: f32,

    /// How far above the model the camera starts, in radians.
    #[arg(long, default_value_t = 0.4)]
    pitch: f32,

    /// Drop shadows, to price them separately.
    #[arg(long, default_value_t = false)]
    no_shadows: bool,

    /// Present without waiting for vsync, so the frame time measures the work
    /// rather than the display's cadence.
    ///
    /// Note that wgpu falls back to Fifo where a platform cannot honour this —
    /// macOS among them, where it does nothing at all. Use `--offscreen`
    /// there.
    #[arg(long, default_value_t = false)]
    uncapped: bool,

    /// Spawn the robot this many times, in a grid.
    #[arg(long, default_value_t = 1)]
    copies: u32,

    /// Save one frame here, after the warm-up.
    #[arg(long)]
    screenshot: Option<PathBuf>,

    /// Add a reference cube at the model's centre and clear to magenta.
    #[arg(long, default_value_t = false)]
    marker: bool,

    /// Draw this face onto the screen the robot declares.
    #[arg(long)]
    face: Option<String>,

    /// Put a limited rotation gizmo on this joint, by name. Pass `list` to see
    /// which joints the robot declares and what limits they carry.
    #[arg(long)]
    joint: Option<String>,

    /// Hold the ring in its hovered state.
    #[arg(long, default_value_t = false)]
    ring_active: bool,

    /// Drive the joint to this value instead of its authored default.
    #[arg(long)]
    joint_value: Option<f32>,

    /// Give every mesh its own material, as a renderer must when each
    /// element's colour is separately drivable.
    #[arg(long, default_value_t = false)]
    unique_materials: bool,

    /// Render into an offscreen target and leave the window empty.
    #[arg(long, default_value_t = false)]
    offscreen: bool,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = <Cli as clap::Parser>::parse();

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "scene spike".into(),
                    resolution: (1280u32, 800u32).into(),
                    present_mode: if cli.uncapped {
                        PresentMode::AutoNoVsync
                    } else {
                        PresentMode::AutoVsync
                    },
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: cli.assets.to_string_lossy().into_owned(),
                ..default()
            })
            .build()
            // The process installs its own logger before Bevy starts.
            .disable::<bevy::log::LogPlugin>(),
    )
    .add_plugins(ScenePlugin)
    .insert_resource(SceneOptions {
        still: cli.still,
        angle: cli.angle,
        pitch: cli.pitch,
        shadows: !cli.no_shadows,
        marker: cli.marker,
        unique_materials: cli.unique_materials,
        ring_active: cli.ring_active,
        offscreen: cli.offscreen,
        seconds: cli.seconds,
        screenshot: cli.screenshot.clone(),
        uncapped: cli.uncapped,
    })
    .insert_resource(RobotSpec {
        glb: cli.glb.clone(),
        copies: cli.copies,
    });

    // Both read the GLB from disk before the app runs, which is why they are
    // the CLI's to call rather than the scene's to do: a wasm host has no
    // filesystem, and an editor loads its assets after startup. Making this an
    // asset-driven load is what would let either happen.
    if let Some(face) = &cli.face {
        scene::mount_face(&mut app, &cli.assets, &cli.glb, face);
    }
    if let Some(joint) = &cli.joint {
        scene::mount_joint(&mut app, &cli.assets, &cli.glb, joint, cli.joint_value);
    }
    app.run();
}
