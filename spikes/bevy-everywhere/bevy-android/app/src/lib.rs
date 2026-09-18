//! Minimal Bevy 0.19.1 Android spike: a clear color, a spinning cube and the
//! Quori face GLB (from the APK's assets/) under an orthographic camera fitted
//! to the loaded meshes' bounds.

use bevy::camera::primitives::Aabb;
use bevy::camera::{Projection, ScalingMode};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

#[derive(Component)]
struct Spinner;

#[derive(Component)]
struct FitCamera;

/// Frames left during which the camera refits to whatever meshes exist (the
/// GLB spawns a few frames after startup).
#[derive(Resource)]
struct FitBudget(u32);

#[bevy_main]
pub fn main() {
    // `gl`: force wgpu's GLES backend. The emulator's SwiftShader Vulkan device
    // reports max_buffer_size 0 to wgpu 29, which panics Bevy's render world.
    #[cfg(feature = "gl")]
    let plugins = DefaultPlugins.set(bevy::render::RenderPlugin {
        render_creation: bevy::render::settings::WgpuSettings {
            backends: Some(bevy::render::settings::Backends::GL),
            ..default()
        }
        .into(),
        ..default()
    });
    #[cfg(not(feature = "gl"))]
    let plugins = DefaultPlugins;
    App::new()
        .add_plugins(plugins)
        .insert_resource(ClearColor(Color::srgb(0.06, 0.07, 0.09)))
        .insert_resource(FitBudget(180))
        .add_systems(Startup, setup)
        .add_systems(Update, (spin, fit_camera))
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // The face, loaded from the APK's assets/ (bevy_asset's AndroidAssetReader).
    commands.spawn(WorldAssetRoot(
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("quori.glb")),
    ));
    // A spinning cube in front of it: proof of rendering even if the GLB fails.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.6, 0.6, 0.6))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.35, 0.2),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 3.0),
        Spinner,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 3000.0,
            ..default()
        },
        Transform::from_xyz(2.0, 4.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // Orthographic, looking down -Z at the XY plane the face lives in (the
    // vizij view's convention); refitted once the meshes are in.
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: 4.0,
                min_height: 4.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.0, 100.0).looking_at(Vec3::ZERO, Vec3::Y),
        Tonemapping::None,
        Msaa::Off,
        // Per-camera in 0.19 (a component, no longer a resource).
        AmbientLight {
            color: Color::WHITE,
            brightness: 1500.0,
            ..default()
        },
        FitCamera,
    ));
}

fn spin(time: Res<Time>, mut q: Query<&mut Transform, With<Spinner>>) {
    for mut t in &mut q {
        t.rotate_y(time.delta_secs() * 1.2);
        t.rotate_x(time.delta_secs() * 0.7);
    }
}

fn fit_camera(
    mut budget: ResMut<FitBudget>,
    boxes: Query<(&GlobalTransform, &Aabb), Without<Spinner>>,
    mut camera: Query<(&mut Projection, &mut Transform), With<FitCamera>>,
) {
    if budget.0 == 0 {
        return;
    }
    budget.0 -= 1;
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut any = false;
    for (gt, aabb) in &boxes {
        let c = Vec3::from(aabb.center);
        let h = Vec3::from(aabb.half_extents);
        for corner in [
            Vec3::new(-h.x, -h.y, -h.z),
            Vec3::new(h.x, -h.y, -h.z),
            Vec3::new(-h.x, h.y, -h.z),
            Vec3::new(h.x, h.y, -h.z),
            Vec3::new(-h.x, -h.y, h.z),
            Vec3::new(h.x, -h.y, h.z),
            Vec3::new(-h.x, h.y, h.z),
            Vec3::new(h.x, h.y, h.z),
        ] {
            let p = gt.transform_point(c + corner);
            min = min.min(p);
            max = max.max(p);
            any = true;
        }
    }
    if !any {
        return;
    }
    let size = max - min;
    let center = (min + max) * 0.5;
    if budget.0 == 0 {
        info!("fit: bounds min={min:?} max={max:?} size={size:?}");
    }
    let Ok((mut projection, mut transform)) = camera.single_mut() else {
        return;
    };
    if let Projection::Orthographic(ortho) = &mut *projection {
        ortho.scaling_mode = ScalingMode::AutoMin {
            min_width: (size.x * 1.1).max(0.1),
            min_height: (size.y * 1.1).max(0.1),
        };
    }
    *transform =
        Transform::from_xyz(center.x, center.y, max.z + 50.0).looking_at(Vec3::new(center.x, center.y, center.z), Vec3::Y);
}
