//! What a robot costs to draw.
//!
//! The first question of the scene spike is whether Bevy holds frame rate on
//! the Studio scene's real load — a robot with PBR and shadows — rather than
//! on the flat planes a face is made of. Everything else in the spike rests on
//! this, so it is measured before anything is built on top of it.
//!
//! **Measure with `--offscreen`.** A windowed run reports the display, not the
//! renderer: `--uncapped` cannot lift vsync on macOS, where wgpu stays on
//! Fifo, and on a ProMotion panel the refresh rate itself moves, so the same
//! scene reads 8.33ms or 16.67ms depending on where the display has settled.
//! Both are exact refresh intervals, which is how to recognise the trap.
//! Rendering into an image leaves nothing presenting to a surface, and then
//! the frame time is the work.
//!
//! Percentiles rather than an average: a mean hides the stalls that make a
//! viewport feel bad, and the 99th is what a person notices.

mod robot;

use std::path::PathBuf;

use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::PresentMode;
use vizij_render::{
    Face, FaceLayer, FaceMeta, Fit, OffscreenTarget, PoseFeed, ViewOptions, ViewPlugin,
};

#[derive(clap::Parser, Resource, Clone)]
#[command(about = "Render a robot GLB and report what it costs per frame.")]
struct Cli {
    /// The robot to load.
    #[arg(long, default_value = "Quori_2_RevB.glb")]
    glb: String,

    /// Where assets are resolved from. A relative path is rooted at the crate
    /// directory, not the workspace, so absolute is usually what you want.
    #[arg(long, default_value = "fixtures")]
    assets: PathBuf,

    /// Stop after this many seconds and report. Zero runs until closed.
    #[arg(long, default_value_t = 12.0)]
    seconds: f32,

    /// Hold the camera still instead of orbiting, to separate the cost of a
    /// moving view from the cost of the scene.
    #[arg(long, default_value_t = false)]
    still: bool,

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
    ///
    /// Multiplying the real asset states the headroom in the units that
    /// matter: how many robots' worth of scene a frame budget holds.
    #[arg(long, default_value_t = 1)]
    copies: u32,

    /// Save one frame here, after the warm-up.
    #[arg(long)]
    screenshot: Option<PathBuf>,

    /// Add a reference cube at the model's centre and clear to magenta. A
    /// capture that comes back black is ambiguous — nothing rendered, or
    /// nothing was read back — and these two separate those cases.
    #[arg(long, default_value_t = false)]
    marker: bool,

    /// Draw this face onto the screen the robot declares.
    #[arg(long)]
    face: Option<String>,

    /// Render into an offscreen target and leave the window empty.
    ///
    /// With nothing presenting to the surface there is no vsync to wait on, so
    /// frame time becomes the work itself. This is the only way to get past
    /// the display's cadence here, since wgpu ignores `--uncapped` on macOS.
    #[arg(long, default_value_t = false)]
    offscreen: bool,
}

/// Frame durations, collected after a warm-up so shader compilation and asset
/// loading are not counted as render cost.
#[derive(Resource, Default)]
struct Frames {
    samples: Vec<f32>,
    elapsed: f32,
    count: u64,
    reported: bool,
}

/// Proof that the frames above were paid for a robot rather than an empty
/// viewport.
///
/// Reads back an offscreen render target rather than the window. A capture of
/// the window's swapchain comes back pure black on macOS when the window is
/// not the composited, frontmost surface — which is indistinguishable from a
/// scene that genuinely drew nothing, and the frame counter is happy either
/// way. An offscreen target does not depend on the window being on screen.
///
/// Runs during the warm-up, so the second camera it needs is gone before the
/// first frame is sampled.
#[derive(Resource, Default)]
struct Capture {
    image: Option<Handle<Image>>,
    camera: Option<Entity>,
    at_frame: u64,
    done: bool,
}

const CAPTURE_SECONDS: f32 = 1.0;

/// What was actually on screen while the frames above were measured. A frame
/// time means nothing without the load that produced it.
#[derive(Resource, Default)]
struct Census {
    meshes: usize,
    triangles: usize,
    materials: usize,
    taken: bool,
    /// The most meshes any single frame passed to the renderer, as Bevy's own
    /// frustum culling judged it. This is what says the frame times were paid
    /// for a visible robot: a camera pointed at nothing culls everything and
    /// still reports a healthy frame rate.
    visible_peak: usize,
    /// The face's own meshes, counted apart from the robot's.
    face_meshes: usize,
}

/// Whether an entity belongs to the face rather than the scene around it.
fn on_face_layer(layers: Option<&RenderLayers>) -> bool {
    layers.is_some_and(|layers| layers.intersects(&RenderLayers::layer(FACE_LAYER)))
}

