//! Vizij: an arora with a head.
//!
//! `cargo run` opens the Vizij view (Bevy) and runs an arora runtime on a
//! worker thread over a [`RigHal`]. By default the device serves the open
//! local bridge (`arora-websocket`) — editors and apps connect on
//! `ws://127.0.0.1:9000` with zero accounts. Building with
//! `--features semio-studio-bridge` connects to Semio Studio instead
//! (mutually exclusive: the runtime owns exactly one bridge).

use std::sync::Arc;

use anyhow::Result;
use bevy::prelude::*;
use vizij_arora_hal::RigHal;

/// The rig shared between the arora runtime (worker thread) and the view.
#[derive(Resource, Clone)]
struct Rig(RigHal);

fn main() -> Result<()> {
    let rig = RigHal::new();
    let device = spawn_device(rig.clone())?;

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Vizij".to_string(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(Rig(rig))
        .add_systems(Startup, setup_scene)
        .add_systems(Update, apply_rig_pose)
        .run();

    drop(device);
    Ok(())
}

/// Run the arora device on a worker thread: the runtime loop over the rig and
/// the selected bridge. Returns the thread handle; the loop ends when the
/// device is unregistered or the process exits.
fn spawn_device(rig: RigHal) -> Result<std::thread::JoinHandle<()>> {
    Ok(std::thread::Builder::new()
        .name("arora".to_string())
        .spawn(move || {
            if let Err(e) = run_device(rig) {
                log::error!("arora device stopped: {e:?}");
            }
        })?)
}

#[cfg(not(feature = "semio-studio-bridge"))]
fn run_device(rig: RigHal) -> Result<()> {
    use arora_bridge::Bridge;
    use arora_simple_data_store::SimpleDataStore;
    use arora_websocket::bridge::WsBridge;
    use arora_websocket::{AroraWSServer, CancellationToken, ServerConfig};

    arora::run_with_bridge_builder(Arc::new(rig), SimpleDataStore::new(), move || async move {
        let server = Arc::new(AroraWSServer::new(ServerConfig::default()));
        let bridge = WsBridge::new(server.clone()).await;
        tokio::spawn(async move {
            if let Err(e) = server.run(CancellationToken::new()).await {
                log::error!("local bridge server stopped: {e:?}");
            }
        });
        let bridge: Arc<dyn Bridge> = Arc::new(bridge);
        Ok(bridge)
    })
}

#[cfg(feature = "semio-studio-bridge")]
fn run_device(rig: RigHal) -> Result<()> {
    arora::run_with_hal(Arc::new(rig))
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(2.5, 2.5, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
    commands.spawn(PointLightBundle {
        transform: Transform::from_xyz(3.0, 5.0, 3.0),
        ..default()
    });
    // The default rig visual: driven by the store (see `apply_rig_pose`).
    commands.spawn(PbrBundle {
        mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        material: materials.add(Color::srgb(0.5, 0.8, 0.9)),
        ..default()
    });
}

/// Reflect the rig state onto the scene: any writes the runtime (or a bridge
/// client) makes to the rig show up here, every frame.
fn apply_rig_pose(rig: Res<Rig>, mut query: Query<&mut Transform, With<Handle<Mesh>>>) {
    use vizij_api_core_value_shim::*;
    let pose = rig.0.pose();
    for (path, value) in &pose {
        if path.to_string() == "demo/cube/rotation" {
            if let Some([x, y, z, w]) = as_quat(value) {
                for mut transform in &mut query {
                    transform.rotation = Quat::from_xyzw(x, y, z, w);
                }
            }
        }
    }
}

/// Minimal value accessors until the head grows real bindings.
mod vizij_api_core_value_shim {
    pub fn as_quat(value: &vizij_api_core::Value) -> Option<[f32; 4]> {
        match value {
            vizij_api_core::Value::Quat(q) => Some(*q),
            _ => None,
        }
    }
}
