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

mod ring;
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
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin, PanOrbitCameraSystemSet};
use ring::{
    JointBarMaterial, JointRingMaterial, JointRingPlugin, JointRingUniform, DEPTH_IN_FRONT,
};
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

    /// Where the camera starts, as a yaw in radians about the model. Defaults
    /// to looking the robot in the face.
    #[arg(long, default_value_t = -std::f32::consts::FRAC_PI_2)]
    angle: f32,

    /// How far above the model the camera starts, in radians.
    #[arg(long, default_value_t = 0.18)]
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

    /// Put a limited rotation gizmo on this joint, by name. Pass `list` to see
    /// which joints the robot declares and what limits they carry.
    #[arg(long)]
    joint: Option<String>,

    /// Hold the ring in its hovered state. The hover follows the pointer, so
    /// a headless capture cannot otherwise show the shaded ring at all.
    #[arg(long, default_value_t = false)]
    ring_active: bool,

    /// Drive the joint to this value instead of its authored default. Goes
    /// through the same clamp a drag does, so an out-of-range value here shows
    /// the limit holding.
    #[arg(long)]
    joint_value: Option<f32>,

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
/// Reads back the main camera's offscreen target rather than the window. A
/// capture of the window's swapchain comes back pure black whenever the window
/// is not the composited frontmost surface — which is indistinguishable from a
/// scene that genuinely drew nothing, and the frame counter is happy either
/// way. An offscreen target does not depend on the window being on screen.
#[derive(Resource, Default)]
struct Capture {
    at_frame: u64,
    done: bool,
}

/// The image the main camera renders into, when it is not drawing to the
/// window. This is what a screenshot reads.
#[derive(Resource)]
struct OffscreenView(Handle<Image>);

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

/// One joint, its authored range, and the body it turns.
///
/// The gizmo exists to answer what a limited rotator costs in Bevy. Studio
/// gets one from drei's `PivotControls`, which brings the ring, the hover and
/// annotation states, the drag maths and `rotationLimits` with it; Bevy has no
/// equivalent, so each of those is drawn or written here.
#[derive(Resource)]
struct JointGizmo {
    joint: robot::Joint,
    /// The body this turns, once the scene has spawned and been found.
    body: Option<Entity>,
    /// The body's pose before any of this touched it.
    rest: Transform,
    value: f32,
    dragging: bool,
    hovered: bool,
    /// The range as the ring draws it, measured from one end.
    range: ring::VisualRange,
    /// The two tori: thin while idle, thick and shaded while in hand.
    idle: Option<Entity>,
    active: Option<Entity>,
}

/// A value asked for from outside, applied once through the same clamp a drag
/// takes. Lets a headless run show a limit holding.
#[derive(Resource, Default)]
struct AskedFor(Option<f32>);

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
    .add_plugins((PanOrbitCameraPlugin, JointRingPlugin))
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
            fit_orbit,
            count_visible,
            capture,
            measure,
        )
            .chain(),
    );

    if cli.face.is_some() {
        mount_face(&mut app, &cli);
    }
    if cli.joint.is_some() {
        mount_joint(&mut app, &cli);
    }
    app.insert_resource(cli).run();
}