/// Where the model actually sits, measured rather than assumed. A glTF carries
/// no promise about its units or which way is up, and a camera placed on a
/// guess renders an empty frame that still reports a healthy frame rate — so
/// the camera is fitted to the bounds the meshes report.
#[derive(Resource, Default)]
struct Framing {
    center: Vec3,
    radius: f32,
    fitted: bool,
}

const WARMUP_SECONDS: f32 = 2.0;

/// The layer the face draws on, so the two scenes sharing this world do not
/// draw into each other's views.
const FACE_LAYER: usize = 1;

#[derive(Component)]
struct OrbitCamera;

/// The screen the robot declared, and the texture a face is drawing into.
#[derive(Resource)]
struct ScreenMount {
    screen: robot::Screen,
    texture: Handle<Image>,
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
    .init_resource::<Frames>()
    .init_resource::<Census>()
    .init_resource::<Framing>()
    .init_resource::<Capture>()
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            take_census,
            spawn_marker,
            orbit_camera,
            count_visible,
            capture,
            measure,
        )
            .chain(),
    );

    if cli.face.is_some() {
        mount_face(&mut app, &cli);
    }
    app.insert_resource(cli).run();
}

/// Puts a face on the screen the robot declares.
///
/// This runs before the app does, because the face crate's own startup reads
/// the target and the layer — and because the screen's size, which sets the
/// texture's, comes out of the robot's JSON rather than out of the world.
fn mount_face(app: &mut App, cli: &Cli) {
    let face_glb = cli.face.clone().expect("checked by the caller");
    let read = |name: &str| std::fs::read(cli.assets.join(name));

    let screen = match read(&cli.glb)
        .map_err(anyhow::Error::from)
        .and_then(|bytes| robot::find_screen(&bytes))
    {
        Ok(screen) => screen,
        Err(error) => {
            log::error!("no screen to draw a face on: {error:#}");
            return;
        }
    };
    let meta = match read(&face_glb)
        .map_err(anyhow::Error::from)
        .and_then(|bytes| FaceMeta::from_glb_bytes(&bytes))
    {
        Ok(meta) => meta,
        Err(error) => {
            log::error!("could not read {face_glb}: {error:#}");
            return;
        }
    };

    let (width, height) = screen.texture_size();
    log::info!(
        "screen: {:.3}x{:.3}m at {:.2?}, drawing {face_glb} into {width}x{height}",
        screen.width,
        screen.height,
        screen.transform.translation,
    );

    // Sized from the screen's own metres and pixels-per-metre, as Studio sizes
    // its RenderTexture. No COPY_SRC: this texture is sampled by the quad's
    // material on the GPU and never read back.
    let texture = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            width,
            height,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        ));

    app.insert_resource(OffscreenTarget(texture.clone()))
        .insert_resource(FaceLayer(RenderLayers::layer(FACE_LAYER)))
        .insert_resource(Face {
            meta,
            glb_path: face_glb,
        })
        .insert_resource(ViewOptions {
            background: Color::BLACK,
            fit: Fit::Contain,
            zoom: Vec2::ONE,
            ambient: std::f32::consts::FRAC_PI_2,
            unlit: false,
        })
        // No device drives this spike, so the face holds its authored pose.
        // What is being proved here is the path to the screen, not the values.
        .insert_resource(PoseFeed::new(Vec::new))
        .insert_resource(ScreenMount { screen, texture })
        .add_plugins(ViewPlugin)
        .add_systems(Startup, spawn_screen);
}

/// Builds the quad the robot's screen only describes, and hangs the face's
/// texture on it.
fn spawn_screen(
    mount: Res<ScreenMount>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(mount.screen.width, mount.screen.height))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(mount.texture.clone()),
            // The face bakes its own lighting, so the scene's lights must not
            // get a second say over it.
            unlit: true,
            ..default()
        })),
        mount.screen.transform,
    ));
}

