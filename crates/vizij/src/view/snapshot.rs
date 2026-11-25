//! Headless offscreen rendering: render the face to a texture without a
//! window, read the pixels back, save a PNG. The comparison harness against
//! the web renderer.
//!
//! The harness is the shared layer's ([`arora_visuals::snapshot`]); what is
//! vizij's is pointing every face camera at its image, which the view does
//! through [`OffscreenTarget`], the same resource a headless run sets
//! without the harness.

use bevy::prelude::*;

pub use arora_visuals::snapshot::{capture, ensure_ready, save_png, SnapshotTarget, FORMAT};

use super::OffscreenTarget;

/// Windowless app base: `DefaultPlugins` without a window or winit, driven by
/// manual updates, with every face camera drawing into the image a capture
/// reads back.
pub struct SnapshotPlugin {
    pub width: u32,
    pub height: u32,
}

impl Plugin for SnapshotPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(arora_visuals::snapshot::SnapshotPlugin {
            width: self.width,
            height: self.height,
        });
        let target = app.world().resource::<SnapshotTarget>().0.clone();
        app.insert_resource(OffscreenTarget(target));
    }
}