/// Selects the joint to put a gizmo on, or lists what there is.
fn mount_joint(app: &mut App, cli: &Cli) {
    let wanted = cli.joint.clone().expect("checked by the caller");
    let joints = match std::fs::read(cli.assets.join(&cli.glb))
        .map_err(anyhow::Error::from)
        .and_then(|bytes| robot::find_joints(&bytes))
    {
        Ok(joints) => joints,
        Err(error) => {
            log::error!("could not read the robot's joints: {error:#}");
            return;
        }
    };

    if wanted == "list" {
        log::info!("{} joints:", joints.len());
        for joint in &joints {
            let limits = if joint.is_limited() {
                format!("[{:.3}, {:.3}]", joint.min, joint.max)
            } else {
                "unlimited".to_string()
            };
            log::info!(
                "  {:24} {:11} {limits:22} default {:.3}  axis {:.0?}",
                joint.name,
                joint.kind.as_str(),
                joint.default,
                joint.axis,
            );
        }
        return;
    }

    let Some(joint) = joints.into_iter().find(|j| j.name == wanted) else {
        log::error!("no joint called {wanted:?}; pass --joint list to see them");
        return;
    };
    // A fixed joint holds its child in place; offering a control for it would
    // be a handle that moves something the robot cannot move.
    if !joint.kind.is_drivable() {
        log::error!(
            "{:?} is {} — it does not move, so it takes no control",
            joint.name,
            joint.kind.as_str()
        );
        return;
    }
    log::info!(
        "gizmo on {:?} ({}), range [{:.3}, {:.3}], resting at {:.3}",
        joint.name,
        joint.kind.as_str(),
        joint.min,
        joint.max,
        joint.default
    );
    app.insert_resource(JointGizmo {
        value: joint.default,
        range: ring::VisualRange::of(joint.min, joint.max),
        joint,
        body: None,
        rest: Transform::IDENTITY,
        dragging: false,
        hovered: false,
        idle: None,
        active: None,
    })
    .insert_resource(AskedFor(cli.joint_value))
    .add_systems(Startup, (spawn_label, gizmos_on_top))
    .add_systems(
        Update,
        // Before the camera reads the same button, so a press that grabs the
        // ring has already taken the camera out of the running.
        (bind_joint, drive_joint, draw_joint, drive_bar, place_label)
            .chain()
            .before(PanOrbitCameraSystemSet),
    );
}

/// Finds the body the joint turns, once the scene has spawned.
///
/// The robot's glTF nodes carry no names, so Bevy names them `GltfNode{index}`
/// — and that index is the only thing tying `RobotData` to a spawned entity.
#[allow(clippy::too_many_arguments)]
fn bind_joint(
    mut gizmo: ResMut<JointGizmo>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut plain: ResMut<Assets<StandardMaterial>>,
    mut rings: ResMut<Assets<JointRingMaterial>>,
    mut bars: ResMut<Assets<JointBarMaterial>>,
    names: Query<(Entity, &Name, &Transform, &GlobalTransform)>,
    frames: Res<Frames>,
) {
    if gizmo.body.is_some() || frames.elapsed < 0.5 {
        return;
    }
    let wanted = format!("GltfNode{}", gizmo.joint.child_node);
    let Some((entity, _, transform, global)) =
        names.iter().find(|(_, name, _, _)| name.as_str() == wanted)
    else {
        return;
    };
    gizmo.rest = *transform;
    gizmo.body = Some(entity);
    let range = gizmo.range;
    let colour = gizmo
        .joint
        .color
        .unwrap_or(Srgba::hex("e8c547").expect("literal"));
    let uniform = JointRingUniform {
        min: range.min,
        max: range.max,
        current: range.value(gizmo.value),
        color: colour.to_vec4(),
        background: Srgba::hex("333333").expect("literal").to_vec4(),
        ..default()
    };
    // Thin while idle, thick once in hand — the swap is the hover, as it is in
    // Studio's `axis-rotator` and `axis-slider` alike.
    let idle_material = plain.add(StandardMaterial {
        base_color: Color::from(colour).with_alpha(0.5),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        depth_bias: DEPTH_IN_FRONT,
        ..default()
    });
    let angular = gizmo.joint.kind.is_angular();

    let (idle_mesh, active_mesh) = if angular {
        (
            meshes.add(Torus {
                minor_radius: HANDLE_RADIUS * 0.02,
                major_radius: HANDLE_RADIUS,
            }),
            meshes.add(Torus {
                minor_radius: HANDLE_RADIUS * 0.1,
                major_radius: HANDLE_RADIUS,
            }),
        )
    } else {
        // A bar as long as the joint's travel, so its size states the range
        // the way the ring's arc does.
        let travel = (range.max - range.min).max(1e-3);
        let width = HANDLE_RADIUS;
        (
            meshes.add(Cuboid::new(width * 0.04, travel, width * 0.04)),
            meshes.add(Cuboid::new(width * 0.18, travel, width * 0.18)),
        )
    };

    gizmo.idle = Some(
        commands
            .spawn((Mesh3d(idle_mesh), MeshMaterial3d(idle_material)))
            .id(),
    );
    gizmo.active = Some(if angular {
        commands
            .spawn((
                Mesh3d(active_mesh),
                MeshMaterial3d(rings.add(JointRingMaterial { ring: uniform })),
                Visibility::Hidden,
            ))
            .id()
    } else {
        commands
            .spawn((
                Mesh3d(active_mesh),
                MeshMaterial3d(bars.add(JointBarMaterial { bar: uniform })),
                Visibility::Hidden,
            ))
            .id()
    });
    log::info!(
        "{:?} drives {wanted}; axis {:.2?} locally, {:.2?} in the world",
        gizmo.joint.name,
        gizmo.joint.axis,
        (global.rotation() * gizmo.joint.axis).normalize_or(Vec3::Y),
    );
}