fn setup(
    mut commands: Commands,
    cli: Res<Cli>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    log::info!(
        "loading {} ({})",
        cli.glb,
        if cli.uncapped { "uncapped" } else { "vsync" }
    );
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(cli.glb.clone()));
    // A square-ish grid, spaced by roughly a robot's width. The exact spacing
    // does not matter — the camera fits whatever bounds result.
    let stride = (cli.copies as f32).sqrt().ceil().max(1.0) as u32;
    for i in 0..cli.copies.max(1) {
        let (x, z) = ((i % stride) as f32, (i / stride) as f32);
        commands.spawn((
            WorldAssetRoot(scene.clone()),
            Transform::from_xyz((x - (stride - 1) as f32 * 0.5) * 0.8, 0.0, z * 0.8),
        ));
    }

    // Placed properly once the model's bounds are known; see `take_census`.
    let mut camera = commands.spawn((
        Camera3d::default(),
        // Ambient light rides the camera in 0.19 rather than being a resource.
        AmbientLight {
            color: Color::WHITE,
            brightness: 220.0,
            ..default()
        },
        OrbitCamera,
    ));
    if cli.offscreen {
        let target = images.add(Image::new_target_texture(
            1280,
            800,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        ));
        camera.insert(RenderTarget::Image(target.into()));
    }

    // A directional light is a direction, not a position, so this one needs no
    // fitting — only its shadow cascades do, once the scale is known.
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadow_maps_enabled: !cli.no_shadows,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// A slow orbit at a distance the model's own size sets, so the measurement
/// covers a moving camera rather than a static frame the GPU can coast on.
fn orbit_camera(
    cli: Res<Cli>,
    time: Res<Time>,
    framing: Res<Framing>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<OrbitCamera>>,
) {
    if !framing.fitted {
        return;
    }
    let angle = if cli.still {
        0.6
    } else {
        time.elapsed_secs() * 0.35
    };
    // Far enough out that the bounding sphere fits the vertical field of view,
    // with a little air around it.
    let distance = framing.radius * 2.6;
    for (mut transform, mut projection) in &mut cameras {
        *transform = Transform::from_translation(
            framing.center
                + Vec3::new(
                    distance * angle.cos(),
                    framing.radius * 0.35,
                    distance * angle.sin(),
                ),
        )
        .looking_at(framing.center, Vec3::Y);

        // The default near/far pair assumes a metre-scale world; a model in
        // millimetres would sit entirely behind the far plane.
        if framing.is_changed() {
            *projection = Projection::Perspective(PerspectiveProjection {
                near: framing.radius * 0.01,
                far: framing.radius * 20.0,
                ..default()
            });
        }
    }
}

/// Counts what the glTF actually brought in, and measures where it sits.
///
/// Waits out the first half-second: transform propagation and bounds
/// calculation both run after `Update`, so a census taken on the frame the
/// meshes appear would measure them at the origin with no bounds at all.
fn take_census(
    frames: Res<Frames>,
    mut census: ResMut<Census>,
    mut framing: ResMut<Framing>,
    meshes: Query<(
        &Mesh3d,
        &GlobalTransform,
        Option<&Aabb>,
        Option<&RenderLayers>,
    )>,
    materials: Query<&MeshMaterial3d<StandardMaterial>>,
    assets: Res<Assets<Mesh>>,
) {
    if census.taken || frames.elapsed < 0.5 {
        return;
    }
    let mut count = 0;
    let mut triangles = 0;
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for (handle, global, aabb, layers) in &meshes {
        // The face lives in this world too, off to one side and metres wide.
        // Counting it would price the wrong scene, and fitting the camera to
        // it would push the robot into the distance.
        if on_face_layer(layers) {
            continue;
        }
        count += 1;
        if let Some(mesh) = assets.get(&handle.0) {
            triangles += match mesh.indices() {
                Some(indices) => indices.len() / 3,
                None => mesh.count_vertices() / 3,
            };
        }
        let Some(aabb) = aabb else { continue };
        // Every corner, because a rotated box's extents are not its transform's.
        for i in 0..8 {
            let corner = Vec3::new(
                if i & 1 == 0 { -1.0 } else { 1.0 },
                if i & 2 == 0 { -1.0 } else { 1.0 },
                if i & 4 == 0 { -1.0 } else { 1.0 },
            );
            let local = Vec3::from(aabb.center) + Vec3::from(aabb.half_extents) * corner;
            let world = global.transform_point(local);
            min = min.min(world);
            max = max.max(world);
        }
    }
    if count == 0 || !min.is_finite() || !max.is_finite() {
        return;
    }
    // Field by field, so the running visibility peak is not reset.
    census.meshes = count;
    census.triangles = triangles;
    census.materials = materials.iter().count();
    census.taken = true;
    census.face_meshes = meshes
        .iter()
        .filter(|(_, _, _, layers)| on_face_layer(*layers))
        .count();
    *framing = Framing {
        center: (min + max) * 0.5,
        radius: ((max - min).length() * 0.5).max(f32::EPSILON),
        fitted: true,
    };
    log::info!(
        "scene: {} meshes, {} triangles, {} materials",
        census.meshes,
        census.triangles,
        census.materials
    );
    log::info!(
        "bounds: min {:.2?} max {:.2?} — {:.2} across, centred at {:.2?}",
        min,
        max,
        (max - min).length(),
        framing.center,
    );
}

/// Tracks how many meshes survived culling, which is the engine's own answer
/// to "is the camera looking at the robot".
fn count_visible(
    mut census: ResMut<Census>,
    meshes: Query<(&ViewVisibility, Option<&RenderLayers>), With<Mesh3d>>,
) {
    let visible = meshes
        .iter()
        .filter(|(visible, layers)| visible.get() && !on_face_layer(*layers))
        .count();
    census.visible_peak = census.visible_peak.max(visible);
}

