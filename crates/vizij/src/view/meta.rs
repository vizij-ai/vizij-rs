//! Vizij face metadata, read from the GLB's JSON chunk.
//!
//! A face is a Semio asset with one more part. Its `RobotData` — elements,
//! animatables, bounds — is read by the shared layer ([`AssetMeta`]), as a
//! robot's is, so a face binds and draws as it would in a robot scene. What
//! only a face carries is the scene-root node's `VIZIJ_bundle` extension: its
//! graphs, programs and neutral pose, which drive the device rather than the
//! view, read here beside it ([`FaceMeta`]).

use std::ops::{Deref, DerefMut};

use anyhow::Result;
pub use arora_visuals::meta::*;
use vizij_arora_host::Bundle;

/// A face's metadata: its asset metadata, and its `VIZIJ_bundle`.
#[derive(Debug, Clone, Default)]
pub struct FaceMeta {
    /// What the view draws and binds: the `RobotData` half.
    pub asset: AssetMeta,
    /// The face's `VIZIJ_bundle` — its graphs, programs, and neutral pose. The
    /// device composition and neutral staging are its methods
    /// ([`Bundle::compose`], [`Bundle::neutral_stage_writes`]). A GLB without
    /// one has an empty bundle.
    pub bundle: Bundle,
}

impl FaceMeta {
    /// Read a face's metadata from its GLB bytes — the one way in, on every
    /// target; reading a file is the caller's.
    pub fn from_glb_bytes(bytes: &[u8]) -> Result<Self> {
        let json = glb_json_chunk(bytes)?;
        Ok(Self {
            asset: AssetMeta::from_gltf_json(&json)?,
            bundle: Bundle::from_gltf_json(&json).unwrap_or_default(),
        })
    }
}

impl Deref for FaceMeta {
    type Target = AssetMeta;

    fn deref(&self) -> &AssetMeta {
        &self.asset
    }
}

impl DerefMut for FaceMeta {
    fn deref_mut(&mut self) -> &mut AssetMeta {
        &mut self.asset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_glb() {
        assert!(FaceMeta::from_glb_bytes(b"not a glb at all....").is_err());
    }
}
