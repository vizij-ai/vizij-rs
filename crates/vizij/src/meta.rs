//! The face GLB as this app reads it: the render metadata plus the host bundle.
//!
//! One GLB carries two Vizij payloads. The per-node `RobotData` extension is
//! render data and lives in `vizij-render`; the scene root's
//! `VIZIJ_bundle` extension — graphs, programs, neutral pose — is what the
//! device runs. Both are read from one parse of the GLB's JSON chunk.

use std::ops::Deref;
use std::path::Path;

use anyhow::{Context, Result};
use vizij_arora_host::Bundle;

/// A face, read once: what the renderer draws and what the device runs.
#[derive(Debug, Clone)]
pub struct FaceMeta {
    /// The render half, handed to `vizij-render`.
    pub render: vizij_render::FaceMeta,
    /// The face's graphs, programs and neutral pose.
    pub bundle: Bundle,
}

impl FaceMeta {
    pub fn from_glb_file(path: &Path) -> Result<Self> {
        let bytes =
            std::fs::read(path).with_context(|| format!("cannot read GLB {}", path.display()))?;
        Self::from_glb_bytes(&bytes)
    }

    pub fn from_glb_bytes(bytes: &[u8]) -> Result<Self> {
        let json = vizij_render::meta::glb_json_chunk(bytes)?;
        Ok(Self {
            render: vizij_render::FaceMeta::from_gltf_json(&json)?,
            bundle: Bundle::from_gltf_json(&json).unwrap_or_default(),
        })
    }
}

/// The render half reads through, so `meta.elements` and `meta.root_bounds`
/// address it directly.
impl Deref for FaceMeta {
    type Target = vizij_render::FaceMeta;

    fn deref(&self) -> &Self::Target {
        &self.render
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