/// The plane the joint turns in, in world space.
///
/// The authored axis is expressed in the joint's own frame, so it has to be
/// carried out by the joint's world rotation before anything in world space —
/// the ring, the pointer ray — can be measured against it.
/// The circle a joint turns through: where it is centred, the axis it turns
/// about, and two perpendicular directions spanning its plane.
///
/// One basis, stated once. Bevy's gizmo primitives disagree about which plane
/// they draw in — `circle` uses XY about +Z, while `arc_3d` starts at +X and
/// sweeps about +Y through XZ — so anything deriving its own frame from those
/// conventions draws a ring and its arc at right angles.
struct JointPlane {
    centre: Vec3,
    axis: Vec3,
    u: Vec3,
    v: Vec3,
}

impl JointPlane {
    /// Orients a torus mesh so its own angle runs the way this plane measures
    /// angles, starting from `origin`.
    ///
    /// Two corrections live here. The mesh's up is the plane's axis
    /// **negated**, because a torus winds the opposite way about its own axis
    /// from the way this basis measures — without that the ring fills against
    /// the drag. A torus is symmetric about its plane, so flipping its up
    /// changes the winding and nothing about how it looks. And the zero is
    /// turned to `origin`, so the span the shader draws lands on the
    /// directions the joint can actually reach rather than starting wherever
    /// `u` happened to fall.
    fn for_torus(&self, origin: f32) -> Quat {
        let start = self.u * origin.cos() + self.v * origin.sin();
        Quat::from_mat3(&Mat3::from_cols(start, -self.axis, self.axis.cross(start)))
    }
}

fn joint_plane(joint: &robot::Joint, body: &GlobalTransform) -> JointPlane {
    // Read from the body the joint turns, not from the joint node parsed out
    // of the glTF. The turn is applied in the body's own frame, so the axis
    // rides that frame out into the world, and the pivot is the body's own
    // origin. Taking both live also means the ring follows whatever the joints
    // above it are doing, which a pose parsed once at load cannot.
    let (_, rotation, centre) = body.to_scale_rotation_translation();
    let axis = (rotation * joint.axis).normalize_or(Vec3::Y);
    let u = axis.any_orthonormal_vector();
    JointPlane {
        centre,
        axis,
        u,
        v: axis.cross(u),
    }
}

/// Where a pointer ray meets the joint's plane: the angle about the axis, and
/// how far out from the centre it landed.
fn angle_under_pointer(
    joint: &robot::Joint,
    body: &GlobalTransform,
    camera: &Camera,
    camera_at: &GlobalTransform,
    cursor: Vec2,
) -> Option<(f32, f32)> {
    let Ok(ray) = camera.viewport_to_world(camera_at, cursor) else {
        return None;
    };
    let plane = joint_plane(joint, body);
    let (centre, axis) = (plane.centre, plane.axis);

    // A ray parallel to the plane never meets it, and one nearly parallel
    // meets it so far away that the angle is noise.
    let slope = axis.dot(*ray.direction);
    if slope.abs() < 1e-3 {
        return None;
    }
    let distance = (centre - ray.origin).dot(axis) / slope;
    if distance <= 0.0 {
        return None;
    }
    let offset = ray.origin + *ray.direction * distance - centre;
    Some((
        f32::atan2(offset.dot(plane.v), offset.dot(plane.u)),
        offset.length(),
    ))
}