/// An unmissable cube at the model's centre, sized to it. If this renders and
/// the robot does not, the problem is the robot rather than the view.
fn spawn_marker(
    cli: Res<Cli>,
    framing: Res<Framing>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
    mut spawned: Local<bool>,
) {
    if !cli.marker || *spawned || !framing.fitted {
        return;
    }
    *spawned = true;
    let size = framing.radius * 0.5;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(size, size, size))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 1.0, 0.2),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(framing.center),
    ));
    log::info!("marker cube: {:.2}m at {:.2?}", size, framing.center);
}

/// Renders one frame into an offscreen target and reads it back, in three
/// steps: point a second camera at the target, give it two frames to draw, then
/// save it and take the camera away again.
fn capture(
    cli: Res<Cli>,
    frames: Res<Frames>,
    framing: Res<Framing>,
    mut capture: ResMut<Capture>,
    mut images: ResMut<Assets<Image>>,
    cameras: Query<(&Transform, &Projection), With<OrbitCamera>>,
    mut commands: Commands,
) {
    let Some(path) = cli.screenshot.clone() else {
        return;
    };
    if capture.done || !framing.fitted || frames.elapsed < CAPTURE_SECONDS {
        return;
    }
    let Ok((transform, projection)) = cameras.single() else {
        return;
    };

    let Some(camera) = capture.camera else {
        let mut target = Image::new_target_texture(
            1280,
            800,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        );
        // `new_target_texture` asks for everything needed to render *into* the
        // texture but not to copy back *out* of it, so without this the
        // readback returns the zero-filled CPU side — a black image that looks
        // exactly like a scene that drew nothing.
        target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let image = images.add(target);
        // Same viewpoint as the window's camera, drawn before it.
        capture.camera = Some(
            commands
                .spawn((
                    Camera3d::default(),
                    Camera {
                        order: -1,
                        clear_color: if cli.marker {
                            ClearColorConfig::Custom(Color::srgb(1.0, 0.0, 1.0))
                        } else {
                            ClearColorConfig::Default
                        },
                        ..default()
                    },
                    RenderTarget::Image(image.clone().into()),
                    *transform,
                    projection.clone(),
                    AmbientLight {
                        color: Color::WHITE,
                        brightness: 220.0,
                        ..default()
                    },
                ))
                .id(),
        );
        capture.image = Some(image);
        capture.at_frame = frames.count;
        return;
    };

    if frames.count < capture.at_frame + 2 {
        return;
    }
    capture.done = true;
    log::info!("saving a frame to {}", path.display());
    let image = capture
        .image
        .clone()
        .expect("image spawned with the camera");
    commands
        .spawn(Screenshot::image(image))
        .observe(save_to_disk(path));
    commands.entity(camera).despawn();
}

fn measure(
    cli: Res<Cli>,
    time: Res<Time>,
    mut frames: ResMut<Frames>,
    census: Res<Census>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    frames.elapsed += dt;
    frames.count += 1;
    if frames.elapsed > WARMUP_SECONDS {
        frames.samples.push(dt * 1000.0);
    }
    if cli.seconds > 0.0 && frames.elapsed >= cli.seconds && !frames.reported {
        frames.reported = true;
        report(&cli, &frames, &census);
        exit.write(AppExit::Success);
    }
}

fn report(cli: &Cli, frames: &Frames, census: &Census) {
    if frames.samples.is_empty() {
        log::warn!("no frames measured");
        return;
    }
    let mut sorted = frames.samples.clone();
    sorted.sort_by(f32::total_cmp);
    let at = |q: f32| sorted[((sorted.len() - 1) as f32 * q) as usize];
    let mean: f32 = sorted.iter().sum::<f32>() / sorted.len() as f32;
    log::info!(
        "{} frames over {:.1}s — {} camera, shadows {}, {}",
        sorted.len(),
        frames.elapsed - WARMUP_SECONDS,
        if cli.still { "still" } else { "orbiting" },
        if cli.no_shadows { "off" } else { "on" },
        if cli.uncapped { "uncapped" } else { "vsync" },
    );
    log::info!(
        "frame ms  p50 {:.2}  p95 {:.2}  p99 {:.2}  max {:.2}  mean {:.2}  ({:.0} fps at p50)",
        at(0.50),
        at(0.95),
        at(0.99),
        sorted[sorted.len() - 1],
        mean,
        1000.0 / at(0.50),
    );
    if census.taken {
        log::info!(
            "drawing {}/{} meshes visible after culling / {} triangles / {} materials{}",
            census.visible_peak,
            census.meshes,
            census.triangles,
            census.materials,
            match census.face_meshes {
                0 => String::new(),
                n => format!(" / plus {n} face meshes on their own layer"),
            }
        );
    }
}
