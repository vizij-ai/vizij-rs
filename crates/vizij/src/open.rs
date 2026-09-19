//! Opening another face while the window runs: a GLB dropped on the window,
//! or picked in the system's file dialog (`O`). Either way the bytes go to
//! the running device through its [`DeviceHandle`]; the device restarts on
//! the new face and the view follows.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::FileDragAndDrop;
use vizij::device::native::DeviceHandle;

pub struct OpenPlugin(pub DeviceHandle);

impl Plugin for OpenPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Handle(self.0.clone()))
            .insert_non_send(Dialog(None))
            .add_systems(Update, (dropped, dialog));
    }
}

#[derive(Resource)]
struct Handle(DeviceHandle);

/// The file dialog in flight, if one is open; its future is `Send`, but it
/// is created on the main thread (macOS shows it as a sheet on the window).
struct Dialog(Option<Pin<Box<dyn Future<Output = Option<rfd::FileHandle>> + Send>>>);

/// A GLB dropped on the window loads.
fn dropped(mut drops: MessageReader<FileDragAndDrop>, handle: Res<Handle>) {
    for drop in drops.read() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = drop {
            match std::fs::read(path_buf) {
                Ok(glb) => {
                    log::info!("opening the dropped face {}", path_buf.display());
                    handle.0.reload(glb);
                }
                Err(e) => log::error!("cannot read the dropped {}: {e}", path_buf.display()),
            }
        }
    }
}

/// `O` opens the system's file dialog (async: a sheet, the frames go on);
/// the pick, once made, loads. The dialog is created on the main thread —
/// [`NonSendMarker`] pins the system there — and polled here each frame with
/// a no-op waker, which loses nothing: the dialog's future re-arms on every
/// poll.
fn dialog(
    _main_thread: NonSendMarker,
    keys: Res<ButtonInput<KeyCode>>,
    mut dialog: NonSendMut<Dialog>,
    handle: Res<Handle>,
) {
    if dialog.0.is_none() && keys.just_pressed(KeyCode::KeyO) {
        let picker = rfd::AsyncFileDialog::new()
            .add_filter("face", &["glb"])
            .set_title("Open a face");
        dialog.0 = Some(Box::pin(picker.pick_file()));
    }
    let Some(pending) = dialog.0.as_mut() else {
        return;
    };
    let mut cx = Context::from_waker(Waker::noop());
    if let Poll::Ready(picked) = pending.as_mut().poll(&mut cx) {
        dialog.0 = None;
        if let Some(file) = picked {
            let path = file.path().to_path_buf();
            match std::fs::read(&path) {
                Ok(glb) => {
                    log::info!("opening {}", path.display());
                    handle.0.reload(glb);
                }
                Err(e) => log::error!("cannot read {}: {e}", path.display()),
            }
        }
    }
}