const HANDLE_RADIUS: f32 = 0.12;

/// Whether a hit on the joint's plane counts as being on the ring.
///
/// A band rather than the tube's own surface, because the plane a ray is
/// tested against reaches across the whole scene: without this a click
/// anywhere would take hold of the joint. Studio does the same thing with a
/// fatter invisible torus in front of the visible one.
fn near_ring(distance: f32) -> bool {
    (HANDLE_RADIUS * 0.45..=HANDLE_RADIUS * 1.7).contains(&distance)
}

/// Drags the joint within its limits, and writes the result onto the body.
///
/// This is a real rotator rather than a pointer-delta: the cursor ray is
/// projected onto the plane the joint turns in and read as an angle, so the
/// handle stays under the pointer whatever the camera is doing. Grabbing
/// records the offset between the pointer's angle and the joint's value, so
/// the handle does not jump to the cursor on the first frame.
#[allow(clippy::too_many_arguments)]
fn drive_joint(
    cli: Res<Cli>,
    mut gizmo: ResMut<JointGizmo>,
    mut asked: ResMut<AskedFor>,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut cameras: Query<(&Camera, &GlobalTransform, &mut PanOrbitCamera), With<OrbitCamera>>,
    mut grab: Local<Option<f32>>,
    mut bodies: Query<(&mut Transform, &GlobalTransform)>,
) {
    let Some(body) = gizmo.body else {
        return;
    };

    let Ok(body_at) = bodies.get(body).map(|(_, at)| *at) else {
        return;
    };
    let pointer = windows
        .iter()
        .find_map(|window| window.cursor_position())
        .zip(cameras.iter().next())
        .and_then(|(cursor, (camera, camera_at, _))| {
            angle_under_pointer(&gizmo.joint, &body_at, camera, camera_at, cursor)
        });

    // Near the ring is what "hovered" means, and it is the same test a press
    // has to pass — so what lights up under the pointer is exactly what a
    // click would take hold of.
    gizmo.hovered = cli.ring_active || pointer.is_some_and(|(_, distance)| near_ring(distance));

    if !buttons.pressed(MouseButton::Left) {
        *grab = None;
    } else if grab.is_none() {
        // Only a press landing near the ring takes hold of it. Without this a
        // click anywhere grabs the joint, because the plane the ray is tested
        // against extends across the whole scene.
        if let Some((angle, distance)) = pointer {
            if near_ring(distance) {
                *grab = Some(angle - gizmo.value);
            }
        }
    }
    gizmo.dragging = grab.is_some();

    let wanted = match (asked.0.take(), *grab, pointer) {
        (Some(value), _, _) => Some(value),
        (None, Some(offset), Some((angle, _))) => Some(angle - offset),
        _ => None,
    };
    if let Some(wanted) = wanted {
        // The authored range is the whole point: a joint driven past its limit
        // is not a pose the robot can hold.
        gizmo.value = if gizmo.joint.is_limited() {
            wanted.clamp(gizmo.joint.min, gizmo.joint.max)
        } else {
            wanted
        };
        if (wanted - gizmo.value).abs() > 1e-4 {
            log::info!(
                "{:?} asked for {wanted:.3}, held at {:.3}",
                gizmo.joint.name,
                gizmo.value
            );
        }
    }

    // The ring and the camera share the left button, so whichever the press
    // landed on keeps it for the whole drag. Without this the model spins
    // while the joint turns.
    for (_, _, mut orbit) in &mut cameras {
        orbit.enabled = grab.is_none();
    }

    let Ok((mut transform, _)) = bodies.get_mut(body) else {
        return;
    };
    // Radians about the axis, or metres along it. Reading a joint's value as
    // the wrong one of those is silent — the body just moves wrongly — which is
    // why the kind decides here rather than being assumed.
    let moved = gizmo.value - gizmo.joint.default;
    *transform = if gizmo.joint.kind.is_angular() {
        Transform {
            rotation: gizmo.rest.rotation * Quat::from_axis_angle(gizmo.joint.axis, moved),
            ..gizmo.rest
        }
    } else {
        Transform {
            translation: gizmo.rest.translation + gizmo.rest.rotation * gizmo.joint.axis * moved,
            ..gizmo.rest
        }
    };
}

/// Sits the rings on the joint's plane and tells the shader where the joint
/// stands.
///
/// Bevy's torus is built around +Y, so the plane's axis becomes the mesh's up.
/// The shader reads its angle from the mesh's own winding, which starts at +X
/// — the same direction this basis calls zero — so the two agree without an
/// offset.
fn draw_joint(
    gizmo: Res<JointGizmo>,
    bodies: Query<&GlobalTransform>,
    mut rings: ResMut<Assets<JointRingMaterial>>,
    mut parts: Query<(&mut Transform, &mut Visibility)>,
    handles: Query<&MeshMaterial3d<JointRingMaterial>>,
) {
    let Some(body) = gizmo.body.and_then(|body| bodies.get(body).ok()) else {
        return;
    };
    let plane = joint_plane(&gizmo.joint, body);
    let placement = if gizmo.joint.kind.is_angular() {
        // A turning joint pivots where it stands, so the ring sits on it.
        Transform {
            translation: plane.centre,
            rotation: plane.for_torus(gizmo.range.origin),
            scale: Vec3::ONE,
        }
    } else {
        // A sliding one carries its body away, so the bar has to stay put
        // while the body travels along it — anchored to the middle of the
        // travel rather than to wherever the body currently is.
        let middle = (gizmo.joint.min + gizmo.joint.max) * 0.5;
        Transform {
            // Offset clear of the link. A ring is wider than the thing it
            // turns and so reads from outside it; a bar laid on the axis sits
            // inside the housing it slides, where no depth bias will save it.
            translation: plane.centre
                + plane.axis * (middle - gizmo.value)
                + plane.u * HANDLE_RADIUS,
            rotation: Quat::from_rotation_arc(Vec3::Y, plane.axis),
            scale: Vec3::ONE,
        }
    };
    let in_hand = gizmo.hovered || gizmo.dragging;

    for (part, shown) in [(gizmo.idle, !in_hand), (gizmo.active, in_hand)] {
        let Some(part) = part else { continue };
        let Ok((mut transform, mut visibility)) = parts.get_mut(part) else {
            continue;
        };
        *transform = placement;
        *visibility = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    // The travelled span runs from the range's start to where the joint is, so
    // the shader wants the value in the same wrapped space as the limits.
    let Some(active) = gizmo.active else {
        return;
    };
    let Ok(handle) = handles.get(active) else {
        return;
    };
    // `get_mut` hands back a change-detection guard, not a plain reference.
    if let Some(mut material) = rings.get_mut(&handle.0) {
        material.ring.current = gizmo.range.value(gizmo.value);
    }
}

/// The sliding joint's bar, kept current the same way its ring counterpart is.
fn drive_bar(
    gizmo: Res<JointGizmo>,
    mut bars: ResMut<Assets<JointBarMaterial>>,
    handles: Query<&MeshMaterial3d<JointBarMaterial>>,
) {
    let Some(handle) = gizmo.active.and_then(|part| handles.get(part).ok()) else {
        return;
    };
    if let Some(mut material) = bars.get_mut(&handle.0) {
        material.bar.current = gizmo.range.value(gizmo.value);
    }
}

/// The gizmo's readout: which joint, and where it stands in degrees.
///
/// Drawn as UI and placed by projecting the joint into the viewport each
/// frame — the same shape as the HTML overlay the web renderer drives from its
/// anchor callback.
#[derive(Component)]
struct JointLabel;

/// The label, and whether it has been pointed at a camera yet.
type LabelQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Node,
        &'static mut Visibility,
        &'static Children,
        Has<UiTargetCamera>,
    ),
    With<JointLabel>,
>;

/// Draws the gizmo in front of the robot rather than inside it.
///
/// A control buried in the geometry it controls cannot be grabbed, and the
/// neck is exactly the kind of thing a joint ring sits inside.
fn gizmos_on_top(mut store: ResMut<GizmoConfigStore>) {
    store.config_mut::<DefaultGizmoConfigGroup>().0.depth_bias = -1.0;
}

fn spawn_label(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.09, 0.09, 0.11, 0.92)),
            JointLabel,
        ))
        .with_child((
            Text::new(""),
            TextFont {
                font_size: bevy::text::FontSize::Px(13.0),
                ..default()
            },
        ));
}

fn place_label(
    gizmo: Res<JointGizmo>,
    bodies: Query<&GlobalTransform>,
    cameras: Query<(Entity, &Camera, &GlobalTransform), With<OrbitCamera>>,
    mut labels: LabelQuery,
    mut texts: Query<(&mut Text, &mut TextColor)>,
    mut commands: Commands,
) {
    let Ok((label, mut node, mut visibility, children, targeted)) = labels.single_mut() else {
        return;
    };
    let Some((camera_entity, camera, camera_at)) = cameras.iter().next() else {
        return;
    };
    // UI draws to the primary window unless told otherwise, so a label left
    // untargeted is missing from every offscreen capture while looking fine on
    // screen.
    if !targeted {
        commands.entity(label).insert(UiTargetCamera(camera_entity));
    }
    let Some(anchor) = gizmo
        .body
        .and_then(|body| bodies.get(body).ok())
        .map(|body| body.translation())
    else {
        return;
    };
    let Ok(screen) = camera.world_to_viewport(camera_at, anchor) else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Inherited;
    // Clear of the ring rather than over it, so the handle stays legible. The
    // ring's own screen size is what decides how far clear that is.
    let edge = camera
        .world_to_viewport(camera_at, anchor + camera_at.right() * HANDLE_RADIUS)
        .map(|edge| (edge.x - screen.x).abs())
        .unwrap_or(40.0);
    node.left = Val::Px(screen.x + edge + 14.0);
    node.top = Val::Px(screen.y - 10.0);

    let colour: Color = gizmo
        .joint
        .color
        .unwrap_or(Srgba::hex("e8c547").unwrap())
        .into();
    for child in children.iter() {
        if let Ok((mut text, mut text_colour)) = texts.get_mut(child) {
            // Bevy's bundled fallback font carries ASCII only, so a degree
            // sign renders as tofu. A real build ships a font and can use one.
            text.0 = if gizmo.joint.kind.is_angular() {
                format!("{}   {:.1} deg", gizmo.joint.name, gizmo.value.to_degrees())
            } else {
                // A sliding joint's value is a distance; reporting it in
                // degrees is the same category error as rotating it.
                format!("{}   {:.0} mm", gizmo.joint.name, gizmo.value * 1000.0)
            };
            text_colour.0 = colour;
        }
    }
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
        // Update, not Startup: the node it attaches to does not exist until
        // the glTF scene has spawned.
        .add_systems(Update, spawn_screen);
}

/// Builds the quad the robot's screen only describes, and hangs the face's
/// texture on it.
///
/// The quad goes *under* the screen's own glTF node, not at the world position
/// that node resolves to. A screen is bolted to a robot that moves: parent it
/// and every joint above it carries it for free; place it in world space and
/// it stays behind the moment a neck turns.
///
/// Runs until the node appears, since the glTF scene spawns asynchronously.
fn spawn_screen(
    mount: Res<ScreenMount>,
    mut placed: Local<bool>,
    names: Query<(Entity, &Name)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    if *placed {
        return;
    }
    // Unnamed glTF nodes are named for their index, which is the only handle
    // on a node the robot's own metadata refers to.
    let wanted = format!("GltfNode{}", mount.screen.node);
    let Some((node, _)) = names.iter().find(|(_, name)| name.as_str() == wanted) else {
        return;
    };
    *placed = true;
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(mount.screen.width, mount.screen.height))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(mount.texture.clone()),
            // The face bakes its own lighting, so the scene's lights must not
            // get a second say over it.
            unlit: true,
            ..default()
        })),
        Transform::IDENTITY,
        ChildOf(node),
    ));
    log::info!("screen quad attached under {wanted}");
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
        // three.js OrbitControls' bindings, which is what the scene's users
        // already have in their hands: left orbits, right pans. The joint
        // gizmo shares the left button and takes priority when the press
        // lands on it.
        PanOrbitCamera {
            button_orbit: MouseButton::Left,
            button_pan: MouseButton::Right,
            ..default()
        },
        OrbitCamera,
    ));
    // Saving a frame implies rendering offscreen, because the window's own
    // swapchain cannot be read back here: a capture of it returns pure black
    // whenever the window is not the composited frontmost surface, which is
    // the normal case for a run started from a terminal.
    if cli.offscreen || cli.screenshot.is_some() {
        let mut target = Image::new_target_texture(
            1280,
            800,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        );
        // `new_target_texture` asks for everything needed to render *into* the
        // texture but not to copy back *out* of it, and without this the
        // readback quietly returns the zero-filled CPU side — a black image
        // indistinguishable from a scene that drew nothing.
        target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let target = images.add(target);
        camera.insert(RenderTarget::Image(target.clone().into()));
        commands.insert_resource(OffscreenView(target));
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

/// Points the camera controller at the model, once its bounds are known.
///
/// The controls themselves are `bevy_panorbit_camera`'s. A hand-rolled orbit
/// gets drag direction wrong on trackpads and has no inertia, focus handling
/// or zoom limits, and none of that is what this spike is measuring.
///
/// Orbit is on the right button and pan on the middle one, because the left
/// belongs to the joint gizmo — a camera control and a joint control competing
/// for one drag makes both feel broken.
fn fit_orbit(
    cli: Res<Cli>,
    time: Res<Time>,
    framing: Res<Framing>,
    mut cameras: Query<(&mut PanOrbitCamera, &mut Projection), With<OrbitCamera>>,
) {
    if !framing.fitted {
        return;
    }
    // Far enough out that the bounding sphere fits the vertical field of view,
    // with a little air around it.
    let distance = framing.radius * 2.6;
    for (mut orbit, mut projection) in &mut cameras {
        if framing.is_changed() {
            orbit.focus = framing.center;
            orbit.target_focus = framing.center;
            orbit.radius = Some(distance);
            orbit.target_radius = distance;
            orbit.yaw = Some(cli.angle);
            orbit.target_yaw = cli.angle;
            orbit.pitch = Some(cli.pitch);
            orbit.target_pitch = cli.pitch;
            orbit.zoom_lower_limit = framing.radius * 0.4;
            orbit.zoom_upper_limit = Some(framing.radius * 20.0);
            // The controller initialises itself from the camera's transform,
            // and this camera has none worth reading; without this it keeps
            // its own state and ignores everything set above.
            orbit.force_update = true;

            // The default near/far pair assumes a metre-scale world; a model in
            // millimetres would sit entirely behind the far plane.
            *projection = Projection::Perspective(PerspectiveProjection {
                near: framing.radius * 0.01,
                far: framing.radius * 40.0,
                ..default()
            });
        }

        // A turning camera is what the frame-time measurement wants; a still
        // one is what inspecting a joint wants. Driving the target rather than
        // the angle leaves the controller free to take over mid-turn.
        if !cli.still {
            orbit.target_yaw = cli.angle + time.elapsed_secs() * 0.35;
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
    view: Option<Res<OffscreenView>>,
    mut capture: ResMut<Capture>,
    mut commands: Commands,
) {
    let (Some(path), Some(view)) = (cli.screenshot.clone(), view) else {
        return;
    };
    if capture.done || !framing.fitted || frames.elapsed < CAPTURE_SECONDS {
        return;
    }
    // A couple of frames after the camera was placed, so the target holds the
    // fitted view rather than the one before it.
    if capture.at_frame == 0 {
        capture.at_frame = frames.count;
        return;
    }
    if frames.count < capture.at_frame + 3 {
        return;
    }
    capture.done = true;
    log::info!("saving a frame to {}", path.display());
    commands
        .spawn(Screenshot::image(view.0.clone()))
        .observe(save_to_disk(path));
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
